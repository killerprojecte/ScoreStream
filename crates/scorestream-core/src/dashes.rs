//! Ordered authored dashes endpoints; deliberately not sounding hairpin relations.
use crate::{Diagnostic, Document, DocumentKind, Mark, Rational, Record, Scalar, SourceRange};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
pub const EXTENSION: &str = "scorestream.dashes@1";
pub const KIND: &str = "scorestream.dashes";
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DashesType {
    Start,
    Stop,
}
impl DashesType {
    pub fn name(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Stop => "stop",
        }
    }
    pub fn from_name(s: &str) -> Option<Self> {
        match s {
            "start" => Some(Self::Start),
            "stop" => Some(Self::Stop),
            _ => None,
        }
    }
}
pub type DashesWords = crate::words::WordsText;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DashesEndpoint {
    pub words: Option<DashesWords>,
    pub system: Option<String>,
    pub part: String,
    pub measure: String,
    pub offset: Rational,
    pub staff: Option<u32>,
    pub placement: Option<crate::notation::Placement>,
    pub kind: DashesType,
    pub number: Option<u8>,
    pub default_y: Option<Rational>,
    pub relative_x: Option<Rational>,
}
pub type Hosts = BTreeMap<(String, String), (Rational, Rational, u32)>;
pub fn valid(endpoints: &[DashesEndpoint], hosts: &Hosts) -> bool {
    endpoints.windows(2).all(|pair| {
        let a = &pair[0];
        let b = &pair[1];
        a.part < b.part
            || a.part == b.part
                && hosts
                    .get(&(a.part.clone(), a.measure.clone()))
                    .zip(hosts.get(&(b.part.clone(), b.measure.clone())))
                    .is_some_and(|(a, b)| a.0 <= b.0)
    }) && endpoints.iter().all(|e| {
        hosts
            .get(&(e.part.clone(), e.measure.clone()))
            .is_some_and(|(_, len, staves)| {
                !e.offset.is_negative()
                    && &e.offset <= len
                    && e.staff.is_none_or(|s| s > 0 && s <= *staves)
            })
            && e.system
                .as_deref()
                .is_none_or(|s| matches!(s, "only-top" | "also-top" | "none"))
            && (e.kind == DashesType::Start) == e.words.is_some()
            && e.words.as_ref().is_none_or(|w| {
                w.valid()
                    && w.font_weight.is_none()
                    && w.default_x.is_none()
                    && w.relative_y.is_none()
            })
            && e.number.is_none_or(|n| (1..=16).contains(&n))
            && [&e.default_y, &e.relative_x]
                .into_iter()
                .flatten()
                .all(crate::geometry::decimal)
    })
}
pub fn is_kind(s: &str) -> bool {
    s == KIND || s.starts_with("scorestream.dashes.")
}
pub fn encode(e: &DashesEndpoint, part: String, measure: String, index: usize) -> Mark {
    let mut attributes = BTreeMap::from([
        ("part".into(), Scalar::String(part)),
        ("index".into(), Scalar::Integer(index as i64)),
        ("type".into(), Scalar::String(e.kind.name().into())),
    ]);
    for (key, value) in [("default_y", &e.default_y), ("relative_x", &e.relative_x)] {
        if let Some(value) = value {
            attributes.insert(key.into(), Scalar::String(value.to_string()));
        }
    }
    if let Some(n) = e.number {
        attributes.insert("number".into(), Scalar::Integer(n.into()));
    }
    if let Some(s) = e.staff {
        attributes.insert("staff".into(), Scalar::Integer(s.into()));
    }
    if let Some(p) = e.placement {
        attributes.insert(
            "placement".into(),
            Scalar::String(
                match p {
                    crate::notation::Placement::Above => "above",
                    crate::notation::Placement::Below => "below",
                }
                .into(),
            ),
        );
    }
    if let Some(system) = &e.system {
        attributes.insert("system".into(), Scalar::String(system.clone()));
    }
    if let Some(w) = &e.words {
        attributes.insert("text".into(), Scalar::String(w.text.clone()));
        for (k, v) in [
            ("font_family", w.font_family.clone()),
            ("font_style", w.font_style.clone()),
            ("font_size", w.font_size.as_ref().map(ToString::to_string)),
            (
                "words_default_y",
                w.default_y.as_ref().map(ToString::to_string),
            ),
            (
                "words_relative_x",
                w.relative_x.as_ref().map(ToString::to_string),
            ),
        ] {
            if let Some(v) = v {
                attributes.insert(k.into(), Scalar::String(v));
            }
        }
    }
    Mark {
        target: format!("#{measure}"),
        kind: KIND.into(),
        attributes,
        source: SourceRange::default(),
    }
}
pub fn decode(doc: &Document) -> Result<Vec<DashesEndpoint>, Vec<Diagnostic>> {
    let fail = || {
        vec![Diagnostic::error("dashes.wire","Dashes require declared authored endpoint capability, complete score, exact measure/scope/onset, contiguous unique indices and valid typed fields",doc.source)]
    };
    let enabled = doc.required_extensions.iter().any(|e| e == EXTENSION);
    let mut hosts = Hosts::new();
    let staves: BTreeMap<_, _> = doc
        .parts
        .iter()
        .map(|p| (p.id.as_str(), p.staves))
        .collect();
    for f in &doc.frames {
        for r in &f.records {
            if let Record::Measure(m) = r {
                if let Some(n) = staves.get(m.target.as_str()) {
                    hosts.insert(
                        (m.target.clone(), m.id.clone()),
                        (f.time.clone(), m.length.clone(), *n),
                    );
                }
            }
        }
    }
    let mut endpoints = BTreeMap::new();
    for f in &doc.frames {
        for r in &f.records {
            let Record::Mark(m) = r else { continue };
            if !is_kind(&m.kind) {
                continue;
            }
            if !enabled
                || doc.document_kind != DocumentKind::Score
                || m.kind != KIND
                || m.attributes.keys().any(|k| {
                    ![
                        "part",
                        "index",
                        "type",
                        "number",
                        "staff",
                        "placement",
                        "default_y",
                        "relative_x",
                        "system",
                        "text",
                        "font_family",
                        "font_size",
                        "font_style",
                        "words_default_y",
                        "words_relative_x",
                    ]
                    .contains(&k.as_str())
                })
            {
                return Err(fail());
            }
            let text = |key| {
                m.attributes
                    .get(key)
                    .and_then(Scalar::as_str)
                    .ok_or_else(fail)
            };
            let optional_number = |key| -> Result<Option<Rational>, Vec<Diagnostic>> {
                m.attributes
                    .get(key)
                    .map(|v| v.as_str().ok_or_else(fail)?.parse().map_err(|_| fail()))
                    .transpose()
            };
            let integer = |key| -> Result<Option<i64>, Vec<Diagnostic>> {
                m.attributes
                    .get(key)
                    .map(|v| v.as_i64().ok_or_else(fail))
                    .transpose()
            };
            let part = text("part")?.to_owned();
            let measure = m.target.strip_prefix('#').ok_or_else(fail)?.to_owned();
            let host = hosts
                .get(&(part.clone(), measure.clone()))
                .ok_or_else(fail)?;
            let index = integer("index")?.filter(|i| *i >= 0).ok_or_else(fail)?;
            let optional_text = |key| -> Result<Option<String>, Vec<Diagnostic>> {
                m.attributes
                    .get(key)
                    .map(|v| v.as_str().map(str::to_owned).ok_or_else(fail))
                    .transpose()
            };
            let words = if let Some(text) = optional_text("text")? {
                Some(DashesWords {
                    font_weight: None,
                    default_x: None,
                    relative_y: None,
                    text,
                    font_family: optional_text("font_family")?,
                    font_style: optional_text("font_style")?,
                    font_size: optional_number("font_size")?,
                    default_y: optional_number("words_default_y")?,
                    relative_x: optional_number("words_relative_x")?,
                })
            } else {
                if [
                    "font_family",
                    "font_style",
                    "font_size",
                    "words_default_y",
                    "words_relative_x",
                ]
                .iter()
                .any(|k| m.attributes.contains_key(*k))
                {
                    return Err(fail());
                }
                None
            };
            let endpoint = DashesEndpoint {
                words,
                system: optional_text("system")?,
                part,
                measure,
                offset: &f.time - &host.0,
                staff: integer("staff")?
                    .map(|n| u32::try_from(n).map_err(|_| fail()))
                    .transpose()?,
                placement: match m.attributes.get("placement") {
                    None => None,
                    Some(Scalar::String(s)) if s == "above" => {
                        Some(crate::notation::Placement::Above)
                    }
                    Some(Scalar::String(s)) if s == "below" => {
                        Some(crate::notation::Placement::Below)
                    }
                    _ => return Err(fail()),
                },
                kind: DashesType::from_name(text("type")?).ok_or_else(fail)?,
                number: integer("number")?
                    .map(|n| u8::try_from(n).map_err(|_| fail()))
                    .transpose()?,
                default_y: optional_number("default_y")?,
                relative_x: optional_number("relative_x")?,
            };
            if endpoints.insert(index, endpoint).is_some() {
                return Err(fail());
            }
        }
    }
    if enabled && (endpoints.is_empty() || doc.document_kind != DocumentKind::Score)
        || endpoints.keys().enumerate().any(|(i, k)| i as i64 != *k)
    {
        return Err(fail());
    }
    let endpoints: Vec<_> = endpoints.into_values().collect();
    if !valid(&endpoints, &hosts) {
        return Err(fail());
    }
    Ok(endpoints)
}
/// Audit open/stop structure in authored document order, not temporal curve validity.
/// Never manufacture missing endpoints or sounding relationships.
pub fn unresolved_pairs(endpoints: &[DashesEndpoint]) -> usize {
    let mut open = BTreeMap::new();
    let mut unresolved = 0;
    for e in endpoints {
        let key = (e.part.as_str(), e.staff, e.number.unwrap_or(1));
        if e.kind == DashesType::Stop {
            if open.remove(&key).is_none() {
                unresolved += 1;
            }
        } else if open.insert(key, ()).is_some() {
            unresolved += 1;
        }
    }
    unresolved + open.len()
}
