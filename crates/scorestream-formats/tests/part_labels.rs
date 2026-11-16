use scorestream_formats::{ScoreAdapter, musicxml::MusicXml, sfs::SfsAdapter};
fn score(labels: &str) -> String {
    format!(
        r#"<score-partwise version="4.0"><part-list><score-part id="p"><part-name>Legacy Piano</part-name>{labels}<score-instrument id="i"><instrument-name>piano</instrument-name></score-instrument></score-part></part-list><part id="p"><measure id="m" number="1"><attributes><divisions>1</divisions></attributes><note id="n"><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><voice>1</voice></note></measure></part></score-partwise>"#
    )
}
fn display() -> &'static str {
    r#"<part-name-display><display-text>E</display-text><accidental-text>flat</accidental-text><display-text> Horn 2 </display-text></part-name-display>"#
}
fn roundtrip(xml: &str) -> (scorestream_core::score::ScoreDocument, String) {
    let m = MusicXml.read(xml).unwrap().value;
    let s = SfsAdapter.write(&m).unwrap().value;
    assert_eq!(m, SfsAdapter.read(&s).unwrap().value);
    assert_eq!(
        m,
        MusicXml
            .read(&MusicXml.write(&m).unwrap().value)
            .unwrap()
            .value
    );
    (m, s)
}
#[test]
fn authored_part_labels_roundtrip_without_rewriting_names_routing_or_sound() {
    let (m, s) = roundtrip(&score(
        &format!(
            "{}<part-abbreviation> Hn 2\n </part-abbreviation>",
            display()
        )
        .replace(
            "\n", "
",
        ),
    ));
    let p = &m.parts[0];
    assert_eq!(p.name.as_deref(), Some("Legacy Piano"));
    let l = p.labels.as_ref().unwrap();
    assert_eq!(
        l.abbreviation.as_deref(),
        Some(
            " Hn 2
 "
        )
    );
    assert_eq!(l.name_display.as_ref().unwrap().trailing_text, " Horn 2 ");
    assert_eq!(l.name_display.as_ref().unwrap().leading_text, "E");
    let mut plain = m.clone();
    plain.parts[0].labels = None;
    let mut a = serde_json::to_value(scorestream_app::playback::plan_source(&s).unwrap()).unwrap();
    let mut b = serde_json::to_value(
        scorestream_app::playback::plan_source(&SfsAdapter.write(&plain).unwrap().value).unwrap(),
    )
    .unwrap();
    for k in ["warnings", "diagnostics"] {
        a.as_object_mut().unwrap().remove(k);
        b.as_object_mut().unwrap().remove(k);
    }
    assert_eq!(a, b);
    let view = scorestream_app::Workspace::from_source(&s)
        .unwrap()
        .current();
    assert!(view.valid);
    assert!(
        view.diagnostics
            .iter()
            .any(|d| d.code == "part_labels.authored_only")
    );
    let doc = scorestream_formats::sfs::to_document(&m).unwrap().value;
    assert_eq!(doc.parts[0].display_name.as_deref(), Some("Legacy Piano"));
    assert_eq!(m.parts[0].instrument, plain.parts[0].instrument);
    assert_eq!(
        m.parts[0].instrument_metadata,
        plain.parts[0].instrument_metadata
    );
}
#[test]
fn absence_empty_abbreviation_and_literal_nonpitch_prefix_are_distinct() {
    let (none, _) = roundtrip(&score(""));
    assert_eq!(none.parts[0].labels, None);
    let (empty, _) = roundtrip(&score("<part-abbreviation/>"));
    assert_eq!(
        empty.parts[0]
            .labels
            .as_ref()
            .unwrap()
            .abbreviation
            .as_deref(),
        Some("")
    );
    let (m, _) = roundtrip(&score(&display().replace(">E<", ">Not a pitch &amp; E<")));
    let l = m.parts[0].labels.as_ref().unwrap();
    assert_eq!(l.abbreviation, None);
    assert_eq!(
        l.name_display.as_ref().unwrap().leading_text,
        "Not a pitch & E"
    );
}
#[test]
fn unrepresented_label_shapes_styles_and_accidentals_reject() {
    let report = MusicXml
        .read(&score(&display().replace("flat", "natural")))
        .unwrap_err();
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.feature == "part_labels.display_shape"
                && d.effect == scorestream_formats::MappingEffect::Unsupported)
    );
    for bad in [
        "<part-abbreviation font-style=\"italic\">Vln</part-abbreviation>".into(),
        "<part-abbreviation><b>Vln</b></part-abbreviation>".into(),
        "<part-abbreviation>a</part-abbreviation><part-abbreviation>b</part-abbreviation>".into(),
        display().replace("flat", "natural"),
        display().replace("<display-text>E</display-text>", ""),
        display().replace("<accidental-text>", "<accidental-text font-size=\"10\">"),
        display().replace(
            "</part-name-display>",
            "<display-text>extra</display-text></part-name-display>",
        ),
        "<part-abbreviation-display><display-text>x</display-text></part-abbreviation-display>"
            .into(),
    ] {
        assert!(MusicXml.read(&score(&bad)).is_err(), "{bad}");
    }
}
#[test]
fn native_labels_have_exact_part_host_time_closed_complete_fields() {
    let (m, _) = roundtrip(&score(&format!(
        "{}<part-abbreviation>Hn</part-abbreviation>",
        display()
    )));
    for mode in 0..7 {
        let mut d = scorestream_formats::sfs::to_document(&m).unwrap().value;
        if mode == 0 {
            d.required_extensions.clear();
        }
        let frame=d.frames.iter_mut().find(|f|f.records.iter().any(|r|matches!(r,scorestream_core::Record::Mark(m) if m.kind==scorestream_core::part_labels::KIND))).unwrap();
        if mode == 1 {
            frame.time = "1".parse().unwrap();
        }
        let mark = frame
            .records
            .iter_mut()
            .find_map(|r| match r {
                scorestream_core::Record::Mark(m)
                    if m.kind == scorestream_core::part_labels::KIND =>
                {
                    Some(m)
                }
                _ => None,
            })
            .unwrap();
        use scorestream_core::Scalar;
        match mode {
            2 => {
                mark.target = "missing".into();
            }
            3 => {
                mark.attributes.remove("name_leading_text");
            }
            4 => {
                mark.attributes
                    .insert("name_accidental".into(), Scalar::String("sharp".into()));
            }
            5 => {
                mark.attributes
                    .insert("abbreviation".into(), Scalar::String("".into()));
            }
            6 => {
                let duplicate = mark.clone();
                frame
                    .records
                    .push(scorestream_core::Record::Mark(duplicate));
            }
            _ => {}
        }
        assert!(scorestream_formats::sfs::from_document(&d).is_err());
    }
    let mut bad = m;
    bad.parts[0].labels.as_mut().unwrap().abbreviation = Some("".into());
    assert!(MusicXml.write(&bad).is_err());
}
