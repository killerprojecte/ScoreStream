use scorestream_app::playback::plan_source;
use scorestream_audio::{
    BankVerification, RenderOptions, RouteAssignment, assignments::source_sha256, midi_mixer::*,
    validate_plan,
};
fn base() -> String {
    "!score 1.1 unit=q\n!part p instrument=flute staves=1\n!part q instrument=flute staves=1\n!voice v part=p staff=1\n!voice w part=q staff=1\n@0\n| p m len=8\n| q n len=8\nv C4:1 id=a\n.\n@4\nw C4:1 id=b\n.\n!end at=8\n".into()
}
fn source(a: &str, b: &str) -> String {
    base().replace("@0\n",&format!("!requires scorestream.instrument@1\n@0\n+ p scorestream.instrument channel=1 device=\"\" device_ref=\"i\" midi_ref=\"i\" name=\"Flute\" pan=0 port=1 program=74 source_id=\"i\" volume={a}\n+ q scorestream.instrument channel=1 device=\"\" device_ref=\"j\" midi_ref=\"j\" name=\"Flute\" pan=0 port=2 program=74 source_id=\"j\" volume={b}\n"))
}
fn routes() -> Vec<RouteAssignment> {
    ["p", "q"]
        .into_iter()
        .map(|p| RouteAssignment {
            part: p.into(),
            instrument: "flute".into(),
            bank: 0,
            program: 73,
        })
        .collect()
}
fn prepare(s: &str) -> LocalMidiMix {
    prepare_local_midi_mix(
        s,
        &source_sha256(s),
        &routes(),
        LocalMidiPolicy::InitialGmCc7CenteredV1,
    )
    .unwrap()
}
#[test]
fn exact_percent_rounding_and_external_addresses_remain_separate() {
    let s = source("393701/5000", "50");
    let mix = prepare(&s);
    mix.verify_source(&s).unwrap();
    assert!(mix.verify_source(&(s.clone() + "\n")).is_err());
    let a = &mix.routes()[0];
    assert_eq!(a.native_channel, 0);
    assert_eq!(
        a.authored_metadata
            .midi_instrument
            .as_ref()
            .unwrap()
            .channel,
        Some(1)
    );
    assert_eq!(
        a.authored_metadata.midi_device.as_ref().unwrap().port,
        Some(1)
    );
    let v = a.volume.as_ref().unwrap();
    assert_eq!(v.cc7, 100);
    assert_eq!(v.quantization_error_percent.to_string(), "-27/635000");
    assert_eq!(a.cc10, Some(64));
    let b = &mix.routes()[1];
    assert_eq!(b.native_channel, 1);
    assert_eq!(
        b.authored_metadata.midi_device.as_ref().unwrap().port,
        Some(2)
    );
    assert_eq!(b.volume.as_ref().unwrap().cc7, 64); // exact half-step ties upward
    assert!(
        plan_source(&s).is_err(),
        "Mixer qualification never relaxes core playback"
    );
    let absent = prepare(
        &s.replace(" port=2", "")
            .replace(" pan=0", "")
            .replace(" volume=50", ""),
    );
    assert_eq!(
        absent.routes()[1]
            .authored_metadata
            .midi_device
            .as_ref()
            .unwrap()
            .port,
        None
    );
    assert_eq!(absent.routes()[1].cc10, None);
    assert!(absent.routes()[1].volume.is_none());
}
#[test]
fn mismatches_noncenter_named_device_stale_hash_and_missing_bank_assertion_fail() {
    let s = source("100", "0");
    for bad in [
        s.replace("pan=0", "pan=1"),
        s.replace("device=\"\"", "device=\"External\""),
        s.replace("program=74", "program=73"),
    ] {
        assert!(
            prepare_local_midi_mix(
                &bad,
                &source_sha256(&bad),
                &routes(),
                LocalMidiPolicy::InitialGmCc7CenteredV1
            )
            .is_err()
        );
    }
    assert!(
        prepare_local_midi_mix(
            &s,
            &"0".repeat(64),
            &routes(),
            LocalMidiPolicy::InitialGmCc7CenteredV1
        )
        .is_err()
    );
    let mut other = routes();
    other[0].bank = 1;
    assert!(
        prepare_local_midi_mix(
            &s,
            &source_sha256(&s),
            &other,
            LocalMidiPolicy::InitialGmCc7CenteredV1
        )
        .is_err()
    );
    let mut options = RenderOptions {
        local_midi_mix: Some(prepare(&s)),
        ..Default::default()
    };
    let plan = plan_source(&base()).unwrap();
    assert!(validate_plan(&plan, &routes(), &options).is_err());
    options.bank_verification = Some(BankVerification {
        sha256: "0".repeat(64),
        preset_names: vec!["Flute TB".into(); 2],
    });
    validate_plan(&plan, &routes(), &options).unwrap();
    let mut changed = routes();
    changed.swap(0, 1);
    assert!(validate_plan(&plan, &changed, &options).is_err());
}
#[test]
#[ignore = "read-only exact preserved movement I source; no playback qualification or rendering"]
fn actual_movement_i_all_18_initial_routes_are_exact() {
    let s = std::fs::read_to_string(std::env::var("SCORESTREAM_I_NATIVE_SOURCE").unwrap()).unwrap();
    assert_eq!(
        source_sha256(&s),
        "ffac34c86e31df50b028ca7afe0e91f822c2b122af28b55c5e0c1ba10769f77b"
    );
    let instruments = [
        ("flute", 73),
        ("flute", 73),
        ("oboe", 68),
        ("oboe", 68),
        ("clarinet", 71),
        ("clarinet", 71),
        ("bassoon", 70),
        ("bassoon", 70),
        ("horn", 60),
        ("horn", 60),
        ("trumpet", 56),
        ("trumpet", 56),
        ("timpani", 47),
        ("violin", 40),
        ("violin", 40),
        ("viola", 41),
        ("cello", 42),
        ("double_bass", 43),
    ];
    let routes: Vec<_> = instruments
        .iter()
        .enumerate()
        .map(|(i, (instrument, program))| RouteAssignment {
            part: format!("P{}", i + 1),
            instrument: (*instrument).into(),
            bank: 0,
            program: *program,
        })
        .collect();
    let mix = prepare_local_midi_mix(
        &s,
        &source_sha256(&s),
        &routes,
        LocalMidiPolicy::InitialGmCc7CenteredV1,
    )
    .unwrap();
    assert_eq!(mix.routes().len(), 18);
    for (i, r) in mix.routes().iter().enumerate() {
        assert_eq!(r.native_channel, i as u16);
        assert_eq!(r.volume.as_ref().unwrap().cc7, 100);
        assert_eq!(r.cc10, Some(64));
        assert_eq!(
            r.volume
                .as_ref()
                .unwrap()
                .quantization_error_percent
                .to_string(),
            "-27/635000"
        );
    }
    assert!(
        mix.routes()[10]
            .authored_metadata
            .midi_device
            .as_ref()
            .unwrap()
            .port
            .is_none()
    );
    if let Ok(path) = std::env::var("SCORESTREAM_MIXER_RECEIPT") {
        std::fs::write(path, serde_json::to_vec_pretty(&mix).unwrap()).unwrap();
    }
}
#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires installed FluidSynth/TimGM6mb; run test binary directly with no LD_* overrides"]
fn native_routes_set_readback_and_apply_distinct_volume_with_centered_pan() {
    use scorestream_audio::FluidSynthBackend;
    use std::sync::atomic::AtomicBool;
    let s = source("100", "0");
    let mix = prepare(&s);
    // Synthetic notes deliberately compiled independently. This probes the native mixer,
    // not a bypass for authored source performance or an actual-score render.
    let plan = plan_source(&base()).unwrap();
    let options = RenderOptions {
        sample_rate: 24000,
        tail_seconds: 0.2,
        local_midi_mix: Some(mix),
        bank_verification: Some(BankVerification {
            sha256: "c5378b62028c920cb11e4803327983fee2f2cdff5dc89c708e39da417e51c854".into(),
            preset_names: vec!["Flute TB".into(); 2],
        }),
        ..Default::default()
    };
    let backend = FluidSynthBackend::open_installed().unwrap();
    let mut pcm = Vec::new();
    let report = backend
        .render_pcm(
            &plan,
            &routes(),
            std::path::Path::new("/usr/share/sounds/sf2/TimGM6mb.sf2"),
            &options,
            &AtomicBool::new(false),
            &mut |_, block: &[f32]| {
                pcm.extend_from_slice(block);
                Ok(())
            },
        )
        .unwrap();
    assert_eq!(report.routes[0].native_channel, 0);
    assert_eq!(report.routes[1].native_channel, 1);
    assert!(report.local_midi_mix.is_some());
    let energy = |start: usize, end: usize, channel: usize| -> f64 {
        (start..end)
            .map(|i| f64::from(pcm[2 * i + channel]).powi(2))
            .sum()
    };
    let left = energy(2400, 9600, 0);
    let right = energy(2400, 9600, 1);
    assert!(left > 0.00001 && right > 0.00001);
    assert!(
        (left / right - 1.0).abs() < 0.05,
        "centered mono Flute preset: {left}/{right}"
    );
    let muted = energy(50400, 57600, 0) + energy(50400, 57600, 1);
    assert!(
        muted < (left + right) * 0.001,
        "Independent zero-volume second route was not muted: {muted}"
    );
}
#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires exact I source and installed TimGM6mb; synthetic C4 per-route probe, never actual-score audio"]
fn native_all_18_source_selected_presets_and_controllers_are_verified() {
    use scorestream_audio::FluidSynthBackend;
    use std::sync::atomic::AtomicBool;
    let source =
        std::fs::read_to_string(std::env::var("SCORESTREAM_I_NATIVE_SOURCE").unwrap()).unwrap();
    assert_eq!(
        source_sha256(&source),
        "ffac34c86e31df50b028ca7afe0e91f822c2b122af28b55c5e0c1ba10769f77b"
    );
    let palette = [
        ("flute", 73, "Flute TB"),
        ("flute", 73, "Flute TB"),
        ("oboe", 68, "Oboe (Orch)"),
        ("oboe", 68, "Oboe (Orch)"),
        ("clarinet", 71, "Clarinet"),
        ("clarinet", 71, "Clarinet"),
        ("bassoon", 70, "Bassoon"),
        ("bassoon", 70, "Bassoon"),
        ("horn", 60, "French Horns"),
        ("horn", 60, "French Horns"),
        ("trumpet", 56, "SoloTrumpet"),
        ("trumpet", 56, "SoloTrumpet"),
        ("timpani", 47, "Timpani"),
        ("violin", 40, "Violin"),
        ("violin", 40, "Violin"),
        ("viola", 41, "Viola"),
        ("cello", 42, "Cello"),
        ("double_bass", 43, "Contrabass"),
    ];
    let routes: Vec<_> = palette
        .iter()
        .enumerate()
        .map(|(i, (instrument, program, _))| RouteAssignment {
            part: format!("P{}", i + 1),
            instrument: (*instrument).into(),
            bank: 0,
            program: *program,
        })
        .collect();
    let mix = prepare_local_midi_mix(
        &source,
        &source_sha256(&source),
        &routes,
        LocalMidiPolicy::InitialGmCc7CenteredV1,
    )
    .unwrap();
    let mut synthetic = String::from("!score 1.1 unit=q\n");
    for (i, r) in routes.iter().enumerate() {
        synthetic.push_str(&format!(
            "!part {} instrument={} staves=1\n!voice v{i} part={} staff=1\n",
            r.part, r.instrument, r.part
        ));
    }
    synthetic.push_str("@0\n");
    for (i, r) in routes.iter().enumerate() {
        synthetic.push_str(&format!("| {} m{i} len=1\nv{i} C4:1 id=e{i}\n", r.part));
    }
    synthetic.push_str(".\n!end at=1\n");
    let plan = plan_source(&synthetic).unwrap();
    let options = RenderOptions {
        gain: 0.005,
        sample_rate: 24000,
        tail_seconds: 0.2,
        local_midi_mix: Some(mix),
        bank_verification: Some(BankVerification {
            sha256: "c5378b62028c920cb11e4803327983fee2f2cdff5dc89c708e39da417e51c854".into(),
            preset_names: palette.iter().map(|p| p.2.to_string()).collect(),
        }),
        ..Default::default()
    };
    let report = FluidSynthBackend::open_installed()
        .unwrap()
        .render_pcm(
            &plan,
            &routes,
            std::path::Path::new("/usr/share/sounds/sf2/TimGM6mb.sf2"),
            &options,
            &AtomicBool::new(false),
            &mut |_, _: &[f32]| Ok(()),
        )
        .unwrap();
    assert_eq!(report.routes.len(), 18);
    assert!(report.peak > 0.0);
    for (i, r) in report.routes.iter().enumerate() {
        assert_eq!(r.native_channel, i as u16);
        assert_eq!(r.preset_name, palette[i].2);
    }
    if let Ok(path) = std::env::var("SCORESTREAM_MIXER_NATIVE_REPORT") {
        std::fs::write(path,serde_json::to_vec_pretty(&serde_json::json!({"scope":"Synthetic C4 simultaneous route/preset/controller probe only; NOT actual I audio or sound coverage certification","report":report})).unwrap()).unwrap();
    }
}
