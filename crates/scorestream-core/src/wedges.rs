//! Ordered authored wedge endpoints; deliberately not sounding hairpin relations.
use crate::{Diagnostic, Document, DocumentKind, Mark, Rational, Record, Scalar, SourceRange};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
pub const EXTENSION: &str = "scorestream.wedge@1";
pub const KIND: &str = "scorestream.wedge";
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WedgeType {
    Crescendo,
    Diminuendo,
    Stop,
}
impl WedgeType {
    pub fn name(self) -> &'static str {
        match self {
            Self::Crescendo => "crescendo",
            Self::Diminuendo => "diminuendo",
            Self::Stop => "stop",
        }
    }
    pub fn from_name(s: &str) -> Option<Self> {
        match s {
            "crescendo" => Some(Self::Crescendo),
            "diminuendo" => Some(Self::Diminuendo),
            "stop" => Some(Self::Stop),
            _ => None,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WedgeEndpoint {
    pub part: String,
    pub measure: String,
    pub offset: Rational,
    pub staff: Option<u32>,
    pub placement: Option<crate::notation::Placement>,
    pub kind: WedgeType,
    pub number: Option<u8>,
    pub default_y: Option<Rational>,
    pub spread: Option<Rational>,
}
pub type Hosts = BTreeMap<(String, String), (Rational, Rational, u32)>;
pub fn valid(endpoints: &[WedgeEndpoint], hosts: &Hosts) -> bool {
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
            && e.number.is_none_or(|n| (1..=16).contains(&n))
            && [&e.default_y, &e.spread]
                .into_iter()
                .flatten()
                .all(crate::geometry::decimal)
    })
}
pub fn is_kind(s: &str) -> bool {
    s == KIND || s.starts_with("scorestream.wedge.")
}
pub fn encode(e: &WedgeEndpoint, part: String, measure: String, index: usize) -> Mark {
    let mut attributes = BTreeMap::from([
        ("part".into(), Scalar::String(part)),
        ("index".into(), Scalar::Integer(index as i64)),
        ("type".into(), Scalar::String(e.kind.name().into())),
    ]);
    for (key, value) in [("default_y", &e.default_y), ("spread", &e.spread)] {
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
    Mark {
        target: format!("#{measure}"),
        kind: KIND.into(),
        attributes,
        source: SourceRange::default(),
    }
}
pub fn decode(doc: &Document) -> Result<Vec<WedgeEndpoint>, Vec<Diagnostic>> {
    let fail = || {
        vec![Diagnostic::error("wedge.wire","Wedges require declared authored endpoint capability, complete score, exact measure/scope/onset, contiguous unique indices and valid typed fields",doc.source)]
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
                        "spread",
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
            let endpoint = WedgeEndpoint {
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
                kind: WedgeType::from_name(text("type")?).ok_or_else(fail)?,
                number: integer("number")?
                    .map(|n| u8::try_from(n).map_err(|_| fail()))
                    .transpose()?,
                default_y: optional_number("default_y")?,
                spread: optional_number("spread")?,
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
pub fn unresolved_pairs(endpoints: &[WedgeEndpoint]) -> usize {
    let mut open = BTreeMap::new();
    let mut unresolved = 0;
    for e in endpoints {
        let key = (e.part.as_str(), e.staff, e.number.unwrap_or(1));
        if e.kind == WedgeType::Stop {
            if open.remove(&key).is_none() {
                unresolved += 1;
            }
        } else if open.insert(key, ()).is_some() {
            unresolved += 1;
        }
    }
    unresolved + open.len()
}
