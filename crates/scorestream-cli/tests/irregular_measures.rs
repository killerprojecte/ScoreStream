use std::{
    fs,
    path::Path,
    process::{Command, Output},
};
fn run(input: &str, output: &str, path: &Path, contract: Option<&Path>) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_scorestream"));
    cmd.args(["convert", input, output]).arg(path);
    if let Some(contract) = contract {
        cmd.arg("--musicxml-irregular-measures").arg(contract);
    }
    cmd.output().unwrap()
}
fn rejected(result: &Output, feature: &str) {
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());
    let report: serde_json::Value = serde_json::from_slice(&result.stderr).unwrap();
    assert!(
        report["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["feature"] == feature),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}
#[test]
fn exact_cli_contract_is_explicit_roundtrips_and_rejects_stale_evidence() {
    let dir =
        std::env::temp_dir().join(format!("scorestream-irregular-cli-{}", std::process::id()));
    fs::create_dir_all(&dir).unwrap();
    let xml = dir.join("input.musicxml");
    let json = dir.join("contract.json");
    let sfs = dir.join("score.sfs");
    let source = r#"<score-partwise version="4.0"><part-list><score-part id="p"><part-name>Piano</part-name><score-instrument id="i"><instrument-name>piano</instrument-name></score-instrument></score-part></part-list><part id="p"><measure number="268"><attributes><divisions>1</divisions><time><beats>2</beats><beat-type>4</beat-type></time></attributes><note><rest/><duration>4</duration><voice>1</voice><type>whole</type></note></measure><measure number="269"><note><rest/><duration>2</duration><voice>1</voice><type>half</type></note></measure></part></score-partwise>"#;
    fs::write(&xml, source).unwrap();
    let contract = r#"[{"ordinal":1,"label":"268","written_meter":{"beats":[2],"beat_type":4},"actual_length":"4"}]"#;
    fs::write(&json, contract).unwrap();
    rejected(&run("musicxml", "sfs", &xml, None), "measure.overfull");
    let converted = run("musicxml", "sfs", &xml, Some(&json));
    assert!(
        converted.status.success(),
        "{}",
        String::from_utf8_lossy(&converted.stderr)
    );
    fs::write(&sfs, &converted.stdout).unwrap();
    let plain = run("sfs", "musicxml", &sfs, None);
    rejected(&plain, "measure.length");
    let exported = run("sfs", "musicxml", &sfs, Some(&json));
    assert!(
        exported.status.success(),
        "{}",
        String::from_utf8_lossy(&exported.stderr)
    );
    fs::write(&xml, &exported.stdout).unwrap();
    assert_eq!(
        run("musicxml", "sfs", &xml, Some(&json)).stdout,
        converted.stdout
    );
    for invalid in [
        contract.replace("\"268\"", "\"267\""),
        contract.replace("\"4\"", "\"5\""),
        contract.replace("\"ordinal\":1", "\"ordinal\":2"),
    ] {
        fs::write(&json, invalid).unwrap();
        let bad = run("musicxml", "sfs", &xml, Some(&json));
        assert!(!bad.status.success());
        assert!(bad.stdout.is_empty());
    }
    for invalid in ["not json", "{}", "[{\"unexpected\":1}]"] {
        fs::write(&json, invalid).unwrap();
        let bad = run("musicxml", "sfs", &xml, Some(&json));
        assert_eq!(bad.status.code(), Some(2));
        assert!(bad.stdout.is_empty());
    }
    fs::write(&json, contract).unwrap();
    assert_eq!(run("sfs", "sfs", &sfs, Some(&json)).status.code(), Some(2));
    fs::remove_dir_all(dir).unwrap();
}
