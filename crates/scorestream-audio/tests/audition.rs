#![cfg(target_os = "linux")]
#[path = "../examples/audition/support.rs"]
mod support;
use serde_json::{Value, json};
use support::{Audition, Config};
const SOURCE: &str = include_str!("../examples/audition/fixture.sfs");
fn config() -> Value {
    serde_json::from_str(include_str!("../examples/audition/fixture.json")).unwrap()
}
fn compile(s: &str, c: Value) -> Result<Audition, Box<dyn std::error::Error>> {
    Audition::compile(s, serde_json::from_value(c)?)
}
#[test]
fn actual_joint_policies_have_no_implicit_duration_or_route() {
    let d = support::discover(SOURCE).unwrap();
    assert_eq!(d["graces"][0]["event"], "occ1:g");
    assert_eq!(d["fermatas"].as_array().unwrap().len(), 2);
    let report = compile(SOURCE, config()).unwrap().preflight().unwrap();
    assert_eq!(report["audible_notes"], 5);
    assert_eq!(report["pizzicato_attacks"], 2);
    assert_eq!(report["duration_seconds"], 3.0);
    assert_eq!(report["native_bank_verified"], false);
    let mut c = config();
    c["timing"]["holds"][0]
        .as_object_mut()
        .unwrap()
        .remove("added_seconds");
    assert!(serde_json::from_value::<Config>(c).is_err());
    let mut c = config();
    c["rendering"].as_object_mut().unwrap().remove("gain");
    assert!(serde_json::from_value::<Config>(c).is_err());
}
#[test]
fn stale_source_and_policies_cannot_fall_back_to_ordinary() {
    assert!(compile(&(SOURCE.to_owned() + "\n"), config()).is_err());
    let mut c = config();
    c["timing"] = json!({"kind":"ordinary"});
    assert!(compile(SOURCE, c).is_err());
    let mut c = config();
    c["timing"]["graces"][0]["principal"] = "wrong-occurrence".into();
    assert!(compile(SOURCE, c).is_err());
    let mut c = config();
    c["invent_missing_notes"] = true.into();
    assert!(serde_json::from_value::<Config>(c).is_err());
}
fn ordinary(s: &str) -> Value {
    let mut c = config();
    c["source_sha256"] = scorestream_audio::assignments::source_sha256(s).into();
    c["timing"] = json!({"kind":"ordinary"});
    c["pizzicato"] = "reject_unresolved".into();
    c
}
#[test]
fn concrete_native_envelope_and_same_key_blockers_remain_errors() {
    let envelope = "!score 1.1 unit=q\n!part p instrument=violin staves=1\n!voice v part=p staff=1\n@0\n| p m len=2\n= p sound_dynamics=50\nv C4:2 id=a\n.\n@1\n= p sound_dynamics=100\n.\n!end at=2\n";
    let err = compile(envelope, ordinary(envelope))
        .err()
        .unwrap()
        .to_string();
    assert!(err.contains("gain envelope"), "{err}");
    let overlap = "!score 1.1 unit=q\n!part p instrument=violin staves=1\n!voice v part=p staff=1\n!voice w part=p staff=1\n@0\n| p m len=2\nv C4:2 id=a\n.\n@1\nw C4:1 id=b\n.\n!end at=2\n";
    let err = compile(overlap, ordinary(overlap))
        .err()
        .unwrap()
        .to_string();
    assert!(err.contains("Overlapping same-key notes"), "{err}");
}
#[test]
#[ignore = "requires installed authorized local TimGM6mb and FluidSynth; launch binary directly"]
fn configured_qualified_audition_publishes_verified_atomic_wav() {
    let audition = compile(SOURCE, config()).unwrap();
    let bank = scorestream_audio::default_soundfont().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("qualified.wav");
    let report = audition.render(&bank, &output).unwrap();
    let bytes = std::fs::read(&output).unwrap();
    assert_eq!(&bytes[..4], b"RIFF");
    assert_eq!(report["report"]["render"]["note_count"], 5);
    assert_eq!(report["report"]["technique_channels"][0]["attacks"], 2);
    assert!(report["report"]["render"]["peak"].as_f64().unwrap() > 0.0001);
    assert_eq!(
        bytes.len() as u64,
        44 + report["report"]["render"]["frames"].as_u64().unwrap() * 4
    );
    assert!(audition.render(&bank, &output).is_err());
    assert_eq!(std::fs::read(&output).unwrap(), bytes);
    let mut bad = config();
    bad["palette"][0]["preset_name"] = "Not Violin".into();
    let bad = compile(SOURCE, bad).unwrap();
    let absent = dir.path().join("not-published.wav");
    assert!(bad.render(&bank, &absent).is_err());
    assert!(!absent.exists());
}
