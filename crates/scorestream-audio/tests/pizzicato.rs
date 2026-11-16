#![cfg(target_os = "linux")]
use scorestream_app::playback::{self, PizzicatoSoundPolicy, TechniquePlaybackPlan};
use scorestream_audio::{pizzicato::*, *};
use std::sync::atomic::AtomicBool;
const SHA: &str = "c5378b62028c920cb11e4803327983fee2f2cdff5dc89c708e39da417e51c854";
fn source() -> String {
    "!score 1.1 unit=q\n!requires scorestream.words@1\n!requires scorestream.pizzicato@1\n!part p instrument=violin staves=1\n!voice v part=p staff=1\n!voice w part=p staff=1\n@0\n| p m len=4\n+ #m scorestream.words part=p index=0 text=\"pizz.\" pizzicato=\"yes\" pizzicato_at=1\nv C4:2 id=bow\n.\n@1\nw C4:1 id=pluck\n.\n@2\n+ #m scorestream.words part=p index=1 text=\"arco\" pizzicato=\"no\" pizzicato_at=2\nv D4:1 id=arco\n.\n!end at=4\n".into()
}
fn plan(s: &str) -> TechniquePlaybackPlan {
    playback::plan_source_with_pizzicato(
        s,
        PizzicatoSoundPolicy::GeneralMidiSharedStringsApproximation,
    )
    .unwrap()
}
fn routes() -> Vec<RouteAssignment> {
    vec![RouteAssignment {
        part: "p".into(),
        instrument: "violin".into(),
        bank: 0,
        program: 40,
    }]
}
fn options() -> RenderOptions {
    RenderOptions {
        bank_verification: Some(BankVerification {
            sha256: SHA.into(),
            preset_names: vec!["Violin".into()],
        }),
        ..Default::default()
    }
}
#[test]
fn explicit_attack_receipt_preserves_identity_timing_and_concurrent_bowed_key() {
    let s = source();
    assert!(
        playback::plan_source(&s)
            .unwrap_err()
            .contains("pizzicato.not_realized")
    );
    let p = plan(&s);
    assert_eq!(
        p.pizzicato_attacks().iter().cloned().collect::<Vec<_>>(),
        vec!["pluck"]
    );
    assert!(
        p.plan()
            .notes
            .iter()
            .all(|n| n.instrument == "violin" && n.part == "p")
    );
    let base = playback::plan_source(&s.replace("pizzicato=\"yes\"", "pizzicato=\"no\"")).unwrap();
    assert_eq!(p.plan().notes, base.notes);
    assert!(validate_plan(p.plan(), &routes(), &options()).is_err());
    validate_pizzicato_plan(
        &p,
        &routes(),
        &options(),
        PizzicatoSamplePolicy::InstalledTimGm6mbSharedStringsV1,
    )
    .unwrap();
}
#[test]
fn key_profile_and_bank_require_exact_authorization() {
    let high = plan(&source().replace("w C4:1", "w C9:1"));
    assert!(
        validate_pizzicato_plan(
            &high,
            &routes(),
            &options(),
            PizzicatoSamplePolicy::InstalledTimGm6mbSharedStringsV1
        )
        .unwrap_err()
        .to_string()
        .contains("0..108")
    );
    let mut bad = options();
    bad.bank_verification.as_mut().unwrap().sha256 = "0".repeat(64);
    assert!(
        validate_pizzicato_plan(
            &plan(&source()),
            &routes(),
            &bad,
            PizzicatoSamplePolicy::InstalledTimGm6mbSharedStringsV1
        )
        .is_err()
    );
    assert!(
        validate_pizzicato_plan(
            &plan(&source()),
            &routes(),
            &RenderOptions::default(),
            PizzicatoSamplePolicy::InstalledTimGm6mbSharedStringsV1
        )
        .is_err()
    );
}
#[test]
fn tie_continuation_does_not_switch_an_existing_bowed_attack() {
    let s = source()
        .replace("v C4:2 id=bow", "v C4:1 id=bow\n~ tie t start=#bow")
        .replace("w C4:1 id=pluck", "v C4:1 id=pluck\n~ tie t end=#pluck");
    let p = plan(&s);
    assert!(p.pizzicato_attacks().is_empty());
    assert_eq!(p.plan().notes.len(), 2);
}
#[test]
#[ignore = "already-installed authorized TimGM6mb and FluidSynth only; launch binary directly"]
fn actual_installed_pizzicato_pcm_and_atomic_wav_are_verified() {
    let backend = FluidSynthBackend::open_installed().unwrap();
    let p = plan(&source());
    let bank = default_soundfont().unwrap();
    let cancel = AtomicBool::new(false);
    let mut pcm = Vec::new();
    let report = backend
        .render_pcm_with_pizzicato(
            &p,
            &routes(),
            &bank,
            &options(),
            PizzicatoSamplePolicy::InstalledTimGm6mbSharedStringsV1,
            &cancel,
            &mut |_, s: &[f32]| {
                pcm.extend_from_slice(s);
                Ok(())
            },
        )
        .unwrap();
    assert!(report.render.peak > 0.0001);
    assert_eq!(report.render.routes[1].preset_name, "Pizzicato");
    assert_eq!(report.technique_channels[0].attacks, 1);
    assert_eq!(report.render.note_count, 3);
    let isolated = source()
        .replace("v C4:2 id=bow", "v r:2 id=bow")
        .replace("v D4:1 id=arco", "v r:1 id=arco");
    let mut isolated_pcm = Vec::new();
    let isolated_report = backend
        .render_pcm_with_pizzicato(
            &plan(&isolated),
            &routes(),
            &bank,
            &options(),
            PizzicatoSamplePolicy::InstalledTimGm6mbSharedStringsV1,
            &cancel,
            &mut |_, s: &[f32]| {
                isolated_pcm.extend_from_slice(s);
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(isolated_report.render.note_count, 1);
    assert!(isolated_report.render.peak > 0.0001);
    let bowed =
        playback::plan_source(&isolated.replace("pizzicato=\"yes\"", "pizzicato=\"no\"")).unwrap();
    let mut bowed_pcm = Vec::new();
    backend
        .render_pcm(
            &bowed,
            &routes(),
            &bank,
            &options(),
            &cancel,
            &mut |_, s: &[f32]| {
                bowed_pcm.extend_from_slice(s);
                Ok(())
            },
        )
        .unwrap();
    assert_ne!(isolated_pcm, bowed_pcm);
    let mut bad = options();
    bad.bank_verification.as_mut().unwrap().preset_names[0] = "not-the-preset".into();
    let mut writes = 0;
    assert!(
        backend
            .render_pcm_with_pizzicato(
                &p,
                &routes(),
                &bank,
                &bad,
                PizzicatoSamplePolicy::InstalledTimGm6mbSharedStringsV1,
                &cancel,
                &mut |_, _: &[f32]| {
                    writes += 1;
                    Ok(())
                }
            )
            .is_err()
    );
    assert_eq!(writes, 0);
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("actual.wav");
    let report = backend
        .render_wav_with_pizzicato(
            &p,
            &routes(),
            &bank,
            &options(),
            PizzicatoSamplePolicy::InstalledTimGm6mbSharedStringsV1,
            &cancel,
            &path,
        )
        .unwrap();
    let bytes = std::fs::read(path).unwrap();
    assert_eq!(&bytes[..4], b"RIFF");
    assert_eq!(bytes.len() as u64, 44 + report.render.frames * 4);
}

#[test]
fn simultaneous_same_key_on_one_pizzicato_lane_is_not_silently_retriggered() {
    let s = source().replace("w C4:1 id=pluck", "w C4:1 id=pluck\nv C4:1 id=other");
    let p = plan(&s);
    assert_eq!(p.pizzicato_attacks().len(), 2);
    assert!(
        validate_pizzicato_plan(
            &p,
            &routes(),
            &options(),
            PizzicatoSamplePolicy::InstalledTimGm6mbSharedStringsV1
        )
        .is_err()
    );
}

#[test]
fn catalog_alias_is_not_rewritten_or_guessed_from_a_label() {
    let p = plan(&source().replace("instrument=violin", "instrument=contrabass"));
    let routes = vec![RouteAssignment {
        part: "p".into(),
        instrument: "contrabass".into(),
        bank: 0,
        program: 43,
    }];
    let mut options = options();
    options.bank_verification.as_mut().unwrap().preset_names = vec!["Contrabass".into()];
    validate_pizzicato_plan(
        &p,
        &routes,
        &options,
        PizzicatoSamplePolicy::InstalledTimGm6mbSharedStringsV1,
    )
    .unwrap();
    assert!(p.plan().notes.iter().all(|n| n.instrument == "contrabass"));
    let custom = plan(&source().replace("instrument=violin", "instrument=my_violin"));
    assert!(
        validate_pizzicato_plan(
            &custom,
            &routes,
            &options,
            PizzicatoSamplePolicy::InstalledTimGm6mbSharedStringsV1
        )
        .is_err()
    );
}
