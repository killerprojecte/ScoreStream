use scorestream_formats::{ScoreAdapter, musicxml::MusicXml, sfs::SfsAdapter};
fn score(body: &str) -> String {
    format!(
        r#"<score-partwise version="4.0"><part-list><score-part id="p"><part-name>Piano</part-name><score-instrument id="i"><instrument-name>piano</instrument-name></score-instrument></score-part></part-list><part id="p"><measure id="m" number="1"><attributes><divisions>1</divisions><time><beats>4</beats><beat-type>4</beat-type></time></attributes>{body}</measure></part></score-partwise>"#
    )
}
fn note(id: &str, value: Option<&str>, dur: u8, children: &str) -> String {
    let dynamics = value
        .map(|v| format!(" dynamics=\"{v}\""))
        .unwrap_or_default();
    format!(
        r#"<note id="{id}"{dynamics}>{children}<pitch><step>C</step><octave>4</octave></pitch><duration>{dur}</duration><voice>1</voice></note>"#
    )
}
fn convert(source: &str) -> (scorestream_core::score::ScoreDocument, String) {
    let model = MusicXml.read(source).unwrap().value;
    let sfs = SfsAdapter.write(&model).unwrap().value;
    assert_eq!(model, SfsAdapter.read(&sfs).unwrap().value);
    assert_eq!(
        model,
        MusicXml
            .read(&MusicXml.write(&model).unwrap().value)
            .unwrap()
            .value
    );
    (model, sfs)
}
#[test]
fn local_exact_attack_value_preserves_zero_absence_and_never_becomes_scope_state() {
    for value in ["44.44", "0", "132.22", "0.001"] {
        let source = score(&format!(
            "{}{}",
            note("a", Some(value), 2, ""),
            note("b", None, 2, "")
        ));
        let (model, sfs) = convert(&source);
        assert!(model.sound_dynamics.is_empty());
        assert!(
            model.parts[0].measures[0].events[1]
                .attack_dynamics
                .is_none()
        );
        let plan = scorestream_app::playback::plan_source(&sfs).unwrap();
        assert_eq!(plan.event_positions.len(), 2);
        let following = plan.notes.iter().find(|n| n.id == "b").unwrap();
        assert_eq!(following.velocity, 80);
        if value == "0" {
            assert_eq!(plan.notes.len(), 1);
            assert!(
                plan.diagnostics
                    .iter()
                    .any(|d| d.code == "note_dynamics.silent_attack")
            );
        } else {
            let n = plan.notes.iter().find(|n| n.id == "a").unwrap();
            let expected = value.parse::<f64>().unwrap() * 0.9;
            assert!((n.velocity as f64 * n.gain_envelope[0].gain - expected).abs() < 1e-9);
            assert!(
                n.gain_envelope
                    .iter()
                    .all(|p| p.gain == n.gain_envelope[0].gain)
            );
        }
    }
}
#[test]
fn chord_members_and_numeric_state_remain_separate_owners() {
    let source = score(&format!(
        "<sound dynamics=\"44.44\"/>{}{}{}",
        note("a", Some("100"), 2, ""),
        note("c", Some("50"), 2, "<chord/>"),
        note("b", None, 2, "")
    ));
    let (_, sfs) = convert(&source);
    let plan = scorestream_app::playback::plan_source(&sfs).unwrap();
    assert_eq!(
        plan.notes
            .iter()
            .map(|n| (n.id.as_str(), n.velocity))
            .collect::<Vec<_>>(),
        vec![("a", 90), ("c", 45), ("b", 40)]
    );
}
#[test]
fn real_tie_head_has_one_attack_and_continuation_value_never_retriggers() {
    let start = "<tie type=\"start\"/><notations><tied type=\"start\"/></notations>";
    let stop = "<tie type=\"stop\"/><notations><tied type=\"stop\"/></notations>";
    for value in ["50", "0"] {
        let (_, sfs) = convert(&score(&format!(
            "{}{}",
            note("a", Some(value), 2, start),
            note("b", None, 2, stop)
        )));
        let plan = scorestream_app::playback::plan_source(&sfs).unwrap();
        assert_eq!(plan.event_positions.len(), 2);
        assert_eq!(plan.notes.len(), if value == "0" { 0 } else { 1 });
        if value != "0" {
            assert_eq!(plan.notes[0].velocity, 45);
            assert!(plan.notes[0].duration_seconds > 1.8);
        }
    }
    let (_, sfs) = convert(&score(&format!(
        "{}{}",
        note("a", None, 2, start),
        note("b", Some("50"), 2, stop)
    )));
    assert!(
        scorestream_app::playback::plan_source(&sfs)
            .unwrap_err()
            .contains("note_dynamics.tie_continuation_unsupported")
    );
}
#[test]
fn changing_expression_is_not_turned_into_an_invented_sustain_policy() {
    let source = score(&format!(
        "<sound dynamics=\"44.44\"/><sound dynamics=\"100\"><offset>1</offset></sound>{}",
        note("a", Some("80"), 4, "")
    ));
    let (_, sfs) = convert(&source);
    assert!(
        scorestream_app::playback::plan_source(&sfs)
            .unwrap_err()
            .contains("note_dynamics.expression_unsupported")
    );
}
#[test]
fn invalid_input_and_unrepresentable_sound_are_distinct() {
    for value in ["-1", "NaN", "1e3", "1/3", ""] {
        assert!(
            MusicXml
                .read(&score(&note("a", Some(value), 4, "")))
                .is_err()
        );
    }
    let (_, sfs) = convert(&score(&note("a", Some("200"), 4, "")));
    assert!(
        scorestream_app::playback::plan_source(&sfs)
            .unwrap_err()
            .contains("note_dynamics.range")
    );
    let mut model = MusicXml
        .read(&score(&note("a", None, 4, "")))
        .unwrap()
        .value;
    model.parts[0].measures[0].events[0].attack_dynamics = Some("1/3".parse().unwrap());
    assert!(SfsAdapter.write(&model).is_err());
    let (_, sfs) = convert(&score(&note("a", Some("10"), 4, "")));
    let mut doc = scorestream_codec::parse(&sfs).unwrap();
    for frame in &mut doc.frames {
        for rec in &mut frame.records {
            if let scorestream_core::Record::Event(e) = rec {
                e.attributes.insert(
                    "attack_dynamics".into(),
                    scorestream_core::Scalar::String("-1".into()),
                );
            }
        }
    }
    assert!(scorestream_formats::sfs::from_document(&doc).is_err());
}
#[test]
fn grace_value_is_stored_but_does_not_bypass_timing_or_sound_qualification() {
    let source = score(&format!(
        r#"<note id="g" dynamics="50"><grace/><pitch><step>D</step><octave>4</octave></pitch><voice>1</voice><type>16th</type></note>{}"#,
        note("a", None, 4, "")
    ));
    let (model, sfs) = convert(&source);
    assert_eq!(model.graces[0].attack_dynamics, Some("50".parse().unwrap()));
    assert!(scorestream_app::playback::plan_source(&sfs).is_err());
}
#[test]
fn explicit_grace_clock_does_not_silently_drop_stored_attack_intent() {
    use scorestream_core::notation::{
        ExplicitGraceGroup, ExplicitGracePolicy, GraceAllocation, GraceQuarterAllocation,
        GraceSoundPolicy,
    };
    let source = score(&format!(
        r#"<note id="g" dynamics="50"><grace/><pitch><step>D</step><octave>4</octave></pitch><voice>1</voice><type>16th</type></note>{}"#,
        note("a", None, 4, "")
    ));
    let (_, sfs) = convert(&source);
    let occurrence = scorestream_app::playback::discover_source_graces(&sfs)
        .unwrap()
        .occurrences
        .remove(0);
    let policy = ExplicitGracePolicy {
        groups: vec![ExplicitGraceGroup {
            principal: occurrence.principal,
            allocation: GraceAllocation::ExplicitFollowingQuarters {
                members: vec![GraceQuarterAllocation {
                    event: occurrence.id,
                    quarters: "1/4".parse().unwrap(),
                }],
            },
        }],
    };
    assert!(
        scorestream_app::playback::plan_source_with_explicit_graces(
            &sfs,
            &policy,
            GraceSoundPolicy::FullGraceThenGatedRemainder
        )
        .unwrap_err()
        .contains("note_dynamics.owner_unsupported")
    );
}
#[test]
fn visual_qualifiers_and_long_exact_decimals_do_not_create_new_import_barriers() {
    let body = format!(
        r#"<direction><direction-type><dynamics><other-dynamics>sempre </other-dynamics><p/></dynamics></direction-type><sound dynamics="44.44"/></direction>{}"#,
        note("a", Some("100"), 4, "")
    );
    let (_, sfs) = convert(&score(&body));
    let plan = scorestream_app::playback::plan_source(&sfs).unwrap();
    assert_eq!(plan.notes[0].velocity, 90);
    assert!(
        plan.diagnostics
            .iter()
            .any(|d| d.code == "dynamic_qualifier_not_realized")
    );
    let long = format!("0.{}1", "0".repeat(80));
    let (model, sfs) = convert(&score(&note("a", Some(&long), 4, "")));
    assert!(
        model.parts[0].measures[0].events[0]
            .attack_dynamics
            .as_ref()
            .unwrap()
            .is_positive()
    );
    assert!(scorestream_app::playback::plan_source(&sfs).is_ok());
}
#[test]
fn repeated_played_occurrences_keep_written_owner_without_persistent_assignment() {
    let body = format!(
        "<barline location=\"left\"><repeat direction=\"forward\"/></barline>{}<barline location=\"right\"><repeat direction=\"backward\" times=\"2\"/></barline>",
        note("a", Some("50"), 4, "")
    );
    let source = score(&body)
        .replace(
            "<barline location=\"left\"><repeat direction=\"forward\"/></barline>",
            "",
        )
        .replace(
            "<attributes>",
            "<barline location=\"left\"><repeat direction=\"forward\"/></barline><attributes>",
        );
    let (_, sfs) = convert(&source);
    let plan = scorestream_app::playback::plan_source(&sfs).unwrap();
    assert_eq!(plan.notes.len(), 2);
    assert_ne!(plan.notes[0].id, plan.notes[1].id);
    assert!(
        plan.notes
            .iter()
            .all(|n| n.written_id.as_deref() == Some("a") && n.velocity == 45)
    );
    assert_eq!(plan.event_positions.len(), 2);
    assert!(
        plan.event_positions
            .iter()
            .all(|p| p.id == "a" && p.occurrence_id.is_some())
    );
}
