use scorestream_vst3_catalog::*;
use std::{
    fs,
    path::{Path, PathBuf},
};

const CID: &str = "00112233445566778899AABBCCDDEEFF";
fn manifest(name: &str) -> String {
    format!(
        r#"{{ // Deliberately JSON5, not plain JSON.
      Name: '{name}', Version: '1.0',
      'Factory Info': {{ Vendor: 'Synthetic fixture', Flags: {{ Unicode: true, }}, }},
      Classes: [{{ CID: '{CID}', Name: 'Test voice', Category: 'Audio Module Class',
        'Sub Categories': ['Instrument', 'Sampler',], 'Class Flags': 0x1,
        Cardinality: 2147483647, SDKVersion: 'VST 3.8',
        Snapshots: [{{ Path: '../../never-open-this-file', 'Scale Factor': 1.0, }}],
      }},],
      Compatibility: [{{ New: '{CID}', Old: ['FFEEDDCCBBAA99887766554433221100',], }}],
    }}"#
    )
}
fn elf(machine: u16, little: bool, bits: u8) -> Vec<u8> {
    let mut b = vec![0; if bits == 64 { 64 } else { 52 }];
    b[..4].copy_from_slice(b"\x7fELF");
    b[4] = if bits == 64 { 2 } else { 1 };
    b[5] = if little { 1 } else { 2 };
    b[6] = 1;
    b[16..18].copy_from_slice(&if little {
        3u16.to_le_bytes()
    } else {
        3u16.to_be_bytes()
    });
    b[18..20].copy_from_slice(&if little {
        machine.to_le_bytes()
    } else {
        machine.to_be_bytes()
    });
    b[20..24].copy_from_slice(&if little {
        1u32.to_le_bytes()
    } else {
        1u32.to_be_bytes()
    });
    b
}
fn pe(machine: u16) -> Vec<u8> {
    let mut b = vec![0; 128];
    b[..2].copy_from_slice(b"MZ");
    b[0x3c..0x40].copy_from_slice(&64u32.to_le_bytes());
    b[64..68].copy_from_slice(b"PE\0\0");
    b[68..70].copy_from_slice(&machine.to_le_bytes());
    b[84..86].copy_from_slice(&40u16.to_le_bytes());
    b[86..88].copy_from_slice(&0x2000u16.to_le_bytes());
    b[88..90].copy_from_slice(&0x20bu16.to_le_bytes());
    b
}
fn mach(cpu: u32) -> Vec<u8> {
    let mut b = vec![0; 32];
    b[..4].copy_from_slice(&0xfeedfacfu32.to_le_bytes());
    b[4..8].copy_from_slice(&cpu.to_le_bytes());
    b[12..16].copy_from_slice(&8u32.to_le_bytes());
    b
}
fn universal() -> Vec<u8> {
    let mut b = vec![0; 128];
    b[..4].copy_from_slice(&0xcafebabeu32.to_be_bytes());
    b[4..8].copy_from_slice(&2u32.to_be_bytes());
    for (i, cpu) in [0x01000007u32, 0x0100000c].into_iter().enumerate() {
        let at = 8 + i * 20;
        b[at..at + 4].copy_from_slice(&cpu.to_be_bytes());
        b[at + 8..at + 12].copy_from_slice(&(64u32 + i as u32 * 32).to_be_bytes());
        b[at + 12..at + 16].copy_from_slice(&32u32.to_be_bytes());
        b[64 + i * 32..96 + i * 32].copy_from_slice(&mach(cpu));
    }
    b
}
fn write(path: &Path, bytes: impl AsRef<[u8]>) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}
fn bundle(root: &Path, name: &str, metadata: Option<&str>) -> PathBuf {
    let dir = root.join(format!("{name}.vst3"));
    fs::create_dir_all(&dir).unwrap();
    if let Some(text) = metadata {
        write(&dir.join("Contents/Resources/moduleinfo.json"), text);
    }
    write(
        &dir.join(format!("Contents/x86_64-linux/{name}.so")),
        elf(62, true, 64),
    );
    dir
}

#[test]
fn full_json5_metadata_has_stable_normalized_ids_and_claims_only() {
    let parsed = parse_moduleinfo(&manifest("fixture")).unwrap();
    assert_eq!(parsed.classes.len(), 1);
    assert!(parsed.classes[0].claims_instrument());
    assert_eq!(parsed.classes[0].cid.logical_key(), format!("vst3:{CID}"));
    assert_eq!(parsed.classes[0].class_flags, Some(1));
    assert_eq!(
        ClassId::parse(&CID.to_lowercase()).unwrap(),
        parsed.classes[0].cid
    );
    assert_eq!(parsed.compatibility_claims[0].new, parsed.classes[0].cid);
}
#[test]
fn escaped_unicode_multiline_strings_and_decimal_json5_are_parsed_by_dependency() {
    let text = manifest("Cl\\u0061ss").replace("'1.0'", "'one\\\ntwo'");
    let parsed = parse_moduleinfo(&text).unwrap();
    assert_eq!(parsed.name, "Class");
    assert_eq!(parsed.version, "onetwo");
}
#[test]
fn malformed_ids_duplicate_fields_and_duplicate_classes_are_not_accepted() {
    assert!(ClassId::parse("{00112233-4455-6677-8899-AABBCCDDEEFF}").is_err());
    assert!(ClassId::parse("zz112233445566778899AABBCCDDEEFF00").is_err());
    assert!(parse_moduleinfo(&manifest("a").replace("Name: 'a'", "Name: 'a', Name: 'b'")).is_err());
    let text = format!(
        r#"{{Name:'a',Classes:[{{CID:'{CID}',Name:'a',Category:'x'}},{{CID:'{CID}',Name:'b',Category:'x'}}]}}"#
    );
    assert!(parse_moduleinfo(&text).is_err());
    assert!(
        parse_moduleinfo(&manifest("a").replace("'Class Flags': 0x1", "'Class Flags': Infinity"))
            .is_err()
    );
}
#[test]
fn size_depth_and_strings_are_bounded_without_confusing_braces_in_comments() {
    let text = format!("{}{}{}", "[".repeat(33), "0", "]".repeat(33));
    assert!(matches!(
        parse_moduleinfo(&text),
        Err(CatalogError::Limit(_))
    ));
    assert!(parse_moduleinfo(&" ".repeat(1024 * 1024 + 1)).is_err());
    assert!(parse_moduleinfo(&manifest(&"x".repeat(4097))).is_err());
    assert!(
        parse_moduleinfo(&format!(
            "// {}\u{2028}{}",
            "[".repeat(100),
            manifest("[inside string]")
        ))
        .is_ok()
    );
}
#[test]
fn compatibility_does_not_point_to_an_undeclared_new_class() {
    let text = manifest("a").replace(
        &format!("New: '{CID}'"),
        "New: 'AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA'",
    );
    assert!(parse_moduleinfo(&text).is_err());
}
#[test]
fn root_suggestions_are_pure_ordered_and_do_not_invent_windows_folders() {
    let linux = suggested_roots(
        Platform::Linux,
        RootContext {
            home: Some("/synthetic/user"),
            application: Some("/app"),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        linux.iter().map(|x| x.path.as_str()).collect::<Vec<_>>(),
        vec![
            "/synthetic/user/.vst3",
            "/usr/lib64/vst3",
            "/usr/lib/vst3",
            "/usr/local/lib64/vst3",
            "/usr/local/lib/vst3",
            "/app/vst3"
        ]
    );
    let mac = suggested_roots(
        Platform::MacOs,
        RootContext {
            application: Some("/Fixture.app"),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(mac[2].path, "/Fixture.app/Contents/VST3");
    assert!(
        suggested_roots(Platform::Windows, RootContext::default())
            .unwrap()
            .is_empty()
    );
    let win = suggested_roots(
        Platform::Windows,
        RootContext {
            local_app_data: Some(r"C:\Users\fixture\AppData\Local"),
            common_program_files: Some(r"C:\Program Files\Common Files"),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        win[0].path,
        r"C:\Users\fixture\AppData\Local\Programs\Common\VST3"
    );
    assert_eq!(win[1].path, r"C:\Program Files\Common Files\VST3");
    assert!(
        suggested_roots(
            Platform::Linux,
            RootContext {
                home: Some("relative"),
                ..Default::default()
            }
        )
        .is_err()
    );
}
#[test]
fn elf_pe_and_macho_headers_are_read_without_loader_apis() {
    for (b, expected) in [
        (elf(62, true, 64), Architecture::X86_64),
        (elf(40, false, 32), Architecture::Arm),
        (pe(0xa641), Architecture::Arm64Ec),
        (pe(0xa64e), Architecture::Arm64X),
        (mach(0x0100000c), Architecture::Arm64),
    ] {
        let parsed = inspect_header(&b, b.len() as u64).unwrap();
        assert_eq!(parsed.architectures, vec![expected]);
        assert!(!parsed.loadability_verified);
    }
    let b = universal();
    let info = inspect_header(&b, b.len() as u64).unwrap();
    assert_eq!(
        info.architectures,
        vec![Architecture::X86_64, Architecture::Arm64]
    );
    assert_eq!(info.format, BinaryFormat::MachOUniversal);
}
#[test]
fn truncated_headers_and_invalid_offsets_fail_without_panics() {
    for b in [elf(62, true, 64), pe(0x8664), mach(0x01000007), universal()] {
        for end in 0..b.len() {
            let _ = inspect_header(&b[..end], b.len() as u64);
        }
    }
    let mut b = pe(0x8664);
    b[0x3c..0x40].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(inspect_header(&b, b.len() as u64).is_err());
    let mut b = universal();
    b[20..24].copy_from_slice(&u32::MAX.to_be_bytes());
    assert!(inspect_header(&b, b.len() as u64).is_err());
    let mut b = universal();
    b[36..40].copy_from_slice(&64u32.to_be_bytes());
    assert!(inspect_header(&b, b.len() as u64).is_err());
}
#[test]
fn synthetic_merged_bundle_reports_metadata_and_cross_platform_headers_only() {
    let temp = tempfile::tempdir().unwrap();
    let dir = bundle(temp.path(), "Test", Some(&manifest("Test")));
    write(&dir.join("Contents/x86_64-win/Test.vst3"), pe(0x8664));
    write(&dir.join("Contents/MacOS/Test"), universal());
    let report = scan_roots(&[temp.path().into()], &ScanLimits::default()).unwrap();
    assert!(report.complete);
    assert!(!report.executes_plugin_code);
    assert_eq!(report.candidates.len(), 1);
    let c = &report.candidates[0];
    assert_eq!(c.binaries.len(), 3);
    assert!(!c.host_verified);
    assert_eq!(c.status, "metadata_only_unverified");
    assert_eq!(c.manifest_sha256.as_ref().unwrap().len(), 64);
    assert!(c.binaries.iter().all(|b| !b.header.loadability_verified));
}
#[test]
fn legacy_location_is_supported_but_invalid_modern_manifest_never_falls_back() {
    let temp = tempfile::tempdir().unwrap();
    let dir = bundle(temp.path(), "Legacy", None);
    write(&dir.join("Contents/moduleinfo.json"), manifest("old"));
    let report = scan_roots(&[temp.path().into()], &ScanLimits::default()).unwrap();
    assert_eq!(report.candidates[0].metadata.as_ref().unwrap().name, "old");
    write(&dir.join("Contents/Resources/moduleinfo.json"), "malformed");
    let report = scan_roots(&[temp.path().into()], &ScanLimits::default()).unwrap();
    assert!(report.candidates[0].metadata.is_none());
    assert_eq!(report.candidates[0].status, "metadata_invalid");
}
#[test]
fn optional_manifest_and_legacy_dll_never_trigger_an_execution_fallback() {
    let temp = tempfile::tempdir().unwrap();
    bundle(temp.path(), "NoMeta", None);
    write(&temp.path().join("Old.vst3"), pe(0x8664));
    let report = scan_roots(&[temp.path().into()], &ScanLimits::default()).unwrap();
    assert_eq!(report.candidates.len(), 2);
    assert!(
        report
            .candidates
            .iter()
            .all(|c| c.metadata.is_none() && !c.host_verified)
    );
    assert_eq!(
        report
            .candidates
            .iter()
            .filter(|c| c.legacy_single_file)
            .count(),
        1
    );
}
#[test]
fn duplicate_processor_claims_remain_visible_in_explicit_root_priority() {
    let temp = tempfile::tempdir().unwrap();
    let first = temp.path().join("first");
    let second = temp.path().join("second");
    bundle(&first, "Z", Some(&manifest("preferred")));
    bundle(&second, "A", Some(&manifest("later")));
    let report = scan_roots(&[first.clone(), second, first], &ScanLimits::default()).unwrap();
    assert_eq!(report.candidates.len(), 2);
    assert_eq!(report.duplicate_classes.len(), 1);
    assert_eq!(report.duplicate_classes[0].first_candidate, 0);
    assert_eq!(report.duplicate_classes[0].later_candidates, vec![1]);
    assert_eq!(
        report.candidates[0].metadata.as_ref().unwrap().name,
        "preferred"
    );
}
#[test]
fn byte_entry_depth_and_candidate_budgets_are_explicit() {
    let temp = tempfile::tempdir().unwrap();
    bundle(temp.path(), "One", Some(&manifest("one")));
    bundle(temp.path(), "Two", None);
    let limits = ScanLimits {
        max_manifest_bytes: 8,
        ..Default::default()
    };
    let report = scan_roots(&[temp.path().into()], &limits).unwrap();
    assert!(report.candidates[0].metadata.is_none());
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.code == "manifest-unreadable")
    );
    for limits in [
        ScanLimits {
            max_entries: 1,
            ..Default::default()
        },
        ScanLimits {
            max_candidates: 1,
            ..Default::default()
        },
    ] {
        assert!(!scan_roots(&[temp.path().into()], &limits).unwrap().complete);
    }
    let inner = temp.path().join("nested/deeper");
    bundle(&inner, "Deep", None);
    let report = scan_roots(
        &[temp.path().into()],
        &ScanLimits {
            max_depth: 0,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(!report.complete);
    assert!(report.diagnostics.iter().any(|d| d.code == "depth-limited"));
    assert!(scan_roots(&[PathBuf::from("relative")], &ScanLimits::default()).is_err());
}
#[test]
fn ordering_and_json_output_are_reproducible_for_stable_trees() {
    let temp = tempfile::tempdir().unwrap();
    bundle(temp.path(), "Z", Some(&manifest("Z")));
    bundle(temp.path(), "A", Some(&manifest("A")));
    let a = scan_roots(&[temp.path().into()], &ScanLimits::default()).unwrap();
    let b = scan_roots(&[temp.path().into()], &ScanLimits::default()).unwrap();
    assert_eq!(
        serde_json::to_string(&a).unwrap(),
        serde_json::to_string(&b).unwrap()
    );
    assert_eq!(a.candidates[0].metadata.as_ref().unwrap().name, "A");
}
#[cfg(unix)]
#[test]
fn symlinks_outside_roots_and_loops_are_skipped_without_reading_targets() {
    use std::os::unix::fs::symlink;
    let temp = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    bundle(outside.path(), "Secret", Some(&manifest("must not read")));
    symlink(outside.path(), temp.path().join("loop")).unwrap();
    symlink(
        outside.path().join("Secret.vst3"),
        temp.path().join("Secret.vst3"),
    )
    .unwrap();
    let dir = bundle(temp.path(), "Local", None);
    fs::create_dir_all(dir.join("Contents/Resources")).unwrap();
    symlink(
        outside
            .path()
            .join("Secret.vst3/Contents/Resources/moduleinfo.json"),
        dir.join("Contents/Resources/moduleinfo.json"),
    )
    .unwrap();
    let report = scan_roots(&[temp.path().into()], &ScanLimits::default()).unwrap();
    assert_eq!(report.candidates.len(), 1);
    assert!(report.candidates[0].metadata.is_none());
    assert!(report.diagnostics.iter().any(|d| d.code == "entry-skipped"));
}
#[test]
fn random_short_headers_do_not_panic_or_execute_anything() {
    let mut seed = 12345u64;
    for size in 0..512 {
        let bytes: Vec<u8> = (0..size)
            .map(|_| {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
                (seed >> 32) as u8
            })
            .collect();
        let _ = inspect_header(&bytes, bytes.len() as u64);
    }
}

#[test]
fn metadata_revision_changes_without_changing_the_logical_class_identity() {
    let temp = tempfile::tempdir().unwrap();
    let dir = bundle(temp.path(), "Test", Some(&manifest("v1")));
    let a = scan_roots(&[temp.path().into()], &ScanLimits::default()).unwrap();
    write(
        &dir.join("Contents/Resources/moduleinfo.json"),
        manifest("v2"),
    );
    let b = scan_roots(&[temp.path().into()], &ScanLimits::default()).unwrap();
    assert_ne!(
        a.candidates[0].manifest_sha256,
        b.candidates[0].manifest_sha256
    );
    assert_eq!(
        a.candidates[0].metadata.as_ref().unwrap().classes[0].cid,
        b.candidates[0].metadata.as_ref().unwrap().classes[0].cid
    );
}

#[test]
fn header_architecture_overrides_a_misleading_directory_hint() {
    let temp = tempfile::tempdir().unwrap();
    let dir = bundle(temp.path(), "Wrong", None);
    write(
        &dir.join("Contents/x86_64-linux/Wrong.so"),
        elf(183, true, 64),
    );
    let report = scan_roots(&[temp.path().into()], &ScanLimits::default()).unwrap();
    assert_eq!(
        report.candidates[0].binaries[0].header.architectures,
        vec![Architecture::Arm64]
    );
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.code == "layout-architecture-conflict")
    );
}

#[test]
fn total_retained_file_data_is_bounded_and_incomplete_is_explicit() {
    let temp = tempfile::tempdir().unwrap();
    bundle(temp.path(), "One", Some(&manifest("one")));
    let report = scan_roots(
        &[temp.path().into()],
        &ScanLimits {
            max_total_file_bytes: 100,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(!report.complete);
    assert!(report.retained_file_bytes <= 100);
    assert!(report.candidates[0].metadata.is_none());
}

#[test]
fn overlapping_roots_do_not_create_false_duplicate_classes() {
    let temp = tempfile::tempdir().unwrap();
    let nested = temp.path().join("nested");
    bundle(&nested, "Shared", Some(&manifest("shared")));
    let report = scan_roots(&[temp.path().into(), nested], &ScanLimits::default()).unwrap();
    assert_eq!(report.candidates.len(), 1);
    assert!(report.duplicate_classes.is_empty());
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.code == "duplicate-candidate")
    );
}

#[test]
fn small_files_below_remaining_total_budget_do_not_make_scan_incomplete() {
    let temp = tempfile::tempdir().unwrap();
    write(&temp.path().join("Tiny.vst3"), elf(62, true, 64));
    let report = scan_roots(
        &[temp.path().into()],
        &ScanLimits {
            max_total_file_bytes: 100,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(report.complete);
    assert_eq!(report.retained_file_bytes, 64);
}

#[cfg(unix)]
#[test]
fn symlinked_manifest_ancestors_are_rejected_before_presence_probes() {
    use std::os::unix::fs::symlink;
    let temp = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    write(&outside.path().join("moduleinfo.json"), manifest("outside"));
    let dir = bundle(temp.path(), "Linked", None);
    symlink(outside.path(), dir.join("Contents/Resources")).unwrap();
    let report = scan_roots(&[temp.path().into()], &ScanLimits::default()).unwrap();
    assert_eq!(report.candidates[0].status, "metadata_invalid");
    assert!(report.candidates[0].manifest_path.is_none());
    assert!(report.candidates[0].manifest_sha256.is_none());
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.code == "manifest-unreadable" && d.message.contains("Symlink"))
    );
}
