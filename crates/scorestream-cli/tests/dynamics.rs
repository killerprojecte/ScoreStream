use std::{path::PathBuf, process::Command};
#[test]
fn cli_explicit_dynamics_preserve_absence_exact_values_and_held_steps() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../scorestream-formats/tests/fixtures/dynamics-explicit.musicxml");
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
    assert!(sfs.contains("sound_dynamics=1111/25"));
    assert!(sfs.contains("sound_dynamics=10667/100"));
    let report: serde_json::Value = serde_json::from_slice(&output.stderr).unwrap();
    assert!(
        report["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["feature"] == "sound_dynamics.initial_unassigned")
    );
    let plan = scorestream_app::playback::plan_source(&sfs).unwrap();
    assert_eq!(plan.notes.len(), 2);
    assert_eq!(plan.notes[0].velocity, 80);
    let held = &plan.notes[1];
    assert_eq!(held.velocity, 40);
    let points = held
        .gain_envelope
        .iter()
        .filter(|p| p.offset_seconds == 0.5)
        .collect::<Vec<_>>();
    assert_eq!(points.len(), 2);
    assert!((40.0 * points[0].gain - 39.996).abs() < 1e-10);
    assert!((40.0 * points[1].gain - 96.003).abs() < 1e-10);
    let source = std::fs::read_to_string(fixture)
        .unwrap()
        .replace("<dynamics><p/></dynamics>", "<wedge type=\"crescendo\"/>");
    let path = std::env::temp_dir().join(format!(
        "scorestream-dynamics-reject-{}.musicxml",
        std::process::id()
    ));
    std::fs::write(&path, source).unwrap();
    let rejected = Command::new(env!("CARGO_BIN_EXE_scorestream"))
        .args(["convert", "musicxml", "sfs"])
        .arg(&path)
        .output()
        .unwrap();
    std::fs::remove_file(path).unwrap();
    assert!(!rejected.status.success());
    assert!(rejected.stdout.is_empty());
    let report: serde_json::Value = serde_json::from_slice(&rejected.stderr).unwrap();
    assert!(
        report["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["feature"] == "element.wedge")
    );
}
