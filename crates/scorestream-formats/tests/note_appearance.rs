use scorestream_core::{Pitch, Rational, Record, Scalar, Severity, score::*};
use scorestream_formats::{
    ScoreAdapter,
    musicxml::{self, IrregularMeasure, MusicXml, MusicXmlOptions},
    sfs::{self, SfsAdapter},
};
fn r(s: &str) -> Rational {
    s.parse().unwrap()
}
fn xml(notes: &str) -> String {
    format!(
        r#"<score-partwise version="4.0"><part-list><score-part id="p"><part-name>Flute</part-name><score-instrument id="i"><instrument-name>flute</instrument-name></score-instrument></score-part></part-list><part id="p"><measure id="m" number="1"><attributes><divisions>1</divisions><time><beats>3</beats><beat-type>4</beat-type></time></attributes>{notes}</measure></part></score-partwise>"#
    )
}
fn rest(flag: &str, duration: &str) -> String {
    xml(&format!(
        r#"<note id="n"><rest{flag}/><duration>{duration}</duration><voice>1</voice></note>"#
    ))
}
fn pitched(extra: &str) -> String {
    xml(&format!(
        r#"<note id="n"><pitch><step>C</step><octave>4</octave></pitch><duration>3</duration><voice>1</voice>{extra}</note>"#
    ))
}
fn roundtrip(source: &str) -> ScoreDocument {
    let score = MusicXml.read(source).unwrap().value;
    let wire = SfsAdapter.write(&score).unwrap().value;
    let common = SfsAdapter.read(&wire).unwrap().value;
    assert_eq!(score, common);
    let encoded = MusicXml.write(&common).unwrap().value;
    assert_eq!(MusicXml.read(&encoded).unwrap().value, common);
    let ast = scorestream_codec::parse(&wire).unwrap();
    let canonical = scorestream_codec::canonical(&ast);
    assert_eq!(SfsAdapter.read(&canonical).unwrap().value, common);
    score
}
#[test]
fn stems_are_independent_authored_fields_without_inventing_type_or_timing() {
    for name in [None, Some("up"), Some("down"), Some("none"), Some("double")] {
        let extra = name
            .map(|n| format!("<stem>{n}</stem>"))
            .unwrap_or_default();
        let score = roundtrip(&pitched(&extra));
        let event = &score.parts[0].measures[0].events[0];
        assert_eq!(event.stem, name.and_then(StemDirection::from_name));
        assert!(event.written.is_none());
        assert_eq!((&event.offset, &event.duration), (&r("0"), &r("3")));
        assert!(matches!(event.pitch, Pitch::Pitched { .. }));
        let source = SfsAdapter.write(&score).unwrap().value;
        let with = scorestream_app::playback::plan_source(&source).unwrap();
        let mut plain = score.clone();
        plain.parts[0].measures[0].events[0].stem = None;
        let without =
            scorestream_app::playback::plan_source(&SfsAdapter.write(&plain).unwrap().value)
                .unwrap();
        assert_eq!(
            serde_json::to_value(with).unwrap(),
            serde_json::to_value(without).unwrap()
        );
    }
}
#[test]
fn full_rest_identity_does_not_infer_whole_spelling_or_duration() {
    for flag in [None, Some(false), Some(true)] {
        let a = flag
            .map(|v| format!(" measure=\"{}\"", if v { "yes" } else { "no" }))
            .unwrap_or_default();
        let score = roundtrip(&rest(&a, "3"));
        let e = &score.parts[0].measures[0].events[0];
        assert_eq!(e.measure_rest, flag);
        assert_eq!(e.duration, r("3"));
        assert_eq!(e.pitch, Pitch::Rest);
        assert!(e.written.is_none());
        let wire = SfsAdapter.write(&score).unwrap().value;
        let plan = scorestream_app::playback::plan_source(&wire).unwrap();
        let mut plain = score.clone();
        plain.parts[0].measures[0].events[0].measure_rest = None;
        assert_eq!(
            serde_json::to_value(plan).unwrap(),
            serde_json::to_value(
                scorestream_app::playback::plan_source(&SfsAdapter.write(&plain).unwrap().value)
                    .unwrap()
            )
            .unwrap()
        );
    }
    let mut explicit = MusicXml.read(&rest(" measure=\"yes\"", "3")).unwrap().value;
    explicit.parts[0].measures[0].events[0].written = Some(WrittenDuration {
        note_type: NoteType::Whole,
        dots: 0,
        time_modification: None,
        size: None,
    });
    // Exact authored type is retained even when it differs from elapsed rest time.
    let wire = SfsAdapter.write(&explicit).unwrap().value;
    assert_eq!(SfsAdapter.read(&wire).unwrap().value, explicit);
    assert_eq!(
        MusicXml
            .read(&MusicXml.write(&explicit).unwrap().value)
            .unwrap()
            .value,
        explicit
    );
}
#[test]
fn invalid_full_rest_contexts_and_unmapped_appearance_remain_blocking() {
    assert!(MusicXml.read(&rest(" measure=\"yes\"", "1")).is_err());
    for source in [
        rest(" measure=\"maybe\"", "3"),
        rest(" display-step=\"C\"", "3"),
        rest("", "3").replace(
            "<rest/>",
            "<rest><display-step>C</display-step><display-octave>4</display-octave></rest>",
        ),
        pitched("<stem>sideways</stem>"),
        pitched("<stem color=\"#123456\">up</stem>"),
        pitched("<stem>\u{85}up</stem>"),
        pitched("<stem><unknown/></stem>"),
        rest(" measure=\"yes\"", "3")
            .replace("<note id=\"n\">", "<note id=\"n\" print-spacing=\"no\">"),
    ] {
        assert!(MusicXml.read(&source).is_err(), "accepted {source}");
        let options = MusicXmlOptions {
            geometry: musicxml::GeometryPolicy::RegenerateNoteAndMeasureCoordinates,
            ..Default::default()
        };
        assert!(
            musicxml::read_with_options(&source, &options).is_err(),
            "geometry widened {source}"
        );
    }
    let score = MusicXml.read(&rest(" measure=\"yes\"", "3")).unwrap().value;
    for mutate in [0, 1, 2] {
        let mut bad = score.clone();
        let e = &mut bad.parts[0].measures[0].events[0];
        match mutate {
            0 => e.duration = r("1"),
            1 => e.offset = r("1"),
            _ => {
                e.pitch = Pitch::Pitched {
                    step: 'C',
                    alteration: r("0"),
                    octave: 4.into(),
                }
            }
        }
        assert!(SfsAdapter.write(&bad).is_err());
        assert!(MusicXml.write(&bad).is_err());
    }
}
#[test]
fn stems_survive_graces_chord_members_and_legacy_rest_stemlets() {
    let source = xml(
        r#"<note id="g"><grace slash="yes"/><pitch><step>D</step><octave>4</octave></pitch><voice>1</voice><type>eighth</type><stem>up</stem></note><note id="n"><pitch><step>C</step><octave>4</octave></pitch><duration>3</duration><voice>1</voice><stem>down</stem></note>"#,
    );
    let score = roundtrip(&source);
    assert_eq!(score.graces[0].stem, Some(StemDirection::Up));
    assert_eq!(score.graces[0].timing, GraceTiming::default());
    let wire = SfsAdapter.write(&score).unwrap().value;
    assert!(scorestream_app::playback::plan_source(&wire).is_err()); // fidelity guard unchanged
    let chord=pitched("<stem>up</stem>").replace("</measure>",r#"<note id="c"><chord/><pitch><step>E</step><octave>4</octave></pitch><duration>3</duration><voice>1</voice><stem>down</stem></note></measure>"#);
    let score = roundtrip(&chord);
    assert_eq!(score.parts[0].measures[0].events.len(), 2);
    let stemlet = rest("", "3").replace("</note>", "<stem>up</stem></note>");
    assert_eq!(
        roundtrip(&stemlet).parts[0].measures[0].events[0].stem,
        Some(StemDirection::Up)
    );
}
#[test]
fn full_rest_validates_actual_irregular_extent_without_rewriting_meter() {
    let source = rest(" measure=\"yes\"", "5");
    assert!(MusicXml.read(&source).is_err());
    let options = MusicXmlOptions {
        irregular_measures: vec![IrregularMeasure {
            ordinal: 1,
            label: "1".into(),
            written_meter: Meter {
                beats: vec![3],
                beat_type: 4,
            },
            actual_length: r("5"),
        }],
        ..Default::default()
    };
    let common = musicxml::read_with_options(&source, &options)
        .unwrap()
        .value;
    assert_eq!(common.parts[0].measures[0].length, r("5"));
    assert_eq!(
        common.parts[0].measures[0].states[0]
            .meter
            .as_ref()
            .unwrap()
            .quarters(),
        r("3")
    );
    let sfs = SfsAdapter.write(&common).unwrap().value;
    assert_eq!(SfsAdapter.read(&sfs).unwrap().value, common);
    let output = musicxml::write_with_options(&common, &options)
        .unwrap()
        .value;
    assert_eq!(
        musicxml::read_with_options(&output, &options)
            .unwrap()
            .value,
        common
    );
}
#[test]
fn direct_ast_and_native_validation_keep_legacy_fields_and_check_full_rest_ownership() {
    let score = MusicXml.read(&rest(" measure=\"yes\"", "3")).unwrap().value;
    let doc = sfs::to_document(&score).unwrap().value;
    let json = serde_json::to_string(&doc).unwrap();
    let doc: scorestream_core::Document = serde_json::from_str(&json).unwrap();
    assert!(
        !scorestream_core::validate(&doc, None)
            .iter()
            .any(|d| d.severity == Severity::Error)
    );
    for mutation in 0..4 {
        let mut bad = doc.clone();
        for frame in &mut bad.frames {
            for rec in &mut frame.records {
                if let Record::Event(e) = rec {
                    match mutation {
                        0 => e
                            .attributes
                            .insert("measure_rest".into(), Scalar::String("yes".into())),
                        1 => {
                            e.pitch = Pitch::Pitched {
                                step: 'C',
                                alteration: r("0"),
                                octave: 4.into(),
                            };
                            None
                        }
                        2 => {
                            e.duration = scorestream_core::Duration::Measured(r("2"));
                            None
                        }
                        _ => None,
                    };
                }
            }
            if mutation == 3 {
                frame.records.retain(|r| !matches!(r, Record::Measure(_)));
            }
        }
        assert!(
            scorestream_core::validate(&bad, None)
                .iter()
                .any(|d| d.severity == Severity::Error),
            "mutation {mutation}"
        );
        assert!(sfs::from_document(&bad).is_err());
    }
    // Old common-model JSON lacks both fields; it must still deserialize and stay absent.
    let mut value = serde_json::to_value(&score).unwrap();
    let e = &mut value["parts"][0]["measures"][0]["events"][0];
    e.as_object_mut().unwrap().remove("measure_rest");
    e.as_object_mut().unwrap().remove("stem");
    let legacy: ScoreDocument = serde_json::from_value(value).unwrap();
    assert!(legacy.parts[0].measures[0].events[0].measure_rest.is_none());
    assert!(legacy.parts[0].measures[0].events[0].stem.is_none());
    roundtrip(&MusicXml.write(&legacy).unwrap().value);
}

#[test]
fn full_rest_ownership_honors_staff_overrides_and_same_onset_voice_state() {
    let score = MusicXml.read(&rest(" measure=\"yes\"", "3")).unwrap().value;
    let mut doc = sfs::to_document(&score).unwrap().value;
    let pid = doc.parts[0].id.clone();
    let voice = doc.voices[0].id.clone();
    doc.parts[0].staves = 2;
    for record in &mut doc.frames[0].records {
        match record {
            Record::Measure(m) => m.target = format!("{pid}.s2"),
            Record::Event(e) => {
                e.attributes.remove("staff");
            }
            _ => {}
        }
    }
    assert!(
        scorestream_core::validate(&doc, None)
            .iter()
            .any(|d| d.code == "rest.measure_extent")
    );
    let mut attrs = scorestream_core::Attributes::new();
    attrs.insert("staff".into(), Scalar::Integer(2));
    // Appended after the event: state still takes effect at this same onset.
    doc.frames[0]
        .records
        .push(Record::State(scorestream_core::State {
            target: voice,
            attributes: attrs,
            source: Default::default(),
        }));
    assert!(
        !scorestream_core::validate(&doc, None)
            .iter()
            .any(|d| d.severity == Severity::Error)
    );
    for record in &mut doc.frames[0].records {
        if let Record::Event(e) = record {
            e.attributes.insert("staff".into(), Scalar::Integer(1));
        }
    }
    assert!(
        scorestream_core::validate(&doc, None)
            .iter()
            .any(|d| d.code == "rest.measure_extent")
    );
    for record in &mut doc.frames[0].records {
        if let Record::Event(e) = record {
            e.attributes
                .insert("measure_rest".into(), Scalar::Boolean(false));
        }
    }
    // Explicit no does not assert full-measure ownership.
    assert!(
        !scorestream_core::validate(&doc, None)
            .iter()
            .any(|d| d.severity == Severity::Error)
    );
}

#[test]
fn source_stem_fragments_preserve_pitch_type_and_coordinate_policy() {
    // Actual original movement I, P1, measures 262 and 19 respectively; only the
    // small surrounding context is a fixture, not an alternate original score.
    for (stem, octave, y) in [("up", 4, "-40"), ("down", 6, "30")] {
        let note = format!(
            r#"<note default-x="13" default-y="{y}">
        <pitch>
          <step>E</step>
          <alter>-1</alter>
          <octave>{octave}</octave>
          </pitch>
        <duration>4</duration>
        <voice>1</voice>
        <type>quarter</type>
        <stem>{stem}</stem>
        </note>"#
        );
        let source = xml(&note)
            .replace("<divisions>1</divisions>", "<divisions>4</divisions>")
            .replace("<beats>3</beats>", "<beats>2</beats>");
        assert!(MusicXml.read(&source).is_err());
        let options = MusicXmlOptions {
            geometry: musicxml::GeometryPolicy::RegenerateNoteAndMeasureCoordinates,
            ..Default::default()
        };
        let score = musicxml::read_with_options(&source, &options)
            .unwrap()
            .value;
        let event = &score.parts[0].measures[0].events[0];
        assert_eq!(event.stem, StemDirection::from_name(stem));
        assert_eq!(event.duration, r("1"));
        assert_eq!(
            event.pitch,
            Pitch::Pitched {
                step: 'E',
                alteration: r("-1"),
                octave: octave.into()
            }
        );
        assert_eq!(event.written.as_ref().unwrap().note_type, NoteType::Quarter);
        assert_eq!(
            SfsAdapter
                .read(&SfsAdapter.write(&score).unwrap().value)
                .unwrap()
                .value,
            score
        );
        assert_eq!(
            MusicXml
                .read(&MusicXml.write(&score).unwrap().value)
                .unwrap()
                .value,
            score
        );
    }
}

#[test]
fn duration_does_not_promote_ordinary_typed_rests_to_full_measure_identity() {
    // Original I m342 P2 has a full rest; P3/P4 have ordinary half rests with
    // the same two-quarter duration. This bounded fixture exercises that distinction.
    for full in [false, true] {
        let source = rest(if full { " measure=\"yes\"" } else { "" }, "2")
            .replace("<beats>3</beats>", "<beats>2</beats>");
        let source = if full {
            source
        } else {
            source.replace("</note>", "<type>half</type></note>")
        };
        let score = roundtrip(&source);
        let event = &score.parts[0].measures[0].events[0];
        assert_eq!(event.measure_rest, if full { Some(true) } else { None });
        assert_eq!(
            event.written.as_ref().map(|w| w.note_type),
            if full { None } else { Some(NoteType::Half) }
        );
        assert_eq!(event.duration, r("2"));
    }
    // Ordinary eighth rest filling a pickup remains ordinary; no identity inference.
    let pickup = rest("", "1")
        .replace("<divisions>1</divisions>", "<divisions>2</divisions>")
        .replace("number=\"1\"", "number=\"0\" implicit=\"yes\"")
        .replace("</note>", "<type>eighth</type></note>");
    let score = roundtrip(&pickup);
    assert_eq!(score.parts[0].measures[0].length, r("1/2"));
    assert_eq!(score.parts[0].measures[0].events[0].measure_rest, None);
    // Full rest with a fermata remains a rest plus its independent written sign.
    let source = rest(" measure=\"yes\"", "3").replace(
        "</note>",
        "<notations><fermata type=\"upright\" relative-y=\"5\"/></notations></note>",
    );
    let score = roundtrip(&source);
    assert_eq!(score.fermatas.len(), 1);
    assert_eq!(
        score.fermatas[0].event,
        score.parts[0].measures[0].events[0].id
    );
    assert_eq!(
        score.parts[0].measures[0].events[0].measure_rest,
        Some(true)
    );
}
