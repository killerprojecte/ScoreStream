//! Authored declared-part labels, independent of instrument identity and routing.
use crate::{Diagnostic, Document, DocumentKind, Mark, Record, Scalar, SourceRange};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
pub const EXTENSION: &str = "scorestream.part_labels@1";
pub const KIND: &str = "scorestream.part_labels";
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NameAccidental {
    Flat,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartNameDisplay {
    pub leading_text: String,
    pub accidental: NameAccidental,
    pub trailing_text: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PartLabels {
    pub abbreviation: Option<String>,
    pub name_display: Option<PartNameDisplay>,
}
impl PartLabels {
    pub fn valid(&self) -> bool {
        (self.abbreviation.is_some() || self.name_display.is_some())
            && self
                .abbreviation
                .as_deref()
                .into_iter()
                .chain(
                    self.name_display
                        .iter()
                        .flat_map(|d| [d.leading_text.as_str(), d.trailing_text.as_str()]),
                )
                .all(crate::instrument_metadata::text_valid)
    }
}
pub fn is_kind(s: &str) -> bool {
    s == KIND || s.starts_with("scorestream.part_labels.")
}
pub fn encode(labels: &PartLabels, part: String) -> Mark {
    let mut attributes = BTreeMap::new();
    if let Some(a) = &labels.abbreviation {
        attributes.insert("abbreviation".into(), Scalar::String(a.clone()));
    }
    if let Some(d) = &labels.name_display {
        attributes.insert(
            "name_leading_text".into(),
            Scalar::String(d.leading_text.clone()),
        );
        attributes.insert(
            "name_accidental".into(),
            Scalar::String(
                match d.accidental {
                    NameAccidental::Flat => "flat",
                }
                .into(),
            ),
        );
        attributes.insert(
            "name_trailing_text".into(),
            Scalar::String(d.trailing_text.clone()),
        );
    }
    Mark {
        target: part,
        kind: KIND.into(),
        attributes,
        source: SourceRange::default(),
    }
}
pub fn decode(doc: &Document) -> Result<BTreeMap<String, PartLabels>, Vec<Diagnostic>> {
    let fail = || {
        vec![Diagnostic::error("part_labels.wire","Part labels require a declared capability, complete score, unique declared-part host at time zero and closed complete label fields",doc.source)]
    };
    let enabled = doc.required_extensions.iter().any(|e| e == EXTENSION);
    let parts: BTreeSet<_> = doc.parts.iter().map(|p| p.id.as_str()).collect();
    let mut result = BTreeMap::new();
    for f in &doc.frames {
        for r in &f.records {
            let Record::Mark(m) = r else { continue };
            if !is_kind(&m.kind) {
                continue;
            }
            if !enabled
                || doc.document_kind != DocumentKind::Score
                || f.time != crate::Rational::zero()
                || !parts.contains(m.target.as_str())
                || m.kind != KIND
                || m.attributes.keys().any(|k| {
                    ![
                        "abbreviation",
                        "name_leading_text",
                        "name_accidental",
                        "name_trailing_text",
                    ]
                    .contains(&k.as_str())
                })
            {
                return Err(fail());
            }
            let text = |key| -> Result<Option<String>, Vec<Diagnostic>> {
                m.attributes
                    .get(key)
                    .map(|v| v.as_str().map(str::to_owned).ok_or_else(fail))
                    .transpose()
            };
            let name_display = match (
                text("name_leading_text")?,
                text("name_accidental")?,
                text("name_trailing_text")?,
            ) {
                (None, None, None) => None,
                (Some(leading_text), Some(a), Some(trailing_text)) if a == "flat" => {
                    Some(PartNameDisplay {
                        leading_text,
                        accidental: NameAccidental::Flat,
                        trailing_text,
                    })
                }
                _ => return Err(fail()),
            };
            let value = PartLabels {
                abbreviation: text("abbreviation")?,
                name_display,
            };
            if !value.valid() || result.insert(m.target.clone(), value).is_some() {
                return Err(fail());
            }
        }
    }
    if enabled && (result.is_empty() || doc.document_kind != DocumentKind::Score) {
        return Err(fail());
    }
    Ok(result)
}
