use std::{path::PathBuf, process::Command};
#[test]
fn cli_imported_ties_reach_actual_playback_without_reattacks() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../scorestream-formats/tests/fixtures/ties-transposing-chain.musicxml");
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
    assert_eq!(plan.notes.len(), 1);
    assert_eq!(plan.event_positions.len(), 3);
    assert_eq!(plan.notes[0].midi_note, 67.);
    let source = std::fs::read_to_string(fixture).unwrap().replace(
        "<tie type=\"start\"/>",
        "<tie type=\"start\" time-only=\"1\"/>",
    );
    let path = std::env::temp_dir().join(format!(
        "scorestream-tie-reject-{}.musicxml",
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
        "No partial score is printed after rejected semantics"
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert!(
        report["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["feature"] == "tie.repeat_condition")
    );
}
