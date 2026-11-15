use scorestream_codec::{parse, parse_with_prevalidation, Diagnostic, SourceRange};
use std::cell::Cell;
const SOURCE: &str = "!score 1.1 unit=q\n!part p instrument=piano staves=1\n!voice v part=p staff=1\n@0\nv C4:1\n.\n!end at=1\n";
#[test]
fn read_only_preflight_runs_before_semantic_validation_and_id_assignment() {
    let called = Cell::new(false);
    let result = parse_with_prevalidation(SOURCE, |doc| {
        called.set(true);
        assert!(doc.events().any(|(_, e)| e.id().is_none()));
        Ok(())
    })
    .unwrap();
    assert!(called.get());
    assert!(result.events().all(|(_, e)| e.id().is_some()));
    assert_eq!(result, parse(SOURCE).unwrap());
}
#[test]
fn successful_preflight_cannot_bypass_existing_validation() {
    let invalid = SOURCE.replace("!end at=1", "!end at=0");
    let called = Cell::new(false);
    let errors = parse_with_prevalidation(&invalid, |_| {
        called.set(true);
        Ok(())
    })
    .unwrap_err();
    assert!(called.get());
    assert_eq!(errors, parse(&invalid).unwrap_err());
}
#[test]
fn failed_preflight_returns_no_document_or_partial_success() {
    let errors = parse_with_prevalidation(SOURCE, |_| {
        Err(vec![Diagnostic::error(
            "test.budget",
            "Before validation",
            SourceRange::default(),
        )])
    })
    .unwrap_err();
    assert_eq!(errors.len(), 1);
    assert_eq!(errors[0].code, "test.budget");
}
