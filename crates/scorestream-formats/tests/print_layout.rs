use scorestream_core::{
    Rational,
    print_layout::{Layout, PrintLayout},
};
use scorestream_formats::{ScoreAdapter, musicxml::MusicXml, sfs::SfsAdapter};
const SYSTEM: &str = r#"<print new-page="no" page-number=" iv &amp; A&#10; "><system-layout><system-margins><left-margin>-0.125</left-margin><right-margin>0</right-margin></system-margins><top-system-distance>+12.50</top-system-distance></system-layout></print>"#;
const STAFF: &str =
    r#"<print><staff-layout number="1"><staff-distance>0</staff-distance></staff-layout></print>"#;
fn xml(print: &str) -> String {
    format!(
        r#"<score-partwise version="4.0"><part-list><score-part id="P"><part-name>Piano</part-name><score-instrument id="piano"><instrument-name>piano</instrument-name></score-instrument></score-part></part-list><part id="P"><measure id="m" number="one">{print}<attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time><clef><sign>G</sign><line>2</line></clef></attributes><note id="n"><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><type>whole</type></note></measure></part></score-partwise>"#
    )
}
#[test]
fn typed_native_xml_roundtrip_preserves_flags_labels_and_exact_tenths() {
    for print in [
        SYSTEM,
        STAFF,
        "<print/>",
        "<print new-page=\"yes\" page-number=\"\"/>",
    ] {
        let score = MusicXml.read(&xml(print)).unwrap().value;
        let native = SfsAdapter.write(&score).unwrap().value;
        assert!(native.contains("scorestream.print@1"));
        let recovered = SfsAdapter.read(&native).unwrap().value;
        assert_eq!(score.print_layout, recovered.print_layout);
        let output = MusicXml.write(&recovered).unwrap().value;
        let again = MusicXml.read(&output).unwrap().value;
        assert_eq!(score.print_layout, again.print_layout);
    }
    let p = MusicXml
        .read(&xml(SYSTEM))
        .unwrap()
        .value
        .print_layout
        .remove(0);
    assert_eq!(p.new_page, Some(false));
    assert_eq!(p.page_number.as_deref(), Some(" iv & A\n "));
    assert_eq!(
        p.layout,
        Layout::System {
            left_margin: "-1/8".parse().unwrap(),
            right_margin: Rational::zero(),
            top_system_distance: "25/2".parse().unwrap()
        }
    );
    let p = MusicXml
        .read(&xml(STAFF))
        .unwrap()
        .value
        .print_layout
        .remove(0);
    assert_eq!(p.new_page, None);
    assert_eq!(p.page_number, None);
}
#[test]
fn wider_print_shapes_duplicate_and_mid_measure_controls_are_not_discarded() {
    for print in [
        SYSTEM.replace("new-page=\"no\"", "new-page=\"maybe\""),
        SYSTEM.replace("new-page", "new-system"),
        SYSTEM.replace("-0.125", "NaN"),
        SYSTEM.replace("-0.125", "1/3"),
        SYSTEM.replace("top-system-distance", "system-distance"),
        STAFF.replace("number=\"1\"", "number=\"2\""),
        STAFF.replace("number=\"1\"", ""),
        format!("{SYSTEM}{STAFF}"),
        "<print><page-layout/></print>".into(),
        SYSTEM.replace(
            "</print>",
            &format!(
                "{} </print>",
                STAFF
                    .trim_start_matches("<print>")
                    .trim_end_matches("</print>")
            ),
        ),
    ] {
        assert!(MusicXml.read(&xml(&print)).is_err(), "{print}");
    }
    let late = xml("").replace("</measure>", &format!("{STAFF}</measure>"));
    assert!(MusicXml.read(&late).is_err());
}
#[test]
fn public_and_native_models_validate_ownership_onset_and_closed_fields() {
    let score = MusicXml.read(&xml(STAFF)).unwrap().value;
    for p in [
        PrintLayout {
            part: "foreign".into(),
            ..score.print_layout[0].clone()
        },
        PrintLayout {
            measure: "missing".into(),
            ..score.print_layout[0].clone()
        },
        PrintLayout {
            layout: Layout::Staff {
                number: 0,
                distance: Rational::zero(),
            },
            ..score.print_layout[0].clone()
        },
        PrintLayout {
            layout: Layout::Staff {
                number: 1,
                distance: "1/3".parse().unwrap(),
            },
            ..score.print_layout[0].clone()
        },
    ] {
        let mut bad = score.clone();
        bad.print_layout = vec![p];
        assert!(SfsAdapter.write(&bad).is_err());
        assert!(MusicXml.write(&bad).is_err());
    }
    let native = SfsAdapter.write(&score).unwrap().value;
    let doc = scorestream_codec::parse(&native).unwrap();
    let mut bad = doc.clone();
    bad.required_extensions.clear();
    assert!(scorestream_core::print_layout::decode(&bad).is_err());
    assert!(
        scorestream_core::validate(&bad, None)
            .iter()
            .any(|d| d.code == "print.wire")
    );
    let mut duplicated = doc.clone();
    for frame in &mut duplicated.frames {
        let more:Vec<_>=frame.records.iter().filter(|r|matches!(r,scorestream_core::Record::Mark(m) if m.kind==scorestream_core::print_layout::KIND)).cloned().collect();
        frame.records.extend(more);
    }
    assert!(scorestream_core::print_layout::decode(&duplicated).is_err());
    let mut unknown = doc.clone();
    unknown.required_extensions = vec!["scorestream.print@2".into()];
    assert!(
        scorestream_core::validate(&unknown, None)
            .iter()
            .any(|d| d.code == "extension_unsupported")
    );
    for (key, val) in [
        ("extra", scorestream_core::Scalar::Integer(1)),
        ("left_margin", scorestream_core::Scalar::String("0".into())),
        ("part", scorestream_core::Scalar::String("foreign".into())),
    ] {
        let mut bad = doc.clone();
        for f in &mut bad.frames {
            for r in &mut f.records {
                if let scorestream_core::Record::Mark(m) = r {
                    if m.kind == scorestream_core::print_layout::KIND {
                        m.attributes.insert(key.into(), val.clone());
                    }
                }
            }
        }
        assert!(scorestream_core::print_layout::decode(&bad).is_err());
    }
    let mut bad = doc.clone();
    let mut marks = vec![];
    for f in &mut bad.frames {
        f.records.retain(|r|{if matches!(r,scorestream_core::Record::Mark(m) if m.kind==scorestream_core::print_layout::KIND){marks.push(r.clone());false}else{true}});
    }
    let mut late = bad.frames[0].clone();
    late.time = Rational::from_integer(1);
    late.records = marks;
    bad.frames.push(late);
    assert!(scorestream_core::print_layout::decode(&bad).is_err());
}
#[test]
fn print_metadata_does_not_change_playback_or_limit_decimal_precision() {
    let high = format!("0.{}1", "0".repeat(90));
    let score = MusicXml
        .read(&xml(&STAFF.replace(">0<", &format!(">{high}<"))))
        .unwrap()
        .value;
    let mut plain = score.clone();
    plain.print_layout.clear();
    assert_eq!(
        scorestream_app::playback::plan_source(&SfsAdapter.write(&score).unwrap().value).unwrap(),
        scorestream_app::playback::plan_source(&SfsAdapter.write(&plain).unwrap().value).unwrap()
    );
    assert!(MusicXml.write(&score).unwrap().value.contains(&high));
}

#[test]
fn same_measure_labels_in_two_parts_keep_distinct_hosts() {
    let declaration = r#"<score-part id="Q"><part-name>Second</part-name><score-instrument id="other"><instrument-name>piano</instrument-name></score-instrument></score-part>"#;
    let second = xml(STAFF)
        .split("</part-list>")
        .nth(1)
        .unwrap()
        .trim_end_matches("</score-partwise>")
        .replace("<part id=\"P\">", "<part id=\"Q\">")
        .replace("id=\"m\"", "id=\"other_measure\"")
        .replace("id=\"n\"", "id=\"other_note\"");
    let input = xml(SYSTEM)
        .replace("</part-list>", &format!("{declaration}</part-list>"))
        .replace("</score-partwise>", &format!("{second}</score-partwise>"));
    let score = MusicXml.read(&input).unwrap().value;
    assert_eq!(score.print_layout.len(), 2);
    assert_eq!(score.print_layout[0].part, "P");
    assert_eq!(score.print_layout[0].measure, "m");
    assert_eq!(score.print_layout[1].part, "Q");
    assert_eq!(score.print_layout[1].measure, "other_measure");
    let recovered = SfsAdapter
        .read(&SfsAdapter.write(&score).unwrap().value)
        .unwrap()
        .value;
    for p in &score.print_layout {
        assert!(recovered.print_layout.contains(p));
    }
    let written = MusicXml.write(&recovered).unwrap().value;
    let again = MusicXml.read(&written).unwrap().value;
    for p in &score.print_layout {
        assert!(again.print_layout.contains(p));
    }
}
