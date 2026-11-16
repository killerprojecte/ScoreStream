use super::*;
use crate::midi_mixer::{LocalMidiPolicy, prepare_local_midi_mix};

fn source() -> String {
    "!score 1.1 unit=q\n!requires scorestream.instrument@1\n!part p instrument=violin staves=1\n!part q instrument=violin staves=1\n!voice v part=p staff=1\n!voice w part=q staff=1\n@0\n| p m len=2\n| q n len=2\n+ p scorestream.instrument midi_ref=\"i\" name=\"Violin\" pan=0 program=41 source_id=\"i\" volume=100\n+ q scorestream.instrument midi_ref=\"j\" name=\"Violin\" pan=0 program=41 source_id=\"j\" volume=100\nv C4:64/375 id=a\n.\n@1/375\nw C4:127/375 id=b\n.\n@512/375\nv C4:64/375 id=c\n.\n!end at=2\n".into()
}
fn routes() -> Vec<RouteAssignment> {
    ["p", "q"]
        .into_iter()
        .map(|p| RouteAssignment {
            part: p.into(),
            instrument: "violin".into(),
            bank: 0,
            program: 40,
        })
        .collect()
}
fn note(id: &str, channel: u16) -> crate::plan::Note {
    crate::plan::Note {
        event_id: id.into(),
        channel,
        key: 60,
        velocity: 80,
        start: 0,
        end: 0,
    }
}
#[test]
#[ignore = "requires installed trusted FluidSynth and exact TimGM; run executable directly without LD overrides"]
fn source_bound_dry_groups_preserve_independent_releases_and_safe_reuse() {
    let api = Api::open().unwrap();
    let source = source();
    let routes = routes();
    let options = RenderOptions {
        sample_rate: 48000,
        gain: 0.04,
        polyphony: 256,
        local_midi_mix: Some(
            prepare_local_midi_mix(
                &source,
                &crate::assignments::source_sha256(&source),
                &routes,
                LocalMidiPolicy::InitialGmCc7CenteredV1,
            )
            .unwrap(),
        ),
        bank_verification: Some(crate::BankVerification {
            sha256: "c5378b62028c920cb11e4803327983fee2f2cdff5dc89c708e39da417e51c854".into(),
            preset_names: vec!["Violin".into(); 2],
        }),
        ..Default::default()
    };
    let cancel = AtomicBool::new(false);
    let mut bank = Bank::open(
        Path::new("/usr/share/sounds/sf2/TimGM6mb.sf2"),
        &options,
        &cancel,
    )
    .unwrap();
    assert!(
        DrySynth::new(
            &api,
            &mut bank,
            &(source.clone() + "\n"),
            &routes,
            &options,
            &cancel
        )
        .is_err()
    );
    let mut wrong = options.clone();
    wrong.bank_verification.as_mut().unwrap().sha256 = "wrong".into();
    assert!(DrySynth::new(&api, &mut bank, &source, &routes, &wrong, &cancel).is_err());
    let run = |first: bool, second: bool| {
        let mut bank = Bank::open(
            Path::new("/usr/share/sounds/sf2/TimGM6mb.sf2"),
            &options,
            &cancel,
        )
        .unwrap();
        let mut synth =
            DrySynth::new(&api, &mut bank, &source, &routes, &options, &cancel).unwrap();
        assert_eq!(
            synth.source_sha256(),
            crate::assignments::source_sha256(&source)
        );
        assert_eq!(synth.quantum(), 64);
        assert!(synth.event(&note("alias", 2), true).is_err());
        assert!(synth.render(63).is_err());
        let mut output = vec![Vec::new(); 4];
        let mut released_audible = false;
        let mut first_empty = None;
        for frame in (0..65536).step_by(64) {
            let state = synth.snapshot().unwrap();
            assert_eq!(state.frame(), frame);
            if first && frame == 0 {
                synth.event(&note("a", 0), true).unwrap();
            }
            if second && frame == 64 {
                synth.event(&note("b", 1), true).unwrap();
            }
            if first && frame == 4096 {
                synth.event(&note("a", 0), false).unwrap();
                assert!(synth.snapshot().unwrap().playing_per_channel()[0] > 0);
                assert!(synth.event(&note("early", 0), true).is_err());
                assert!(synth.event(&note("wrong", 0), false).is_err());
            }
            if second && frame == 8192 {
                synth.event(&note("b", 1), false).unwrap();
            }
            if first && frame > 4096 && state.playing_per_channel()[0] == 0 && first_empty.is_none()
            {
                first_empty = Some(frame);
            }
            if first && frame == 32768 {
                assert!(first_empty.is_some());
                synth.event(&note("c", 0), true).unwrap();
            }
            if first && frame == 36864 {
                synth.event(&note("c", 0), false).unwrap();
            }
            let block = synth.render(64).unwrap();
            if first
                && frame >= 4096
                && frame < 8192
                && block[..2].iter().flatten().any(|x| *x != 0.)
            {
                released_audible = true;
            }
            for (to, from) in output.iter_mut().zip(block) {
                to.extend(from);
            }
        }
        if first {
            assert!(released_audible);
            assert!(first_empty.unwrap() > 4096);
        }
        (output, first_empty)
    };
    let (together, empty) = run(true, true);
    let (solo_first, solo_empty) = run(true, false);
    let (solo_second, _) = run(false, true);
    assert_eq!(empty, solo_empty);
    assert_eq!(together[..2], solo_first[..2]);
    assert_eq!(together[2..], solo_second[2..]);
    assert!(together.iter().flatten().any(|x| *x != 0.));
    for channel in &together[..2] {
        assert!(
            channel[empty.unwrap() as usize..32768]
                .iter()
                .all(|x| *x == 0.)
        );
    }
    println!(
        "dry source={} bank={} groups=2 first_empty_frame={:?}; all samples and release tails equal same-history isolated lane references",
        crate::assignments::source_sha256(&source),
        bank.hash,
        empty
    );
}

#[test]
#[ignore = "requires installed trusted FluidSynth and exact TimGM; no WAV output"]
fn opaque_plan_native_owners_and_original_envelopes_compose_without_cross_talk() {
    use crate::dry_lanes::{DryEnvelopeSchedule, DryLane, DryTailPolicy, LaneVoiceState};
    use scorestream_app::playback::*;
    use scorestream_codec::{fermata_timing::*, notation::*};
    let source = source()
        .replace(
            "v C4:64/375 id=a",
            "= p sound_dynamics=100\n= q sound_dynamics=100\nv C4:64/345 id=a",
        )
        .replace("@512/375", "@667/8000\n= p sound_dynamics=50\n.\n@512/375");
    let compile = |s: &str| {
        plan_source_with_practice_joint_policies_and_pizzicato(
            s,
            PracticeTieNoteOnPolicy::InertWithoutNewNoteOnV1,
            PracticeHoldEndpointPolicy::ReleaseUnmarkedAtBoundaryBeforeHoldV1,
            PracticeNoteExpressionPolicy::AuthoredNoteOnWithRelativeTrajectoryV1,
            PracticeDiscoveryPolicy::WrittenNotesWithoutWedgeDashesOrnamentsSlurV1,
            &["p", "q"].map(|p| PracticeMidiSelection {
                part: p.into(),
                instrument: "violin".into(),
                bank: 0,
                program_zero_based: 40,
            }),
            ExplicitArticulationPolicy::SimpleStaccatoAccentV1,
            &ExplicitGracePolicy { groups: vec![] },
            GraceSoundPolicy::FullGraceThenGatedRemainder,
            &ExplicitSynchronizedFermataPolicy {
                groups: vec![ExplicitSynchronizedHoldGroup {
                    boundary: "192/125".parse().unwrap(),
                    hold: ExplicitHoldGroup {
                        id: "native-tail-test".into(),
                        members: vec![EventId("occ0:c".into())],
                        added_seconds: "1/750".parse().unwrap(),
                    },
                }],
            },
            FermataSoundPolicy::SustainSelectedAnchors,
            PizzicatoSoundPolicy::GeneralMidiSharedStringsApproximation,
        )
        .unwrap()
    };
    let source = source.replace("!requires scorestream.instrument@1", "!requires scorestream.instrument@1\n!requires scorestream.articulation@1\n!requires scorestream.fermata@1")

        .replace("w C4:127/375 id=b", "w C4:128/375 id=b\n+ #b scorestream.articulation.event event_index=0\n+ #b scorestream.articulation.notations event_index=0 wrapper_index=0\n+ #b scorestream.articulation.group event_index=0 group_index=0 wrapper_index=0\n+ #b scorestream.articulation.item event_index=0 group_index=0 item_index=0 sign=staccato wrapper_index=0")
        .replace("v C4:64/375 id=c", "v C4:64/375 id=c\n+ #c scorestream.fermata");
    let plan = compile(&source);
    assert_eq!(plan.plan().notes.len(), 3);
    let original_plan = serde_json::to_value(plan.plan()).unwrap();
    let schedule =
        DryEnvelopeSchedule::prepare(&plan, 48000, DryTailPolicy::HoldLastGainAtNoteOffV1).unwrap();
    schedule.verify_plan(&plan).unwrap();
    assert!(
        schedule
            .verify_plan(&compile(&(source.clone() + "\n")))
            .is_err()
    );
    let routes = routes();
    let options = RenderOptions {
        sample_rate: 48000,
        gain: 0.04,
        polyphony: 256,
        local_midi_mix: Some(
            prepare_local_midi_mix(
                &source,
                plan.source_sha256(),
                &routes,
                LocalMidiPolicy::InitialGmCc7CenteredV1,
            )
            .unwrap(),
        ),
        bank_verification: Some(crate::BankVerification {
            sha256: "c5378b62028c920cb11e4803327983fee2f2cdff5dc89c708e39da417e51c854".into(),
            preset_names: vec!["Violin".into(); 2],
        }),
        ..Default::default()
    };
    let notes: Vec<_> = plan
        .plan()
        .notes
        .iter()
        .zip(schedule.notes())
        .map(|(n, r)| {
            assert_eq!(n.id, r.event);
            assert_eq!(n.gain_envelope, r.original_envelope);
            assert_eq!(n.midi_note, 60.);
            assert!(
                r.start_frame % 64 == 0 && r.noteoff_frame % 64 == 0,
                "{:?}",
                r
            );
            crate::plan::Note {
                event_id: n.id.clone(),
                channel: if n.part == "p" { 0 } else { 1 },
                key: n.midi_note as u8,
                velocity: n.velocity,
                start: r.start_frame,
                end: r.noteoff_frame,
            }
        })
        .collect();
    let a = notes
        .iter()
        .position(|n| n.event_id.ends_with(":a"))
        .unwrap();
    let b = notes
        .iter()
        .position(|n| n.event_id.ends_with(":b"))
        .unwrap();
    let c = notes
        .iter()
        .position(|n| n.event_id.ends_with(":c"))
        .unwrap();
    assert_eq!(
        (
            notes[a].start,
            notes[a].end,
            notes[b].start,
            notes[b].end,
            notes[c].start,
            notes[c].end
        ),
        (0, 4096, 64, 4160, 32768, 36928)
    );
    assert!(
        schedule.notes()[a]
            .original_envelope
            .iter()
            .any(|p| p.gain != 1.)
    );
    assert!(
        schedule.notes()[b]
            .original_envelope
            .iter()
            .all(|p| p.gain == 1.)
    );
    let api = Api::open().unwrap();
    let cancel = AtomicBool::new(false);
    let run = |shaped: bool, enabled: [bool; 2]| {
        let mut bank = Bank::open(
            Path::new("/usr/share/sounds/sf2/TimGM6mb.sf2"),
            &options,
            &cancel,
        )
        .unwrap();
        let mut synth =
            DrySynth::new(&api, &mut bank, &source, &routes, &options, &cancel).unwrap();
        assert_eq!(synth.source_sha256(), schedule.source_sha256());
        let mut owners = [DryLane::default(), DryLane::default()];
        let mut active = [None; 2];
        let mut out = vec![Vec::<f64>::new(); 4];
        for frame in (0..65536).step_by(64) {
            for (i, n) in notes
                .iter()
                .enumerate()
                .filter(|(_, n)| enabled[n.channel as usize])
            {
                let lane = n.channel as usize;
                if n.end == frame {
                    owners[lane].noteoff(i, frame).unwrap();
                    synth.event(n, false).unwrap();
                }
            }
            for (i, n) in notes
                .iter()
                .enumerate()
                .filter(|(_, n)| enabled[n.channel as usize])
            {
                let lane = n.channel as usize;
                if n.start == frame {
                    owners[lane].admit(lane, i, frame, &synth).unwrap();
                    synth.event(n, true).unwrap();
                    active[lane] = Some(i);
                }
            }
            if frame == 4096 && enabled[0] {
                assert!(owners[0].admit(0, c, frame, &synth).is_err());
                assert!(synth.playing_voices(0).unwrap() > 0);
            }
            let raw = synth.render(64).unwrap();
            let mut block: Vec<Vec<f64>> = raw
                .into_iter()
                .map(|v| v.into_iter().map(f64::from).collect())
                .collect();
            if shaped {
                for lane in 0..2 {
                    if let Some(i) = active[lane] {
                        let (left, right) = block.split_at_mut(lane * 2 + 1);
                        schedule
                            .apply(i, frame, &mut left[lane * 2], &mut right[0])
                            .unwrap();
                    }
                }
            }
            for (to, from) in out.iter_mut().zip(block) {
                to.extend(from);
            }
        }
        out
    };
    let raw = run(false, [true, true]);
    let shaped = run(true, [true, true]);
    let solo_p = run(true, [true, false]);
    let solo_q = run(true, [false, true]);
    assert_eq!(shaped[..2], solo_p[..2]);
    assert_eq!(shaped[2..], solo_q[2..]);
    assert_eq!(shaped[2..], raw[2..]); // unit curve overlap and full tail untouched
    assert_ne!(shaped[..2], raw[..2]);
    // Independent point-wise reference, includes non-quantum authored step and
    // explicit NoteOff endpoint gain carried through all release samples.
    let mut max_error = 0f64;
    for channel in 0..2 {
        for frame in 0..65536usize {
            let i = if frame < 32768 { a } else { c };
            let r = &schedule.notes()[i];
            let t = if frame as u64 >= r.noteoff_frame {
                r.duration_seconds
            } else {
                (frame as f64 - r.start_frame as f64) / 48000.
            };
            let points = &r.original_envelope;
            let mut left_t = 0.;
            let mut gain = 1.;
            for p in points {
                if p.offset_seconds > t {
                    gain += (p.gain - gain) * (t - left_t) / (p.offset_seconds - left_t);
                    break;
                }
                left_t = p.offset_seconds;
                gain = p.gain;
            }
            max_error = max_error.max((shaped[channel][frame] - raw[channel][frame] * gain).abs());
        }
    }
    assert_eq!(max_error, 0.);
    assert_eq!(serde_json::to_value(plan.plan()).unwrap(), original_plan);
    let admission = crate::dry_admission::probe(
        &source,
        &plan,
        &routes,
        Path::new("/usr/share/sounds/sf2/TimGM6mb.sf2"),
        &options,
        DryTailPolicy::HoldLastGainAtNoteOffV1,
        crate::pizzicato::PizzicatoSamplePolicy::InstalledTimGm6mbSharedStringsV1,
        &cancel,
    )
    .unwrap();
    assert_eq!(admission.admitted_notes, 3);
    assert_eq!(admission.noteoffs, 3);
    assert!(admission.peak_owned_lanes >= 2);
    assert_eq!(admission.playing_lanes_at_end, 0);
    assert_eq!(admission.notes[a].rendered_on, 0);
    assert_eq!(admission.notes[c].rendered_on, 32768);
    assert_eq!(admission.notes[a].lane, admission.notes[c].lane);
    assert!(admission.sample_peak > 0.);
    let mut short_window = options.clone();
    short_window.tail_seconds = 0.;
    let exhausted = crate::dry_admission::probe_until_idle(
        &source,
        &plan,
        &routes,
        Path::new("/usr/share/sounds/sf2/TimGM6mb.sf2"),
        &short_window,
        DryTailPolicy::HoldLastGainAtNoteOffV1,
        crate::pizzicato::PizzicatoSamplePolicy::InstalledTimGm6mbSharedStringsV1,
        &cancel,
    )
    .unwrap();
    assert_eq!(exhausted.playing_lanes_at_end, 0);
    assert!(exhausted.first_idle_frame.unwrap() > exhausted.last_rendered_noteoff);
    assert!(exhausted.completed_frames > exhausted.selected_window_frames);
    assert_eq!(
        exhausted.completed_frames,
        exhausted.first_idle_frame.unwrap()
    );
    let expected: serde_json::Value =
        serde_json::from_slice(&serde_json::to_vec(&exhausted).unwrap()).unwrap();
    let directory = tempfile::tempdir().unwrap();
    let output = directory.path().join("dry.wav");
    let saved = crate::dry_admission::render_verified_wav(
        &source,
        &plan,
        &routes,
        Path::new("/usr/share/sounds/sf2/TimGM6mb.sf2"),
        &short_window,
        DryTailPolicy::HoldLastGainAtNoteOffV1,
        crate::pizzicato::PizzicatoSamplePolicy::InstalledTimGm6mbSharedStringsV1,
        &cancel,
        &expected,
        &output,
    )
    .unwrap();
    let bytes = std::fs::read(&output).unwrap();
    assert_eq!(bytes.len() as u64, 44 + saved.completed_frames * 4);
    assert_eq!(&bytes[..4], b"RIFF");
    assert!(bytes[44..].iter().any(|b| *b != 0));
    assert!(
        crate::dry_admission::render_verified_wav(
            &source,
            &plan,
            &routes,
            Path::new("/usr/share/sounds/sf2/TimGM6mb.sf2"),
            &short_window,
            DryTailPolicy::HoldLastGainAtNoteOffV1,
            crate::pizzicato::PizzicatoSamplePolicy::InstalledTimGm6mbSharedStringsV1,
            &cancel,
            &expected,
            &output
        )
        .is_err()
    );
    assert_eq!(std::fs::read(&output).unwrap(), bytes);
    let failed = directory.path().join("must-not-publish.wav");
    let mut incorrect = expected.clone();
    incorrect["sample_peak"] = serde_json::json!(0);
    assert!(
        crate::dry_admission::render_verified_wav(
            &source,
            &plan,
            &routes,
            Path::new("/usr/share/sounds/sf2/TimGM6mb.sf2"),
            &short_window,
            DryTailPolicy::HoldLastGainAtNoteOffV1,
            crate::pizzicato::PizzicatoSamplePolicy::InstalledTimGm6mbSharedStringsV1,
            &cancel,
            &incorrect,
            &failed
        )
        .is_err()
    );
    assert!(!failed.exists());
    assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);

    assert_eq!(serde_json::to_value(plan.plan()).unwrap(), original_plan);

    println!(
        "composed source={} plan={} events={}{}{} exact original envelope preserved; pointwise error={max_error}; other lane/tail unchanged",
        schedule.source_sha256(),
        schedule.plan_sha256(),
        notes[a].event_id,
        notes[b].event_id,
        notes[c].event_id
    );
}

#[test]
#[ignore = "requires installed trusted FluidSynth and exact TimGM; no WAV output"]
fn idle_reselection_uses_only_authorized_presets_and_restores_absent_cc_baseline() {
    let source=source().replace("!part q instrument=violin", "!part q instrument=flute")
        .replace("+ q scorestream.instrument midi_ref=\"j\" name=\"Violin\" pan=0 program=41 source_id=\"j\" volume=100", "+ q scorestream.instrument midi_ref=\"j\" name=\"Flute\" program=74 source_id=\"j\"");
    let mut routes = routes();
    routes[1].instrument = "flute".into();
    routes[1].program = 73;
    let mix = prepare_local_midi_mix(
        &source,
        &crate::assignments::source_sha256(&source),
        &routes,
        LocalMidiPolicy::InitialGmCc7CenteredV1,
    )
    .unwrap();
    assert!(mix.routes()[1].cc10.is_none());
    assert!(mix.routes()[1].volume.is_none());
    let options = RenderOptions {
        sample_rate: 48000,
        gain: 0.04,
        polyphony: 256,
        local_midi_mix: Some(mix),
        bank_verification: Some(crate::BankVerification {
            sha256: "c5378b62028c920cb11e4803327983fee2f2cdff5dc89c708e39da417e51c854".into(),
            preset_names: vec!["Violin".into(), "Flute TB".into()],
        }),
        ..Default::default()
    };
    let before = serde_json::to_value(&options.local_midi_mix).unwrap();
    let api = Api::open().unwrap();
    let cancel = AtomicBool::new(false);
    let run = |other: bool, flute: bool| {
        let mut bank = Bank::open(
            Path::new("/usr/share/sounds/sf2/TimGM6mb.sf2"),
            &options,
            &cancel,
        )
        .unwrap();
        let mut synth =
            DrySynth::new(&api, &mut bank, &source, &routes, &options, &cancel).unwrap();
        assert!(synth.reselect_idle_route(0, 2).is_err());
        // Prove source-absent controller is captured at native initialization,
        // then restored after a different source route explicitly sets CC7=127.
        let original = synth.reselect_idle_route(1, 1).unwrap();
        assert_eq!(original.cc7, 100);
        assert_eq!(original.cc10, 64);
        assert_eq!(synth.reselect_idle_route(1, 0).unwrap().cc7, 127);
        assert_eq!(synth.reselect_idle_route(1, 1).unwrap(), original);
        let mut output = vec![Vec::new(); 4];
        let mut binding = None;
        for frame in (0..65536).step_by(64) {
            if frame == 0 {
                synth.event(&note("a", 0), true).unwrap();
                assert!(synth.reselect_idle_route(0, 1).is_err());
            }
            if frame == 64 && other {
                synth.event(&note("b", 1), true).unwrap();
            }
            if frame == 4096 {
                synth.event(&note("a", 0), false).unwrap();
                assert!(synth.reselect_idle_route(0, 1).is_err());
            }
            if frame == 8192 && other {
                synth.event(&note("b", 1), false).unwrap();
            }
            if frame == 32768 {
                binding = Some(
                    synth
                        .reselect_idle_route(0, if flute { 1 } else { 0 })
                        .unwrap(),
                );
                synth.event(&note("c", 0), true).unwrap();
            }
            if frame == 36864 {
                synth.event(&note("c", 0), false).unwrap();
            }
            for (to, from) in output.iter_mut().zip(synth.render(64).unwrap()) {
                to.extend(from);
            }
        }
        (output, binding.unwrap())
    };
    let (pair, rebound) = run(true, true);
    let (solo, _) = run(false, true);
    let (unchanged, _) = run(true, false);
    assert_eq!((rebound.program, rebound.cc7, rebound.cc10), (73, 100, 64));
    assert_eq!(pair[..2], solo[..2]);
    assert_eq!(pair[2..], unchanged[2..]); // re-selection never changes other lane/tail
    assert_eq!(pair[0][..32768], unchanged[0][..32768]);
    assert_eq!(pair[1][..32768], unchanged[1][..32768]);
    assert_ne!(pair[0][32768..], unchanged[0][32768..]); // genuinely different preset
    assert_eq!(
        serde_json::to_value(&options.local_midi_mix).unwrap(),
        before
    );
    println!(
        "idle reselect source={} binding={:?}; original absent CC fields preserved; prior tail and independent lane bit-identical",
        crate::assignments::source_sha256(&source),
        rebound
    );
}
