//! Authored producer claims, never adapter capabilities, resource locations or playback policy.
use crate::{Diagnostic, Document, DocumentKind, Mark, Rational, Record, Scalar, SourceRange};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
pub const EXTENSION: &str = "scorestream.provenance@1";
pub const HEADER: &str = "scorestream.provenance";
pub const ENTRY: &str = "scorestream.provenance.entry";
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Provenance {
    /// None is absent; Some(empty) preserves the explicit empty encoding container.
    pub encoding: Option<Vec<EncodingEntry>>,
    /// Only an explicitly empty miscellaneous container is currently represented.
    pub miscellaneous_empty: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum EncodingEntry {
    Software {
        text: String,
    },
    EncodingDate {
        date: String,
    },
    Supports {
        supported: bool,
        element: String,
        attribute: Option<String>,
        value: Option<String>,
    },
}
fn text_valid(s: &str) -> bool {
    s.chars().all(|c|matches!(c,'\t'|'\n'|'\r'|'\u{20}'..='\u{d7ff}'|'\u{e000}'..='\u{fffd}'|'\u{10000}'..='\u{10ffff}'))
}
pub fn token(s: &str) -> String {
    s.split([' ', '\t', '\r', '\n'])
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}
/// XML Schema date without timezone. Keep the author's spelling, not the local date.
pub fn date_valid(s: &str) -> bool {
    let s = s.trim_matches([' ', '\t', '\r', '\n']);
    let mut parts = s.rsplitn(3, '-');
    let (Some(day), Some(month), Some(year)) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    let year = year.strip_prefix('-').unwrap_or(year);
    if year.len() < 4
        || year.len() > 4 && year.starts_with('0')
        || !year.bytes().all(|b| b.is_ascii_digit())
        || year.bytes().all(|b| b == b'0')
        || day.len() != 2
        || month.len() != 2
        || !day.bytes().chain(month.bytes()).all(|b| b.is_ascii_digit())
    {
        return false;
    }
    let modulo = year
        .bytes()
        .fold(0u32, |n, b| (n * 10 + u32::from(b - b'0')) % 400);
    let leap = modulo % 4 == 0 && (modulo % 100 != 0 || modulo == 0);
    let month = month.parse::<u32>().unwrap();
    let day = day.parse::<u32>().unwrap();
    let max = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if leap {
                29
            } else {
                28
            }
        }
        _ => 0,
    };
    day > 0 && day <= max
}
impl EncodingEntry {
    pub fn valid(&self) -> bool {
        match self {
            Self::Software { text } => text_valid(text),
            Self::EncodingDate { date } => date_valid(date),
            Self::Supports {
                element,
                attribute,
                value,
                ..
            } => {
                crate::lyrics::verse_valid(&token(element))
                    && attribute
                        .as_ref()
                        .is_none_or(|s| crate::lyrics::verse_valid(&token(s)))
                    && value
                        .as_ref()
                        .is_none_or(|s| attribute.is_some() && text_valid(s))
            }
        }
    }
}
impl Provenance {
    pub fn valid(&self) -> bool {
        (self.encoding.is_some() || self.miscellaneous_empty)
            && self.encoding.iter().flatten().all(EncodingEntry::valid)
    }
}
pub fn is_kind(s: &str) -> bool {
    s == HEADER || s.starts_with("scorestream.provenance.")
}
pub fn encode(value: &Provenance) -> Vec<Mark> {
    let mark = |kind: &str, attributes| Mark {
        target: "score".into(),
        kind: kind.into(),
        attributes,
        source: SourceRange::default(),
    };
    let mut rows = vec![mark(
        HEADER,
        BTreeMap::from([
            (
                "encoding_present".into(),
                Scalar::Boolean(value.encoding.is_some()),
            ),
            (
                "miscellaneous_empty".into(),
                Scalar::Boolean(value.miscellaneous_empty),
            ),
        ]),
    )];
    for (index, entry) in value.encoding.iter().flatten().enumerate() {
        let mut a = BTreeMap::from([(
            "index".into(),
            Scalar::Integer(i64::try_from(index).expect("encodable vector index")),
        )]);
        let mut put = |key: &str, text: &str| {
            a.insert(key.into(), Scalar::String(text.into()));
        };
        match entry {
            EncodingEntry::Software { text } => {
                put("kind", "software");
                put("text", text);
            }
            EncodingEntry::EncodingDate { date } => {
                put("kind", "encoding_date");
                put("date", date);
            }
            EncodingEntry::Supports {
                supported,
                element,
                attribute,
                value,
            } => {
                put("kind", "supports");
                put("element", element);
                if let Some(s) = attribute {
                    put("attribute", s);
                }
                if let Some(s) = value {
                    put("value", s);
                }
                a.insert("supported".into(), Scalar::Boolean(*supported));
            }
        }
        rows.push(mark(ENTRY, a));
    }
    rows
}
pub fn decode(doc: &Document) -> Result<Option<Provenance>, Vec<Diagnostic>> {
    let fail = || {
        vec![Diagnostic::error(
            "provenance.wire",
            "Expected one score-start provenance header and ordered typed producer claims; no capabilities are inferred",
            doc.source,
        )]
    };
    let enabled = doc.required_extensions.iter().any(|s| s == EXTENSION);
    let mut header = None;
    let mut entries = BTreeMap::new();
    for f in &doc.frames {
        for r in &f.records {
            let Record::Mark(m) = r else { continue };
            if !is_kind(&m.kind) {
                continue;
            }
            if !enabled
                || doc.document_kind != DocumentKind::Score
                || f.time != Rational::zero()
                || m.target != "score"
            {
                return Err(fail());
            }
            let text = |key: &str| match m.attributes.get(key) {
                None => Ok(None),
                Some(Scalar::String(s)) => Ok(Some(s.clone())),
                _ => Err(fail()),
            };
            if m.kind == HEADER {
                if header.is_some() || m.attributes.len() != 2 {
                    return Err(fail());
                }
                let present = m
                    .attributes
                    .get("encoding_present")
                    .and_then(Scalar::as_bool)
                    .ok_or_else(fail)?;
                let miscellaneous_empty = m
                    .attributes
                    .get("miscellaneous_empty")
                    .and_then(Scalar::as_bool)
                    .ok_or_else(fail)?;
                header = Some(Provenance {
                    encoding: present.then(Vec::new),
                    miscellaneous_empty,
                });
                continue;
            }
            if m.kind != ENTRY {
                return Err(fail());
            }
            let index = m
                .attributes
                .get("index")
                .and_then(Scalar::as_i64)
                .filter(|i| *i >= 0)
                .ok_or_else(fail)?;
            let kind = text("kind")?.ok_or_else(fail)?;
            let (entry, allowed) = match kind.as_str() {
                "software" => (
                    EncodingEntry::Software {
                        text: text("text")?.ok_or_else(fail)?,
                    },
                    vec!["index", "kind", "text"],
                ),
                "encoding_date" => (
                    EncodingEntry::EncodingDate {
                        date: text("date")?.ok_or_else(fail)?,
                    },
                    vec!["index", "kind", "date"],
                ),
                "supports" => (
                    EncodingEntry::Supports {
                        supported: m
                            .attributes
                            .get("supported")
                            .and_then(Scalar::as_bool)
                            .ok_or_else(fail)?,
                        element: text("element")?.ok_or_else(fail)?,
                        attribute: text("attribute")?,
                        value: text("value")?,
                    },
                    vec![
                        "index",
                        "kind",
                        "supported",
                        "element",
                        "attribute",
                        "value",
                    ],
                ),
                _ => return Err(fail()),
            };
            if m.attributes.keys().any(|s| !allowed.contains(&s.as_str()))
                || !entry.valid()
                || entries.insert(index, entry).is_some()
            {
                return Err(fail());
            }
        }
    }
    if !enabled && header.is_none() {
        return Ok(None);
    }
    let mut value = header.ok_or_else(fail)?;
    if entries.keys().enumerate().any(|(i, k)| i as i64 != *k)
        || value.encoding.is_none() && !entries.is_empty()
    {
        return Err(fail());
    }
    if let Some(list) = &mut value.encoding {
        *list = entries.into_values().collect();
    }
    if !value.valid() {
        return Err(fail());
    }
    Ok(Some(value))
}
