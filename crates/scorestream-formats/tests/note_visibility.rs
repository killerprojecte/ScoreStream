use scorestream_core::{Record, Scalar, score::ScoreDocument};
use scorestream_formats::{
    ScoreAdapter,
    musicxml::{self, GeometryPolicy, MusicXml, MusicXmlOptions},
    sfs::SfsAdapter,
};
fn xml(notes: &str) -> String {
    format!(
        r#"<score-partwise version="4.0"><part-list><score-part id="p"><part-name>Piano</part-name><score-instrument id="i"><instrument-name>piano</instrument-name></score-instrument></score-part></part-list><part id="p"><measure id="m" number="1"><attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes>{notes}</measure></part></score-partwise>"#
    )
}
fn note(id: &str, visibility: &str, body: &str) -> String {
    format!(r#"<note id="{id}"{visibility}>{body}</note>"#)
}
const PITCH: &str =
    "<pitch><step>C</step><octave>4</octave></pitch><duration>4</duration><type>whole</type>";
fn roundtrip(score: &ScoreDocument) -> (ScoreDocument, String) {
    let wire = SfsAdapter.write(score).unwrap().value;
    let restored = SfsAdapter.read(&wire).unwrap().value;
    assert_eq!(score, &restored);
    assert_eq!(wire, SfsAdapter.write(&restored).unwrap().value);
    let output = MusicXml.write(&restored).unwrap().value;
    let again = MusicXml.read(&output).unwrap().value;
    assert_eq!(score, &again);
    assert_eq!(output, MusicXml.write(&again).unwrap().value);
    (again, wire)
}
#[test]
fn absent_yes_no_are_distinct_on_notes_and_do_not_change_sound() {
    let mut plans = vec![];
    for (attribute, expected) in [
        ("", None),
        (" print-object=\"yes\"", Some(true)),
        (" print-object=\"no\"", Some(false)),
    ] {
        let score = MusicXml
            .read(&xml(&note("n", attribute, PITCH)))
            .unwrap()
            .value;
        assert_eq!(score.parts[0].measures[0].events[0].visible, expected);
        let json = serde_json::to_value(&score).unwrap();
        assert_eq!(
            serde_json::from_value::<ScoreDocument>(json).unwrap(),
            score
        );
        let (_, wire) = roundtrip(&score);
        assert_eq!(wire.contains("visible="), expected.is_some());
        let output = MusicXml.write(&score).unwrap().value;
        assert_eq!(output.contains("print-object="), expected.is_some());
        plans.push(scorestream_app::playback::plan_source(&wire).unwrap());
    }
    assert_eq!(plans[0], plans[1]);
    assert_eq!(plans[0], plans[2]);
    assert_eq!(plans[2].notes.len(), 1);
    assert_eq!(plans[2].notes[0].midi_note, 60.0);
}
#[test]
fn independent_rest_and_chord_member_visibility_is_not_muting_or_accidental_visibility() {
    let notes = r#"<note id="n" print-object="no"><pitch><step>C</step><octave>4</octave></pitch><duration>2</duration><type>half</type><accidental>natural</accidental></note><note id="c" print-object="yes"><chord/><pitch><step>E</step><octave>4</octave></pitch><duration>2</duration><type>half</type></note><note id="r" print-object="no"><rest/><duration>2</duration><type>quarter</type><dot/></note>"#;
    let score = MusicXml.read(&xml(notes)).unwrap().value;
    let (_, wire) = roundtrip(&score);
    let e = &score.parts[0].measures[0].events;
    assert_eq!(
        e.iter().map(|e| e.visible).collect::<Vec<_>>(),
        vec![Some(false), Some(true), Some(false)]
    );
    assert!(e[0].accidental.is_some());
    assert_eq!(e[2].written.as_ref().unwrap().dots, 1);
    let mut visible = score.clone();
    for e in &mut visible.parts[0].measures[0].events {
        e.visible = None;
    }
    assert_eq!(
        scorestream_app::playback::plan_source(&wire).unwrap(),
        scorestream_app::playback::plan_source(&SfsAdapter.write(&visible).unwrap().value).unwrap()
    );
}
#[test]
fn grace_visibility_preserves_owner_slash_written_value_and_explicit_timing() {
    for v in ["", " print-object=\"yes\"", " print-object=\"no\""] {
        let grace = note(
            "g",
            v,
            "<grace slash=\"yes\" steal-time-following=\"12.5\"/><pitch><step>D</step><octave>4</octave></pitch><type>eighth</type><dot/>",
        );
        let score = MusicXml
            .read(&xml(&(grace + &note("n", "", PITCH))))
            .unwrap()
            .value;
        let (again, wire) = roundtrip(&score);
        // Authored grace performance remains unsupported; visibility must not change that policy.
        assert!(
            scorestream_app::playback::plan_source(&wire)
                .unwrap_err()
                .contains("grace.performance_unsupported")
        );
        assert_eq!(
            again.graces[0].visible,
            if v.is_empty() {
                None
            } else {
                Some(v.contains("yes"))
            }
        );
        assert_eq!(again.graces[0].anchor.0, "n");
        assert_eq!(again.graces[0].slash, Some(true));
        assert_eq!(
            again.graces[0].timing.steal_following,
            Some("25/2".parse().unwrap())
        );
        assert_eq!(again.graces[0].written.as_ref().unwrap().dots, 1);
    }
}
#[test]
fn full_measure_rest_and_existing_native_visible_attribute_roundtrip() {
    let score = MusicXml
        .read(&xml(&note(
            "r",
            " print-object=\"no\"",
            "<rest measure=\"yes\"/><duration>4</duration>",
        )))
        .unwrap()
        .value;
    let (_, wire) = roundtrip(&score);
    assert!(wire.contains("visible=false"));
    assert!(wire.contains("measure_rest=true"));
    let doc = scorestream_codec::parse(&wire).unwrap();
    for value in [Scalar::String("no".into()), Scalar::Integer(0)] {
        let mut bad = doc.clone();
        for f in &mut bad.frames {
            for r in &mut f.records {
                if let Record::Event(e) = r {
                    e.attributes.insert("visible".into(), value.clone());
                }
            }
        }
        assert!(scorestream_formats::sfs::from_document(&bad).is_err());
    }
}
#[test]
fn unsupported_print_controls_and_invalid_yes_no_remain_atomic() {
    for attribute in [
        "print-object=\"maybe\"",
        "print-object=\"true\"",
        "print-object=\"0\"",
        "print-spacing=\"no\"",
        "print-dot=\"yes\"",
        "print-lyric=\"yes\"",
        "print-leger=\"no\"",
        "print-object=\"no\" print-spacing=\"no\"",
    ] {
        let input = xml(&note("n", &format!(" {attribute}"), PITCH));
        assert!(MusicXml.read(&input).is_err(), "{attribute}");
    }
    for input in [
        xml(&note(
            "n",
            "",
            &PITCH.replace(
                "<type>whole</type>",
                "<type>whole</type><dot print-object=\"no\"/>",
            ),
        )),
        xml(&note(
            "n",
            "",
            &PITCH.replace(
                "<type>whole</type>",
                "<type>whole</type><accidental print-object=\"no\">natural</accidental>",
            ),
        )),
    ] {
        assert!(MusicXml.read(&input).is_err());
    }
}
#[test]
fn original_hidden_rest_samples_preserve_geometry_without_synthesizing_noteheads() {
    for (x, y, duration, divisions) in [("51.2", "-20", "4", "4"), ("48.07", "-173.73", "2", "2")] {
        let n = format!(
            r#"<note default-x="{x}" default-y="{y}" print-object="no"><rest/><duration>{duration}</duration><voice>1</voice><type>quarter</type></note>"#
        );
        let input = xml(&n).replace(
            "<divisions>1</divisions>",
            &format!("<divisions>{divisions}</divisions>"),
        );
        let options = MusicXmlOptions {
            geometry: GeometryPolicy::PreserveNoteAndMeasureCoordinates,
            ..Default::default()
        };
        let score = musicxml::read_with_options(&input, &options).unwrap().value;
        let sfs = SfsAdapter.write(&score).unwrap().value;
        let restored = SfsAdapter.read(&sfs).unwrap().value;
        let output = MusicXml.write(&restored).unwrap().value;
        let again = musicxml::read_with_options(&output, &options)
            .unwrap()
            .value;
        assert_eq!(score, again);
        let e = &again.parts[0].measures[0].events[0];
        assert_eq!(e.visible, Some(false));
        assert!(matches!(e.pitch, scorestream_core::Pitch::Rest));
        assert_eq!(e.duration, scorestream_core::Rational::one());
        assert!(output.contains("print-object=\"no\""));
    }
}

#[test]
fn yes_no_tokens_normalize_only_xml_whitespace() {
    for (value, expected) in [("  yes  ", true), ("&#9;no&#10;&#13;", false)] {
        let score = MusicXml
            .read(&xml(&note(
                "n",
                &format!(" print-object=\"{value}\""),
                PITCH,
            )))
            .unwrap()
            .value;
        assert_eq!(score.parts[0].measures[0].events[0].visible, Some(expected));
        roundtrip(&score);
    }
    for value in ["n o", "\u{a0}no", "yes\u{85}", ""] {
        assert!(
            MusicXml
                .read(&xml(&note(
                    "n",
                    &format!(" print-object=\"{value}\""),
                    PITCH
                )))
                .is_err()
        );
    }
}

#[test]
fn authored_notehead_roundtrips_without_changing_playback() {
    use scorestream_core::score::NoteheadShape;
    let original = MusicXml.read(&xml(&note("n", "", PITCH))).unwrap().value;
    let plan = |score: &ScoreDocument| scorestream_app::playback::plan_source(&SfsAdapter.write(score).unwrap().value).unwrap();
    let expected_plan = plan(&original);
    assert_eq!(original.parts[0].measures[0].events[0].notehead, None);
    for (name, shape) in [
        ("normal", NoteheadShape::Normal), ("x", NoteheadShape::X),
        ("diamond", NoteheadShape::Diamond), ("triangle", NoteheadShape::Triangle),
        ("slash", NoteheadShape::Slash), ("none", NoteheadShape::None),
    ] {
        let head = format!("<notehead>{name}</notehead>");
        let score = MusicXml.read(&xml(&note("n", "", &format!("{PITCH}{head}")))).unwrap().value;
        let (restored, wire) = roundtrip(&score);
        assert_eq!(restored.parts[0].measures[0].events[0].notehead, Some(shape));
        assert!(wire.contains(&format!("notehead={name}")));
        assert!(MusicXml.write(&restored).unwrap().value.contains(&head));
        assert_eq!(plan(&restored), expected_plan);
    }
}

#[test]
fn authored_notehead_does_not_drop_extra_attributes_or_custom_glyphs() {
    let unsupported = [
        "<notehead filled=\"no\">diamond</notehead>",
        "<notehead smufl=\"noteheadClusterSquareWhite\">other</notehead>",
        "<notehead>circle-x</notehead>",
        "<notehead>none</notehead><notehead>normal</notehead>",
        "<notehead/>",
    ];
    for tag in unsupported {
        assert!(MusicXml.read(&xml(&note("n", "", &format!("{PITCH}{tag}")))).is_err(), "{tag}");
    }
}

#[test]
fn authored_grace_notehead_belongs_only_to_the_grace() {
    let grace = note("g", "", "<grace slash=\"yes\"/><pitch><step>D</step><octave>4</octave></pitch><type>eighth</type><notehead>diamond</notehead>");
    let score = MusicXml.read(&xml(&(grace + &note("n", "", PITCH)))).unwrap().value;
    let (restored, _) = roundtrip(&score);
    assert_eq!(restored.graces[0].notehead, Some(scorestream_core::score::NoteheadShape::Diamond));
    assert_eq!(restored.graces[0].anchor.0, "n");
    assert_eq!(restored.parts[0].measures[0].events[0].notehead, None);
}
