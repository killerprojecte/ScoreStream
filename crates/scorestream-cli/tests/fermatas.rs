use std::{path::PathBuf, process::Command};
#[test]
fn cli_preserves_fermata_capability_and_rejects_unmapped_notation_atomically() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../scorestream-formats/tests/fixtures/fermata-note-rest.musicxml");
    let output = Command::new(env!("CARGO_BIN_EXE_scorestream"))
        .args(["convert", "musicxml", "sfs"])
        .arg(&fixture)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let source = String::from_utf8(output.stdout).unwrap();
    assert!(source.contains("!requires scorestream.fermata@1"));
    assert!(source.contains("+ #stop scorestream.fermata default_y=1/25"));
    assert!(source.contains("+ #rest scorestream.fermata"));
    let report: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert!(
        report["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["feature"] == "fermata.authored_only")
    );
    assert!(
        scorestream_app::playback::plan_source(&source)
            .unwrap_err()
            .contains("fermata.performance_unsupported")
    );
    let path = std::env::temp_dir().join(format!(
        "scorestream-fermata-reject-{}.musicxml",
        std::process::id()
    ));
    std::fs::write(
        &path,
        std::fs::read_to_string(fixture).unwrap().replace(
            "<fermata type=\"upright\"",
            "<fermata hold-seconds=\"2\" type=\"upright\"",
        ),
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_scorestream"))
        .args(["convert", "musicxml", "sfs"])
        .arg(&path)
        .output()
        .unwrap();
    std::fs::remove_file(path).unwrap();
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    let report: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert!(
        report["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["feature"] == "attribute.hold-seconds")
    );
}
