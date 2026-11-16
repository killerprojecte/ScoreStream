//! Exact authored metronome presentation attached to the existing numeric mark.
use crate::{
    words::WordsText, Diagnostic, Document, DocumentKind, Mark, Rational, Record, Scalar,
    SourceRange,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
pub const EXTENSION: &str = "scorestream.metronome_display@1";
pub const KIND: &str = "scorestream.metronome_display";
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetronomeDisplay {
    pub measure: String,
    pub placement: Option<crate::notation::Placement>,
    pub system: Option<String>,
    pub parentheses: Option<bool>,
    pub default_x: Option<Rational>,
    pub default_y: Option<Rational>,
    pub relative_x: Option<Rational>,
    pub relative_y: Option<Rational>,
    pub leading_words: Option<WordsText>,
    pub trailing_words: Option<WordsText>,
}
impl MetronomeDisplay {
    pub fn valid(&self) -> bool {
        (self.placement.is_some()
            || self.system.is_some()
            || self.parentheses.is_some()
            || self.leading_words.is_some()
            || [
                &self.default_x,
                &self.default_y,
                &self.relative_x,
                &self.relative_y,
            ]
            .into_iter()
            .any(Option::is_some))
            && self
                .system
                .as_deref()
                .is_none_or(|s| matches!(s, "only-top" | "also-top" | "none"))
            && (self.trailing_words.is_none() || self.leading_words.is_some())
            && [
                &self.default_x,
                &self.default_y,
                &self.relative_x,
                &self.relative_y,
            ]
            .into_iter()
            .flatten()
            .all(crate::geometry::decimal)
            && [&self.leading_words, &self.trailing_words]
                .into_iter()
                .flatten()
                .all(|w| {
                    w.valid()
                        && std::iter::once(w.text.as_str())
                            .chain(w.font_family.as_deref())
                            .all(crate::instrument_metadata::text_valid)
                })
    }
}
const WORD_FIELDS: [&str; 9] = [
    "text",
    "font_family",
    "font_size",
    "font_style",
    "font_weight",
    "default_x",
    "default_y",
    "relative_x",
    "relative_y",
];
pub fn is_kind(s: &str) -> bool {
    s == KIND || s.starts_with("scorestream.metronome_display.")
}
pub fn encode(value: &MetronomeDisplay, id: &str, measure: String) -> Mark {
    let mut attributes = BTreeMap::from([("measure".into(), Scalar::String(measure))]);
    for (k, v) in [
        ("system", value.system.as_deref()),
        (
            "placement",
            value.placement.map(|p| match p {
                crate::notation::Placement::Above => "above",
                crate::notation::Placement::Below => "below",
            }),
        ),
    ] {
        if let Some(v) = v {
            attributes.insert(k.into(), Scalar::String(v.into()));
        }
    }
    if let Some(v) = value.parentheses {
        attributes.insert("parentheses".into(), Scalar::Boolean(v));
    }
    for (k, v) in [
        ("default_x", &value.default_x),
        ("default_y", &value.default_y),
        ("relative_x", &value.relative_x),
        ("relative_y", &value.relative_y),
    ] {
        if let Some(v) = v {
            attributes.insert(k.into(), Scalar::String(v.to_string()));
        }
    }
    for (prefix, w) in [
        ("leading", &value.leading_words),
        ("trailing", &value.trailing_words),
    ] {
        if let Some(w) = w {
            attributes.insert(format!("{prefix}_text"), Scalar::String(w.text.clone()));
            for (k, v) in [
                ("font_family", w.font_family.as_deref()),
                ("font_style", w.font_style.as_deref()),
                ("font_weight", w.font_weight.as_deref()),
            ] {
                if let Some(v) = v {
                    attributes.insert(format!("{prefix}_{k}"), Scalar::String(v.into()));
                }
            }
            for (k, v) in [
                ("font_size", &w.font_size),
                ("default_x", &w.default_x),
                ("default_y", &w.default_y),
                ("relative_x", &w.relative_x),
                ("relative_y", &w.relative_y),
            ] {
                if let Some(v) = v {
                    attributes.insert(format!("{prefix}_{k}"), Scalar::String(v.to_string()));
                }
            }
        }
    }
    Mark {
        target: format!("#{id}"),
        kind: KIND.into(),
        attributes,
        source: SourceRange::default(),
    }
}
pub fn decode(doc: &Document) -> Result<BTreeMap<String, MetronomeDisplay>, Vec<Diagnostic>> {
    let fail = || {
        vec![Diagnostic::error("metronome_display.wire","Metronome presentation requires declared capability, a unique identified numeric metronome at the same onset, exact owning measure and closed typed presentation fields",doc.source)]
    };
    let enabled = doc.required_extensions.iter().any(|e| e == EXTENSION);
    let mut hosts = BTreeMap::new();
    let mut measures = BTreeMap::new();
    for f in &doc.frames {
        for r in &f.records {
            match r {
                Record::Mark(m) if m.kind == "metronome" => {
                    if let Some(id) = m.attributes.get("id").and_then(Scalar::as_str) {
                        if hosts.insert(id, (m, &f.time)).is_some() {
                            return Err(fail());
                        }
                    }
                }
                Record::Measure(m) => {
                    measures.insert((m.target.as_str(), m.id.as_str()), (&f.time, &m.length));
                }
                _ => {}
            }
        }
    }
    // Check collisions against the small host map rather than indexing all events.
    if doc.parts.iter().any(|p| hosts.contains_key(p.id.as_str()))
        || doc.voices.iter().any(|v| hosts.contains_key(v.id.as_str()))
    {
        return Err(fail());
    }
    for r in doc.frames.iter().flat_map(|f| &f.records) {
        let id = match r {
            Record::Event(e) => e.attributes.get("id").and_then(Scalar::as_str),
            Record::Measure(m) => Some(m.id.as_str()),
            Record::Relation(r) => Some(r.id.as_str()),
            Record::Mark(m) if m.kind == crate::instrument_extension::KIND => {
                m.attributes.get("source_id").and_then(Scalar::as_str)
            }
            _ => None,
        };
        if id.is_some_and(|id| hosts.contains_key(id)) {
            return Err(fail());
        }
    }
    let mut result = BTreeMap::new();
    for f in &doc.frames {
        for r in &f.records {
            let Record::Mark(m) = r else { continue };
            if !is_kind(&m.kind) {
                continue;
            }
            if !enabled || doc.document_kind != DocumentKind::Score || m.kind != KIND {
                return Err(fail());
            }
            for k in m.attributes.keys() {
                if ![
                    "measure",
                    "placement",
                    "system",
                    "parentheses",
                    "default_x",
                    "default_y",
                    "relative_x",
                    "relative_y",
                ]
                .contains(&k.as_str())
                    && !["leading_", "trailing_"]
                        .iter()
                        .any(|p| k.strip_prefix(p).is_some_and(|s| WORD_FIELDS.contains(&s)))
                {
                    return Err(fail());
                }
            }
            let text = |k: &str| -> Result<Option<String>, Vec<Diagnostic>> {
                m.attributes
                    .get(k)
                    .map(|v| v.as_str().map(str::to_owned).ok_or_else(fail))
                    .transpose()
            };
            let number = |k: &str| -> Result<Option<Rational>, Vec<Diagnostic>> {
                text(k)?.map(|v| v.parse().map_err(|_| fail())).transpose()
            };
            let words = |p: &str| -> Result<Option<WordsText>, Vec<Diagnostic>> {
                let Some(value) = text(&format!("{p}_text"))? else {
                    if m.attributes.keys().any(|k| k.starts_with(&format!("{p}_"))) {
                        return Err(fail());
                    }
                    return Ok(None);
                };
                Ok(Some(WordsText {
                    text: value,
                    font_family: text(&format!("{p}_font_family"))?,
                    font_style: text(&format!("{p}_font_style"))?,
                    font_weight: text(&format!("{p}_font_weight"))?,
                    font_size: number(&format!("{p}_font_size"))?,
                    default_x: number(&format!("{p}_default_x"))?,
                    default_y: number(&format!("{p}_default_y"))?,
                    relative_x: number(&format!("{p}_relative_x"))?,
                    relative_y: number(&format!("{p}_relative_y"))?,
                }))
            };
            let id = m.target.strip_prefix('#').ok_or_else(fail)?;
            let (host, at) = hosts.get(id).ok_or_else(fail)?;
            if **at != f.time {
                return Err(fail());
            }
            let part = if doc.parts.iter().any(|p| p.id == host.target) {
                host.target.as_str()
            } else {
                host.target
                    .rsplit_once(".s")
                    .filter(|(p, s)| {
                        s.parse::<u32>().ok().is_some_and(|s| {
                            doc.parts
                                .iter()
                                .any(|v| v.id == *p && s > 0 && s <= v.staves)
                        })
                    })
                    .map(|(p, _)| p)
                    .ok_or_else(fail)?
            };
            let measure = text("measure")?.ok_or_else(fail)?;
            let (start, len) = measures.get(&(part, measure.as_str())).ok_or_else(fail)?;
            if f.time < **start || f.time >= &**start + *len {
                return Err(fail());
            }
            let value = MetronomeDisplay {
                measure,
                placement: match text("placement")?.as_deref() {
                    None => None,
                    Some("above") => Some(crate::notation::Placement::Above),
                    Some("below") => Some(crate::notation::Placement::Below),
                    _ => return Err(fail()),
                },
                system: text("system")?,
                parentheses: match m.attributes.get("parentheses") {
                    None => None,
                    Some(Scalar::Boolean(b)) => Some(*b),
                    _ => return Err(fail()),
                },
                default_x: number("default_x")?,
                default_y: number("default_y")?,
                relative_x: number("relative_x")?,
                relative_y: number("relative_y")?,
                leading_words: words("leading")?,
                trailing_words: words("trailing")?,
            };
            if !value.valid() || result.insert(id.into(), value).is_some() {
                return Err(fail());
            }
        }
    }
    if enabled && (result.is_empty() || doc.document_kind != DocumentKind::Score) {
        return Err(fail());
    }
    Ok(result)
}
