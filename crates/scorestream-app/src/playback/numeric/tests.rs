use super::*;
use crate::playback::{compile, plan_source};

fn source(body: &str) -> String {
    format!(
        "!score 1.1 unit=q\n!part p instrument=piano staves=2\n!voice v part=p staff=1\n!voice w part=p staff=2\n@0\n| p m len=4\n{body}\n!end at=4\n"
    )
}
fn run(
    controls: &[(&str, &str, &str)],
    body: &str,
) -> Result<crate::playback::PlaybackPlan, String> {
    let doc = scorestream_codec::parse(&source(body)).unwrap();
    let mut score = lower_document(&doc).unwrap();
    for (scope, at, value) in controls {
        score.directions.push(Direction {
            at: at.parse().unwrap(),
            end: None,
            scope: if *scope == "p" {
                Scope::Part(PartId("p".into()))
            } else {
                Scope::Staff(StaffId {
                    part: PartId("p".into()),
                    number: scope.parse().unwrap(),
                })
            },
            kind: DirectionKind::SoundDynamics {
                percent: value.parse().unwrap(),
            },
            placement: None,
            origin: Origin {
                object_id: "numeric_test".into(),
                source: Default::default(),
            },
        });
    }
    compile(&score, &doc)
}
fn close(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-10, "{a} != {b}");
}
const NOTE: &str = "v C4:4 id=a\n+ #a articulation value=tenuto\n.";
#[test]
fn exact_percentage_keeps_fraction_and_display_only_sign() {
    let p = run(
        &[("p", "0", "1111/25")],
        "v C4:4 id=a\n+ p dynamic value=p sound=false\n.",
    )
    .unwrap();
    assert_eq!(p.notes[0].velocity, 40);
    for point in &p.notes[0].gain_envelope {
        close(point.gain, 0.9999);
    }
    assert!(
        p.diagnostics
            .iter()
            .any(|d| d.effect == crate::playback::RealizationEffect::NotationOnly)
    );
}
#[test]
fn zero_is_silent_and_later_held_control_is_a_step() {
    let p = run(
        &[
            ("p", "0", "0"),
            ("p", "1", "100"),
            ("p", "2", "1111/25"),
            ("p", "4", "0"),
        ],
        NOTE,
    )
    .unwrap();
    let n = &p.notes[0];
    assert_eq!(n.velocity, 1);
    let got: Vec<_> = n
        .gain_envelope
        .iter()
        .map(|p| (p.offset_seconds, p.gain))
        .collect();
    assert_eq!(got.len(), 6);
    for (p, (time, gain)) in n.gain_envelope.iter().zip([
        (0., 0.),
        (0.5, 0.),
        (0.5, 90.),
        (1., 90.),
        (1., 39.996),
        (2., 39.996),
    ]) {
        close(p.offset_seconds, time);
        close(p.gain, gain);
    }
}
#[test]
fn scopes_and_default_are_temporal() {
    let p=run(&[("p","1","100"),("1","2","50"),("p","3","10")], "v C4:4 id=a\nw E4:4 id=b\n+ #a articulation value=tenuto\n+ #b articulation value=tenuto\n.").unwrap();
    for n in &p.notes {
        assert_eq!(n.velocity, 80);
        close(n.gain_envelope[0].gain, 1.);
    }
    close(p.notes[0].gain_envelope.last().unwrap().gain, 45. / 80.);
    close(p.notes[1].gain_envelope.last().unwrap().gain, 9. / 80.);
}
#[test]
fn equal_controls_coalesce_but_conflicts_and_range_reject() {
    assert!(run(&[("p", "0", "100"), ("p", "0", "100")], NOTE).is_ok());
    assert!(
        run(&[("p", "0", "100"), ("p", "0", "99")], NOTE)
            .unwrap_err()
            .contains("Conflicting")
    );
    for value in [
        "-1",
        "142",
        "10000000000000000000000000000000000000000000000000000000000000000",
    ] {
        assert!(
            run(&[("p", "0", value)], NOTE)
                .unwrap_err()
                .contains("range")
        );
    }
    let p = run(&[("p", "0", "1270/9")], NOTE).unwrap();
    assert_eq!(p.notes[0].velocity, 127);
}
#[test]
fn unsupported_mixes_fail_closed_and_visual_sf_fp_are_permitted() {
    for mark in [
        "+ p dynamic value=p",
        "+ #a articulation value=accent",
        "+ #a articulation value=strong-accent",
    ] {
        assert!(
            run(&[("p", "0", "100")], &format!("v C4:4 id=a\n{mark}\n."))
                .unwrap_err()
                .contains("cannot yet be combined")
        );
    }
    let hairpin =
        "v C4:2 id=a\n~ crescendo h start=#a\n.\n@2\nv D4:2 id=b\n~ crescendo h end=#b\n.";
    assert!(
        run(&[("p", "0", "100")], hairpin)
            .unwrap_err()
            .contains("cannot yet be combined")
    );
    for sign in ["sf", "sfz", "fp"] {
        let p = run(
            &[("p", "0", "1111/25")],
            &format!("v C4:4 id=a\n+ p dynamic value={sign} sound=false\n."),
        )
        .unwrap();
        assert_eq!(p.notes[0].velocity, 40);
        close(p.notes[0].gain_envelope[0].gain, 0.9999);
    }
}
#[test]
fn tie_gate_tempo_and_rest_preserve_control_boundaries() {
    let body = "v C4:2 id=a\n~ tie t start=#a\nw r:4 id=r\n.\n@1\n= score tempo=q:60\n.\n@2\nv C4:2 id=b\n~ tie t end=#b\n+ #b articulation value=staccato\n.";
    let p = run(&[("p", "0", "100"), ("p", "2", "50")], body).unwrap();
    assert_eq!(p.notes.len(), 1);
    let n = &p.notes[0];
    close(n.duration_seconds, 2.5);
    close(n.gain_envelope[1].offset_seconds, 1.5);
    close(n.gain_envelope[2].gain, 0.5);
}
#[test]
fn sfs_lowering_retains_exact_control_and_sf_spelling() {
    let text =
        source("= p sound_dynamics=1111/25\nv C4:4 id=a\n+ p dynamic value=sf sound=false\n.");
    let doc = scorestream_codec::parse(&text).unwrap();
    let score = lower_document(&doc).unwrap();
    assert!(score.directions.iter().any(|d|matches!(&d.kind,DirectionKind::SoundDynamics{percent} if percent == &"1111/25".parse().unwrap())));
    assert!(score.directions.iter().any(|d| matches!(
        d.kind,
        DirectionKind::Dynamic {
            sign: DynamicSign::Sforzato,
            sound: false
        }
    )));
    assert_eq!(plan_source(&text).unwrap().notes[0].velocity, 40);
}
#[test]
fn old_json_defaults_symbolic_sound_to_true() {
    let d: DirectionKind =
        serde_json::from_str(r#"{"kind":"dynamic","sign":{"kind":"sforzando"}}"#).unwrap();
    assert!(matches!(d, DirectionKind::Dynamic { sound: true, .. }));
}
#[test]
fn later_zero_silences_held_note_without_changing_attack() {
    let p = run(&[("p", "0", "100"), ("p", "1", "0")], NOTE).unwrap();
    assert_eq!(p.notes[0].velocity, 90);
    close(p.notes[0].gain_envelope[0].gain, 1.);
    close(p.notes[0].gain_envelope.last().unwrap().gain, 0.);
}
#[test]
fn display_only_without_numeric_keeps_default_and_sf_uses_legacy_policy() {
    let visual = plan_source(&source("v C4:4 id=a\n+ p dynamic value=fp sound=false\n.")).unwrap();
    assert_eq!(visual.notes[0].velocity, 80);
    assert!(visual.notes[0].gain_envelope.iter().all(|p| p.gain == 1.));
    let sf = plan_source(&source("v C4:4 id=a\n+ p dynamic value=sf\n.")).unwrap();
    assert_eq!(sf.notes[0].velocity, 112);
}
#[test]
fn bounded_staff_override_restores_current_part_value() {
    let doc = scorestream_codec::parse(&source(NOTE)).unwrap();
    let mut score = lower_document(&doc).unwrap();
    for (scope, at, end, value) in [
        (Scope::Part(PartId("p".into())), "0", None, "100"),
        (Scope::Part(PartId("p".into())), "2", None, "60"),
        (
            Scope::Staff(StaffId {
                part: PartId("p".into()),
                number: 1,
            }),
            "1",
            Some("3"),
            "20",
        ),
    ] {
        score.directions.push(Direction {
            scope,
            at: at.parse().unwrap(),
            end: end.map(|v| v.parse().unwrap()),
            kind: DirectionKind::SoundDynamics {
                percent: value.parse().unwrap(),
            },
            placement: None,
            origin: Origin {
                object_id: "finite_test".into(),
                source: Default::default(),
            },
        });
    }
    let p = compile(&score, &doc).unwrap();
    let points = &p.notes[0].gain_envelope;
    assert_eq!(points.len(), 6); // A shadowed part update adds no audible boundary.
    close(points[2].gain, 0.2);
    close(points[4].offset_seconds, 1.5);
    close(points[4].gain, 0.6);
}
#[test]
fn unrepresentable_positive_magnitude_is_not_silently_zeroed() {
    let tiny = format!("1/1{}", "0".repeat(400));
    assert!(
        run(&[("p", "0", &tiny)], NOTE)
            .unwrap_err()
            .contains("range")
    );
}
#[test]
fn symbolic_sign_rejects_unknown_nested_json() {
    assert!(
        serde_json::from_str::<DirectionKind>(
            r#"{"kind":"dynamic","sign":{"kind":"sforzando","ignored":true}}"#
        )
        .is_err()
    );
}
