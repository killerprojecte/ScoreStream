use scorestream_codec::*;
use serde_json::{json, Value};

fn score(metadata: &str) -> String {
    format!("!score 1.1 unit=q\n{metadata}\n!part p instrument=piano staves=1\n!voice v part=p staff=1\n@0\nv C4:1 id=a\n.\n!end at=1\n")
}
fn fails(metadata: &str, code: &str) -> Vec<Diagnostic> {
    let diagnostics = parse(&score(metadata)).unwrap_err();
    assert!(
        diagnostics.iter().any(|d| d.code == code),
        "{diagnostics:?}"
    );
    diagnostics
}

#[test]
fn all_metadata_fields_are_first_class_and_keep_provenance() {
    let document = parse(&score(r#"!meta title="春日" description="A study\nSecond line" authors=["林青", "Alex Chen"] header="Rehearsal" footer="Draft""#)).unwrap();
    assert_eq!(document.metadata.title.as_deref(), Some("春日"));
    assert_eq!(
        document.metadata.description.as_deref(),
        Some("A study\nSecond line")
    );
    assert_eq!(document.metadata.authors, ["林青", "Alex Chen"]);
    assert_eq!(document.metadata.header.as_deref(), Some("Rehearsal"));
    assert_eq!(document.metadata.footer.as_deref(), Some("Draft"));
    assert_eq!(document.metadata.source["title"].line, 2);
    assert_eq!(document.metadata.source["title"].column, 7);
    assert_eq!(document.metadata.source["title"].end_column, 17);
    let value = serde_json::to_value(&document).unwrap();
    assert_eq!(value["metadata"]["authors"], json!(["林青", "Alex Chen"]));
    assert!(value["metadata"].get("source").is_none());
    assert_eq!(value["format_version"], "1.1");
}

#[test]
fn canonical_metadata_is_deterministic_compact_and_idempotent() {
    let source = score("!meta footer=\"Draft\" authors=[ \"B\" ,\t\"A \\\"Name\\\"\" ]\n!meta title=\"春日\" description=\"Line\\nTwo\" header=\"Rehearsal\"");
    let first = canonical(&parse(&source).unwrap());
    assert_eq!(
        first.lines().find(|s| s.starts_with("!meta")).unwrap(),
        r#"!meta authors=["B","A \"Name\""] description="Line\nTwo" footer="Draft" header="Rehearsal" title="春日""#
    );
    assert_eq!(first.matches("!meta").count(), 1);
    assert_eq!(canonical(&parse(&first).unwrap()), first);
}

#[test]
fn legacy_metadata_retains_names_values_and_roles() {
    let source = score(
        r#"!meta title="T" subtitle="S" composer="C" lyricist="L" arranger="A" work_number="W" movement="M" rights="R" language="zh""#,
    );
    let doc = parse(&source).unwrap();
    assert!(doc.metadata.authors.is_empty());
    assert_eq!(doc.metadata.composer.as_deref(), Some("C"));
    let value = serde_json::to_value(&doc).unwrap();
    assert_eq!(value["metadata"].as_object().unwrap().len(), 9);
    let restored: Document = serde_json::from_value(value).unwrap();
    assert!(validate(&restored, None).is_empty());
    assert_eq!(canonical(&restored), canonical(&doc));
}

#[test]
fn missing_and_empty_fields_do_not_create_a_title_or_author() {
    let doc = parse(&score("")).unwrap();
    assert!(doc.metadata.is_empty());
    assert_eq!(serde_json::to_value(&doc).unwrap()["metadata"], json!({}));
    let doc = parse(&score(r#"!meta authors=[] title="" header="""#)).unwrap();
    assert_eq!(doc.metadata.title.as_deref(), Some(""));
    assert_eq!(
        serde_json::to_value(&doc).unwrap()["metadata"],
        json!({"title":"","header":""})
    );
    assert!(!canonical(&doc).contains("authors="));
    assert!(!canonical(&parse(&score("!meta authors=[]")).unwrap()).contains("!meta"));
}

#[test]
fn repeated_fields_fail_even_when_identical_or_empty() {
    for source in [
        "!meta title=\"A\" title=\"A\"",
        "!meta title=\"A\"\n!meta title=\"B\"",
        "!meta authors=[]\n!meta authors=[]",
    ] {
        fails(source, "attribute_duplicate");
    }
}

#[test]
fn author_array_requires_strings_and_well_formed_single_line_json() {
    for source in [
        r#"!meta authors="A""#,
        "!meta authors=[1]",
        "!meta authors=[null]",
        "!meta authors=[true]",
        r#"!meta authors=["A",]"#,
        r#"!meta authors=["A""#,
        r#"!meta authors=[["A"]]"#,
        r#"!meta authors=["\uD800"]"#,
    ] {
        fails(source, "metadata_authors_type");
    }
    for source in [
        r#"!meta authors=[""]"#,
        r#"!meta authors=["  "]"#,
        r#"!meta authors=["A\nB"]"#,
        r#"!meta authors=["A\tB"]"#,
        r#"!meta authors=["A\u0000B"]"#,
    ] {
        fails(source, "metadata_author");
    }
    assert!(
        parse(&score(r#"!meta authors=["A","A"]"#)).is_ok(),
        "author order/identity is not silently deduplicated"
    );
}

#[test]
fn metadata_validation_is_typed_for_json_and_reports_field_origin() {
    for value in [
        json!({"title":3}),
        json!({"title":null}),
        json!({"authors":"A"}),
        json!({"authors":[1]}),
        json!({"authors":null}),
        json!({"unknown":"x"}),
        json!({"source":{}}),
    ] {
        assert!(serde_json::from_value::<DocumentMetadata>(value).is_err());
    }
    let diagnostics = fails(r#"!meta title="A" authors=[" "]"#, "metadata_author");
    let d = diagnostics
        .iter()
        .find(|d| d.code == "metadata_author")
        .unwrap();
    assert_eq!(d.object_id.as_deref(), Some("metadata.authors"));
    assert_eq!(d.source.line, 2);
    assert_eq!(d.source.column, 17);
    let mut doc = parse(&score("")).unwrap();
    doc.metadata = serde_json::from_value(json!({"footer":"NUL\u{0}"})).unwrap();
    assert!(validate(&doc, None)
        .iter()
        .any(|d| d.code == "metadata_text"));
}

#[test]
fn markup_is_plain_content_and_author_arrays_do_not_change_other_token_rules() {
    let doc = parse(&score(
        r#"!meta title="<b>A & B</b>" authors=["A ] # \"B\""] footer="{page}""#,
    ))
    .unwrap();
    assert_eq!(doc.metadata.title.as_deref(), Some("<b>A & B</b>"));
    assert_eq!(doc.metadata.footer.as_deref(), Some("{page}"));
    assert_eq!(doc.metadata.authors, ["A ] # \"B\""]);
    assert_eq!(
        canonical(&parse(&canonical(&doc)).unwrap()),
        canonical(&doc)
    );
    fails(r#"!meta invented="A""#, "field_unknown");
    fails("!meta title=unquoted", "string_invalid");
    let ordinary = "!score 1.1 unit=q\n!part p instrument=piano staves=1\n!voice v part=p staff=1\n@0\nv C4:1 id=a\n+ #a words text=[\"A\", \"B\"]\n.\n!end at=1\n";
    assert!(parse(ordinary).is_err());
}

#[test]
fn metadata_remains_declaration_only_and_forbidden_in_fragments() {
    let late = score("").replace("@0\n", "@0\n!meta title=\"Late\"\n");
    assert!(parse(&late)
        .unwrap_err()
        .iter()
        .any(|d| d.code == "declaration_order"));
    let fragment = "!fragment 1.1 unit=q span=1\n!meta title=\"No\"\n!end at=1\n";
    assert!(parse_with_context(fragment, &FragmentContext::default())
        .unwrap_err()
        .iter()
        .any(|d| d.code == "fragment_global_state"));
}

#[test]
fn descriptor_metadata_fields_match_typed_model_and_full_example() {
    let descriptor: Value = serde_json::from_str(include_str!(
        "../../../design/spec/scorestream-language.json"
    ))
    .unwrap();
    let fields = descriptor["vocabulary"]["metadata"].as_object().unwrap();
    assert_eq!(fields.len(), 13);
    for (name, contract) in fields {
        let sample = if name == "authors" {
            r#"["A", "B"]"#
        } else {
            r#""Value""#
        };
        let doc = parse(&score(&format!("!meta {name}={sample}"))).unwrap();
        assert!(serde_json::to_value(doc).unwrap()["metadata"]
            .get(name)
            .is_some());
        assert_eq!(
            attribute_type("metadata", "", name),
            contract["type"].as_str()
        );
    }
    let source = include_str!("../../../design/examples/sfs/document-metadata.sfs");
    let doc = parse(source).unwrap();
    assert_eq!(doc.metadata.title.as_deref(), Some("春日练习"));
    assert_eq!(doc.event_count(), 2);
}

#[test]
fn legacy_escaped_controls_remain_readable_without_silent_loss() {
    let doc = parse(&score(r#"!meta title="Legacy\u0000text" composer="A\bB""#)).unwrap();
    assert_eq!(doc.metadata.title.as_deref(), Some("Legacy\u{0}text"));
    assert!(doc
        .diagnostics
        .iter()
        .all(|d| d.severity == Severity::Warning));
    assert_eq!(
        doc.diagnostics
            .iter()
            .filter(|d| d.code == "metadata_text")
            .count(),
        2
    );
    assert_eq!(
        canonical(&parse(&canonical(&doc)).unwrap()),
        canonical(&doc)
    );
}

#[test]
fn unicode_line_separators_are_not_author_names() {
    fails(r#"!meta authors=["A\u2028B"]"#, "metadata_author");
    fails(r#"!meta authors=["A\u2029B"]"#, "metadata_author");
}
