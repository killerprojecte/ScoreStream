//! Closed authored MusicXML print layout; independent of performance and reflow.
use crate::{Diagnostic, Document, DocumentKind, Mark, Rational, Record, Scalar, SourceRange};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
pub const EXTENSION: &str = "scorestream.print@1";
pub const KIND: &str = "scorestream.print";
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrintLayout {
    pub part: String,
    pub measure: String,
    pub new_page: Option<bool>,
    pub page_number: Option<String>,
    pub layout: Layout,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Layout {
    None,
    System {
        left_margin: Rational,
        right_margin: Rational,
        top_system_distance: Rational,
    },
    Staff {
        number: u32,
        distance: Rational,
    },
}
pub fn is_kind(kind: &str) -> bool {
    kind == KIND || kind.starts_with("scorestream.print.")
}
pub fn valid_fields(p: &PrintLayout) -> bool {
    if p.page_number.as_ref().is_some_and(|s|s.chars().any(|c|!matches!(c, '\t' | '\n' | '\r' | '\u{20}'..='\u{d7ff}' | '\u{e000}'..='\u{fffd}' | '\u{10000}'..='\u{10ffff}'))) { return false; }
    let numbers = match &p.layout {
        Layout::None => vec![],
        Layout::System {
            left_margin,
            right_margin,
            top_system_distance,
        } => vec![left_margin, right_margin, top_system_distance],
        Layout::Staff { number, distance } => {
            if *number == 0 {
                return false;
            }
            vec![distance]
        }
    };
    numbers.into_iter().all(crate::geometry::decimal)
}
pub fn encode(p: &PrintLayout, part: String, measure: String) -> Mark {
    let mut attributes = BTreeMap::from([("part".into(), Scalar::String(part))]);
    if let Some(v) = p.new_page {
        attributes.insert("new_page".into(), Scalar::Boolean(v));
    }
    if let Some(v) = &p.page_number {
        attributes.insert("page_number".into(), Scalar::String(v.clone()));
    }
    let kind = match &p.layout {
        Layout::None => "none",
        Layout::System {
            left_margin,
            right_margin,
            top_system_distance,
        } => {
            for (k, v) in [
                ("left_margin", left_margin),
                ("right_margin", right_margin),
                ("top_system_distance", top_system_distance),
            ] {
                attributes.insert(k.into(), Scalar::String(v.to_string()));
            }
            "system"
        }
        Layout::Staff { number, distance } => {
            attributes.insert("staff".into(), Scalar::Integer(i64::from(*number)));
            attributes.insert(
                "staff_distance".into(),
                Scalar::String(distance.to_string()),
            );
            "staff"
        }
    };
    attributes.insert("layout".into(), Scalar::String(kind.into()));
    Mark {
        target: format!("#{measure}"),
        kind: KIND.into(),
        attributes,
        source: SourceRange::default(),
    }
}
pub fn decode(doc: &Document) -> Result<Vec<PrintLayout>, Vec<Diagnostic>> {
    let error = |message: &str, source| vec![Diagnostic::error("print.wire", message, source)];
    let enabled = doc.required_extensions.iter().any(|e| e == EXTENSION);
    let mut measures = BTreeMap::new();
    let staves: BTreeMap<_, _> = doc
        .parts
        .iter()
        .map(|p| (p.id.as_str(), p.staves))
        .collect();
    for frame in &doc.frames {
        for record in &frame.records {
            if let Record::Measure(m) = record {
                measures.insert(m.id.as_str(), (&frame.time, m.target.as_str()));
            }
        }
    }
    let mut result = vec![];
    let mut seen = BTreeSet::new();
    for frame in &doc.frames {
        for record in &frame.records {
            let Record::Mark(m) = record else { continue };
            if !is_kind(&m.kind) {
                continue;
            }
            if !enabled || doc.document_kind != DocumentKind::Score || m.kind != KIND {
                return Err(error(
                    "Print layout requires a complete score and known scorestream.print@1 record",
                    m.source,
                ));
            }
            let fail = || error("Invalid print fields, ownership, or onset", m.source);
            let a = &m.attributes;
            let part = a.get("part").and_then(Scalar::as_str).ok_or_else(fail)?;
            let measure = m.target.strip_prefix('#').ok_or_else(fail)?;
            if measures.get(measure) != Some(&(&frame.time, part)) || !seen.insert(measure) {
                return Err(fail());
            }
            let decimal = |key: &str| -> Result<Rational, Vec<Diagnostic>> {
                let r = a
                    .get(key)
                    .and_then(Scalar::as_str)
                    .and_then(|s| s.parse::<Rational>().ok())
                    .ok_or_else(fail)?;
                if !crate::geometry::decimal(&r) {
                    return Err(fail());
                }
                Ok(r)
            };
            let mut allowed = vec!["part", "layout", "new_page", "page_number"];
            let layout = match a.get("layout").and_then(Scalar::as_str) {
                Some("none") => Layout::None,
                Some("system") => {
                    allowed.extend(["left_margin", "right_margin", "top_system_distance"]);
                    Layout::System {
                        left_margin: decimal("left_margin")?,
                        right_margin: decimal("right_margin")?,
                        top_system_distance: decimal("top_system_distance")?,
                    }
                }
                Some("staff") => {
                    allowed.extend(["staff", "staff_distance"]);
                    let number = a
                        .get("staff")
                        .and_then(Scalar::as_i64)
                        .and_then(|n| u32::try_from(n).ok())
                        .filter(|n| *n > 0)
                        .ok_or_else(fail)?;
                    Layout::Staff {
                        number,
                        distance: decimal("staff_distance")?,
                    }
                }
                _ => return Err(fail()),
            };
            if a.keys().any(|k| !allowed.contains(&k.as_str())) {
                return Err(fail());
            }
            let new_page = a
                .get("new_page")
                .map(|v| v.as_bool().ok_or_else(fail))
                .transpose()?;
            let page_number = a
                .get("page_number")
                .map(|v| v.as_str().map(str::to_string).ok_or_else(fail))
                .transpose()?;
            if let Layout::Staff { number, .. } = &layout {
                if staves.get(part).is_none_or(|n| number > n) {
                    return Err(fail());
                }
            }
            let value = PrintLayout {
                part: part.into(),
                measure: measure.into(),
                new_page,
                page_number,
                layout,
            };
            if !valid_fields(&value) {
                return Err(fail());
            }
            result.push(value);
        }
    }
    if enabled && result.is_empty() {
        return Err(error("Print layout extension requires records", doc.source));
    }
    Ok(result)
}
pub fn validate(doc: &Document, out: &mut Vec<Diagnostic>) {
    if let Err(mut errors) = decode(doc) {
        out.append(&mut errors);
    }
}
