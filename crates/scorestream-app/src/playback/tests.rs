use super::*;
use std::collections::BTreeMap;
fn score(body: &str, end: &str) -> String {
    format!(
        "!score 1.1 unit=q\n!part p instrument=piano staves=1\n!voice v part=p staff=1\n@0\n| p m len={end}\n{body}\n!end at={end}\n"
    )
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} != {b}");
}
#[test]
fn part_display_name_does_not_change_playback_routing() {
    let source = score("v C4:1 id=a\n.", "1");
    let plain = plan_source(&source).unwrap();
    for label in ["Flute 1", "", "<label> & strings"] {
        let named = source.replace(
            "instrument=piano staves=1",
            &format!(
                "instrument=piano staves=1 display_name={}",
                serde_json::to_string(label).unwrap(),
            ),
        );
        let plan = plan_source(&named).unwrap();
        assert_eq!(plan.notes[0].instrument, "piano");
        assert_eq!(plan.notes[0].midi_note, plain.notes[0].midi_note);
        assert_eq!(
            plan.notes[0].duration_seconds,
            plain.notes[0].duration_seconds
        );
        assert_eq!(plan.duration_seconds, plain.duration_seconds);
    }
}
#[test]
fn tempo_units_and_changes_integrate_exactly() {
    let p=plan_source(&score("= score tempo=q.:60\nv C4:3 id=a\n+ #a articulation value=tenuto\n.\n@3\n= score tempo=e:120\nv D4:1 id=b\n+ #b articulation value=tenuto\n.","4")).unwrap();
    close(p.notes[0].duration_seconds, 2.);
    close(p.notes[1].start_seconds, 2.);
    close(p.notes[1].duration_seconds, 1.);
    close(p.duration_seconds, 3.);
}
#[test]
fn sounding_tie_has_one_attack_and_full_continuity() {
    let p=plan_source(&score("v C4:1 id=a\n~ tie hold start=#a\n.\n@1\nv C4:1 id=b\n~ tie hold continue=#b\n.\n@2\nv C4:1 id=c\n~ tie hold end=#c\n+ #c articulation value=tenuto\n.","3")).unwrap();
    assert_eq!(p.notes.len(), 1);
    close(p.notes[0].duration_seconds, 1.5);
    let q = plan_source(&score(
        "v C4:1 id=a\n~ tie hold start=#a sound=false\n.\n@1\nv C4:1 id=b\n~ tie hold end=#b\n.",
        "2",
    ))
    .unwrap();
    assert_eq!(q.notes.len(), 2);
}
#[test]
fn dynamics_are_scoped_and_articulations_change_gate_and_attack() {
    let p=plan_source(&score("+ score dynamic value=ff\n+ p dynamic value=p\nv C4:1 id=a\n+ #a articulation value=staccato\n.\n@1\nv D4:1 id=b\n+ #b articulation value=accent\n.","2")).unwrap();
    assert_eq!(p.notes[0].velocity, 48);
    close(p.notes[0].duration_seconds, 0.25);
    assert_eq!(p.notes[1].velocity, 64);
    assert!(p.notes[1].gain_envelope.iter().any(|e| e.gain < 1.));
}
#[test]
fn held_note_hairpin_changes_gain_not_only_next_note_velocity() {
    let p=plan_source(&score("+ p dynamic value=p\nv C4:4 id=a\n~ crescendo swell start=#a\n.\n@4\n+ p dynamic value=f\nv D4:1 id=b\n~ crescendo swell end=#b\n.","5")).unwrap();
    assert_eq!(p.notes[0].velocity, 48);
    let last = p.notes[0].gain_envelope.last().unwrap();
    assert!(last.gain > 1.9);
    assert_eq!(p.notes[1].velocity, 96);
}
#[test]
fn pedal_extends_release_and_continue_can_lift() {
    let p=plan_source(&score("v C4:1 id=a\n~ pedal ped start=#a\n.\n@1\nv D4:1 id=b\n~ pedal ped continue=#b value=0\n.\n@2\nv E4:1 id=c\n~ pedal ped end=#c\n.","3")).unwrap();
    close(p.notes[0].duration_seconds, 0.5);
    close(p.notes[1].duration_seconds, 0.46);
}
#[test]
fn event_dynamics_do_not_leak_and_fp_has_decay() {
    let p = plan_source(&score(
        "v C4:1 id=a\n+ #a dynamic value=fp\n.\n@1\nv D4:1 id=b\n.",
        "2",
    ))
    .unwrap();
    assert_eq!(p.notes[0].velocity, 96);
    assert_eq!(p.notes[1].velocity, 80);
    close(p.notes[0].gain_envelope[1].gain, 0.5);
}
#[test]
fn transposition_microtones_and_frame_wide_instrument_changes() {
    let p=plan_source(&score("v C(+1/2)4:1 id=a\n= p instrument=flute transpose_chromatic=2 transpose_diatonic=1 transpose_octaves=1\n.","1")).unwrap();
    close(p.notes[0].midi_note, 74.5);
    assert_eq!(p.notes[0].instrument, "flute");
    close(p.notes[0].frequency_hz, 440. * 2_f64.powf(5.5 / 12.));
}
#[test]
fn routes_and_events_are_deterministic_and_all_timing_finite() {
    let p = plan_source(include_str!("../../../../examples/playback-expressive.sfs")).unwrap();
    assert_eq!(
        p,
        plan_source(include_str!("../../../../examples/playback-expressive.sfs")).unwrap()
    );
    assert_eq!(p.routes.len(), 2);
    assert_eq!(p.notes.len(), 11);
    close(p.duration_seconds, 2. + 10. / 3.);
    for n in &p.notes {
        assert!(n.duration_seconds > 0. && n.frequency_hz.is_finite());
        assert!((1..=127).contains(&n.velocity));
        assert!(
            n.gain_envelope
                .windows(2)
                .all(|p| p[0].offset_seconds <= p[1].offset_seconds)
        );
    }
}
#[test]
fn rests_are_silent_and_invalid_sources_do_not_play() {
    let p = plan_source(&score("v r:1 id=a\n.", "1")).unwrap();
    assert!(p.notes.is_empty());
    close(p.duration_seconds, 0.5);
    assert!(plan_source("invalid").is_err());
    assert_eq!(plan_source(&score("= score tempo=q:1/1000000000\nv C4:1 id=a\n.", "1")).unwrap().duration_seconds, 60_000_000_000.0);
}
#[test]
fn ten_thousand_events_remain_bounded_and_no_clock_drift() {
    let mut body = String::from("= score tempo=q:120\nv C4:1/100 id=n0\n.\n");
    for i in 1..10_000 {
        let at = &Rational::from_integer(i) / &Rational::from_integer(100);
        body.push_str(&format!("@{at}\nv C4:1/100 id=n{i}\n.\n"));
    }
    let start = std::time::Instant::now();
    let p = plan_source(&score(&body, "100")).unwrap();
    assert_eq!(p.notes.len(), 10_000);
    close(p.notes.last().unwrap().start_seconds, 49.995);
    close(p.duration_seconds, 50.);
    // Deliberately generous regression guard on debug CI, not a real-time guarantee.
    assert!(start.elapsed().as_secs() < 20);
}

fn expression_value(source: &str, id: &str, at: &str) -> f64 {
    let doc = scorestream_codec::parse(source).unwrap();
    let model = lower_document(&doc).unwrap();
    let expr = Expression::new(&model, &mut Diagnostics::default()).unwrap();
    let event = model.events.iter().find(|e| e.id.0 == id).unwrap();
    expr.value(event, &at.parse().unwrap())
}
#[test]
fn scoped_sfz_preserves_prior_p_and_compounds_without_adding_levels() {
    let s = score(
        "+ v dynamic value=p\nv C4:1 id=a\n.\n@1\n+ v dynamic value=sfz\nv D4:1 id=b\n.\n@2\nv E4:1 id=c\n.",
        "3",
    );
    let p = plan_source(&s).unwrap();
    assert_eq!(
        p.notes.iter().map(|n| n.velocity).collect::<Vec<_>>(),
        [48, 112, 48]
    );
    close(p.notes[1].gain_envelope[1].gain, 48. / 112.);
}
#[test]
fn narrower_event_assignment_ends_and_new_ancestor_takes_effect() {
    let s = score(
        "+ p dynamic value=mp\nv C4:2 id=a\n+ #a dynamic value=p\n.\n@1\n+ p dynamic value=f\n.\n@2\nv D4:1 id=b\n.",
        "3",
    );
    assert_eq!(expression_value(&s, "a", "1"), 48.);
    assert_eq!(expression_value(&s, "a", "2"), 96.);
    assert_eq!(plan_source(&s).unwrap().notes[1].velocity, 96);
}
#[test]
fn bounded_hairpin_does_not_leave_an_implicit_permanent_offset() {
    let s = score(
        "+ p dynamic value=p\nv C4:2 id=a\n~ crescendo h start=#a\n.\n@2\n+ p dynamic value=mf\nv D4:2 id=b\n.\n@4\nv E4:1 id=c\n~ crescendo h end=#c\n.",
        "5",
    );
    close(expression_value(&s, "a", "1"), 54.);
    close(expression_value(&s, "b", "3"), 66.);
    close(expression_value(&s, "c", "4"), 80.);
}
#[test]
fn same_scope_explicit_dynamic_truncates_ramp_and_terminal_is_excluded() {
    let s = score(
        "+ v dynamic value=p\nv C4:2 id=a\n~ crescendo h start=#a\n.\n@2\n+ v dynamic value=mf\nv D4:2 id=b\n.\n@4\n+ v dynamic value=f\nv E4:1 id=c\n~ crescendo h end=#c\n.",
        "5",
    );
    close(expression_value(&s, "a", "1"), 60.);
    close(expression_value(&s, "b", "2"), 80.);
    close(expression_value(&s, "c", "4"), 96.);
}
#[test]
fn event_override_shadows_voice_ramp_without_multiplying_it() {
    let s = score(
        "+ v dynamic value=p\nv C4:1 id=a\n~ crescendo h start=#a\n.\n@1\nv D4:1 id=b\n+ #b dynamic value=pp\n.\n@2\nv E4:2 id=c\n.\n@4\n+ v dynamic value=f\nv F4:1 id=d\n~ crescendo h end=#d\n.",
        "5",
    );
    close(expression_value(&s, "b", "3/2"), 36.);
    close(expression_value(&s, "c", "3"), 84.);
}
#[test]
fn independent_gate_and_attack_domains_are_order_independent() {
    let a = score(
        "+ p dynamic value=p\nv C4:1 id=a\n+ #a articulation value=staccato\n+ #a articulation value=accent\n+ #a articulation value=strong-accent\n.",
        "1",
    );
    let b = score(
        "v C4:1 id=a\n+ #a articulation value=strong-accent\n+ #a articulation value=accent\n+ #a articulation value=staccato\n+ p dynamic value=p\n.",
        "1",
    );
    let p = plan_source(&a).unwrap();
    assert_eq!(p, plan_source(&b).unwrap());
    assert_eq!(p.notes[0].velocity, 72);
    close(p.notes[0].duration_seconds, 0.25);
}

#[test]
fn plain_expression_words_are_typed_and_explicitly_not_guessed() {
    let p = plan_source(&score(
        "v C4:1 id=a\n+ #a words text=\"dolce\" id=word\n.",
        "1",
    ))
    .unwrap();
    assert_eq!(p.notes[0].velocity, 80);
    assert!(p.diagnostics.iter().any(|d| d.code == "words_not_realized"
        && d.object_id == "word"
        && d.effect == RealizationEffect::NotationOnly));
}
#[test]
fn registered_dynamic_levels_map_without_unknown_default_fallback() {
    for (name, expected) in [
        ("pppp", 16),
        ("ppp", 24),
        ("pp", 36),
        ("p", 48),
        ("mp", 64),
        ("mf", 80),
        ("f", 96),
        ("ff", 112),
        ("fff", 120),
        ("ffff", 127),
    ] {
        let p = plan_source(&score(
            &format!("v C4:1 id=a\n+ #a dynamic value={name}\n."),
            "1",
        ))
        .unwrap();
        assert_eq!(p.notes[0].velocity, expected, "{name}");
    }
}
#[test]
fn scope_study_has_expected_attacks_and_structured_interval_diagnostics() {
    let p = plan_source(include_str!(
        "../../../../examples/expression-scope-study.sfs"
    ))
    .unwrap();
    let values: BTreeMap<_, _> = p
        .notes
        .iter()
        .map(|n| (n.id.as_str(), n.velocity))
        .collect();
    assert_eq!(values["a"], 48);
    assert_eq!(values["b"], 36);
    assert_eq!(values["c"], 76);
    assert_eq!(values["d"], 80);
    assert_eq!(values["e"], 112);
    assert_eq!(values["f"], 80);
    assert_eq!(values["bass"], 64);
    assert_eq!(values["bass2"], 96);
    assert!(
        p.diagnostics
            .iter()
            .any(|d| d.code == "hairpin_terminated" && d.object_id == "swell")
    );
}

#[test]
fn written_timeline_integrates_tempo_changes_inside_notes_and_rests() {
    let p = plan_source(&score(
        "v C4:2 id=held\n+ #held articulation value=tenuto\n.\n@1\n= score tempo=q:60\n.\n@2\nv r:2 id=rest\n.\n@3\n= score tempo=q:120\n.\n@4\nv _:1 id=spacer\n.",
        "5",
    ))
    .unwrap();
    assert_eq!(p.notes.len(), 1);
    assert_eq!(p.event_positions.len(), 3);
    for (position, (id, start, end)) in
        p.event_positions
            .iter()
            .zip([("held", 0., 1.5), ("rest", 1.5, 3.), ("spacer", 3., 3.5)])
    {
        assert_eq!(position.id, id);
        close(position.start_seconds, start);
        close(position.end_seconds, end);
    }
    close(p.notes[0].duration_seconds, 1.5);
    close(p.duration_seconds, 3.5);
    close(p.measures[0].end_seconds, p.duration_seconds);
}

#[test]
fn tie_tail_has_its_own_written_interval_across_tempo_change() {
    let p = plan_source(&score(
        "v C4:1 id=head\n~ tie t start=#head\n.\n@1\n= score tempo=q:60\nv C4:2 id=tail\n~ tie t end=#tail\n+ #tail articulation value=tenuto\n.\n@2\n= score tempo=q:120\n.",
        "3",
    ))
    .unwrap();
    assert_eq!(p.notes.len(), 1);
    assert_eq!(p.event_positions.len(), 2);
    assert_eq!(p.event_positions[1].id, "tail");
    close(p.event_positions[0].end_seconds, 0.5);
    close(p.event_positions[1].start_seconds, 0.5);
    close(p.event_positions[1].end_seconds, 2.);
    close(p.notes[0].duration_seconds, 2.);
    close(p.duration_seconds, 2.);
}

#[test]
fn measures_keep_explicit_nonuniform_boundaries_and_labels() {
    let source = "!score 1.1 unit=q\n!part p instrument=piano staves=1\n!voice v part=p staff=1\n@0\n| p pickup len=1 label=\"0\"\nv r:1 id=a\n.\n@1\n| p main len=3 label=\"1\"\nv r:3 id=b\n.\n@2\n= score tempo=q:60\n.\n@4\n| p last len=2\nv r:2 id=c\n.\n!end at=6\n";
    let p = plan_source(source).unwrap();
    assert!(p.notes.is_empty());
    assert_eq!(p.measures.len(), 3);
    for (measure, (id, label, start, end)) in p.measures.iter().zip([
        ("pickup", Some("0"), 0., 0.5),
        ("main", Some("1"), 0.5, 3.),
        ("last", None, 3., 5.),
    ]) {
        assert_eq!(measure.id, id);
        assert_eq!(measure.part, "p");
        assert_eq!(measure.label.as_deref(), label);
        close(measure.start_seconds, start);
        close(measure.end_seconds, end);
    }
    close(p.duration_seconds, 5.);
}

#[test]
fn written_intervals_do_not_inherit_articulation_gates_or_pedal() {
    let p = plan_source(&score(
        "v C4:1 id=a\n+ #a articulation value=staccato\n~ pedal ped start=#a\n.\n@1\nv r:1 id=rest\n.\n@2\nv D4:1 id=b\n~ pedal ped end=#b\n.",
        "3",
    ))
    .unwrap();
    close(p.event_positions[0].end_seconds, 0.5);
    close(p.notes[0].duration_seconds, 1.);
    close(p.event_positions[1].start_seconds, 0.5);
    close(p.event_positions[1].end_seconds, 1.);
    close(p.duration_seconds, 1.5);
}

#[test]
fn older_serialized_plans_default_to_empty_timelines() {
    let p: PlaybackPlan =
        serde_json::from_str(r#"{"duration_seconds":0.0,"notes":[],"routes":[],"warnings":[]}"#)
            .unwrap();
    assert!(p.event_positions.is_empty());
    assert!(p.measures.is_empty());
    let current = plan_source(&score("v r:1 id=a\n.", "1")).unwrap();
    let roundtrip: PlaybackPlan =
        serde_json::from_str(&serde_json::to_string(&current).unwrap()).unwrap();
    assert_eq!(current, roundtrip);
}

#[test]
fn silent_cue_and_zero_duration_grace_keep_score_positions() {
    let p = plan_source(&score(
        "v E4:? id=grace kind=grace anchor=#cue order=-1\nv C4:1 id=cue kind=cue\n.",
        "1",
    ))
    .unwrap();
    assert!(p.notes.is_empty());
    assert_eq!(p.event_positions.len(), 2);
    let grace = p.event_positions.iter().find(|e| e.id == "grace").unwrap();
    close(grace.start_seconds, 0.);
    close(grace.end_seconds, 0.);
    let cue = p.event_positions.iter().find(|e| e.id == "cue").unwrap();
    close(cue.start_seconds, 0.);
    close(cue.end_seconds, 0.5);
    close(p.duration_seconds, 0.5);
}

#[test]
fn catalog_identities_preserve_raw_routes_and_never_infer_transposition() {
    use scorestream_codec::instruments::{InstrumentPitchKind, instrument_catalog};
    for definition in instrument_catalog() {
        if definition.pitch_kind != InstrumentPitchKind::Pitched {
            continue;
        }
        for id in std::iter::once(&definition.id).chain(&definition.aliases) {
            let source = score("v C4:1 id=a\n.", "1")
                .replace("instrument=piano", &format!("instrument={id}"));
            let plan = plan_source(&source).unwrap();
            assert_eq!(plan.notes.len(), 1, "{id}");
            assert_eq!(plan.notes[0].instrument, id.0);
            assert_eq!(plan.routes[0].instrument, id.0);
            close(plan.notes[0].midi_note, 60.);
            close(plan.notes[0].frequency_hz, 440. * 2_f64.powf(-9. / 12.));
            assert!(
                scorestream_codec::canonical(&scorestream_codec::parse(&source).unwrap())
                    .contains(&format!("instrument={id}"))
            );
        }
    }
}

#[test]
fn orchestral_identity_changes_preserve_explicit_octave_and_chromatic_state() {
    let source = score(
        "= p instrument=piccolo transpose_chromatic=0 transpose_diatonic=0 transpose_octaves=1\nv C4:1 id=a\n.\n@1\n= p instrument=contrabassoon transpose_chromatic=0 transpose_diatonic=0 transpose_octaves=-1\nv C4:1 id=b\n.\n@2\n= p instrument=tenor_trombone transpose_chromatic=-2 transpose_diatonic=-1 transpose_octaves=0\nv C4:1 id=c\n.",
        "3",
    );
    let plan = plan_source(&source).unwrap();
    assert_eq!(
        plan.notes
            .iter()
            .map(|n| n.instrument.as_str())
            .collect::<Vec<_>>(),
        ["piccolo", "contrabassoon", "tenor_trombone"]
    );
    assert_eq!(
        plan.notes.iter().map(|n| n.midi_note).collect::<Vec<_>>(),
        [72., 48., 58.]
    );
}

#[test]
fn percussion_catalog_membership_does_not_invent_a_drum_map() {
    let source =
        score("v u.snare:1 id=a\n.", "1").replace("instrument=piano", "instrument=snare_drum");
    let plan = plan_source(&source).unwrap();
    assert!(plan.notes.is_empty());
    assert!(
        plan.diagnostics
            .iter()
            .any(|d| d.code == "percussion_unmapped")
    );
}

#[test]
fn ordinary_playback_has_no_fixed_source_event_or_duration_ceiling() {
    // Actual authored events, not comment padding: this crosses all three old gates.
    let count = 100_001;
    let mut source = String::from(
        "!score 1.1 unit=q\n!part p instrument=piano staves=1\n!voice v part=p staff=1\n",
    );
    for i in 0..count {
        source.push_str(&format!(
            "@{i}\n| p measure_{i} len=1\nv C4:1 id=authored_event_{i}\n.\n"
        ));
    }
    source.push_str(&format!("!end at={count}\n"));
    assert!(source.len() > 4 * 1024 * 1024);
    let plan = plan_source(&source).unwrap();
    assert_eq!(plan.notes.len(), count);
    assert_eq!(plan.event_positions.len(), count);
    assert_eq!(plan.duration_seconds, count as f64 / 2.0);
    assert!(plan.duration_seconds > 21_600.0);
}
