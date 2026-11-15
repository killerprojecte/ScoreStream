use scorestream_audio::{RenderOptions, assignments::*};
use scorestream_codec::{
    Record, instrument_extension,
    instrument_metadata::{InstrumentMetadata, MidiDevice, MidiInstrument},
};

fn source() -> String {
    "!score 1.1 unit=q\n!part p instrument=clarinet staves=1\n!voice v part=p staff=1\n@0\n| p m len=4\nv C4:1 id=a\n.\n@1\nv D4:1 id=b\n.\n!end at=4\n".into()
}
fn profile() -> LocalSampleProfile {
    LocalSampleProfile {
        id: "fixture-profile".into(),
        bank_path: std::env::temp_dir().join("not-opened-by-preflight.sf2"),
        bank_sha256: "a".repeat(64),
        license: "synthetic fixture only".into(),
        provenance_uri: "https://example.invalid/synthetic-fixture".into(),
        coverage_evidence_sha256: "b".repeat(64),
        patches: vec![SamplePatch {
            id: "patch".into(),
            instrument: "clarinet".into(),
            bank: 0,
            program: 71,
            exact_preset_name: "Fixture Clarinet".into(),
            regions: vec![SampleCoverage {
                key_low: 60,
                key_high: 62,
                velocity_low: 1,
                velocity_high: 127,
                max_note_seconds: 2.0,
            }],
        }],
    }
}
fn selection() -> Vec<PartSampleSelection> {
    vec![PartSampleSelection {
        part: "p".into(),
        expected_instrument: "clarinet".into(),
        expected_source_instrument_id: None,
        patch_id: "patch".into(),
    }]
}
fn check(
    source: &str,
    profile: &LocalSampleProfile,
    selection: &[PartSampleSelection],
) -> AssignmentPreflight {
    preflight_source(
        source,
        &source_sha256(source),
        profile,
        selection,
        &RenderOptions::default(),
    )
    .unwrap()
}
fn has(value: &AssignmentPreflight, code: &str) -> bool {
    value.report().diagnostics.iter().any(|d| d.code == code)
}
fn with_metadata(value: &InstrumentMetadata) -> String {
    let mut doc = scorestream_codec::parse(&source()).unwrap();
    doc.required_extensions
        .push(instrument_extension::EXTENSION.into());
    doc.frames[0]
        .records
        .push(Record::Mark(instrument_extension::encode(
            "p".into(),
            value,
        )));
    scorestream_codec::canonical(&doc)
}
fn metadata() -> InstrumentMetadata {
    InstrumentMetadata {
        source_id: "P1-I1".into(),
        name: "Source clarinet".into(),
        abbreviation: Some("Cl.".into()),
        sound: Some("wind.reed.clarinet".into()),
        midi_device: None,
        midi_instrument: None,
    }
}

#[test]
fn source_is_exactly_bound_and_preflight_never_requires_a_local_bank() {
    let source = source();
    let checked = check(&source, &profile(), &selection());
    assert!(checked.report().contract_renderable);
    assert!(!checked.report().bank_bytes_and_audible_presets_verified);
    assert_eq!(checked.report().exact_sfs_sha256, source_sha256(&source));
    assert_eq!(checked.report().parts[0].audible_notes_checked, 2);
    assert_eq!(checked.report().parts[0].program_zero_based, Some(71));
    assert_eq!(
        checked.into_qualified().unwrap().report().exact_sfs_sha256,
        source_sha256(&source)
    );
    assert!(
        preflight_source(
            &(source.clone() + "\n"),
            &source_sha256(&source),
            &profile(),
            &selection(),
            &RenderOptions::default()
        )
        .is_err()
    );
    let mut changed_profile = profile();
    changed_profile.patches[0].program = 72;
    assert_ne!(
        check(&source, &profile(), &selection())
            .report()
            .profile_sha256,
        check(&source, &changed_profile, &selection())
            .report()
            .profile_sha256
    );
}

#[test]
fn exact_authored_identity_is_retained_without_midi_defaults() {
    let value = metadata();
    let source = with_metadata(&value);
    let mut selections = selection();
    selections[0].expected_source_instrument_id = Some(value.source_id.clone());
    let checked = check(&source, &profile(), &selections);
    assert_eq!(checked.report().parts[0].authored_metadata, Some(value));
    assert!(checked.report().contract_renderable);
    assert!(checked.into_qualified().is_ok());
    assert!(has(
        &check(&source, &profile(), &selection()),
        "selection.source_identity_mismatch"
    ));
}

#[test]
fn exact_midi_mixers_and_empty_device_are_reported_not_overridden() {
    let mut value = metadata();
    value.midi_device = Some(MidiDevice {
        name: "".into(),
        instrument_ref: None,
        port: None,
    });
    value.midi_instrument = Some(MidiInstrument {
        instrument_ref: value.source_id.clone(),
        channel: Some(2),
        program: Some(72),
        volume: Some("1/3".parse().unwrap()),
        pan: Some("-179/2".parse().unwrap()),
    });
    let source = with_metadata(&value);
    let before = source.clone();
    let mut selections = selection();
    selections[0].expected_source_instrument_id = Some(value.source_id.clone());
    let checked = check(&source, &profile(), &selections);
    assert_eq!(checked.report().parts[0].authored_metadata, Some(value));
    assert_eq!(checked.report().parts[0].audible_notes_checked, 0);
    for code in [
        "source.midi_device_unrealized",
        "source.midi_assignment_unrealized",
        "source.volume_unrealized",
        "source.pan_unrealized",
        "sample.coverage_unassessed",
    ] {
        assert!(has(&checked, code), "{code}");
    }
    assert!(
        checked
            .report()
            .diagnostics
            .iter()
            .any(|d| d.message.contains("instrument.performance_unsupported"))
    );
    assert!(checked.into_qualified().is_err());
    assert_eq!(source, before);
    assert!(scorestream_app::playback::plan_source(&source).is_err());
}

#[test]
fn empty_midi_assignment_does_not_become_an_absent_record() {
    let mut value = metadata();
    value.midi_instrument = Some(MidiInstrument {
        instrument_ref: value.source_id.clone(),
        channel: None,
        program: None,
        volume: None,
        pan: None,
    });
    let mut selections = selection();
    selections[0].expected_source_instrument_id = Some(value.source_id.clone());
    let checked = check(&with_metadata(&value), &profile(), &selections);
    assert!(has(&checked, "source.midi_assignment_unrealized"));
    assert!(!has(&checked, "source.volume_unrealized"));
    assert!(!has(&checked, "source.pan_unrealized"));
    assert_eq!(checked.report().parts[0].authored_metadata, Some(value));
}

#[test]
fn unassigned_stale_and_wrong_instrument_patches_never_substitute() {
    let source = source();
    assert!(has(
        &check(&source, &profile(), &[]),
        "selection.part_unmapped"
    ));
    let mut selections = selection();
    selections[0].expected_instrument = "bassoon".into();
    assert!(has(
        &check(&source, &profile(), &selections),
        "selection.source_identity_mismatch"
    ));
    selections = selection();
    selections[0].patch_id = "missing".into();
    assert!(has(
        &check(&source, &profile(), &selections),
        "selection.patch_unmapped"
    ));
    let mut profile = profile();
    profile.patches[0].instrument = "bassoon".into();
    assert!(has(
        &check(&source, &profile, &selection()),
        "selection.patch_unmapped"
    ));
    let mut selections = selection();
    selections.push(selections[0].clone());
    assert!(
        preflight_source(
            &source,
            &source_sha256(&source),
            &profile,
            &selections,
            &RenderOptions::default()
        )
        .is_err()
    );
    selections = selection();
    selections[0].part = "other".into();
    assert!(
        preflight_source(
            &source,
            &source_sha256(&source),
            &profile,
            &selections,
            &RenderOptions::default()
        )
        .is_err()
    );
}

#[test]
fn uncovered_actual_compiler_key_velocity_or_hold_blocks_with_note_identity() {
    for case in 0..3 {
        let mut profile = profile();
        let region = &mut profile.patches[0].regions[0];
        match case {
            0 => region.key_high = 60,
            1 => region.velocity_high = 79,
            _ => region.max_note_seconds = 0.1,
        }
        let checked = check(&source(), &profile, &selection());
        assert!(has(&checked, "sample.coverage_unmapped"));
        assert!(
            checked
                .report()
                .diagnostics
                .iter()
                .any(|d| d.code == "sample.coverage_unmapped"
                    && d.part.as_deref() == Some("p")
                    && d.note.is_some())
        );
        assert!(checked.into_qualified().is_err());
    }
}

#[test]
fn independent_expression_is_rejected_not_quantized_into_cc() {
    let source = source().replace("v C4:1 id=a", "v C4:1 id=a\n+ #a dynamic value=fp");
    let checked = check(&source, &profile(), &selection());
    assert!(has(&checked, "performance.envelope_unrealized"));
    assert!(has(&checked, "backend.contract_unsupported"));
    assert!(checked.into_qualified().is_err());
}

#[test]
fn silent_parts_are_traced_but_not_allocated_as_native_routes() {
    let source = source().replace(
        "!voice v",
        "!part silent instrument=clarinet staves=1\n!voice v",
    );
    let mut selections = selection();
    let mut silent = selections[0].clone();
    silent.part = "silent".into();
    selections.push(silent);
    let checked = check(&source, &profile(), &selections);
    assert!(checked.report().contract_renderable);
    assert_eq!(checked.report().parts[1].audible_notes_checked, 0);
    assert!(checked.into_qualified().is_ok());
}

#[test]
fn invalid_or_unbounded_profile_is_rejected_before_native_work() {
    let source = source();
    for case in 0..5 {
        let mut profile = profile();
        match case {
            0 => profile.bank_sha256 = "bad".into(),
            1 => profile.bank_path = "relative.sf2".into(),
            2 => profile.patches[0].regions[0].max_note_seconds = f64::INFINITY,
            3 => profile.patches[0].regions[0].velocity_low = 0,
            _ => profile.patches.push(profile.patches[0].clone()),
        }
        assert!(
            preflight_source(
                &source,
                &source_sha256(&source),
                &profile,
                &selection(),
                &RenderOptions::default()
            )
            .is_err()
        );
    }
}

#[test]
fn omitted_grace_stays_structured_and_blocks_source_qualification() {
    let source = source().replace(
        "v C4:1 id=a",
        "v E4:? id=grace kind=grace anchor=#a order=-1\nv C4:1 id=a",
    );
    let legacy = scorestream_app::playback::plan_source(&source).unwrap();
    assert_eq!(legacy.notes.len(), 2); // Legacy compiler behavior is unchanged.
    let checked = check(&source, &profile(), &selection());
    assert_eq!(checked.report().compiler_diagnostics, legacy.diagnostics);
    assert_eq!(checked.report().compiler_warnings, legacy.warnings);
    let omission = checked
        .report()
        .compiler_diagnostics
        .iter()
        .find(|d| d.code == "grace_unrealized")
        .unwrap();
    assert_eq!(omission.object_id, "grace");
    assert_eq!(
        omission.effect,
        scorestream_app::playback::RealizationEffect::Omitted
    );
    assert!(has(&checked, "source.realization_unaccepted"));
    assert!(checked.into_qualified().is_err());
}

#[test]
fn approximate_partial_pedal_and_unresolved_words_block_but_stay_visible() {
    let pedal = source()
        .replace(
            "v C4:1 id=a",
            "v C4:1 id=a\n~ pedal sustain start=#a value=1/4",
        )
        .replace("v D4:1 id=b", "v D4:1 id=b\n~ pedal sustain end=#b value=0");
    let words = source().replace(
        "v C4:1 id=a",
        "v C4:1 id=a\n+ #a words text=\"dolce\" id=word",
    );
    for (source, code) in [(pedal, "pedal_partial"), (words, "words_not_realized")] {
        let checked = check(&source, &profile(), &selection());
        assert!(
            checked
                .report()
                .compiler_diagnostics
                .iter()
                .any(|d| d.code == code)
        );
        assert!(has(&checked, "source.realization_unaccepted"));
        assert!(checked.into_qualified().is_err());
    }
}

#[test]
fn display_only_dynamics_remain_visible_without_inventing_sound() {
    let source = source().replace(
        "v C4:1 id=a",
        "v C4:1 id=a\n+ p dynamic value=p sound=false",
    );
    let checked = check(&source, &profile(), &selection());
    assert!(
        checked
            .report()
            .compiler_diagnostics
            .iter()
            .any(|d| d.code == "dynamic_visual_only")
    );
    assert!(checked.report().contract_renderable);
    assert!(checked.into_qualified().is_ok());
}
