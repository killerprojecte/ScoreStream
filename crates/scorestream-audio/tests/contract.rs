use scorestream_app::playback::{EnvelopePoint, PlaybackPlan, plan_source};
use scorestream_audio::{RenderOptions, RouteAssignment, validate_plan};

fn compiled() -> PlaybackPlan {
    plan_source("!score 1.1 unit=q\n!part p instrument=clarinet staves=1\n!voice v part=p staff=1\n@0\n| p m len=4\nv C4:1 id=a\n.\n@1\nv D4:1 id=b\n.\n!end at=4\n").unwrap()
}
fn routes() -> Vec<RouteAssignment> {
    vec![RouteAssignment {
        part: "p".into(),
        instrument: "clarinet".into(),
        bank: 0,
        program: 71,
    }]
}

#[test]
fn accepts_actual_rust_compiler_plan_and_retains_long_silence() {
    let mut plan = compiled();
    validate_plan(&plan, &routes(), &RenderOptions::default()).unwrap();
    plan.duration_seconds = 3600.0;
    validate_plan(&plan, &routes(), &RenderOptions::default()).unwrap();
}

#[test]
fn rejects_microtones_and_never_substitutes_equal_temperament() {
    let mut plan = compiled();
    plan.notes[0].midi_note += 0.5;
    assert!(
        validate_plan(&plan, &routes(), &RenderOptions::default())
            .unwrap_err()
            .to_string()
            .contains("Microtonal")
    );
}

#[test]
fn rejects_independent_gain_expression_instead_of_channel_cc_approximation() {
    let mut plan = compiled();
    plan.notes[0].gain_envelope = vec![EnvelopePoint {
        offset_seconds: 0.1,
        gain: 0.6,
    }];
    assert!(
        validate_plan(&plan, &routes(), &RenderOptions::default())
            .unwrap_err()
            .to_string()
            .contains("Per-note")
    );
}

#[test]
fn rejects_bad_numbers_pitch_velocity_and_timing() {
    let options = RenderOptions::default();
    for case in 0..8 {
        let mut plan = compiled();
        match case {
            0 => plan.duration_seconds = f64::NAN,
            1 => plan.notes[0].start_seconds = f64::INFINITY,
            2 => plan.notes[0].duration_seconds = -0.5,
            3 => plan.notes[0].frequency_hz *= 2.0,
            4 => plan.notes[0].velocity = 0,
            5 => plan.notes[0].velocity = 128,
            6 => plan.notes[0].duration_seconds = 1e-10,
            7 => plan.notes[0].gain_envelope.push(EnvelopePoint {
                offset_seconds: f64::NAN,
                gain: 1.0,
            }),
            _ => unreachable!(),
        }
        assert!(
            validate_plan(&plan, &routes(), &options).is_err(),
            "case {case}"
        );
    }
}

#[test]
fn refuses_missing_duplicate_unused_or_inconsistent_routes() {
    let options = RenderOptions::default();
    let plan = compiled();
    assert!(validate_plan(&plan, &[], &options).is_err());
    assert!(validate_plan(&plan, &[routes()[0].clone(), routes()[0].clone()], &options).is_err());
    let mut altered = plan.clone();
    altered.routes.clear();
    assert!(validate_plan(&altered, &routes(), &options).is_err());
}

#[test]
fn same_key_overlap_is_rejected_but_boundary_retrigger_is_valid() {
    let mut plan = compiled();
    plan.notes[1].midi_note = plan.notes[0].midi_note;
    plan.notes[1].frequency_hz = plan.notes[0].frequency_hz;
    plan.notes[1].start_seconds = plan.notes[0].duration_seconds;
    validate_plan(&plan, &routes(), &RenderOptions::default()).unwrap();
    plan.notes[1].start_seconds -= 0.01;
    assert!(validate_plan(&plan, &routes(), &RenderOptions::default()).is_err());
}

#[test]
fn resource_options_are_explicit_and_validated() {
    let plan = compiled();
    let bad = RenderOptions {
        block_frames: 0,
        ..RenderOptions::default()
    };
    assert!(validate_plan(&plan, &routes(), &bad).is_err());
    let bad = RenderOptions {
        max_notes: 1,
        ..RenderOptions::default()
    };
    assert!(validate_plan(&plan, &routes(), &bad).is_err());
    let bad = RenderOptions {
        gain: f64::NAN,
        ..RenderOptions::default()
    };
    assert!(validate_plan(&plan, &routes(), &bad).is_err());
}
