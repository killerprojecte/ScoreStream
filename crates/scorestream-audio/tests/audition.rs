#![cfg(target_os = "linux")]
use scorestream_audio::audition_session as support;
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

#[test]
fn explicit_practice_review_retains_exact_midi_intent_without_sound_authority() {
    use support::{PracticeReviewPolicy, review_practice};
    let source = SOURCE.replace("name=\"violin\" source_id=\"ip\"", "name=\"violin\" source_id=\"ip\" device=\"\" device_ref=\"ip\" port=2 midi_ref=\"ip\" channel=12 program=41 volume=393701/5000 pan=0");
    let before = source.clone();
    let r = review_practice(
        &source,
        PracticeReviewPolicy::WrittenNotesWithoutWedgeDashesOrnamentsSlurV1,
    )
    .unwrap();
    assert_eq!(source, before);
    assert_eq!(
        r["source_sha256"],
        scorestream_audio::assignments::source_sha256(&source)
    );
    assert_eq!(r["disclosures"].as_array().unwrap().len(), 4);
    assert_eq!(r["independently_verified_midi_route_blocker"], true);
    assert_eq!(r["contract_renderable"], false);
    assert_eq!(r["playback_plan_created"], false);
    let a = &r["authored_instruments"][0]["authored"];
    assert_eq!(a["midi_device"]["port"], 2);
    assert_eq!(a["midi_instrument"]["channel"], 12);
    assert_eq!(a["midi_instrument"]["program"], 41);
    assert_eq!(a["midi_instrument"]["volume"], "393701/5000");
    let invalid = source.replace("program=41", "program=129");
    assert!(
        review_practice(
            &invalid,
            PracticeReviewPolicy::WrittenNotesWithoutWedgeDashesOrnamentsSlurV1
        )
        .is_err()
    );
    assert!(serde_json::from_str::<PracticeReviewPolicy>("\"ignore_everything\"").is_err());
}

#[test]
fn practice_discovery_is_source_bound_and_never_authorizes_audio() {
    use scorestream_audio::midi_mixer::*;
    use scorestream_audio::{RouteAssignment, assignments::source_sha256};
    use scorestream_codec::notation::*;
    let source=SOURCE.replace("@0\n","!requires scorestream.slur@1\n@0\n")
        .replace("name=\"violin\" source_id=\"ip\"","name=\"violin\" source_id=\"ip\" midi_ref=\"ip\" channel=1 program=41 volume=393701/5000 pan=0")
        .replace("name=\"violin\" source_id=\"ir\"","name=\"violin\" source_id=\"ir\" midi_ref=\"ir\" channel=1 program=41 volume=393701/5000 pan=0")
        .replace("v1_1 D4:1 id=plain staff=1", "v1_1 D4:1 id=plain staff=1\n~ slur s start=#plain")
        .replace("v1_1 E4:1 id=held staff=1", "v1_1 E4:1 id=held staff=1\n~ slur s end=#held")
;
    // Keep each relation in its original anchor's existing frame.
    let source = source.replace(".\n~ slur", "~ slur");
    let routes: Vec<_> = ["p", "r"]
        .into_iter()
        .map(|p| RouteAssignment {
            part: p.into(),
            instrument: "violin".into(),
            bank: 0,
            program: 40,
        })
        .collect();
    let mix = prepare_local_midi_mix(
        &source,
        &source_sha256(&source),
        &routes,
        LocalMidiPolicy::InitialGmCc7CenteredV1,
    )
    .unwrap();
    assert!(support::discover(&source).is_err());
    let report = support::discover_practice(&source, &mix).unwrap();
    assert!(
        report.get("practice_discovery_errors").is_none(),
        "{report:#}"
    );
    assert_eq!(report["fermatas"].as_array().unwrap().len(), 2);
    assert_eq!(report["grace_occurrences"], 1);
    assert_eq!(report["playback_plan_created"], false);
    assert_eq!(report["hold_seconds_selected"], false);
    assert!(support::discover_practice(&(source.clone() + "\n"), &mix).is_err());
    let doc = scorestream_codec::parse(&source).unwrap();
    let mut selections: Vec<_> = routes
        .iter()
        .map(|r| PracticeMidiSelection {
            part: r.part.clone(),
            instrument: r.instrument.clone(),
            bank: r.bank,
            program_zero_based: r.program,
        })
        .collect();
    let q = discover_document_practice_joint(
        &doc,
        PracticeDiscoveryPolicy::WrittenNotesWithoutWedgeDashesOrnamentsSlurV1,
        ExplicitArticulationPolicy::SimpleStaccatoAccentV1,
        &selections,
    )
    .unwrap();
    assert_eq!(q.source(), &doc);
    let expected = support::discover(SOURCE).unwrap();
    assert_eq!(report["fermatas"], expected["fermatas"]);
    selections[0].program_zero_based = 41;
    assert!(
        discover_document_practice_joint(
            &doc,
            PracticeDiscoveryPolicy::WrittenNotesWithoutWedgeDashesOrnamentsSlurV1,
            ExplicitArticulationPolicy::SimpleStaccatoAccentV1,
            &selections
        )
        .is_err()
    );
}

fn practice_fixture() -> (String, Config) {
    let source=SOURCE.replace("v2_1 r:2 id=silence", "v2_1 C3:2 id=silence").replace("name=\"violin\" source_id=\"ip\"","name=\"violin\" source_id=\"ip\" device=\"\" device_ref=\"ip\" port=1 midi_ref=\"ip\" channel=1 program=41 volume=393701/5000 pan=0")
        .replace("name=\"violin\" source_id=\"ir\"","name=\"violin\" source_id=\"ir\" device=\"\" device_ref=\"ir\" port=2 midi_ref=\"ir\" channel=1 program=41 volume=393701/5000 pan=0");
    let mut c = config();
    let mut silent = c["palette"][0].clone();
    silent["part"] = "r".into();
    c["palette"].as_array_mut().unwrap().push(silent);
    c["source_sha256"] = scorestream_audio::assignments::source_sha256(&source).into();
    (source, serde_json::from_value(c).unwrap())
}
#[test]
fn practice_joint_clock_and_midi_receipt_are_explicit_without_default_bypass() {
    use scorestream_audio::midi_mixer::LocalMidiPolicy;
    use support::PracticeReviewPolicy;
    let (source, c) = practice_fixture();
    let q = Audition::compile_practice(
        &source,
        scorestream_app::playback::PracticeTieNoteOnPolicy::InertWithoutNewNoteOnV1,
        scorestream_app::playback::PracticeHoldEndpointPolicy::ReleaseUnmarkedAtBoundaryBeforeHoldV1,
        scorestream_app::playback::PracticeNoteExpressionPolicy::AuthoredNoteOnWithRelativeTrajectoryV1,
        c,
        PracticeReviewPolicy::WrittenNotesWithoutWedgeDashesOrnamentsSlurV1,
        LocalMidiPolicy::InitialGmCc7CenteredV1,
    )
    .unwrap();
    let r = q.preflight().unwrap();
    assert_eq!(r["audible_notes"], 6);
    assert_eq!(r["duration_seconds"], 3.0);
    assert_eq!(r["pizzicato_attacks"], 2);
    assert_eq!(
        r["local_midi_mix"]["source_sha256"],
        scorestream_audio::assignments::source_sha256(&source)
    );
    assert_eq!(
        r["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|d| d["code"].as_str().unwrap().starts_with("practice."))
            .count(),
        5
    );
    let (source, c) = practice_fixture();
    assert!(Audition::compile(&source, c).is_err());
}
#[test]
#[ignore = "requires installed authorized TimGM6mb/FluidSynth; direct native binary"]
fn source_matched_practice_midi_pizzicato_renders_with_mirrored_controllers() {
    use scorestream_audio::midi_mixer::LocalMidiPolicy;
    use support::PracticeReviewPolicy;
    let (source, c) = practice_fixture();
    let q = Audition::compile_practice(
        &source,
        scorestream_app::playback::PracticeTieNoteOnPolicy::InertWithoutNewNoteOnV1,
        scorestream_app::playback::PracticeHoldEndpointPolicy::ReleaseUnmarkedAtBoundaryBeforeHoldV1,
        scorestream_app::playback::PracticeNoteExpressionPolicy::AuthoredNoteOnWithRelativeTrajectoryV1,
        c,
        PracticeReviewPolicy::WrittenNotesWithoutWedgeDashesOrnamentsSlurV1,
        LocalMidiPolicy::InitialGmCc7CenteredV1,
    )
    .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("rehearsal.wav");
    let report = q
        .render(&scorestream_audio::default_soundfont().unwrap(), &output)
        .unwrap();
    assert_eq!(report["report"]["render"]["note_count"], 6);
    assert!(report["report"]["render"]["peak"].as_f64().unwrap() > 0.0001);
    assert_eq!(report["report"]["technique_channels"][0]["attacks"], 2);
    let serialized = serde_json::to_string(&report).unwrap();
    assert!(serialized.contains("practice.ornaments_unrepresented"));
    assert!(serialized.contains("inherited_from_native_channel"));
    assert_eq!(&std::fs::read(&output).unwrap()[..4], b"RIFF");
}

#[test]
fn optional_final_ending_config_is_closed_and_default_reject() {
    let c:Config=serde_json::from_value(config()).unwrap();
    assert!(matches!(c.final_ending,support::FinalEnding::Reject));
    let mut selected=config();selected["final_ending"]="continue_to_movement_end".into();
    // Plain/strict invocation may not borrow a practice-only navigation choice.
    assert!(compile(SOURCE,selected.clone()).err().unwrap().to_string().contains("authenticated practice joint"));
    selected["final_ending"]="invent_missing_endpoint".into();
    assert!(serde_json::from_value::<Config>(selected).is_err());
}

#[test]
fn boundary_policy_is_explicit_and_cannot_enter_ordinary_compiler() {
    let default: Config = serde_json::from_value(config()).unwrap();
    assert!(default.boundary_dynamics == support::BoundaryDynamics::Reject);
    let mut selected = config();
    selected["boundary_dynamics"] = json!("adjacent_opening_visit_attack_percent_v1");
    let err = match compile(SOURCE, selected) { Ok(_) => panic!("ordinary path must not accept boundary practice"), Err(e) => e.to_string() };
    assert!(err.contains("Boundary dynamics requires authenticated practice joint path"), "{err}");
    let mut unknown = config();
    unknown["boundary_dynamics"] = json!("ignore_authored_dynamics");
    assert!(serde_json::from_value::<Config>(unknown).is_err());
}

fn captured_practice() -> (String, Audition) {
    let (source, config)=practice_fixture();
    let session=Audition::compile_practice(&source,
        scorestream_app::playback::PracticeTieNoteOnPolicy::InertWithoutNewNoteOnV1,
        scorestream_app::playback::PracticeHoldEndpointPolicy::ReleaseUnmarkedAtBoundaryBeforeHoldV1,
        scorestream_app::playback::PracticeNoteExpressionPolicy::AuthoredNoteOnWithRelativeTrajectoryV1,
        config,support::PracticeReviewPolicy::WrittenNotesWithoutWedgeDashesOrnamentsSlurV1,
        scorestream_audio::midi_mixer::LocalMidiPolicy::InitialGmCc7CenteredV1).unwrap();
    (source,session)
}
#[test]
fn library_session_captures_source_and_rejects_stale_or_cancelled_work_before_io() {
    fn assert_worker_safe<T:Send+Sync>() {}
    assert_worker_safe::<Audition>();assert_worker_safe::<support::DryReadiness>();
    let (mut source,session)=captured_practice();
    let captured=source.clone();
    assert_eq!(session.source_sha256(),scorestream_audio::assignments::source_sha256(&captured));
    source.push('\n');assert!(session.verify_snapshot(&source).is_err());
    session.verify_snapshot(&captured).unwrap();
    let missing=std::path::Path::new("/not-a-selected-bank.sf2");
    let err=session.prepare_dry_snapshot(&source,missing,&std::sync::atomic::AtomicBool::new(false)).err().unwrap();
    assert!(err.to_string().contains("snapshot changed"));
    let err=session.prepare_dry_snapshot(&captured,missing,&std::sync::atomic::AtomicBool::new(true)).err().unwrap();
    assert!(err.to_string().contains("cancelled"));
}
#[test]
#[ignore="requires installed trusted FluidSynth and exact local TimGM; no corpus assets"]
fn library_session_reuses_native_readiness_and_atomic_export_on_one_captured_score() {
    use std::{path::Path,sync::atomic::AtomicBool};
    let (source,session)=captured_practice();
    let cancel=AtomicBool::new(false);
    let prepared=session.prepare_dry_snapshot(&source,Path::new("/usr/share/sounds/sf2/TimGM6mb.sf2"),&cancel).unwrap();
    assert_eq!(prepared.source_sha256(),session.source_sha256());
    assert_eq!(prepared.report()["admitted_notes"],6);
    assert_eq!(prepared.report()["playing_lanes_at_end"],0);
    let directory=tempfile::tempdir().unwrap();let output=directory.path().join("captured.wav");
    let mut other_config=config();
    let mut extra=other_config["palette"][0].clone();extra["part"]="r".into();other_config["palette"].as_array_mut().unwrap().push(extra);
    other_config["source_sha256"]=session.source_sha256().into();
    other_config["timing"]["holds"][0]["added_seconds"]="3/4".into();
    let other=Audition::compile_practice(&source,
        scorestream_app::playback::PracticeTieNoteOnPolicy::InertWithoutNewNoteOnV1,
        scorestream_app::playback::PracticeHoldEndpointPolicy::ReleaseUnmarkedAtBoundaryBeforeHoldV1,
        scorestream_app::playback::PracticeNoteExpressionPolicy::AuthoredNoteOnWithRelativeTrajectoryV1,
        serde_json::from_value(other_config).unwrap(),support::PracticeReviewPolicy::WrittenNotesWithoutWedgeDashesOrnamentsSlurV1,
        scorestream_audio::midi_mixer::LocalMidiPolicy::InitialGmCc7CenteredV1).unwrap();
    assert!(other.render_prepared_snapshot(&source,&prepared,&output,&cancel).is_err());
    assert!(!output.exists());

    assert!(session.render_prepared_snapshot(&(source.clone()+"\n"),&prepared,&output,&cancel).is_err());
    assert!(!output.exists());
    assert!(session.render_prepared_snapshot(&source,&prepared,&output,&AtomicBool::new(true)).is_err());
    assert!(!output.exists());
    let report=session.render_prepared_snapshot(&source,&prepared,&output,&cancel).unwrap();
    let actual:Value=serde_json::from_slice(&serde_json::to_vec(&report).unwrap()).unwrap();
    assert_eq!(actual,*prepared.report());
    let bytes=std::fs::read(&output).unwrap();
    assert_eq!(&bytes[..4],b"RIFF");
    assert_eq!(bytes.len() as u64,44+4*report["completed_frames"].as_u64().unwrap());
    assert!(bytes[44..].iter().any(|v|*v!=0));
    assert!(session.render_prepared_snapshot(&source,&prepared,&output,&cancel).is_err());
    assert_eq!(std::fs::read(&output).unwrap(),bytes);
}

#[path = "../../scorestream-desktop/src/offline_export/lease.rs"]
mod desktop_lease;
#[test]
#[ignore="requires installed trusted FluidSynth and exact local TimGM; no corpus assets"]
fn native_prepared_session_publishes_through_real_window_lease() {
    use std::{path::Path, sync::Mutex};
    let (source, session) = captured_practice();
    let mut leases = desktop_lease::Leases::default();
    let work = leases.begin("main", "r1", &source).unwrap();
    let prepared = session.prepare_dry_snapshot(&source,
        Path::new("/usr/share/sounds/sf2/TimGM6mb.sf2"), work.cancellation()).unwrap();
    leases.ready("main", &work, ()).unwrap(); leases.start_render("main", &work).unwrap();
    let state = Mutex::new(leases);
    let directory = tempfile::tempdir().unwrap(); let output = directory.path().join("gated.wav");
    let report = session.render_prepared_snapshot_with_gate(&source, &prepared, &output,
        work.cancellation(), |pending| {
            state.lock().unwrap().publish("main", &work, || pending.commit())
                .map_err(|_| scorestream_audio::AudioError::Cancelled)?
        }).unwrap();
    let normalized: Value = serde_json::from_slice(&serde_json::to_vec(&report).unwrap()).unwrap();
    assert_eq!(normalized, *prepared.report());
    let bytes = std::fs::read(&output).unwrap();
    assert_eq!(bytes.len() as u64, 44 + 4 * report["completed_frames"].as_u64().unwrap());
    assert!(bytes[44..].iter().any(|v| *v != 0));
    state.lock().unwrap().cancel("main", &work);
    assert_eq!(std::fs::read(&output).unwrap(), bytes);
}

#[test]
#[ignore="requires installed trusted FluidSynth and exact local TimGM; strict desktop path"]
fn strict_joint_fixture_does_not_silently_gain_a_source_mixer() {
    let session = compile(SOURCE, config()).unwrap();
    let cancel = std::sync::atomic::AtomicBool::new(false);
    let error = session.prepare_dry_snapshot(SOURCE,
        std::path::Path::new("/usr/share/sounds/sf2/TimGM6mb.sf2"), &cancel).err().unwrap();
    assert!(error.to_string().contains("requires source mixer"));
}

#[path = "../../scorestream-desktop/src/offline_export/errors.rs"]
mod offline_errors;
