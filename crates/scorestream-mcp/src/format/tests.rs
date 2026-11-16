use super::*;
fn data(args: Value) -> Value {
    result(&args)["structuredContent"]["data"].clone()
}
fn language() -> Value {
    serde_json::from_str(super::super::LANGUAGE).unwrap()
}

#[test]
fn defaults_are_compact_and_keep_the_authoring_contract() {
    let default = result(&json!({}));
    for args in [
        json!({"detail":"quick"}),
        json!({"topics":[]}),
        json!({"detail":"quick","topics":[]}),
        json!({"format_version":"1.1","profile":"core-measured"}),
    ] {
        assert_eq!(result(&args), default);
    }
    let data = &default["structuredContent"]["data"];
    for field in [
        "format",
        "format_version",
        "profile",
        "records",
        "semantic_rules",
        "examples",
        "validation_boundary",
        "mcp_resources",
        "common_attributes",
        "core_shape",
        "pitch",
        "time",
    ] {
        assert!(!data[field].is_null(), "missing {field}");
    }
    assert_eq!(data["records"], language()["records"]);
    assert_eq!(data["examples"].as_array().unwrap().len(), 1);
    assert!(
        data["examples"][0]["source"]
            .as_str()
            .unwrap()
            .contains("!end at=4")
    );
    assert!(data.get("vocabulary").is_none());
    assert!(data.get("editing").is_none());
    assert!(serde_json::to_vec(&default).unwrap().len() < 18_000);
    assert_eq!(
        data["mcp_resources"]["language"],
        "scorestream://spec/1.1/language"
    );
}
#[test]
fn full_preserves_canonical_descriptor_and_wins_over_topics() {
    let mut expected = language();
    expected["status"] = json!(STATUS);
    assert_eq!(data(json!({"detail":"full"})), expected);
    assert_eq!(
        result(&json!({"detail":"full","topics":["pitch"]})),
        result(&json!({"detail":"full"}))
    );
    assert!(
        result(&json!({"detail":"full","topics":["unknown"]}))["isError"]
            .as_bool()
            .unwrap()
    );
}
#[test]
fn topics_add_only_mapped_detail_with_baseline_intact() {
    let source = language();
    let base = data(json!({}));
    assert_eq!(
        TOPICS,
        SECTIONS
            .iter()
            .map(|topic| topic.name)
            .collect::<Vec<_>>()
            .as_slice()
    );
    for topic in SECTIONS {
        let selected = data(json!({"topics":[topic.name]}));
        for field in BASE_FIELDS {
            assert_eq!(
                selected[*field], base[*field],
                "{} changed {field}",
                topic.name
            );
        }
        for path in topic.paths {
            let pointer = format!("/{path}");
            let value = source
                .pointer(&pointer)
                .expect("topic path must exist in descriptor");
            assert!(!value.is_null());
            assert_eq!(
                selected.pointer(&pointer),
                Some(value),
                "{} missing {path}",
                topic.name
            );
        }
        assert_eq!(selected["reference_scope"]["topics"], json!([topic.name]));
        assert!(
            selected["examples"]
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["name"] == "ensemble")
        );
        assert!(
            serde_json::to_vec(&result(&json!({"topics":[topic.name]})))
                .unwrap()
                .len()
                < serde_json::to_vec(&result(&json!({"detail":"full"})))
                    .unwrap()
                    .len()
        );
    }
    let marks = data(json!({"topics":["marks"]}));
    assert!(marks["vocabulary"].get("relations").is_none());
    assert!(marks["vocabulary"].get("event").is_none());
    let editing = data(json!({"topics":["editing"]}));
    assert!(editing.get("vocabulary").is_none());
    assert_eq!(editing["examples"].as_array().unwrap().len(), 2);
    assert!(
        editing["examples"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["name"] == "replacement-fragment")
    );
    let pitch = data(json!({"topics":["pitch"]}));
    assert!(pitch["vocabulary"]["mark_fields"].get("lyric").is_none());
}
#[test]
fn topic_order_is_irrelevant_and_shared_arrays_are_deduplicated() {
    let forward = json!({"topics":["time","relations","state","pitch","editing"]});
    let reverse = json!({"topics":["editing","pitch","state","relations","time"]});
    assert_eq!(result(&forward), result(&reverse));
    let data = data(forward);
    for (field, key) in [("semantic_rules", "id"), ("examples", "name")] {
        let values = data[field].as_array().unwrap();
        let names: BTreeSet<_> = values.iter().map(|v| v[key].as_str().unwrap()).collect();
        assert_eq!(names.len(), values.len());
    }
    let constraints = data["additional_constraints"].as_array().unwrap();
    let unique: BTreeSet<_> = constraints.iter().map(|v| v.as_str().unwrap()).collect();
    assert_eq!(unique.len(), constraints.len());
}
#[test]
fn text_fallback_is_exactly_the_selected_structured_result() {
    let mut cases = vec![
        json!({}),
        json!({"detail":"full"}),
        json!({"topics":TOPICS}),
    ];
    cases.extend(TOPICS.iter().map(|topic| json!({"topics":[topic]})));
    for args in cases {
        let response = result(&args);
        let text = response["content"][0]["text"].as_str().unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(text).unwrap(),
            response["structuredContent"]
        );
        assert!(!text.contains("Full machine-readable format contract:"));
        assert!(!text.contains("# ScoreStream 1.1"));
    }
}
#[test]
fn invalid_arguments_remain_actionable_tool_errors() {
    for args in [
        json!(null),
        json!([]),
        json!(true),
        json!({"detail":null}),
        json!({"detail":"verbose"}),
        json!({"topics":null}),
        json!({"topics":"time"}),
        json!({"topics":[null]}),
        json!({"topics":["time","time"]}),
        json!({"topics":["unknown"]}),
        json!({"format_version":"2.0"}),
        json!({"profile":"free-time"}),
        json!({"unexpected":1}),
    ] {
        let response = result(&args);
        assert_eq!(response["isError"], true, "{args}");
        assert!(response.get("structuredContent").is_none());
        assert!(!response["content"][0]["text"].as_str().unwrap().is_empty());
    }
}
#[test]
fn ordinal_constraint_mapping_tracks_canonical_meanings() {
    let source = language();
    let constraints = source["additional_constraints"].as_array().unwrap();
    let prefixes = [
        "Transposition assignments",
        "tm_normal_type",
        "Measure targets",
        "Event-attached marks",
        "Fragments carry",
        "Unknown extension interpretation",
        "scorestream.barline requires",
        "sound_dynamics is",
        "scorestream.fermata requires",
        "scorestream.beam requires",
        "scorestream.articulation@1 preserves",
    ];
    assert_eq!(constraints.len(), prefixes.len());
    for (value, prefix) in constraints.iter().zip(prefixes) {
        assert!(value.as_str().unwrap().starts_with(prefix));
    }
}

#[test]
fn validation_and_all_topics_include_every_canonical_constraint() {
    let source = language();
    for args in [json!({"topics":["validation"]}), json!({"topics":TOPICS})] {
        assert_eq!(
            data(args)["additional_constraints"],
            source["additional_constraints"]
        );
    }
}

#[test]
fn authored_articulation_topics_include_the_canonical_contract() {
    let source = language();
    let articulation = source["additional_constraints"]
        .as_array()
        .unwrap()
        .iter()
        .find(|value| {
            value
                .as_str()
                .unwrap()
                .starts_with("scorestream.articulation@1 preserves")
        })
        .expect("canonical authored articulation constraint");
    for topic in ["marks", "extensions", "validation"] {
        assert!(
            data(json!({"topics":[topic]}))["additional_constraints"]
                .as_array()
                .unwrap()
                .contains(articulation),
            "{topic} must expose the authored articulation contract"
        );
    }
    for topic in [
        "syntax",
        "pitch",
        "time",
        "polyphony",
        "state",
        "relations",
        "editing",
    ] {
        assert!(
            !data(json!({"topics":[topic]}))["additional_constraints"]
                .as_array()
                .is_some_and(|constraints| constraints.contains(articulation)),
            "{topic} must remain focused"
        );
    }
    let marks = data(json!({"topics":["marks"]}));
    let extensions = data(json!({"topics":["extensions"]}));
    let registered = &extensions["extensions"]["registered"]["scorestream.articulation@1"];
    assert_eq!(
        registered,
        &source["extensions"]["registered"]["scorestream.articulation@1"]
    );
    for kind in registered["mark_kinds"].as_array().unwrap() {
        let kind = kind.as_str().unwrap();
        for field in ["marks", "mark_fields"] {
            assert_eq!(
                marks["vocabulary"][field][kind],
                source["vocabulary"][field][kind]
            );
            assert!(!marks["vocabulary"][field][kind].is_null());
        }
    }
}

#[test]
fn every_selector_resolves_against_the_canonical_descriptor() {
    let source = language();
    let rules: BTreeSet<_> = source["semantic_rules"]
        .as_array()
        .unwrap()
        .iter()
        .map(|rule| rule["id"].as_str().unwrap())
        .collect();
    let examples: BTreeSet<_> = source["examples"]
        .as_array()
        .unwrap()
        .iter()
        .map(|example| example["name"].as_str().unwrap())
        .collect();
    for rule in BASE_RULES {
        assert!(rules.contains(rule));
    }
    assert!(examples.contains("ensemble"));
    for attribute in COMMON_ATTRIBUTES {
        assert!(source["vocabulary"]["event"].get(*attribute).is_some());
    }
    for field in BASIC_DEFAULTS {
        assert!(source["defaults"].get(*field).is_some());
    }
    for topic in SECTIONS {
        for rule in topic.rules {
            assert!(rules.contains(rule), "unknown rule {rule}");
        }
        for example in topic.examples {
            assert!(examples.contains(example), "unknown example {example}");
        }
        for index in topic.constraints {
            assert!(source["additional_constraints"].get(*index).is_some());
        }
    }
}

#[test]
fn advertised_input_schema_matches_runtime_topic_selection() {
    let schema: Value = serde_json::from_str(super::super::INPUT).unwrap();
    assert_eq!(
        schema["properties"]["topics"]["items"]["enum"],
        json!(TOPICS)
    );
    assert_eq!(schema["properties"]["detail"]["default"], "quick");
    assert_eq!(
        schema["properties"]["detail"]["enum"],
        json!(["quick", "full"])
    );
}

#[test]
fn syntax_topic_teaches_typed_metadata_without_polluting_other_topics() {
    let selected = data(json!({"topics":["syntax"]}));
    assert_eq!(
        selected["vocabulary"]["metadata"]["authors"]["type"],
        "string_list"
    );
    for name in ["title", "description", "header", "footer"] {
        assert_eq!(selected["vocabulary"]["metadata"][name]["type"], "string");
    }
    assert!(
        selected["examples"]
            .as_array()
            .unwrap()
            .iter()
            .any(|example| example["name"] == "document-metadata")
    );
    assert!(
        data(json!({"topics":["pitch"]}))["vocabulary"]
            .get("metadata")
            .is_none()
    );
    let base = data(json!({}));
    assert!(
        base["records"]
            .as_array()
            .unwrap()
            .iter()
            .any(|record| record["prefix"] == "!meta"
                && record["syntax"].as_str().unwrap().contains("authors=["))
    );
}
