//! Ordered authored part-list boundaries; no span repair, rendering or sound behavior.
use crate::{Diagnostic, Document, DocumentKind, Mark, Rational, Record, Scalar, SourceRange};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
pub const EXTENSION: &str = "scorestream.part_groups@1";
pub const KIND: &str = "scorestream.part_group";
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GroupPhase {
    Start,
    Stop,
}
impl GroupPhase {
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum GroupSymbol {
    Bracket,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartGroupBoundary {
    /// None denotes the boundary after every declared part, not an unknown part.
    pub before_part: Option<String>,
    pub phase: GroupPhase,
    /// MusicXML token, not a sequence ordinal. None retains the implied default1.
    pub number: Option<String>,
    pub symbol: Option<GroupSymbol>,
    pub barline: Option<bool>,
}
pub fn is_kind(s: &str) -> bool {
    s == KIND
        || s.starts_with("scorestream.part_group.")
        || s == "scorestream.part_groups"
        || s.starts_with("scorestream.part_groups.")
}
fn token(s: &str) -> bool {
    crate::instrument_metadata::text_valid(s)
        && !s.contains(['\t', '\r', '\n'])
        && !s.starts_with(' ')
        && !s.ends_with(' ')
        && !s.contains("  ")
}
pub fn valid(groups: &[PartGroupBoundary], parts: &[&str]) -> bool {
    let positions: BTreeMap<_, _> = parts.iter().enumerate().map(|(i, p)| (*p, i)).collect();
    let mut previous = 0;
    for g in groups {
        let position = match &g.before_part {
            Some(p) => match positions.get(p.as_str()) {
                Some(p) => *p,
                None => return false,
            },
            None => parts.len(),
        };
        if (g.phase == GroupPhase::Start && position == parts.len())
            || (g.phase == GroupPhase::Stop && position == 0)
            || position < previous
            || g.number.as_ref().is_some_and(|s| !token(s))
            || (g.phase == GroupPhase::Stop && (g.symbol.is_some() || g.barline.is_some()))
        {
            return false;
        }
        previous = position;
    }
    true
}
pub fn encode(g: &PartGroupBoundary, index: usize) -> Mark {
    let mut a = BTreeMap::from([
        ("index".into(), Scalar::Integer(index as i64)),
        ("phase".into(), Scalar::String(g.phase.name().into())),
    ]);
    if let Some(p) = &g.before_part {
        a.insert("before_part".into(), Scalar::String(p.clone()));
    }
    if let Some(n) = &g.number {
        a.insert("number".into(), Scalar::String(n.clone()));
    }
    if g.symbol.is_some() {
        a.insert("symbol".into(), Scalar::String("bracket".into()));
    }
    if let Some(b) = g.barline {
        a.insert("barline".into(), Scalar::Boolean(b));
    }
    Mark {
        target: "score".into(),
        kind: KIND.into(),
        attributes: a,
        source: SourceRange::default(),
    }
}
pub fn decode(doc: &Document) -> Result<Vec<PartGroupBoundary>, Vec<Diagnostic>> {
    let fail = || {
        vec![Diagnostic::error(
            "part_groups.wire",
            "Part groups require complete-score capability, time-zero score ownership, contiguous indices, ordered declared-part boundaries and closed presentation fields",
            doc.source,
        )]
    };
    let enabled = doc.required_extensions.iter().any(|e| e == EXTENSION);
    let mut groups = BTreeMap::new();
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
                || m.kind != KIND
            {
                return Err(fail());
            }
            let a = &m.attributes;
            let text = |k: &str| {
                a.get(k)
                    .map(|v| v.as_str().map(str::to_owned).ok_or_else(fail))
                    .transpose()
            };
            let index = a
                .get("index")
                .and_then(Scalar::as_i64)
                .filter(|i| *i >= 0)
                .ok_or_else(fail)?;
            let g = PartGroupBoundary {
                before_part: text("before_part")?,
                phase: a
                    .get("phase")
                    .and_then(Scalar::as_str)
                    .and_then(GroupPhase::from_name)
                    .ok_or_else(fail)?,
                number: text("number")?,
                symbol: a
                    .get("symbol")
                    .map(|v| {
                        if v.as_str() == Some("bracket") {
                            Ok(GroupSymbol::Bracket)
                        } else {
                            Err(fail())
                        }
                    })
                    .transpose()?,
                barline: a
                    .get("barline")
                    .map(|v| v.as_bool().ok_or_else(fail))
                    .transpose()?,
            };
            if a.keys().any(|k| {
                ![
                    "index",
                    "phase",
                    "before_part",
                    "number",
                    "symbol",
                    "barline",
                ]
                .contains(&k.as_str())
            }) || groups.insert(index, g).is_some()
            {
                return Err(fail());
            }
        }
    }
    if enabled && (doc.document_kind != DocumentKind::Score || groups.is_empty())
        || groups.keys().copied().ne(0..groups.len() as i64)
    {
        return Err(fail());
    }
    let groups: Vec<_> = groups.into_values().collect();
    if !valid(
        &groups,
        &doc.parts.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(),
    ) {
        return Err(fail());
    }
    Ok(groups)
}
