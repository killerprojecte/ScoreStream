//! Authored plain score directions, not analytical annotations or inferred performance.
use crate::{Diagnostic, Document, DocumentKind, Mark, Rational, Record, Scalar, SourceRange};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
pub const EXTENSION: &str = "scorestream.words@1";
pub const PIZZICATO_EXTENSION: &str = "scorestream.pizzicato@1";
pub const KIND: &str = "scorestream.words";
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WordsText {
    pub text: String,
    pub font_family: Option<String>,
    pub font_size: Option<Rational>,
    pub font_style: Option<String>,
    pub font_weight: Option<String>,
    pub default_x: Option<Rational>,
    pub default_y: Option<Rational>,
    pub relative_x: Option<Rational>,
    pub relative_y: Option<Rational>,
}
impl WordsText {
    pub fn valid(&self) -> bool {
        self.font_style
            .as_deref()
            .is_none_or(|s| matches!(s, "normal" | "italic"))
            && self
                .font_weight
                .as_deref()
                .is_none_or(|s| matches!(s, "normal" | "bold"))
            && self.font_size.as_ref().is_none_or(Rational::is_positive)
            && [
                &self.font_size,
                &self.default_x,
                &self.default_y,
                &self.relative_x,
                &self.relative_y,
            ]
            .into_iter()
            .flatten()
            .all(crate::geometry::decimal)
    }
}
/// Authored sound state, separate from the words text and its visual position.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PizzicatoMode {
    #[serde(rename = "yes")]
    Pizzicato,
    #[serde(rename = "no")]
    Arco,
}
impl PizzicatoMode {
    pub fn name(self) -> &'static str {
        match self {
            Self::Pizzicato => "yes",
            Self::Arco => "no",
        }
    }
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "yes" => Some(Self::Pizzicato),
            "no" => Some(Self::Arco),
            _ => None,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PizzicatoSound {
    pub mode: PizzicatoMode,
    /// Exact sounding offset from the owning measure start, not the words position.
    pub at: Rational,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WordsDirection {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pizzicato: Option<PizzicatoSound>,
    pub part: String,
    pub measure: String,
    pub offset: Rational,
    pub staff: Option<u32>,
    pub placement: Option<crate::notation::Placement>,
    pub system: Option<String>,
    pub words: WordsText,
    /// Additional ordered words siblings in the same direction-type.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub continuations: Vec<WordsText>,
}
pub type Hosts = crate::wedges::Hosts;
pub fn valid(values: &[WordsDirection], hosts: &Hosts) -> bool {
    let mut states = BTreeMap::new();
    for e in values {
        if let Some(p) = &e.pizzicato {
            let Some((start, _, _)) = hosts.get(&(e.part.clone(), e.measure.clone())) else {
                return false;
            };
            if states
                .insert((e.part.clone(), start + &p.at), p.mode)
                .is_some_and(|prior| prior != p.mode)
            {
                return false;
            }
        }
    }
    values.windows(2).all(|p| {
        p[0].part < p[1].part
            || p[0].part == p[1].part
                && hosts
                    .get(&(p[0].part.clone(), p[0].measure.clone()))
                    .zip(hosts.get(&(p[1].part.clone(), p[1].measure.clone())))
                    .is_some_and(|(a, b)| a.0 <= b.0)
    }) && values.iter().all(|e| {
        hosts
            .get(&(e.part.clone(), e.measure.clone()))
            .is_some_and(|(_, len, staves)| {
                !e.offset.is_negative()
                    && &e.offset <= len
                    && e.staff.is_none_or(|s| s > 0 && s <= *staves)
                    && e.pizzicato
                        .as_ref()
                        .is_none_or(|p| !p.at.is_negative() && &p.at <= len)
            })
            && e.system
                .as_deref()
                .is_none_or(|s| matches!(s, "only-top" | "also-top" | "none"))
            && e.words.valid()
            && (e.pizzicato.is_none() || e.continuations.is_empty())
            && e.continuations.iter().all(WordsText::valid)
    })
}
pub fn is_kind(s: &str) -> bool {
    s == KIND || s.starts_with("scorestream.words.")
}
pub fn encode(e: &WordsDirection, part: String, measure: String, index: usize) -> Mark {
    let w = &e.words;
    let mut attributes = BTreeMap::from([
        ("part".into(), Scalar::String(part)),
        ("index".into(), Scalar::Integer(index as i64)),
        ("text".into(), Scalar::String(w.text.clone())),
    ]);
    if let Some(p) = &e.pizzicato {
        attributes.insert("pizzicato".into(), Scalar::String(p.mode.name().into()));
        attributes.insert("pizzicato_at".into(), Scalar::String(p.at.to_string()));
    }
    for (key, v) in [
        ("system", e.system.as_deref()),
        ("font_family", w.font_family.as_deref()),
        ("font_style", w.font_style.as_deref()),
        ("font_weight", w.font_weight.as_deref()),
        (
            "placement",
            e.placement.map(|p| match p {
                crate::notation::Placement::Above => "above",
                crate::notation::Placement::Below => "below",
            }),
        ),
    ] {
        if let Some(v) = v {
            attributes.insert(key.into(), Scalar::String(v.into()));
        }
    }
    for (key, v) in [
        ("font_size", &w.font_size),
        ("default_x", &w.default_x),
        ("default_y", &w.default_y),
        ("relative_x", &w.relative_x),
        ("relative_y", &w.relative_y),
    ] {
        if let Some(v) = v {
            attributes.insert(key.into(), Scalar::String(v.to_string()));
        }
    }
    if let Some(s) = e.staff {
        attributes.insert("staff".into(), Scalar::Integer(s.into()));
    }
    Mark {
        target: format!("#{measure}"),
        kind: KIND.into(),
        attributes,
        source: SourceRange::default(),
    }
}
/// Each row retains its own typed text/style; row zero is the existing header.
pub fn encode_group(e: &WordsDirection, part: String, measure: String, index: usize) -> Vec<Mark> {
    let mut marks = vec![encode(e, part.clone(), measure.clone(), index)];
    let mut child = e.clone();
    child.continuations.clear();
    for (row, words) in e.continuations.iter().enumerate() {
        child.words = words.clone();
        let mut mark = encode(&child, part.clone(), measure.clone(), index);
        mark.attributes
            .insert("row".into(), Scalar::Integer((row + 1) as i64));
        marks.push(mark);
    }
    marks
}
pub fn decode(doc: &Document) -> Result<Vec<WordsDirection>, Vec<Diagnostic>> {
    let fail = || {
        vec![Diagnostic::error(
            "words.wire",
            "Authored words require declared capability, complete score, exact measure ownership/onset, contiguous indices and closed valid typed fields",
            doc.source,
        )]
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
    let mut entries = BTreeMap::new();
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
                        "pizzicato",
                        "pizzicato_at",
                        "part",
                        "index",
                        "row",
                        "text",
                        "staff",
                        "placement",
                        "system",
                        "font_family",
                        "font_size",
                        "font_style",
                        "font_weight",
                        "default_x",
                        "default_y",
                        "relative_x",
                        "relative_y",
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
            let opt_text = |key| -> Result<Option<String>, Vec<Diagnostic>> {
                m.attributes
                    .get(key)
                    .map(|v| v.as_str().map(str::to_owned).ok_or_else(fail))
                    .transpose()
            };
            let number = |key| -> Result<Option<Rational>, Vec<Diagnostic>> {
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
            let pizzicato = match opt_text("pizzicato")? {
                Some(mode) => {
                    if !doc
                        .required_extensions
                        .iter()
                        .any(|e| e == PIZZICATO_EXTENSION)
                    {
                        return Err(fail());
                    }
                    Some(PizzicatoSound {
                        mode: PizzicatoMode::parse(&mode).ok_or_else(fail)?,
                        at: number("pizzicato_at")?.ok_or_else(fail)?,
                    })
                }
                None => {
                    if m.attributes.contains_key("pizzicato_at") {
                        return Err(fail());
                    }
                    None
                }
            };
            let row = integer("row")?.unwrap_or(0);
            if row < 0 || m.attributes.contains_key("row") && row == 0 {
                return Err(fail());
            }
            let e = WordsDirection {
                pizzicato,
                continuations: Vec::new(),
                part,
                measure,
                offset: &f.time - &host.0,
                staff: integer("staff")?
                    .map(|n| u32::try_from(n).map_err(|_| fail()))
                    .transpose()?,
                placement: match opt_text("placement")?.as_deref() {
                    None => None,
                    Some("above") => Some(crate::notation::Placement::Above),
                    Some("below") => Some(crate::notation::Placement::Below),
                    _ => return Err(fail()),
                },
                system: opt_text("system")?,
                words: WordsText {
                    text: text("text")?.into(),
                    font_family: opt_text("font_family")?,
                    font_style: opt_text("font_style")?,
                    font_weight: opt_text("font_weight")?,
                    font_size: number("font_size")?,
                    default_x: number("default_x")?,
                    default_y: number("default_y")?,
                    relative_x: number("relative_x")?,
                    relative_y: number("relative_y")?,
                },
            };
            if entries.insert((index, row), e).is_some() {
                return Err(fail());
            }
        }
    }
    if enabled && (entries.is_empty() || doc.document_kind != DocumentKind::Score) {
        return Err(fail());
    }
    let mut result: Vec<WordsDirection> = Vec::new();
    for ((index, row), e) in entries {
        if row == 0 {
            if index != result.len() as i64 {
                return Err(fail());
            }
            result.push(e);
        } else {
            if index != (result.len() as i64 - 1) {
                return Err(fail());
            }
            let Some(parent) = result.last_mut() else {
                return Err(fail());
            };
            if row != parent.continuations.len() as i64 + 1 {
                return Err(fail());
            }
            if parent.part != e.part
                || parent.measure != e.measure
                || parent.offset != e.offset
                || parent.staff != e.staff
                || parent.placement != e.placement
                || parent.system != e.system
                || e.pizzicato.is_some() || parent.pizzicato.is_some()
            {
                return Err(fail());
            }
            parent.continuations.push(e.words);
        }
    }
    if doc
        .required_extensions
        .iter()
        .any(|e| e == PIZZICATO_EXTENSION)
        != result.iter().any(|e| e.pizzicato.is_some())
    {
        return Err(fail());
    }
    if !valid(&result, &hosts) {
        return Err(fail());
    }
    Ok(result)
}
