//! Select an authoring reference from the canonical language descriptor.
//! The text fallback carries exactly the same selected application result.
use serde_json::{Value, json};
use std::collections::BTreeSet;

const TOPICS: &[&str] = &[
    "syntax",
    "pitch",
    "time",
    "polyphony",
    "state",
    "marks",
    "relations",
    "editing",
    "extensions",
    "validation",
];
const STATUS: &str = "format discovery implemented; desktop/core initial milestone; operational MCP score tools unavailable";
const BASE_FIELDS: &[&str] = &[
    "format",
    "format_version",
    "profile",
    "core_shape",
    "pitch",
    "time",
    "records",
    "lexical",
    "scope_rules",
    "record_order",
    "validation_boundary",
    "mcp_resources",
];
const BASE_RULES: &[&str] = &[
    "S001", "S002", "S003", "S004", "S005", "S006", "S007", "S008", "S009", "S010", "S012", "S016",
];
const BASIC_DEFAULTS: &[&str] = &["omitted_state", "written_type"];
const COMMON_ATTRIBUTES: &[&str] = &["id", "staff", "chord", "w", "dots", "tm"];

struct Topic {
    name: &'static str,
    paths: &'static [&'static str],
    rules: &'static [&'static str],
    examples: &'static [&'static str],
    constraints: &'static [usize],
}
// Array-valued constraints in the canonical descriptor have no IDs. The tests
// below bind each ordinal to its meaning so descriptor edits cannot silently drift.
const SECTIONS: &[Topic] = &[
    Topic {
        name: "syntax",
        paths: &[
            "defaults",
            "record_order",
            "vocabulary/event",
            "vocabulary/metadata",
        ],
        rules: &[],
        examples: &["document-metadata"],
        constraints: &[],
    },
    Topic {
        name: "pitch",
        paths: &[
            "vocabulary/marks/accidental",
            "vocabulary/mark_fields/accidental",
            "vocabulary/common_mark_attributes",
            "vocabulary/state/transpose_chromatic",
            "vocabulary/state/transpose_diatonic",
            "vocabulary/state/transpose_octaves",
        ],
        rules: &[],
        examples: &[],
        constraints: &[0],
    },
    Topic {
        name: "time",
        paths: &[
            "vocabulary/event/w",
            "vocabulary/event/dots",
            "vocabulary/event/tm",
            "vocabulary/event/tm_normal_type",
            "vocabulary/event/tm_normal_dots",
            "vocabulary/measure",
        ],
        rules: &[],
        examples: &["tuplets-and-ties"],
        constraints: &[1, 2],
    },
    Topic {
        name: "polyphony",
        paths: &[
            "vocabulary/event/staff",
            "vocabulary/event/chord",
            "vocabulary/event/kind",
            "vocabulary/event/anchor",
            "vocabulary/event/order",
            "vocabulary/event/slash",
            "defaults/event_staff",
            "defaults/no_voice_row",
        ],
        rules: &[],
        examples: &["grace-percussion-cross-staff"],
        constraints: &[],
    },
    Topic {
        name: "state",
        paths: &["vocabulary/state", "defaults/omitted_state", "record_order"],
        rules: &[],
        examples: &[],
        constraints: &[0, 7],
    },
    Topic {
        name: "marks",
        paths: &[
            "vocabulary/marks",
            "vocabulary/mark_fields",
            "vocabulary/common_mark_attributes",
        ],
        rules: &["S015"],
        examples: &[],
        constraints: &[3, 7, 8, 9, 10, 11],
    },
    Topic {
        name: "relations",
        paths: &["vocabulary/relations"],
        rules: &["S011"],
        examples: &["tuplets-and-ties"],
        constraints: &[3],
    },
    Topic {
        name: "editing",
        paths: &["editing"],
        rules: &["S013", "S014", "S018"],
        examples: &["replacement-fragment"],
        constraints: &[4],
    },
    Topic {
        name: "extensions",
        paths: &["extensions"],
        rules: &[],
        examples: &[],
        constraints: &[5, 6, 8, 9, 10, 11],
    },
    Topic {
        name: "validation",
        paths: &["authoritative_files"],
        rules: &["S011", "S013", "S014", "S015", "S017", "S018"],
        examples: &[],
        constraints: &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
    },
];

fn tool_error(message: &str) -> Value {
    json!({"content":[{"type":"text","text":message}],"isError":true})
}

pub(super) fn result(args: &Value) -> Value {
    let Some(args) = args.as_object() else {
        return tool_error("Arguments must be an object");
    };
    for (key, value) in args {
        let valid = match key.as_str() {
            "format_version" => value == "1.1",
            "profile" => value == "core-measured",
            "detail" => value == "quick" || value == "full",
            "topics" => value.as_array().is_some_and(|topics| {
                topics.len() <= TOPICS.len()
                    && topics
                        .iter()
                        .all(|v| v.as_str().is_some_and(|v| TOPICS.contains(&v)))
                    && topics
                        .iter()
                        .enumerate()
                        .all(|(i, v)| !topics[..i].contains(v))
            }),
            _ => false,
        };
        if !valid {
            return tool_error(&format!("Unsupported or invalid argument: {key}"));
        }
    }
    let language: Value =
        serde_json::from_str(super::LANGUAGE).expect("embedded language descriptor");
    let mut data = if args.get("detail").is_some_and(|v| v == "full") {
        language
    } else {
        let selected: BTreeSet<&str> = args
            .get("topics")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .collect();
        quick(&language, &selected)
    };
    data["status"] = json!(STATUS);
    let selected = json!({"ok":true,"data":data});
    // Compact JSON avoids the previous full fixture plus pretty-printed duplicate.
    // Clients without structuredContent still receive all selected syntax/examples.
    let text = serde_json::to_string(&selected).expect("JSON reference is serializable");
    json!({"content":[{"type":"text","text":text}],"structuredContent":selected,"isError":false})
}

fn quick(language: &Value, selected: &BTreeSet<&str>) -> Value {
    let mut data = json!({});
    for field in BASE_FIELDS {
        data[*field] = language[*field].clone();
    }
    data["defaults"] = json!({});
    for field in BASIC_DEFAULTS {
        data["defaults"][*field] = language["defaults"][*field].clone();
    }
    data["common_attributes"] = json!({});
    for attribute in COMMON_ATTRIBUTES {
        data["common_attributes"][*attribute] = language["vocabulary"]["event"][*attribute].clone();
    }
    let mut rules: BTreeSet<&str> = BASE_RULES.iter().copied().collect();
    let mut examples: BTreeSet<&str> = ["ensemble"].into_iter().collect();
    let mut constraints = BTreeSet::new();
    for topic in SECTIONS
        .iter()
        .filter(|topic| selected.contains(topic.name))
    {
        for path in topic.paths {
            copy_path(&mut data, language, &path.split('/').collect::<Vec<_>>());
        }
        rules.extend(topic.rules.iter().copied());
        examples.extend(topic.examples.iter().copied());
        constraints.extend(topic.constraints.iter().copied());
    }
    data["semantic_rules"] = filter_named(&language["semantic_rules"], "id", &rules);
    data["examples"] = filter_named(&language["examples"], "name", &examples);
    if !constraints.is_empty() {
        data["additional_constraints"] = Value::Array(
            constraints
                .into_iter()
                .map(|index| language["additional_constraints"][index].clone())
                .collect(),
        );
    }
    data["reference_scope"] = json!({
        "detail":"quick",
        "topics": TOPICS.iter().filter(|topic| selected.contains(**topic)).collect::<Vec<_>>(),
        "available_topics":TOPICS,
        "more":"Call score_get_format with detail=full for the complete descriptor, or topics=[...] to extend this authoring baseline. Full grammar and AST schema are linked in mcp_resources."
    });
    data
}
fn copy_path(target: &mut Value, source: &Value, path: &[&str]) {
    let (key, rest) = path.split_first().expect("nonempty static topic path");
    if rest.is_empty() {
        target[*key] = source[*key].clone();
    } else {
        if target.get(*key).is_none() {
            target[*key] = json!({});
        }
        copy_path(&mut target[*key], &source[*key], rest);
    }
}
fn filter_named(values: &Value, key: &str, names: &BTreeSet<&str>) -> Value {
    Value::Array(
        values
            .as_array()
            .expect("canonical reference array")
            .iter()
            .filter(|item| item[key].as_str().is_some_and(|name| names.contains(name)))
            .cloned()
            .collect(),
    )
}

#[cfg(test)]
mod tests;
