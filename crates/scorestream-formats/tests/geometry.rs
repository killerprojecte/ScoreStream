use scorestream_formats::{
    MappingEffect, MappingReport, ScoreAdapter,
    musicxml::{self, GeometryPolicy, MusicXml, MusicXmlOptions},
    sfs::SfsAdapter,
};
use std::io::{self, BufRead, Cursor, Read, Seek, SeekFrom};
fn options() -> MusicXmlOptions {
    MusicXmlOptions {
        geometry: GeometryPolicy::RegenerateNoteAndMeasureCoordinates,
        ..Default::default()
    }
}
fn fixture() -> String {
    r#"<score-partwise version="4.0"><part-list><score-part id="p"><part-name>Flute</part-name><score-instrument id="i"><instrument-name>flute</instrument-name></score-instrument></score-part></part-list><part id="p"><measure id="m" number="1" width="123.45"><attributes><divisions>1</divisions><time><beats>2</beats><beat-type>4</beat-type></time></attributes><note id="a" default-x="-1.25" default-y="0"><pitch><step>C</step><octave>5</octave></pitch><duration>1</duration><voice>1</voice><type>quarter</type></note><note id="b" default-x="42" default-y="-20"><rest/><duration>1</duration><voice>1</voice><type>quarter</type></note></measure></part></score-partwise>"#.into()
}
fn policy_records(r: &MappingReport) -> Vec<&scorestream_formats::MappingDiagnostic> {
    r.diagnostics
        .iter()
        .filter(|d| d.feature == "geometry.regenerate_coordinates")
        .collect()
}
#[test]
fn coordinates_need_explicit_policy_and_only_geometry_is_regenerated() {
    let source = fixture();
    assert!(MusicXml.read(&source).is_err());
    let old: MusicXmlOptions = serde_json::from_str(r#"{"irregular_measures":[]}"#).unwrap();
    assert_eq!(old.geometry, GeometryPolicy::Strict);
    assert!(musicxml::read_with_options(&source, &old).is_err());
    assert!(!serde_json::to_string(&old).unwrap().contains("geometry"));
    let converted = musicxml::read_with_options(&source, &options()).unwrap();
    let records = policy_records(&converted.report);
    assert_eq!(records.len(), 3);
    for (record, kind, count) in [
        (records[0], "measure/@width", 1),
        (records[1], "note/@default-x", 2),
        (records[2], "note/@default-y", 2),
    ] {
        assert_eq!(record.effect, MappingEffect::PolicyApplied);
        assert!(
            record
                .message
                .contains(&format!("{count} validated {kind}"))
        );
        assert!(record.message.contains("not retained"));
        assert!(record.source.is_some());
    }
    let clean = source
        .replace(" width=\"123.45\"", "")
        .replace(" default-x=\"-1.25\"", "")
        .replace(" default-y=\"0\"", "")
        .replace(" default-x=\"42\"", "")
        .replace(" default-y=\"-20\"", "");
    // An explicitly named small fixture proves invariance; no original is stripped.
    assert_eq!(converted.value, MusicXml.read(&clean).unwrap().value);
    let sfs = SfsAdapter.write(&converted.value).unwrap().value;
    assert_eq!(SfsAdapter.read(&sfs).unwrap().value, converted.value);
    let plan = scorestream_app::playback::plan_source(&sfs).unwrap();
    assert_eq!(plan.notes.len(), 1);
    let out = MusicXml.write(&converted.value).unwrap().value;
    for token in ["default-x", "default-y", "width="] {
        assert!(!out.contains(token));
    }
    assert_eq!(MusicXml.read(&out).unwrap().value, converted.value);
}
#[test]
fn signed_exact_tenths_and_precision_are_validated_not_skipped() {
    for value in ["-2.5", "0", "+0.25", ".5", "2.", "  -4.25  "] {
        assert!(
            musicxml::read_with_options(&fixture().replace("123.45", value), &options()).is_ok()
        );
    }
    for value in ["", "NaN", "Infinity", "1e3", "1/2", "2..3", "--2", "1 2"] {
        for target in ["123.45", "-1.25", "-20"] {
            let e = musicxml::read_with_options(&fixture().replace(target, value), &options())
                .unwrap_err();
            assert!(
                e.diagnostics.iter().any(|d| d.feature == "number.decimal"),
                "{e:?}"
            );
        }
    }
    for space in ["&#160;", "&#x2028;", "&#x2003;"] {
        let input = fixture().replace("123.45", &format!("{space}12{space}"));
        assert!(
            musicxml::read_with_options(&input, &options())
                .unwrap_err()
                .diagnostics
                .iter()
                .any(|d| d.feature == "geometry.number_whitespace")
        );
    }
    let wide = fixture().replace("123.45", &"9".repeat(65));
    assert!(
        musicxml::read_with_options(&wide, &options())
            .unwrap_err()
            .diagnostics
            .iter()
            .any(|d| d.feature == "number.precision")
    );
    assert!(
        musicxml::read_with_options(&fixture().replace("123.45", &"9".repeat(64)), &options())
            .is_ok()
    );
}
#[test]
fn semantic_fields_unknown_descendants_and_other_paths_stay_blocking() {
    for addition in [
        "print-object=\"no\"",
        "dynamics=\"75\"",
        "attack=\"1\"",
        "release=\"1\"",
        "relative-y=\"1\"",
        "placement=\"above\"",
        "unknown=\"1\"",
    ] {
        assert!(
            musicxml::read_with_options(
                &fixture().replacen("id=\"a\"", &format!("id=\"a\" {addition}"), 1),
                &options()
            )
            .is_err()
        );
    }
    for child in [
        "<stem default-y=\"1\">up</stem>",
        "<beam fan=\"accel\">begin</beam>",
        "<accidental bracket=\"yes\">natural</accidental>",
        "<unknown><note default-x=\"1\"/></unknown>",
        "<notations><slur type=\"start\"/></notations>",
    ] {
        assert!(
            musicxml::read_with_options(
                &fixture().replacen("</note>", &format!("{child}</note>"), 1),
                &options()
            )
            .is_err()
        );
    }
    for child in [
        "<defaults><scaling><millimeters>7</millimeters><tenths>40</tenths></scaling></defaults>",
        "<defaults><concert-score/></defaults>",
        "<credit><credit-words>Rights</credit-words></credit>",
    ] {
        assert!(
            musicxml::read_with_options(
                &fixture().replace("<part-list>", &format!("{child}<part-list>")),
                &options()
            )
            .is_err()
        );
    }
    for child in [
        "<print><system-layout/></print>",
        "<print><part-name-display><display-text>Solo</display-text></part-name-display></print>",
    ] {
        assert!(
            musicxml::read_with_options(
                &fixture().replace("<attributes>", &format!("{child}<attributes>")),
                &options()
            )
            .is_err()
        );
    }
    let misplaced = fixture().replace("<pitch>", "<pitch default-x=\"1\">");
    assert!(
        musicxml::read_with_options(&misplaced, &options())
            .unwrap_err()
            .diagnostics
            .iter()
            .any(|d| d.feature == "attribute.default-x")
    );
    let bad_time = fixture().replace("<time>", "<time symbol=\"cut\">");
    assert!(musicxml::read_with_options(&bad_time, &options()).is_err());
}
#[test]
fn audit_survives_mapping_failures_and_bounded_error_truncation() {
    let late = fixture().replace("<divisions>1</divisions>", "");
    let e = musicxml::read_with_options(&late, &options()).unwrap_err();
    assert_eq!(policy_records(&e).len(), 3);
    assert!(
        e.diagnostics
            .iter()
            .any(|d| d.feature == "divisions.missing")
    );
    let many=fixture().replace("</measure>",&format!("{}</measure>","<note default-x=\"1\" default-y=\"2\"><rest/><duration>1</duration><beam fan=\"accel\">begin</beam></note>".repeat(2000)));
    let e = musicxml::read_with_options(&many, &options()).unwrap_err();
    assert_eq!(e.diagnostics.len(), 1024);
    assert!(e.has_errors());
    assert_eq!(policy_records(&e).len(), 3);
    assert!(
        policy_records(&e)[1]
            .message
            .contains("2002 validated note/@default-x")
    );
    assert!(e.diagnostics.last().unwrap().effect == MappingEffect::Unsupported);
}
struct Changing {
    current: Cursor<Vec<u8>>,
    replacement: Option<Vec<u8>>,
}
impl Read for Changing {
    fn read(&mut self, b: &mut [u8]) -> io::Result<usize> {
        self.current.read(b)
    }
}
impl BufRead for Changing {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        self.current.fill_buf()
    }
    fn consume(&mut self, n: usize) {
        self.current.consume(n)
    }
}
impl Seek for Changing {
    fn seek(&mut self, s: SeekFrom) -> io::Result<u64> {
        if s != SeekFrom::Start(0) {
            return self.current.seek(s);
        }
        if let Some(next) = self.replacement.take() {
            self.current = Cursor::new(next);
        }
        self.current.seek(s)
    }
}
#[test]
fn both_passes_validate_geometry_and_fingerprint_even_discarded_coordinates() {
    for (replacement, feature) in [
        (fixture().replace("123.45", "999"), "xml.changed"),
        (fixture().replace("123.45", "NaN"), "number.decimal"),
        (
            fixture().replace("id=\"a\"", "id=\"a\" print-object=\"no\""),
            "attribute.print-object",
        ),
    ] {
        let input = Changing {
            current: Cursor::new(fixture().into_bytes()),
            replacement: Some(replacement.into_bytes()),
        };
        let e = musicxml::read_reader_with_options(input, &options()).unwrap_err();
        assert!(e.diagnostics.iter().any(|d| d.feature == feature), "{e:?}");
        assert_eq!(policy_records(&e).len(), 3);
    }
}
#[test]
#[ignore = "set SCORESTREAM_MXL_FIXTURES to the four untouched original XML/MXL sources"]
fn original_coordinate_counts_are_exact_but_xml_and_mxl_still_reject_semantics() {
    use sha2::{Digest, Sha256};
    use std::{fs::File, io::BufReader, path::PathBuf};
    let root = PathBuf::from(std::env::var("SCORESTREAM_MXL_FIXTURES").unwrap());
    let hashes = [
        "5de9f153c72b3831757d43584c5d5ec792510d36cc5a389a4160aa90697fc17b",
        "c962fecc2e85c070633ad1990b7046c2c23ed5673ad95f4d02eeeac94528f09a",
        "788d9c3cda2fc1b970f5a832683eafb7ce4dc25e7767f18041e3dd1783978b98",
        "cce11a1437d3acd379fbf2f72483d92888b05443f7b04bb1cb5eea9c59cc9298",
    ];
    for movement in 1..=4 {
        let path = root.join(format!("Beethoven_Op.67_{movement}.musicxml"));
        assert_eq!(
            format!("{:x}", Sha256::digest(std::fs::read(&path).unwrap())),
            hashes[movement - 1]
        );
        let xml = musicxml::read_reader_with_options(
            BufReader::new(File::open(&path).unwrap()),
            &options(),
        )
        .unwrap_err();
        let archive = scorestream_formats::mxl::open(
            File::open(root.join(format!("Beethoven_Op.67_{movement}.mxl"))).unwrap(),
        )
        .unwrap();
        let mxl =
            musicxml::read_reader_with_options(archive.into_reader(), &options()).unwrap_err();
        assert_eq!(xml, mxl);
        let records = policy_records(&xml);
        assert_eq!(records.len(), 3);
        let widths = [9036, 4464, 6768, 10258][movement - 1];
        let notes = [15261, 10908, 11488, 30902][movement - 1];
        for (i, count, field) in [
            (0, widths, "measure/@width"),
            (1, notes, "note/@default-x"),
            (2, notes, "note/@default-y"),
        ] {
            assert!(
                records[i]
                    .message
                    .contains(&format!("{count} validated {field}"))
            );
        }
        assert!(xml.has_errors());
        assert_eq!(xml.diagnostics.len(), 1024);
        assert!(!xml.diagnostics.iter().any(|d| {
            let node = d.path.rsplit('/').next().unwrap_or("");
            (d.feature == "attribute.width" && node.starts_with("measure["))
                || (["attribute.default-x", "attribute.default-y"].contains(&d.feature.as_str())
                    && node.starts_with("note["))
        }));
        assert!(
            xml.diagnostics
                .iter()
                .any(|d| d.feature == "element.defaults")
        );
        assert!(xml.diagnostics.iter().any(|d| d.feature == "element.print"));
    }
}
