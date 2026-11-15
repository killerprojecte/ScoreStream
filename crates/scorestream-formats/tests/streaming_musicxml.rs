use scorestream_core::{Pitch, Rational};
use scorestream_formats::{
    Format, MappingEffect, MappingReport, ScoreAdapter,
    musicxml::{self, MusicXml},
};
use std::io::{BufReader, Cursor};

const HEADER: &str = r#"<score-partwise version="4.0"><part-list><score-part id="p"><part-name/></score-part></part-list><part id="p">"#;
const END: &str = "</part></score-partwise>";
const DOCTYPE: &str = r#"<!DOCTYPE score-partwise PUBLIC "-//Recordare//DTD MusicXML 4.0 Partwise//EN" "http://www.musicxml.org/dtds/partwise.dtd">"#;

fn generated(measures: usize, notes: usize) -> String {
    let mut source = String::from(HEADER);
    for measure in 0..measures {
        source.push_str(&format!(r#"<measure number="{}">"#, measure + 1));
        if measure == 0 {
            source.push_str("<attributes><divisions>3</divisions></attributes>");
        }
        for _ in 0..notes {
            source.push_str("<note><pitch><step>C</step><alter>-0.5</alter><octave>4</octave></pitch><duration>1</duration><voice>1</voice><type>eighth</type><time-modification><actual-notes>3</actual-notes><normal-notes>2</normal-notes></time-modification></note>");
        }
        source.push_str("</measure>\n");
    }
    source.push_str(END);
    source
}

#[test]
fn large_score_crosses_both_old_limits_and_roundtrips_all_notes() {
    let source = generated(3_000, 16);
    assert!(source.len() > 8 * 1024 * 1024);
    let summary =
        musicxml::scan(BufReader::with_capacity(37, Cursor::new(source.as_bytes()))).unwrap();
    assert!(summary.elements > 100_000);
    assert_eq!(summary.bytes, source.len() as u64);
    assert_eq!(summary.measures, 3_000);
    assert_eq!(summary.notes, 48_000);
    assert!(summary.max_retained_nodes < 300, "{summary:?}");
    let score = musicxml::read_reader(BufReader::with_capacity(
        127,
        Cursor::new(source.as_bytes()),
    ))
    .unwrap()
    .value;
    let part = &score.parts[0];
    assert_eq!(part.measures.len(), 3_000);
    assert_eq!(
        part.measures.iter().map(|m| m.events.len()).sum::<usize>(),
        48_000
    );
    for (index, measure) in part.measures.iter().enumerate() {
        assert_eq!(
            measure.start,
            &Rational::from_integer(index as i64 * 16) / &"3".parse::<Rational>().unwrap()
        );
        assert_eq!(measure.length, "16/3".parse().unwrap());
        for (offset, event) in measure.events.iter().enumerate() {
            assert_eq!(
                event.offset,
                &Rational::from_integer(offset as i64) / &"3".parse::<Rational>().unwrap()
            );
            assert_eq!(event.duration, "1/3".parse().unwrap());
            assert_eq!(
                event.pitch,
                Pitch::Pitched {
                    step: 'C',
                    alteration: "-1/2".parse().unwrap(),
                    octave: 4.into()
                }
            );
        }
    }
    let output = MusicXml.write(&score).unwrap().value;
    assert!(output.len() > 8 * 1024 * 1024);
    assert_eq!(
        musicxml::read_reader(Cursor::new(output.as_bytes()))
            .unwrap()
            .value,
        score
    );
}

#[test]
fn measure_memory_does_not_grow_with_measure_count() {
    let small = musicxml::scan(Cursor::new(generated(1, 16))).unwrap();
    let large = musicxml::scan(Cursor::new(generated(1_000, 16))).unwrap();
    assert_eq!(small.max_retained_nodes, large.max_retained_nodes);
}

#[test]
fn official_doctype_is_inert_and_does_not_enable_entities() {
    let source = generated(2, 2);
    let with_doctype = format!("{DOCTYPE}{source}");
    assert_eq!(
        MusicXml.read(&source).unwrap().value,
        MusicXml.read(&with_doctype).unwrap().value
    );
    for prefix in [
        r#"<!DOCTYPE score-partwise SYSTEM "file:///etc/passwd">"#,
        r#"<!DOCTYPE score-partwise SYSTEM "https://example.invalid/a.dtd">"#,
        r#"<!DOCTYPE score-partwise [<!ENTITY x "expanded">]>"#,
        r#"<!DOCTYPE score-partwise [<!ENTITY % x SYSTEM "https://example.invalid/a.dtd">%x;]>"#,
    ] {
        assert!(musicxml::scan(Cursor::new(format!("{prefix}{source}"))).is_err());
    }
    assert!(
        musicxml::scan(Cursor::new(format!(
            "{DOCTYPE}{}",
            source.replace("<part-name/>", "<part-name>&custom;</part-name>")
        )))
        .is_err()
    );
    assert!(musicxml::scan(Cursor::new(format!("{DOCTYPE}{DOCTYPE}{source}"))).is_err());
    assert!(musicxml::scan(Cursor::new(format!("{source}{DOCTYPE}"))).is_err());
}

#[test]
fn fragmented_utf8_entities_source_locations_and_xml_errors() {
    let valid = generated(2, 2).replace("<part-name/>", "<part-name>乐器 &amp; 声部</part-name>");
    for capacity in [1, 2, 3, 7, 31] {
        let score = musicxml::read_reader(BufReader::with_capacity(
            capacity,
            Cursor::new(valid.as_bytes()),
        ))
        .unwrap()
        .value;
        assert_eq!(score.parts[0].name.as_deref(), Some("乐器 & 声部"));
    }
    let invalid = generated(2, 2).replace("<note>", "\n<note unsupported='yes'>");
    let report =
        musicxml::read_reader(BufReader::with_capacity(1, Cursor::new(invalid))).unwrap_err();
    let diagnostic = report
        .diagnostics
        .iter()
        .find(|d| d.feature == "attribute.unsupported")
        .unwrap();
    assert_eq!(diagnostic.source.unwrap().line, 2);
    assert_eq!(diagnostic.source.unwrap().column, 1);
    assert_eq!(
        diagnostic.path,
        "/score-partwise[1]/part[1]/measure[1]/note[1]"
    );
    for invalid in [
        generated(2, 2).replace(END, ""),
        generated(2, 2).replace("</duration>", "</pitch>"),
        generated(2, 2).replace("<part-name/>", "<part-name>&#0;</part-name>"),
        generated(2, 2).replace("<part-name/>", "<part-name>&#xD800;</part-name>"),
        generated(2, 2).replace("<part-name/>", "<part-name id='&#0;'/>"),
        "<x>".repeat(50) + &"</x>".repeat(50),
        format!("<x>{}</x>", "x".repeat(1024 * 1024 + 1)),
    ] {
        assert!(musicxml::scan(Cursor::new(invalid)).is_err());
    }
    assert!(
        musicxml::scan(Cursor::new(vec![
            b'<', b'x', b'>', 0xff, b'<', b'/', b'x', b'>'
        ]))
        .is_err()
    );
}

#[test]
fn schema_errors_remain_bounded_but_never_succeed() {
    let source = generated(2_000, 1).replace("<note>", "<direction/><note>");
    let report = MusicXml.read(&source).unwrap_err();
    assert_eq!(report.diagnostics.len(), 1024);
    assert!(report.has_errors());
    assert_eq!(
        report.diagnostics.last().unwrap().feature,
        "diagnostics.truncated"
    );
    let mut report = MappingReport::default();
    for _ in 0..2_000 {
        report.push(
            Format::MusicXml,
            MappingEffect::Normalized,
            "voice.identity",
            "/",
            "normalized",
        );
    }
    assert!(!report.has_errors());
    report.push(
        Format::MusicXml,
        MappingEffect::Unsupported,
        "element.direction",
        "/",
        "blocked",
    );
    assert!(report.has_errors());
    assert_eq!(report.diagnostics.len(), 1024);
}

#[test]
fn forward_ids_and_reordered_parts_keep_declaration_order_identity() {
    let declaration = r#"<score-part id="a"><part-name/></score-part><score-part id="b"><part-name/></score-part>"#;
    let a = r#"<part id="a"><measure number="1"><attributes><divisions>1</divisions></attributes><note><rest/><duration>1</duration></note></measure></part>"#;
    let b = r#"<part id="b"><measure number="1"><attributes><divisions>1</divisions></attributes><note id="mx_n1"><rest/><duration>1</duration></note></measure></part>"#;
    let ordered = format!(
        r#"<score-partwise version="4.0"><part-list>{declaration}</part-list>{a}{b}</score-partwise>"#
    );
    let reversed = format!(
        r#"<score-partwise version="4.0"><part-list>{declaration}</part-list>{b}{a}</score-partwise>"#
    );
    let score = MusicXml.read(&ordered).unwrap().value;
    assert_eq!(score, MusicXml.read(&reversed).unwrap().value);
    assert_eq!(score.parts[0].measures[0].events[0].id.0, "mx_n1_");
}

#[test]
fn input_changed_between_passes_never_returns_a_mixed_score() {
    use std::io::{self, BufRead, Read, Seek, SeekFrom};
    struct Changing {
        current: Cursor<Vec<u8>>,
        replacement: Option<Vec<u8>>,
    }
    impl Read for Changing {
        fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
            self.current.read(out)
        }
    }
    impl BufRead for Changing {
        fn fill_buf(&mut self) -> io::Result<&[u8]> {
            self.current.fill_buf()
        }
        fn consume(&mut self, count: usize) {
            self.current.consume(count);
        }
    }
    impl Seek for Changing {
        fn seek(&mut self, seek: SeekFrom) -> io::Result<u64> {
            if seek != SeekFrom::Start(0) {
                return self.current.seek(seek);
            }
            if let Some(replacement) = self.replacement.take() {
                self.current = Cursor::new(replacement);
            }
            self.current.seek(seek)
        }
    }
    let source = generated(2, 2);
    for changed in [
        source.replace("<step>C</step>", "<step>D</step>"),
        source.replace("<part-name/>", "<part-name>Changed</part-name>"),
        source.replace("version=\"4.0\"", "version=\"3.1\""),
    ] {
        let input = Changing {
            current: Cursor::new(source.as_bytes().to_vec()),
            replacement: Some(changed.into_bytes()),
        };
        let report = musicxml::read_reader(input).unwrap_err();
        assert!(
            report
                .diagnostics
                .iter()
                .any(|d| d.feature == "xml.changed")
        );
    }
}

#[test]
fn discarded_comments_do_not_consume_a_fragment_budget() {
    let comment = format!("<!--{}-->", "x".repeat(900_000));
    let comments = comment.repeat(10);
    for source in [
        generated(2, 1).replacen("</measure>", &format!("</measure>{comments}"), 1),
        generated(1, 1).replacen("</measure>", &format!("{comments}</measure>"), 1),
    ] {
        let summary = musicxml::scan(Cursor::new(source.as_bytes())).unwrap();
        assert!(summary.bytes > 8 * 1024 * 1024);
        assert!(summary.max_retained_nodes < 30);
        assert!(MusicXml.read(&source).is_ok());
    }
}

#[test]
fn pathological_single_structures_keep_local_safety_budgets() {
    let giant_measure = format!(
        "{HEADER}<measure number='1'>{}</measure>{END}",
        "<note/>".repeat(100_001)
    );
    let report = musicxml::scan(Cursor::new(giant_measure)).unwrap_err();
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.effect == MappingEffect::ResourceLimit && d.feature == "xml.structure")
    );
    let giant_header = format!(
        "<score-partwise><identification>{}</identification></score-partwise>",
        format!("<creator>{}</creator>", "x".repeat(900_000)).repeat(10)
    );
    let report = musicxml::scan(Cursor::new(giant_header)).unwrap_err();
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.effect == MappingEffect::ResourceLimit && d.feature == "xml.fragment")
    );
}

#[test]
fn discarded_cdata_and_reference_whitespace_do_not_consume_fragment_budget() {
    let whitespace = format!("<![CDATA[{}]]>&#32;", " ".repeat(900_000)).repeat(10);
    let source = generated(2, 1).replacen("</measure>", &format!("</measure>{whitespace}"), 1);
    let summary = musicxml::scan(Cursor::new(source.as_bytes())).unwrap();
    assert!(summary.bytes > 8 * 1024 * 1024);
    assert!(summary.max_retained_nodes < 30);
    assert!(MusicXml.read(&source).is_ok());
}

#[test]
fn copied_diagnostic_paths_count_toward_fragment_memory_budget() {
    let name = "n".repeat(200);
    let source = format!(
        "<root>{}{}{}</root>",
        format!("<{name}>").repeat(44),
        "<leaf/>".repeat(1_000),
        format!("</{name}>").repeat(44)
    );
    assert!(source.len() < 100_000);
    let report = musicxml::scan(Cursor::new(source)).unwrap_err();
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.feature == "xml.fragment" && d.effect == MappingEffect::ResourceLimit)
    );
}
