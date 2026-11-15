use std::{path::PathBuf, process::Command};
fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../scorestream-formats/tests/fixtures")
        .join(name)
}
#[test]
fn convert_musicxml_to_valid_sfs_and_inspect_model() {
    let output = Command::new(env!("CARGO_BIN_EXE_scorestream"))
        .args(["convert", "musicxml", "sfs"])
        .arg(fixture("two-staff.musicxml"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let source = String::from_utf8(output.stdout).unwrap();
    assert!(scorestream_codec::parse(&source).is_ok());
    assert!(source.contains("E(-1/2)4:1"));
    let inspect = Command::new(env!("CARGO_BIN_EXE_scorestream"))
        .args(["inspect-format", "musicxml"])
        .arg(fixture("two-staff.musicxml"))
        .output()
        .unwrap();
    assert!(inspect.status.success());
    let model: serde_json::Value = serde_json::from_slice(&inspect.stdout).unwrap();
    assert_eq!(model["parts"][0]["staves"], 2);
    assert_eq!(
        model["parts"][0]["measures"][1]["events"][0]["duration"],
        "1/3"
    );
}
#[test]
fn failed_mapping_never_prints_partial_output() {
    let output = Command::new(env!("CARGO_BIN_EXE_scorestream"))
        .args(["convert", "musicxml", "sfs"])
        .arg(fixture("unsupported.musicxml"))
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let report: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert!(
        report["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["feature"] == "element.direction")
    );
}
#[test]
fn unknown_format_and_old_sfs_commands() {
    let unsupported = Command::new(env!("CARGO_BIN_EXE_scorestream"))
        .args(["convert", "mxl", "sfs"])
        .arg(fixture("two-staff.musicxml"))
        .output()
        .unwrap();
    assert_eq!(unsupported.status.code(), Some(2));
    let legacy = Command::new(env!("CARGO_BIN_EXE_scorestream"))
        .arg("validate")
        .arg(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples/first-light.sfs"))
        .output()
        .unwrap();
    assert!(
        legacy.status.success(),
        "{}",
        String::from_utf8_lossy(&legacy.stderr)
    );
}

#[test]
fn scanner_is_separate_from_semantic_conversion() {
    let scan = Command::new(env!("CARGO_BIN_EXE_scorestream"))
        .arg("scan-musicxml")
        .arg(fixture("unsupported.musicxml"))
        .output()
        .unwrap();
    assert!(
        scan.status.success(),
        "{}",
        String::from_utf8_lossy(&scan.stderr)
    );
    let summary: serde_json::Value = serde_json::from_slice(&scan.stdout).unwrap();
    assert!(summary["elements"].as_u64().unwrap() > 0);
    assert!(summary["max_retained_nodes"].as_u64().unwrap() > 0);
    let mapping = Command::new(env!("CARGO_BIN_EXE_scorestream"))
        .args(["convert", "musicxml", "sfs"])
        .arg(fixture("unsupported.musicxml"))
        .output()
        .unwrap();
    assert!(!mapping.status.success());
    assert!(mapping.stdout.is_empty());
}

#[test]
fn conversion_cli_has_no_global_xml_or_sfs_byte_ceiling() {
    let directory = std::env::temp_dir().join(format!(
        "scorestream-large-conversion-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&directory).unwrap();
    let path = directory.join("large.musicxml");
    let source = std::fs::read_to_string(fixture("two-staff.musicxml")).unwrap();
    let comments = format!("<!--{}-->", "x".repeat(900_000)).repeat(10);
    let source = source.replace("</score-partwise>", &format!("{comments}</score-partwise>"));
    assert!(source.len() > 8 * 1024 * 1024);
    std::fs::write(&path, source).unwrap();
    let converted = Command::new(env!("CARGO_BIN_EXE_scorestream"))
        .args(["convert", "musicxml", "sfs"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(
        converted.status.success(),
        "{}",
        String::from_utf8_lossy(&converted.stderr)
    );
    let source = String::from_utf8(converted.stdout).unwrap();
    let expected = scorestream_codec::parse(&source).unwrap();
    let sfs = directory.join("large.sfs");
    std::fs::write(&sfs, format!(";{}\n{source}", "comment".repeat(1_300_000))).unwrap();
    let converted = Command::new(env!("CARGO_BIN_EXE_scorestream"))
        .args(["convert", "sfs", "sfs"])
        .arg(sfs)
        .output()
        .unwrap();
    assert!(
        converted.status.success(),
        "{}",
        String::from_utf8_lossy(&converted.stderr)
    );
    assert_eq!(
        scorestream_codec::canonical(&expected),
        String::from_utf8(converted.stdout).unwrap()
    );
    std::fs::remove_dir_all(directory).unwrap();
}
