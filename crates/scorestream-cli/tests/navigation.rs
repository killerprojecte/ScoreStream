use std::{fs, path::PathBuf, process::Command};
#[test]
fn navigation_conversion_and_success_commands_surface_authoring_limits() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../scorestream-formats/tests/fixtures/navigation-closed.musicxml");
    let converted = Command::new(env!("CARGO_BIN_EXE_scorestream"))
        .args(["convert", "musicxml", "sfs"])
        .arg(fixture)
        .output()
        .unwrap();
    assert!(
        converted.status.success(),
        "{}",
        String::from_utf8_lossy(&converted.stderr)
    );
    assert!(String::from_utf8_lossy(&converted.stderr).contains("navigation.authored_only"));
    let dir = std::env::temp_dir().join(format!("scorestream-navigation-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let file = dir.join("closed.sfs");
    fs::write(&file, &converted.stdout).unwrap();
    for command in ["validate", "canonical", "inspect"] {
        let result = Command::new(env!("CARGO_BIN_EXE_scorestream"))
            .arg(command)
            .arg(&file)
            .output()
            .unwrap();
        assert!(result.status.success());
        let warnings: serde_json::Value = serde_json::from_slice(&result.stderr).unwrap();
        assert!(
            warnings
                .as_array()
                .unwrap()
                .iter()
                .any(|d| d["code"] == "navigation.realization_unavailable")
        );
        if command == "canonical" {
            assert!(scorestream_codec::parse(&String::from_utf8(result.stdout).unwrap()).is_ok());
        }
    }
    let open = String::from_utf8(converted.stdout)
        .unwrap()
        .lines()
        .filter(|line| !line.contains("ending=discontinue"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    fs::write(&file, open).unwrap();
    let inspected = Command::new(env!("CARGO_BIN_EXE_scorestream"))
        .arg("validate")
        .arg(&file)
        .output()
        .unwrap();
    assert!(inspected.status.success());
    assert!(String::from_utf8_lossy(&inspected.stderr).contains("navigation.unclosed_ending"));
    let failed = Command::new(env!("CARGO_BIN_EXE_scorestream"))
        .args(["convert", "sfs", "musicxml"])
        .arg(&file)
        .output()
        .unwrap();
    assert!(!failed.status.success());
    assert!(failed.stdout.is_empty());
    assert!(String::from_utf8_lossy(&failed.stderr).contains("UnclosedFinalEnding"));
    fs::remove_dir_all(dir).unwrap();
}
