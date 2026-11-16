//! Version-pinned vocabulary is read from the bundled normative descriptor.
//! This keeps enum values and required fields in one source of truth.
use crate::*;
use serde_json::Value;
use std::sync::OnceLock;
fn registry() -> &'static Value {
    static DATA: OnceLock<Value> = OnceLock::new();
    DATA.get_or_init(|| {
        serde_json::from_str(include_str!(
            "../../../design/spec/scorestream-language.json"
        ))
        .expect("bundled language descriptor is valid JSON")
    })
}
fn field(category: &str, kind: &str, key: &str) -> &'static Value {
    let v = &registry()["vocabulary"];
    match category {
        "mark" if key == "id" => &v["common_mark_attributes"][key],
        "mark" => &v["mark_fields"][kind]["fields"][key],
        "relation" => &v["relations"][kind]["fields"][key],
        _ => &v[category][key],
    }
}
pub fn attribute_type(category: &str, kind: &str, key: &str) -> Option<&'static str> {
    if category == "state" {
        return match key {
            "tempo" => Some("tempo"),
            "sound_dynamics" => Some("nonnegative_rational"),
            "meter" => Some("meter"),
            "meter_symbol" => Some("time_symbol"),
            "key" => Some("key"),
            "clef" => Some("clef"),
            "staff" => Some("positive_integer"),
            "instrument" => Some("atom"),
            "transpose_chromatic" => Some("rational"),
            "transpose_diatonic" | "transpose_octaves" => Some("integer"),
            _ => None,
        };
    }
    field(category, kind, key)["type"].as_str()
}
pub fn is_text_attribute(category: &str, kind: &str, key: &str) -> bool {
    attribute_type(category, kind, key) == Some("string")
}
pub(crate) fn validate_attributes(
    category: &str,
    kind: &str,
    attrs: &Attributes,
    source: SourceRange,
    out: &mut Vec<Diagnostic>,
) {
    let v = &registry()["vocabulary"];
    if (category == "mark" && v["mark_fields"][kind].is_null())
        || (category == "relation" && v["relations"][kind].is_null())
    {
        out.push(Diagnostic::error(
            "kind_unsupported",
            format!("Unsupported {category} kind: {kind}"),
            source,
        ));
    }
    for (key, scalar) in attrs {
        let Some(ty) = attribute_type(category, kind, key) else {
            out.push(Diagnostic::error(
                if key.starts_with("x.") {
                    "extension_unsupported"
                } else {
                    "field_unknown"
                },
                format!("Unknown {category} field: {key}"),
                source,
            ));
            continue;
        };
        let text = scalar.as_str();
        let integer = scalar.as_i64();
        let valid = match ty {
            "string" => text.is_some(),
            "boolean" => scalar.as_bool().is_some(),
            "integer" => integer.is_some(),
            "positive_integer" => integer.is_some_and(|n| n > 0),
            "nonnegative_integer" => integer.is_some_and(|n| n >= 0),
            "nonzero_integer" => integer.is_some_and(|n| n != 0),
            "identifier" => text.is_some_and(is_identifier),
            "event_reference" => text
                .and_then(|s| s.strip_prefix('#'))
                .is_some_and(is_identifier),
            "rational" | "positive_rational" | "nonnegative_rational" => text
                .and_then(|s| s.parse::<Rational>().ok())
                .is_some_and(|r| {
                    (ty != "positive_rational" || r.is_positive())
                        && (ty != "nonnegative_rational" || !r.is_negative())
                }),
            "positive_integer:positive_integer" => text.is_some_and(valid_ratio),
            "spelled_pitch_class" => text.is_some_and(|s| {
                matches!(format!("{s}0").parse::<Pitch>(), Ok(Pitch::Pitched { .. }))
            }),
            "tempo" => text.is_some_and(|s| {
                s.split_once(':').is_some_and(|(beat, bpm)| {
                    valid_beat(beat) && bpm.parse::<Rational>().is_ok_and(|n| n.is_positive())
                })
            }),
            "time_symbol" => text.is_some_and(|s|crate::score::TimeSymbol::from_name(s).is_some()),
            "meter" => text.is_some_and(|s| {
                s == "none"
                    || s.split_once('/').is_some_and(|(n, d)| {
                        positive_integer(d) && n.split('+').all(positive_integer)
                    })
            }),
            "key" => integer.is_some() || text == Some("unset"),
            "clef" => text.is_some_and(|s| {
                [
                    "G1",
                    "G2",
                    "F3",
                    "F4",
                    "F5",
                    "C1",
                    "C2",
                    "C3",
                    "C4",
                    "C5",
                    "percussion",
                    "TAB",
                ]
                .contains(&s)
            }),
            "atom" => text.is_some_and(|s| {
                !s.is_empty()
                    && !s
                        .chars()
                        .any(|c| c.is_whitespace() || c.is_control() || c == '"')
            }),
            "enum" | "note_type" | "registered_string" | "beat_unit" => text.is_some_and(|s| {
                field(category, kind, key)["values"]
                    .as_array()
                    .is_some_and(|values| values.iter().any(|v| v.as_str() == Some(s)))
            }),
            _ => false,
        };
        if !valid {
            let code = match key.as_str() {
                "dots" | "tm_normal_dots" => "dots_negative",
                "tm" => "time_modification",
                "order" => "grace_order",
                _ => "field_value",
            };
            out.push(Diagnostic::error(
                code,
                format!("Invalid {key}: expected {ty}"),
                source,
            ));
        }
        if category == "relation"
            && kind == "pedal"
            && key == "value"
            && text
                .and_then(|s| s.parse::<Rational>().ok())
                .is_some_and(|r| r.is_negative() || r > Rational::one())
        {
            out.push(Diagnostic::error(
                "pedal_range",
                "Pedal value must be between 0 and 1",
                source,
            ));
        }
    }
    if category == "mark" {
        if let Some(required) = v["mark_fields"][kind]["required"].as_array() {
            for key in required.iter().filter_map(Value::as_str) {
                if !attrs.contains_key(key) {
                    out.push(Diagnostic::error(
                        "field_required",
                        format!("{kind} requires {key}"),
                        source,
                    ));
                }
            }
        }
    }
}
pub(crate) fn required_relation_fields(kind: &str) -> Vec<&'static str> {
    registry()["vocabulary"]["relations"][kind]["required_on_start"]
        .as_array()
        .map(|a| a.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default()
}
pub(crate) fn positive_integer(s: &str) -> bool {
    !s.is_empty() && !s.starts_with('0') && s.bytes().all(|b| b.is_ascii_digit())
}
pub(crate) fn valid_ratio(s: &str) -> bool {
    s.split_once(':')
        .is_some_and(|(a, b)| positive_integer(a) && positive_integer(b))
}
fn valid_beat(s: &str) -> bool {
    ["w", "h", "q", "e", "s", "w.", "h.", "q.", "e.", "s."].contains(&s)
}
