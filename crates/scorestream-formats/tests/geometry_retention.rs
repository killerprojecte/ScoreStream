use scorestream_core::{Rational, geometry::Geometry};
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
        r#"<score-partwise version="4.0"><part-list><score-part id="P"><part-name>Piano</part-name><score-instrument id="piano"><instrument-name>piano</instrument-name></score-instrument></score-part></part-list><part id="P"><measure id="bar" number="1" width="+151.940"><attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time><clef><sign>G</sign><line>2</line></clef></attributes>{notes}</measure></part></score-partwise>"#
    )
}
const NOTES: &str = r#"<note id="n" default-x="0" default-y="-10.125"><pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><type>whole</type></note><note id="c" default-y="0"><chord/><pitch><step>E</step><octave>4</octave></pitch><duration>4</duration><type>whole</type></note>"#;
#[test]
fn exact_coordinate_roundtrip_retains_zero_absence_chord_and_measure_identity() {
    let input = xml(NOTES);
    let converted = musicxml::read_with_options(&input, &options()).unwrap();
    assert_eq!(converted.value.geometry.len(), 3);
    assert!(
        converted
            .report
            .diagnostics
            .iter()
            .any(|d| d.feature == "geometry.preserved_coordinates")
    );
    assert!(converted.value.geometry.contains(&Geometry::Event {
        event: "n".into(),
        default_x: Some(Rational::zero()),
        default_y: Some("-81/8".parse().unwrap())
    }));
    assert!(converted.value.geometry.contains(&Geometry::Event {
        event: "c".into(),
        default_x: None,
        default_y: Some(Rational::zero())
    }));
    let native = SfsAdapter.write(&converted.value).unwrap().value;
    assert!(native.contains("scorestream.geometry@1"));
    let recovered = SfsAdapter.read(&native).unwrap().value;
    for g in &converted.value.geometry {
        assert!(recovered.geometry.contains(g));
    }
    let written = MusicXml.write(&recovered).unwrap().value;
    assert!(written.contains("width=\"151.94\""));
    assert!(written.contains("default-y=\"-10.125\""));
    let second = musicxml::read_with_options(&written, &options())
        .unwrap()
        .value;
    for g in &converted.value.geometry {
        assert!(second.geometry.contains(g));
    }
}
#[test]
fn explicit_strict_and_lossy_policies_are_unchanged() {
    assert!(MusicXml.read(&xml(NOTES)).is_err());
    let old = musicxml::read_with_options(
        &xml(NOTES),
        &MusicXmlOptions {
            geometry: GeometryPolicy::RegenerateNoteAndMeasureCoordinates,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(old.value.geometry.is_empty());
    assert!(
        old.report
            .diagnostics
            .iter()
            .any(|d| d.feature == "geometry.regenerate_coordinates")
    );
}
#[test]
fn unsupported_or_malformed_geometry_never_drops_silently() {
    for input in [
        xml(NOTES).replace("-10.125", "NaN"),
        xml(NOTES).replace("-10.125", "1e3"),
        xml(NOTES).replace("-10.125", "\u{a0}1"),
        xml(NOTES).replace("default-x", "relative-x"),
    ] {
        assert!(musicxml::read_with_options(&input, &options()).is_err());
    }
}
#[test]
fn public_model_rejects_unknown_duplicate_nondecimal_and_wrong_measure_hosts() {
    let score = musicxml::read_with_options(&xml(NOTES), &options())
        .unwrap()
        .value;
    for geometry in [
        Geometry::Event {
            event: "missing".into(),
            default_x: Some(Rational::zero()),
            default_y: None,
        },
        Geometry::Measure {
            part: "wrong".into(),
            measure: "bar".into(),
            width: Rational::zero(),
        },
        Geometry::Event {
            event: "n".into(),
            default_x: Some("1/3".parse().unwrap()),
            default_y: None,
        },
    ] {
        let mut bad = score.clone();
        bad.geometry = vec![geometry];
        assert!(SfsAdapter.write(&bad).is_err());
        assert!(MusicXml.write(&bad).is_err());
    }
    let mut duplicate = score.clone();
    duplicate.geometry.push(score.geometry[0].clone());
    assert!(SfsAdapter.write(&duplicate).is_err());
}
#[test]
fn native_wire_rejects_missing_extension_wrong_time_and_unknown_fields() {
    let score = musicxml::read_with_options(&xml(NOTES), &options())
        .unwrap()
        .value;
    let native = SfsAdapter.write(&score).unwrap().value;
    let doc = scorestream_codec::parse(&native).unwrap();
    let mut bad = doc.clone();
    bad.required_extensions
        .retain(|x| x != "scorestream.geometry@1");
    assert!(scorestream_core::geometry::decode(&bad).is_err());
    let mut bad = doc.clone();
    for frame in &mut bad.frames {
        for r in &mut frame.records {
            if let scorestream_core::Record::Mark(m) = r {
                if m.kind == scorestream_core::geometry::EVENT {
                    m.attributes
                        .insert("other".into(), scorestream_core::Scalar::Integer(1));
                }
            }
        }
    }
    assert!(scorestream_core::geometry::decode(&bad).is_err());
    let mut bad = doc.clone();
    let mut moved = vec![];
    for frame in &mut bad.frames {
        frame.records.retain(|r| {
            if matches!(r, scorestream_core::Record::Mark(m) if scorestream_core::geometry::is_kind(&m.kind)) {
                moved.push(r.clone()); false
            } else { true }
        });
    }
    let mut late = bad.frames[0].clone();
    late.time = Rational::from_integer(1);
    late.records = moved;
    bad.frames.push(late);
    assert!(scorestream_core::geometry::decode(&bad).is_err());
    let mut bad = doc.clone();
    for frame in &mut bad.frames {
        for r in &mut frame.records {
            if let scorestream_core::Record::Mark(m) = r {
                if m.kind == scorestream_core::geometry::MEASURE {
                    m.attributes.insert(
                        "part".into(),
                        scorestream_core::Scalar::String("foreign".into()),
                    );
                }
            }
        }
    }
    assert!(scorestream_core::geometry::decode(&bad).is_err());
}

#[test]
fn geometry_does_not_change_playback_and_arbitrary_decimal_precision_is_exact() {
    let high = format!("0.{}1", "0".repeat(90));
    let input = xml(NOTES).replace("-10.125", &high);
    let preserved = musicxml::read_with_options(&input, &options())
        .unwrap()
        .value;
    let mut omitted = preserved.clone();
    omitted.geometry.clear();
    let a = scorestream_app::playback::plan_source(&SfsAdapter.write(&preserved).unwrap().value)
        .unwrap();
    let b =
        scorestream_app::playback::plan_source(&SfsAdapter.write(&omitted).unwrap().value).unwrap();
    assert_eq!(a, b);
    let output = MusicXml.write(&preserved).unwrap().value;
    assert!(output.contains(&format!("default-y=\"{high}\"")));
}
#[test]
fn grace_coordinates_follow_grace_identity_not_principal() {
    let note = r#"<note id="g" default-x="-0.5" default-y="0"><grace slash="yes"/><pitch><step>D</step><octave>4</octave></pitch><type>eighth</type></note>"#;
    let input = xml(&format!(
        "{note}{}",
        NOTES.split("<note id=\"c\"").next().unwrap()
    ));
    let model = musicxml::read_with_options(&input, &options())
        .unwrap()
        .value;
    let source = SfsAdapter.write(&model).unwrap().value;
    let recovered = SfsAdapter.read(&source).unwrap().value;
    let g = Geometry::Event {
        event: "g".into(),
        default_x: Some("-1/2".parse().unwrap()),
        default_y: Some(Rational::zero()),
    };
    assert!(recovered.geometry.contains(&g));
    let output = MusicXml.write(&recovered).unwrap().value;
    assert!(output.contains("<note id=\"g\" default-x=\"-0.5\" default-y=\"0\"><grace"));
}
