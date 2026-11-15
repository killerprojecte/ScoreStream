//! Explicit opt-in integration tests. Build with Cargo, then execute the test
//! binary directly: Cargo itself injects LD_LIBRARY_PATH, which the native
//! loader intentionally refuses. See README.md; never relax the loader check.
#![cfg(target_os = "linux")]
use scorestream_app::playback::{PlaybackPlan, PlaybackRoute, plan_source};
use scorestream_audio::{
    AudioError, FluidSynthBackend, RenderOptions, RouteAssignment, default_soundfont,
};
use std::{
    fs,
    sync::atomic::{AtomicBool, Ordering},
};

fn fixture() -> (PlaybackPlan, Vec<RouteAssignment>) {
    let plan = plan_source("!score 1.1 unit=q\n!part p instrument=clarinet staves=1\n!voice v part=p staff=1\n@0\n| p m len=4\nv r:1 id=rest\n.\n@1\nv C4:1 id=a\n.\n@2\nv D4:1 id=b\n.\n!end at=4\n").unwrap();
    let routes = vec![RouteAssignment {
        part: "p".into(),
        instrument: "clarinet".into(),
        bank: 0,
        program: 71,
    }];
    (plan, routes)
}

#[test]
#[ignore = "requires an already-installed system FluidSynth and local GM bank"]
fn real_rust_compile_streams_wav_with_exact_header_and_leading_rest() {
    let backend = FluidSynthBackend::open_installed().unwrap();
    assert!(backend.capabilities().offline_pcm);
    assert!(!backend.capabilities().realtime_audio && !backend.capabilities().vst3);
    let (plan, routes) = fixture();
    let bank = default_soundfont().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("compiled.wav");
    let report = backend
        .render_wav(
            &plan,
            &routes,
            &bank,
            &RenderOptions::default(),
            &AtomicBool::new(false),
            &output,
        )
        .unwrap();
    assert!(report.peak > 0.001 && report.peak < 1.0 && report.rms > 0.0001);
    let wav = fs::read(output).unwrap();
    assert_eq!(&wav[..4], b"RIFF");
    assert_eq!(&wav[8..12], b"WAVE");
    assert_eq!(wav.len() as u64, 44 + report.frames * 4);
    assert_eq!(
        u32::from_le_bytes(wav[40..44].try_into().unwrap()) as u64,
        report.frames * 4
    );
    assert!(wav[44..44 + 24_000 * 4].iter().all(|byte| *byte == 0));
    assert!(
        report.peak_native_voices > 0 && report.peak_native_voices < report.native_polyphony_limit
    );
}

#[test]
#[ignore = "requires an already-installed system FluidSynth and local GM bank"]
fn native_blocks_are_invariant_and_do_not_add_movement_seam_silence() {
    let backend = FluidSynthBackend::open_installed().unwrap();
    let (mut plan, routes) = fixture();
    let bank = default_soundfont().unwrap();
    // A new attack follows an exact previous release. Backend must not insert a
    // movement separator; musical attacca/timeline ownership stays upstream.
    // Deliberately use boundaries which are not multiples of FluidSynth's usual
    // 64-frame internal quantum at 48 kHz.
    plan.notes[0].start_seconds = 0.50013;
    assert!(
        plan.notes[0]
            .gain_envelope
            .iter()
            .all(|point| point.gain == 1.0)
    );
    plan.notes[0].gain_envelope.clear(); // Synthetic timing test, constant gain.
    plan.notes[0].duration_seconds = 0.30017;
    plan.notes[1].start_seconds = plan.notes[0].start_seconds + plan.notes[0].duration_seconds;
    let mut outputs = Vec::new();
    for block_frames in [1, 64, 257, 4096] {
        let options = RenderOptions {
            block_frames,
            ..RenderOptions::default()
        };
        let mut samples = Vec::new();
        let mut expected = 0;
        let report = backend
            .render_pcm(
                &plan,
                &routes,
                &bank,
                &options,
                &AtomicBool::new(false),
                &mut |start, block: &[f32]| {
                    assert_eq!(start, expected);
                    assert!(block.len() <= block_frames * 2);
                    expected += (block.len() / 2) as u64;
                    samples.extend_from_slice(block);
                    Ok(())
                },
            )
            .unwrap();
        assert_eq!(expected, report.frames);
        let event_frame = (plan.notes[0].start_seconds * 48_000.0 + 0.5).floor() as usize;
        let first_nonzero = samples
            .chunks_exact(2)
            .position(|pair| pair[0] != 0.0 || pair[1] != 0.0)
            .unwrap();
        assert!(first_nonzero >= event_frame);
        // This installed Clarinet fixture has no initial sample silence. The
        // tolerance is the reported engine quantum, not a sample-accuracy claim.
        assert!(first_nonzero - event_frame <= report.internal_quantum_frames as usize);
        outputs.push(samples);
    }
    for output in &outputs[1..] {
        assert_eq!(&outputs[0], output);
    }
}

#[test]
#[ignore = "requires an already-installed system FluidSynth and local GM bank"]
fn supports_more_than_sixteen_explicit_routes_without_drum_channel_substitution() {
    let backend = FluidSynthBackend::open_installed().unwrap();
    let (mut plan, _) = fixture();
    let note = plan.notes[0].clone();
    plan.notes.clear();
    plan.routes.clear();
    let mut routes = Vec::new();
    for index in 0..23 {
        let part = format!("p{index}");
        let mut current = note.clone();
        current.id = format!("n{index}");
        current.part = part.clone();
        plan.notes.push(current);
        plan.routes.push(PlaybackRoute {
            part: part.clone(),
            instrument: "clarinet".into(),
        });
        routes.push(RouteAssignment {
            part,
            instrument: "clarinet".into(),
            bank: 0,
            program: 71,
        });
    }
    let report = backend
        .render_pcm(
            &plan,
            &routes,
            &default_soundfont().unwrap(),
            &RenderOptions {
                gain: 0.005,
                ..RenderOptions::default()
            },
            &AtomicBool::new(false),
            &mut |_, _: &[f32]| Ok(()),
        )
        .unwrap();
    assert_eq!(report.routes.len(), 23);
    assert!(
        report
            .routes
            .iter()
            .all(|route| route.preset_name == report.routes[0].preset_name)
    );
    assert_eq!(report.routes[22].native_channel, 22);
}

#[test]
#[ignore = "requires an already-installed system FluidSynth and local GM bank"]
fn cancellation_sink_failure_missing_patch_and_overwrite_leave_no_partial_file() {
    let backend = FluidSynthBackend::open_installed().unwrap();
    let (plan, mut routes) = fixture();
    let bank = default_soundfont().unwrap();
    let options = RenderOptions::default();
    let cancel = AtomicBool::new(false);
    let mut writes = 0;
    let error = backend
        .render_pcm(
            &plan,
            &routes,
            &bank,
            &options,
            &cancel,
            &mut |_, _: &[f32]| {
                writes += 1;
                cancel.store(true, Ordering::Relaxed);
                Ok(())
            },
        )
        .unwrap_err();
    assert!(matches!(error, AudioError::Cancelled));
    assert_eq!(writes, 1);
    cancel.store(false, Ordering::Relaxed);
    let error = backend
        .render_pcm(
            &plan,
            &routes,
            &bank,
            &options,
            &cancel,
            &mut |_, _: &[f32]| {
                Err(AudioError::Io(std::io::Error::other(
                    "injected sink failure",
                )))
            },
        )
        .unwrap_err();
    assert!(matches!(error, AudioError::Io(_)));
    let dir = tempfile::tempdir().unwrap();
    let output = dir.path().join("failed.wav");
    routes[0].bank = 16383;
    assert!(
        backend
            .render_wav(&plan, &routes, &bank, &options, &cancel, &output)
            .is_err()
    );
    assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
    routes[0].bank = 0;
    fs::write(&output, b"preserve").unwrap();
    assert!(
        backend
            .render_wav(&plan, &routes, &bank, &options, &cancel, &output)
            .is_err()
    );
    assert_eq!(fs::read(&output).unwrap(), b"preserve");
    let recovered = dir.path().join("recovered.wav");
    backend
        .render_wav(&plan, &routes, &bank, &options, &cancel, &recovered)
        .unwrap();
}

#[test]
#[ignore = "requires an already-installed system FluidSynth and local GM bank"]
fn budget_saturation_fails_instead_of_claiming_lossless_voice_count() {
    let backend = FluidSynthBackend::open_installed().unwrap();
    let (plan, routes) = fixture();
    let options = RenderOptions {
        polyphony: 1,
        ..RenderOptions::default()
    };
    let error = backend
        .render_pcm(
            &plan,
            &routes,
            &default_soundfont().unwrap(),
            &options,
            &AtomicBool::new(false),
            &mut |_, _: &[f32]| Ok(()),
        )
        .unwrap_err();
    assert!(matches!(error, AudioError::PolyphonyLimit { .. }));
}

#[test]
#[ignore = "requires an already-installed system FluidSynth and local GM bank"]
fn streams_more_than_two_minutes_and_four_thousand_notes_with_bounded_pcm() {
    let backend = FluidSynthBackend::open_installed().unwrap();
    let (mut plan, routes) = fixture();
    let template = plan.notes[0].clone();
    plan.notes = (0..5_000)
        .map(|index| {
            let mut note = template.clone();
            note.id = format!("long-{index}");
            note.start_seconds = f64::from(index) * 0.03;
            note.duration_seconds = 0.02;
            assert!(note.gain_envelope.iter().all(|point| point.gain == 1.0));
            note.gain_envelope.clear(); // Synthetic resource test, constant gain.
            note
        })
        .collect();
    plan.duration_seconds = 151.0;
    let options = RenderOptions {
        sample_rate: 8_000,
        block_frames: 257,
        tail_seconds: 0.5,
        gain: 0.005,
        ..RenderOptions::default()
    };
    let mut frames = 0;
    let mut largest_block = 0;
    let report = backend
        .render_pcm(
            &plan,
            &routes,
            &default_soundfont().unwrap(),
            &options,
            &AtomicBool::new(false),
            &mut |start, block: &[f32]| {
                assert_eq!(start, frames);
                frames += (block.len() / 2) as u64;
                largest_block = largest_block.max(block.len());
                // Deliberately discard rather than accumulating the output.
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(report.note_count, 5_000);
    assert_eq!(report.score_frames, 151 * 8_000);
    assert_eq!(report.frames, 151 * 8_000 + 4_000);
    assert_eq!(frames, report.frames);
    assert!(largest_block <= 257 * 2);
    assert!(report.peak > 0.0 && report.rms > 0.0);
}

#[test]
#[ignore = "requires an already-installed system FluidSynth and local GM bank"]
fn sequential_backend_open_render_drop_keeps_native_registries_valid() {
    let (plan, routes) = fixture();
    let bank = default_soundfont().unwrap();
    for _ in 0..12 {
        let backend = FluidSynthBackend::open_installed().unwrap();
        let report = backend
            .render_pcm(
                &plan,
                &routes,
                &bank,
                &RenderOptions::default(),
                &AtomicBool::new(false),
                &mut |_, _: &[f32]| Ok(()),
            )
            .unwrap();
        assert!(report.peak > 0.0);
        // Both settings/synth objects are destroyed each time. Only native code
        // and dependency type registries intentionally remain resident.
        drop(backend);
    }
}
