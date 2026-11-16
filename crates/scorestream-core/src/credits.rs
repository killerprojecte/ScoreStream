//! Ordered authored page credits. No page layout or playback behavior is implied.
use crate::{Diagnostic, Document, DocumentKind, Mark, Rational, Record, Scalar, SourceRange};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
pub const EXTENSION: &str = "scorestream.credits@1";
pub const KIND: &str = "scorestream.credit";
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Credit {
    pub page: Option<u32>,
    pub credit_type: Option<String>,
    pub text: String,
    pub default_x: Option<Rational>,
    pub default_y: Option<Rational>,
    pub justify: Option<Justify>,
    pub valign: Option<VerticalAlign>,
    pub font_size: Option<Rational>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Justify {
    Left,
    Center,
    Right,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VerticalAlign {
    Top,
    Middle,
    Bottom,
    Baseline,
}
impl Justify {
    pub fn name(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Center => "center",
            Self::Right => "right",
        }
    }
    pub fn from_name(s: &str) -> Option<Self> {
        match s {
            "left" => Some(Self::Left),
            "center" => Some(Self::Center),
            "right" => Some(Self::Right),
            _ => None,
        }
    }
}
impl VerticalAlign {
    pub fn name(self) -> &'static str {
        match self {
            Self::Top => "top",
            Self::Middle => "middle",
            Self::Bottom => "bottom",
            Self::Baseline => "baseline",
        }
    }
    pub fn from_name(s: &str) -> Option<Self> {
        match s {
            "top" => Some(Self::Top),
            "middle" => Some(Self::Middle),
            "bottom" => Some(Self::Bottom),
            "baseline" => Some(Self::Baseline),
            _ => None,
        }
    }
}
impl Credit {
    pub fn valid(&self) -> bool {
        self.page != Some(0)
            && [&self.default_x, &self.default_y, &self.font_size]
                .into_iter()
                .flatten()
                .all(crate::geometry::decimal)
            && self.font_size.as_ref().is_none_or(Rational::is_positive)
    }
}
pub fn encode(c: &Credit, index: usize) -> Mark {
    let mut attributes = BTreeMap::new();
    attributes.insert("index".into(), Scalar::Integer(index as i64));
    attributes.insert("text".into(), Scalar::String(c.text.clone()));
    for (key, value) in [
        ("credit_type", c.credit_type.clone()),
        ("default_x", c.default_x.as_ref().map(ToString::to_string)),
        ("default_y", c.default_y.as_ref().map(ToString::to_string)),
        ("font_size", c.font_size.as_ref().map(ToString::to_string)),
        ("justify", c.justify.map(|v| v.name().to_owned())),
        ("valign", c.valign.map(|v| v.name().to_owned())),
    ] {
        if let Some(value) = value {
            attributes.insert(key.into(), Scalar::String(value));
        }
    }
    if let Some(page) = c.page {
        attributes.insert("page".into(), Scalar::Integer(page.into()));
    }
    Mark {
        target: "score".into(),
        kind: KIND.into(),
        attributes,
        source: SourceRange::default(),
    }
}
pub fn is_kind(kind: &str) -> bool {
    kind == KIND || kind.starts_with("scorestream.credit.")
}
pub fn decode(doc: &Document) -> Result<Vec<Credit>, Vec<Diagnostic>> {
    let fail = || {
        vec![Diagnostic::error(
            "credits.wire",
            "Credits require a complete score, declared extension, time-zero score host, contiguous unique indices and valid typed fields",
            doc.source,
        )]
    };
    let enabled = doc.required_extensions.iter().any(|e| e == EXTENSION);
    let mut credits = BTreeMap::new();
    for frame in &doc.frames {
        for r in &frame.records {
            let Record::Mark(m) = r else { continue };
            if !is_kind(&m.kind) {
                continue;
            }
            if m.kind != KIND
                || !enabled
                || doc.document_kind != DocumentKind::Score
                || frame.time != Rational::zero()
                || m.target != "score"
                || m.attributes.keys().any(|k| {
                    ![
                        "index",
                        "page",
                        "text",
                        "credit_type",
                        "default_x",
                        "default_y",
                        "font_size",
                        "justify",
                        "valign",
                    ]
                    .contains(&k.as_str())
                })
            {
                return Err(fail());
            }
            let text = |key: &str| -> Result<Option<String>, Vec<Diagnostic>> {
                match m.attributes.get(key) {
                    None => Ok(None),
                    Some(Scalar::String(s)) => Ok(Some(s.clone())),
                    _ => Err(fail()),
                }
            };
            let number = |key| -> Result<Option<Rational>, Vec<Diagnostic>> {
                text(key)?
                    .map(|s| s.parse().map_err(|_| fail()))
                    .transpose()
            };
            let Some(index) = m
                .attributes
                .get("index")
                .and_then(Scalar::as_i64)
                .filter(|i| *i >= 0)
            else {
                return Err(fail());
            };
            let page = match m.attributes.get("page") {
                None => None,
                Some(Scalar::Integer(p)) => Some(u32::try_from(*p).map_err(|_| fail())?),
                _ => return Err(fail()),
            };
            let c = Credit {
                page,
                text: text("text")?.ok_or_else(fail)?,
                credit_type: text("credit_type")?,
                default_x: number("default_x")?,
                default_y: number("default_y")?,
                font_size: number("font_size")?,
                justify: text("justify")?
                    .map(|s| Justify::from_name(&s).ok_or_else(fail))
                    .transpose()?,
                valign: text("valign")?
                    .map(|s| VerticalAlign::from_name(&s).ok_or_else(fail))
                    .transpose()?,
            };
            if !c.valid() || credits.insert(index, c).is_some() {
                return Err(fail());
            }
        }
    }
    if enabled && (credits.is_empty() || doc.document_kind != DocumentKind::Score)
        || credits.keys().enumerate().any(|(i, k)| i as i64 != *k)
    {
        return Err(fail());
    }
    Ok(credits.into_values().collect())
}
