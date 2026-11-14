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
