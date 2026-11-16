use scorestream_app::playback::*;
use scorestream_audio::{attack_rehearsal::*, midi_mixer::*, pizzicato::*, *};
use scorestream_codec::{fermata_timing::*, notation::*};
fn source() -> String {
    "!score 1.1 unit=q\n!requires scorestream.instrument@1\n!requires scorestream.fermata@1\n!requires scorestream.articulation@1\n!part p instrument=violin staves=1\n!voice v part=p staff=1\n@0\n| p m len=5\n+ p scorestream.instrument midi_ref=\"i\" name=\"Violin\" pan=0 program=41 source_id=\"i\" volume=100\n= p sound_dynamics=50\nv C4:4 id=a\n+ #a scorestream.fermata\n.\n@2\n= p sound_dynamics=100\n.\n@4\nv D4:1 id=art\n+ #art scorestream.articulation.event event_index=0\n+ #art scorestream.articulation.notations event_index=0 wrapper_index=0\n+ #art scorestream.articulation.group event_index=0 group_index=0 wrapper_index=0\n+ #art scorestream.articulation.item event_index=0 group_index=0 item_index=0 sign=staccato wrapper_index=0\n.\n!end at=5\n".into()
}
fn routes() -> Vec<RouteAssignment> {
    vec![RouteAssignment {
        part: "p".into(),
        instrument: "violin".into(),
        bank: 0,
        program: 40,
    }]
}
fn compiled(s: &str) -> TechniquePlaybackPlan {
    plan_source_with_practice_joint_policies_and_pizzicato(
        s,
        PracticeTieNoteOnPolicy::InertWithoutNewNoteOnV1,
        PracticeHoldEndpointPolicy::ReleaseUnmarkedAtBoundaryBeforeHoldV1,
        PracticeNoteExpressionPolicy::AuthoredNoteOnWithRelativeTrajectoryV1,
        PracticeDiscoveryPolicy::WrittenNotesWithoutWedgeDashesOrnamentsSlurV1,
        &[PracticeMidiSelection {
            part: "p".into(),
            instrument: "violin".into(),
            bank: 0,
            program_zero_based: 40,
        }],
        ExplicitArticulationPolicy::SimpleStaccatoAccentV1,
        &ExplicitGracePolicy { groups: vec![] },
        GraceSoundPolicy::FullGraceThenGatedRemainder,
        &ExplicitSynchronizedFermataPolicy {
            groups: vec![ExplicitSynchronizedHoldGroup {
                boundary: "4".parse().unwrap(),
                hold: ExplicitHoldGroup {
                    id: "fixture-hold".into(),
                    members: vec![EventId("occ0:a".into())],
                    added_seconds: "1/4".parse().unwrap(),
                },
            }],
        },
        FermataSoundPolicy::SustainSelectedAnchors,
        PizzicatoSoundPolicy::GeneralMidiSharedStringsApproximation,
    )
    .unwrap()
}
fn options(s: &str) -> RenderOptions {
    RenderOptions {
        local_midi_mix: Some(
            prepare_local_midi_mix(
                s,
                &assignments::source_sha256(s),
                &routes(),
                LocalMidiPolicy::InitialGmCc7CenteredV1,
            )
            .unwrap(),
        ),
        bank_verification: Some(BankVerification {
            sha256: "c5378b62028c920cb11e4803327983fee2f2cdff5dc89c708e39da417e51c854".into(),
            preset_names: vec!["Violin".into()],
        }),
        ..Default::default()
    }
}
#[test]
fn explicit_projection_is_source_plan_bound_and_does_not_mutate_notes() {
    let s = source();
    let plan = compiled(&s);
    let before = serde_json::to_value(plan.plan()).unwrap();
    let mut opts = options(&s);
    assert!(
        validate_pizzicato_plan(
            &plan,
            &routes(),
            &opts,
            PizzicatoSamplePolicy::InstalledTimGm6mbSharedStringsV1
        )
        .is_err()
    );
    let receipt = prepare_attack_only_rehearsal(
        &plan,
        opts.local_midi_mix.as_ref().unwrap(),
        RehearsalEnvelopePolicy::MidiAttackVelocityOnlyV1,
    )
    .unwrap();
    assert_eq!(receipt.counts().affected_notes, 1);
    assert_eq!(receipt.counts().time_varying_envelopes, 1);
    assert_eq!(
        receipt.omissions()[0].original_envelope,
        plan.plan().notes[0].gain_envelope
    );
    opts.attack_only_rehearsal = Some(receipt);
    validate_pizzicato_plan(
        &plan,
        &routes(),
        &opts,
        PizzicatoSamplePolicy::InstalledTimGm6mbSharedStringsV1,
    )
    .unwrap();
    assert!(
        validate_plan(plan.plan(), &routes(), &opts).is_err(),
        "default renderer remains blocked"
    );
    assert_eq!(before, serde_json::to_value(plan.plan()).unwrap());
    let changed = compiled(&(s.clone() + "\n"));
    assert!(
        validate_pizzicato_plan(
            &changed,
            &routes(),
            &opts,
            PizzicatoSamplePolicy::InstalledTimGm6mbSharedStringsV1
        )
        .is_err()
    );
    let overlap = s
        .replace(
            "!voice v part=p staff=1",
            "!voice v part=p staff=1\n!voice w part=p staff=1",
        )
        .replace("v C4:4 id=a", "v C4:4 id=a\nw C4:4 id=b");
    let p = compiled(&overlap);
    let mut o = options(&overlap);
    o.attack_only_rehearsal = Some(
        prepare_attack_only_rehearsal(
            &p,
            o.local_midi_mix.as_ref().unwrap(),
            RehearsalEnvelopePolicy::MidiAttackVelocityOnlyV1,
        )
        .unwrap(),
    );
    assert!(
        validate_pizzicato_plan(
            &p,
            &routes(),
            &o,
            PizzicatoSamplePolicy::InstalledTimGm6mbSharedStringsV1
        )
        .unwrap_err()
        .to_string()
        .contains("Overlapping same-key")
    );
}
#[cfg(target_os = "linux")]
#[test]
#[ignore = "installed TimGM6mb/FluidSynth only; direct binary; synthetic authenticated practice plan, no full-score WAV"]
fn actual_bank_attack_only_pcm_keeps_timing_pitch_velocity_and_disclosures() {
    let s = source().replace("id=a\n", "id=a attack_dynamics=1361/25\n");
    let plan = compiled(&s);
    let mut opts = options(&s);
    opts.sample_rate = 24000;
    opts.tail_seconds = 0.25;
    opts.attack_only_rehearsal = Some(
        prepare_attack_only_rehearsal(
            &plan,
            opts.local_midi_mix.as_ref().unwrap(),
            RehearsalEnvelopePolicy::MidiAttackVelocityOnlyV1,
        )
        .unwrap(),
    );
    let before = plan.plan().clone();
    let report = FluidSynthBackend::open_installed()
        .unwrap()
        .render_pcm_with_pizzicato(
            &plan,
            &routes(),
            std::path::Path::new("/usr/share/sounds/sf2/TimGM6mb.sf2"),
            &opts,
            PizzicatoSamplePolicy::InstalledTimGm6mbSharedStringsV1,
            &std::sync::atomic::AtomicBool::new(false),
            &mut |_, _: &[f32]| Ok(()),
        )
        .unwrap();
    assert_eq!(report.render.note_count, before.notes.len());
    assert!(report.render.peak > 0.0001);
    assert_eq!(plan.plan(), &before);
    assert_eq!(
        report
            .render
            .attack_only_rehearsal
            .as_ref()
            .unwrap()
            .counts()
            .time_varying_envelopes,
        1
    );
    assert!(
        report
            .render
            .warnings
            .iter()
            .any(|w| w.contains("not faithful"))
    );
}

#[test]
fn exact_compiler_provenance_separates_note_on_residual_from_shape() {
    for varying in [false, true] {
        let mut s = source().replace("id=a\n", "id=a attack_dynamics=1361/25\n");
        if !varying {
            s = s.replace("sound_dynamics=100", "sound_dynamics=50");
        }
        let plan = compiled(&s);
        let opts = options(&s);
        let token = prepare_attack_only_rehearsal(
            &plan,
            opts.local_midi_mix.as_ref().unwrap(),
            RehearsalEnvelopePolicy::MidiAttackVelocityOnlyV1,
        )
        .unwrap();
        assert_eq!(token.counts().compiler_note_on_receipts, 1);
        assert_eq!(token.counts().fractional_note_on_residuals, 1);
        assert_eq!(
            token.counts().note_on_relative_trajectories,
            usize::from(varying)
        );
        assert_eq!(token.counts().affected_notes_without_note_on_receipt, 0);
    }
}
