use scorestream_formats::{Format, MappingEffect, MappingReport};
#[test]
fn concrete_first_error_survives_informational_saturation_and_merge() {
    let mut report = MappingReport::default();
    for i in 0..4096 {
        report.push(
            Format::MusicXml,
            MappingEffect::Preserved,
            "retained",
            format!("/{i}"),
            "safe retention",
        );
    }
    assert!(!report.has_errors());
    let mut failure = MappingReport::default();
    failure.push(
        Format::MusicXml,
        MappingEffect::Unsupported,
        "real.unsupported",
        "/part[9]/measure[42]",
        "actual unresolved semantic",
    );
    report.append(failure);
    assert!(report.has_errors());
    assert_eq!(report.diagnostics.len(), 1024);
    let real = report
        .diagnostics
        .iter()
        .find(|d| d.feature == "real.unsupported")
        .unwrap();
    assert_eq!(real.path, "/part[9]/measure[42]");
    for _ in 0..2000 {
        report.push(
            Format::MusicXml,
            MappingEffect::Preserved,
            "retained",
            "/",
            "safe",
        );
    }
    assert!(
        report
            .diagnostics
            .iter()
            .any(|d| d.feature == "real.unsupported")
    );
    assert!(report.finish(()).is_err());
}
#[test]
fn older_receipt_json_defaults_to_one_occurrence() {
    let old = r#"{"diagnostics":[{"format":"music_xml","effect":"preserved","feature":"x","path":"/","message":"x","source":null}]}"#;
    let report: MappingReport = serde_json::from_str(old).unwrap();
    assert_eq!(report.diagnostics[0].occurrences, 1);
    assert_eq!(serde_json::to_string(&report).unwrap(), old);
}
