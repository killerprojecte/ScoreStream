use std::{path::PathBuf, process::Command};
#[test]
fn cli_imported_tempos_control_real_plan_and_unsupported_sound_is_atomic() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../scorestream-formats/tests/fixtures/tempo-pickup.musicxml");
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
    let sfs = String::from_utf8(output.stdout).unwrap();
    let plan = scorestream_app::playback::plan_source(&sfs).unwrap();
    assert_eq!(plan.notes.len(), 2);
    assert_eq!(plan.event_positions.len(), 3);
    assert!((plan.duration_seconds - 2.0625).abs() < 1e-9);
    assert!((plan.measures[0].end_seconds - 0.5).abs() < 1e-9);
    assert!((plan.event_positions[1].end_seconds - 1.0625).abs() < 1e-9);
    let source = std::fs::read_to_string(fixture).unwrap().replace(
        "<sound tempo=\"288\"/>",
        "<sound tempo=\"288\" pan=\"45\"/>",
    );
    let path = std::env::temp_dir().join(format!(
        "scorestream-tempo-reject-{}.musicxml",
        std::process::id()
    ));
    std::fs::write(&path, source).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_scorestream"))
        .args(["convert", "musicxml", "sfs"])
        .arg(&path)
        .output()
        .unwrap();
    std::fs::remove_file(path).unwrap();
    assert!(!output.status.success());
    assert!(
        output.stdout.is_empty(),
        "No converted content may precede an unsupported field failure"
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert!(
        report["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["feature"] == "attribute.pan")
    );
}
