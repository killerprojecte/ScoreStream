use scorestream_codec::*;
use serde_json::json;

fn source(label: &str) -> String {
    format!(
        "!score 1.1 unit=q\n!part p instrument=flute staves=1{label}\n!voice v part=p staff=1\n@0\n| p m len=1\nv C4:1 id=n\n.\n!end at=1\n"
    )
}

#[test]
fn legacy_parts_omit_optional_display_name() {
    let document = parse(&source("")).unwrap();
    assert_eq!(document.parts[0].display_name, None);
    assert!(
        !serde_json::to_value(&document).unwrap()["parts"][0]
            .as_object()
            .unwrap()
            .contains_key("display_name")
    );
    assert_eq!(canonical(&document), source(""));
    let restored: Document =
        serde_json::from_value(serde_json::to_value(&document).unwrap()).unwrap();
    assert_eq!(canonical(&restored), source(""));
}

#[test]
fn display_name_is_plain_exact_text_and_canonical_roundtrips() {
    for name in [
        "",
        "Flute 1",
        "长笛 2 & \"solo\" <label>",
        "  line\nwith\ttabs\rand spaces  ",
    ] {
        let input = source(&format!(
            " display_name={}",
            serde_json::to_string(name).unwrap()
        ));
        let document = parse(&input).unwrap();
        assert_eq!(document.parts[0].display_name.as_deref(), Some(name));
        assert_eq!(document.parts[0].instrument, "flute");
        let text = canonical(&document);
        assert_eq!(canonical(&parse(&text).unwrap()), text);
        let restored: Document =
            serde_json::from_value(serde_json::to_value(&document).unwrap()).unwrap();
        assert_eq!(restored.parts[0].display_name.as_deref(), Some(name));
        assert!(validate(&restored, None).is_empty());
    }
}

#[test]
fn labels_require_strings_and_do_not_open_other_declaration_fields() {
    for field in [
        "display_name=Flute",
        "display_name=1",
        "display_name=null",
        "display_name=true",
        "display_name=[]",
        "display_name={}",
        r#"display_name="A" display_name="B""#,
        r#"display_name="A" invented="B""#,
        r#"display_name="\u0000""#,
        r#"display_name="\ufffe""#,
    ] {
        assert!(parse(&source(&format!(" {field}"))).is_err(), "{field}");
    }
}

#[test]
fn xml_legal_del_and_c1_labels_canonicalize_to_escaped_records() {
    for value in 0x7f..=0x9f {
        let encoded = format!(" display_name=\"a\\u{value:04x}b\"");
        let document = parse(&source(&encoded)).unwrap();
        let canonical = canonical(&document);
        assert!(canonical.contains(&encoded));
        assert_eq!(
            parse(&canonical).unwrap().parts[0].display_name,
            document.parts[0].display_name
        );
    }
}

#[test]
fn json_and_constructed_ast_have_the_same_typed_boundary() {
    let document = parse(&source("")).unwrap();
    for invalid in [json!(null), json!(4), json!(false), json!([]), json!({})] {
        let mut value = serde_json::to_value(&document).unwrap();
        value["parts"][0]["display_name"] = invalid;
        assert!(serde_json::from_value::<Document>(value).is_err());
    }
    let mut invalid = document;
    invalid.parts[0].display_name = Some("bad\0label".into());
    assert!(
        validate(&invalid, None)
            .iter()
            .any(|d| d.code == "part_display_name")
    );
}
