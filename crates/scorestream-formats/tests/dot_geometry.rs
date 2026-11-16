use scorestream_core::{Rational, Record, Scalar, geometry::Geometry};
use scorestream_formats::{
    ScoreAdapter,
    musicxml::{self, GeometryPolicy, MusicXml, MusicXmlOptions},
    sfs::SfsAdapter,
};

fn options() -> MusicXmlOptions {
    MusicXmlOptions {
        geometry: GeometryPolicy::PreserveNoteAndMeasureCoordinates,
        ..Default::default()
    }
}
fn xml(notes: &str) -> String {
    format!(
        r#"<score-partwise version="4.0"><part-list><score-part id="P"><part-name>Piano</part-name><score-instrument id="piano"><instrument-name>piano</instrument-name></score-instrument></score-part></part-list><part id="P"><measure id="m" number="1"><attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time><clef><sign>G</sign><line>2</line></clef></attributes>{notes}</measure></part></score-partwise>"#
    )
}
fn note(dots: &str) -> String {
    format!(
        r#"<note id="n" default-x="12"><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><type>whole</type>{dots}</note>"#
    )
}
fn model(dots: &str) -> scorestream_core::score::ScoreDocument {
    musicxml::read_with_options(&xml(&note(dots)), &options())
        .unwrap()
        .value
}
fn dot(event: &str, index: u32, x: Option<&str>, y: Option<&str>) -> Geometry {
    Geometry::Dot {
        event: event.into(),
        index,
        default_x: x.map(|s| s.parse().unwrap()),
        default_y: y.map(|s| s.parse().unwrap()),
    }
}
#[test]
fn sparse_dot_coordinates_preserve_order_absence_zero_and_exact_value_independently() {
    let score = model(
        r#"<dot/><dot default-x="+0.000"/><dot default-y="-.125"/><dot default-x="+31.000" default-y="-10.50"/>"#,
    );
    let expected = vec![
        dot("n", 1, Some("0"), None),
        dot("n", 2, None, Some("-1/8")),
        dot("n", 3, Some("31"), Some("-21/2")),
    ];
    for g in &expected {
        assert!(score.geometry.contains(g));
    }
    let e = &score.parts[0].measures[0].events[0];
    assert_eq!(e.written.as_ref().unwrap().dots, 4);
    assert_eq!(e.duration, Rational::from_integer(4));
    assert!(score.geometry.contains(&Geometry::Event {
        event: "n".into(),
        default_x: Some(Rational::from_integer(12)),
        default_y: None
    }));
    let wire = SfsAdapter.write(&score).unwrap().value;
    assert_eq!(wire.matches("scorestream.geometry.dot").count(), 3);
    let recovered = SfsAdapter.read(&wire).unwrap().value;
    for g in &expected {
        assert!(recovered.geometry.contains(g));
    }
    assert_eq!(SfsAdapter.write(&recovered).unwrap().value, wire);
    let output = MusicXml.write(&recovered).unwrap().value;
    assert!(output.contains("<dot/>\n        <dot default-x=\"0\"/>\n        <dot default-y=\"-0.125\"/>\n        <dot default-x=\"31\" default-y=\"-10.5\"/>"));
    let again = musicxml::read_with_options(&output, &options())
        .unwrap()
        .value;
    assert_eq!(MusicXml.write(&again).unwrap().value, output);
    assert_eq!(again.parts[0].measures[0].events[0].written, e.written);
    assert_eq!(again.parts[0].measures[0].events[0].duration, e.duration);
    let mut shuffled = recovered.clone();
    shuffled.geometry.reverse();
    assert_eq!(MusicXml.write(&shuffled).unwrap().value, output);
}
#[test]
fn grace_chord_and_rest_dot_coordinates_keep_their_own_event_identity() {
    let notes = r#"<note id="g"><grace/><pitch><step>D</step><octave>4</octave></pitch><type>eighth</type><dot default-y="-1"/></note><note id="h"><pitch><step>F</step><octave>4</octave></pitch><duration>1</duration><type>quarter</type></note><note id="n"><pitch><step>C</step><octave>4</octave></pitch><duration>1</duration><type>quarter</type><dot default-x="0"/></note><note id="c"><chord/><pitch><step>E</step><octave>4</octave></pitch><duration>1</duration><type>quarter</type><dot/><dot default-x="2.5"/></note><note id="r"><rest/><duration>2</duration><type>quarter</type><dot default-y="3"/></note>"#;
    let score = musicxml::read_with_options(&xml(notes), &options())
        .unwrap()
        .value;
    let expected = [
        dot("g", 0, None, Some("-1")),
        dot("n", 0, Some("0"), None),
        dot("c", 1, Some("5/2"), None),
        dot("r", 0, None, Some("3")),
    ];
    let wire = SfsAdapter.write(&score).unwrap().value;
    let recovered = SfsAdapter.read(&wire).unwrap().value;
    let output = MusicXml.write(&recovered).unwrap().value;
    let again = musicxml::read_with_options(&output, &options())
        .unwrap()
        .value;
    for g in expected {
        assert!(again.geometry.contains(&g));
    }
}
#[test]
fn exact_large_decimals_do_not_change_playback() {
    let high = format!("-0.{}1", "0".repeat(150));
    let score = model(&format!("<dot default-x=\"{high}\"/>"));
    assert!(MusicXml.write(&score).unwrap().value.contains(&high));
    let mut omitted = score.clone();
    omitted.geometry.clear();
    assert_eq!(
        scorestream_app::playback::plan_source(&SfsAdapter.write(&score).unwrap().value).unwrap(),
        scorestream_app::playback::plan_source(&SfsAdapter.write(&omitted).unwrap().value).unwrap()
    );
}
#[test]
fn old_strict_and_regeneration_policies_still_reject_dot_coordinates() {
    let input = xml(&note("<dot default-x=\"0\"/>"));
    assert!(MusicXml.read(&input).is_err());
    assert!(
        musicxml::read_with_options(
            &input,
            &MusicXmlOptions {
                geometry: GeometryPolicy::RegenerateNoteAndMeasureCoordinates,
                ..Default::default()
            }
        )
        .is_err()
    );
    let plain = MusicXml
        .read(&xml(&note("<dot/>").replace(" default-x=\"12\"", "")))
        .unwrap()
        .value;
    assert!(plain.geometry.is_empty());
    assert_eq!(
        plain.parts[0].measures[0].events[0]
            .written
            .as_ref()
            .unwrap()
            .dots,
        1
    );
}
#[test]
fn scope_and_exact_decimal_failures_never_silently_strip_fields() {
    for dots in [
        "<dot default-x=\"NaN\"/>",
        "<dot default-x=\"1e3\"/>",
        "<dot default-x=\"1/2\"/>",
        "<dot default-x=\"\u{a0}1\"/>",
        "<dot relative-x=\"1\"/>",
        "<dot color=\"#000000\"/>",
        "<dot default-x=\"1\"><other/></dot>",
        "<dot>1</dot>",
        "<time-modification><actual-notes>3</actual-notes><normal-notes>2</normal-notes><normal-type>quarter</normal-type><normal-dot default-x=\"1\"/></time-modification>",
    ] {
        assert!(
            musicxml::read_with_options(&xml(&note(dots)), &options()).is_err(),
            "{dots}"
        );
    }
    assert!(
        musicxml::read_with_options(
            &xml(&note("<dot default-x=\"0\"/>").replace("<type>whole</type>", "")),
            &options()
        )
        .is_err()
    );
}
#[test]
fn public_model_requires_existing_child_index_unique_owner_and_decimal_fields() {
    let score = model("<dot default-x=\"0\"/>");
    for bad_dot in [
        dot("missing", 0, Some("0"), None),
        dot("n", 1, Some("0"), None),
        dot("n", 0, None, None),
        dot("n", 0, Some("1/3"), None),
    ] {
        let mut bad = score.clone();
        bad.geometry = vec![bad_dot];
        assert!(SfsAdapter.write(&bad).is_err());
        assert!(MusicXml.write(&bad).is_err());
    }
    let mut bad = score.clone();
    bad.geometry.push(dot("n", 0, Some("2"), None));
    assert!(SfsAdapter.write(&bad).is_err());
    let mut bad = score;
    bad.parts[0].measures[0].events[0].written = None;
    assert!(MusicXml.write(&bad).is_err());
}
#[test]
fn wire_validates_index_onset_owner_and_fields_before_conversion() {
    let wire = SfsAdapter
        .write(&model("<dot/><dot default-y=\"-1\"/>"))
        .unwrap()
        .value;
    let doc = scorestream_codec::parse(&wire).unwrap();
    for index in [
        Scalar::Integer(-1),
        Scalar::Integer(2),
        Scalar::String("1".into()),
        Scalar::Integer(i64::MAX),
    ] {
        let mut bad = doc.clone();
        for f in &mut bad.frames {
            for r in &mut f.records {
                if let Record::Mark(m) = r {
                    if m.kind == scorestream_core::geometry::DOT {
                        m.attributes.insert("index".into(), index.clone());
                    }
                }
            }
        }
        assert!(scorestream_core::geometry::decode(&bad).is_err());
    }
    for mode in 0..6 {
        let mut bad = doc.clone();
        if mode == 0 {
            bad.required_extensions.clear();
        }
        let mut duplicate = None;
        for f in &mut bad.frames {
            for r in &mut f.records {
                if let Record::Mark(m) = r {
                    if m.kind == scorestream_core::geometry::DOT {
                        match mode {
                            1 => m.target = "#missing".into(),
                            2 => {
                                m.attributes.insert("other".into(), Scalar::Integer(1));
                            }
                            3 => {
                                m.attributes.remove("index");
                            }
                            4 => duplicate = Some(m.clone()),
                            5 => duplicate = Some(m.clone()),
                            _ => {}
                        }
                    }
                }
            }
        }
        if let Some(m) = duplicate {
            if mode == 5 {
                for f in &mut bad.frames {
                    f.records.retain(
                        |r| !matches!(r,Record::Mark(m) if m.kind==scorestream_core::geometry::DOT),
                    );
                }
                let mut late = bad.frames[0].clone();
                late.time = Rational::from_integer(1);
                late.records = vec![Record::Mark(m)];
                bad.frames.push(late);
            } else {
                bad.frames[0].records.push(Record::Mark(m));
            }
        }
        assert!(
            scorestream_core::geometry::decode(&bad).is_err(),
            "mode {mode}"
        );
    }
}

#[test]
fn pinned_beethoven_dot_samples_retain_signed_coordinates_on_each_movement() {
    // Direct dot samples from the pinned originals. Full original hashes/counts in docs.
    for (x, y) in [
        ("94.85", "-15"),
        ("176.29", "15"),
        ("36.76", "25"),
        ("31", "-15"),
    ] {
        let score = model(&format!("<dot default-x=\"{x}\" default-y=\"{y}\"/>"));
        let output = MusicXml
            .write(
                &SfsAdapter
                    .read(&SfsAdapter.write(&score).unwrap().value)
                    .unwrap()
                    .value,
            )
            .unwrap()
            .value;
        assert!(output.contains(&format!("<dot default-x=\"{x}\" default-y=\"{y}\"/>")));
    }
}
