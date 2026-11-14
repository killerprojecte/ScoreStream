use super::*;
fn score(body: &str, end: &str) -> String {
    format!(
        "!score 1.1 unit=q\n!part p instrument=piano staves=1\n!voice v part=p staff=1\n@0\n| p m len={end}\n{body}\n!end at={end}\n"
    )
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-9, "{a} != {b}");
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
    assert!(plan_source(&score("= score tempo=q:1/1000000000\nv C4:1 id=a\n.", "1")).is_err());
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
