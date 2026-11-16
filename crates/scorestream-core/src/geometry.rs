//! Authored MusicXML tenths, independent of rendered layout or sounding time.
use crate::{Diagnostic, Document, Mark, Rational, Record, Scalar, SourceRange};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
pub const EXTENSION: &str = "scorestream.geometry@1";
pub const EVENT: &str = "scorestream.geometry.event";
pub const DOT: &str = "scorestream.geometry.dot";
pub const MEASURE: &str = "scorestream.geometry.measure";
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "host", rename_all = "snake_case", deny_unknown_fields)]
pub enum Geometry {
    Event {
        event: String,
        default_x: Option<Rational>,
        default_y: Option<Rational>,
    },
    /// Coordinates of an existing augmentation dot, zero-based in authored order.
    /// Sparse: unstyled dots need no record; this never creates or counts dots.
    Dot {
        event: String,
        index: u32,
        default_x: Option<Rational>,
        default_y: Option<Rational>,
    },
    Measure {
        part: String,
        measure: String,
        width: Rational,
    },
}
pub fn is_kind(kind: &str) -> bool {
    kind == "scorestream.geometry" || kind.starts_with("scorestream.geometry.")
}
pub fn decimal(value: &Rational) -> bool {
    let mut denominator = value.denominator().clone();
    for factor in [2u8, 5] {
        while &denominator % factor == 0.into() {
            denominator /= factor;
        }
    }
    denominator == 1.into()
}
pub fn encode(value: &Geometry, host: String, part: Option<String>) -> Mark {
    let mut attributes = BTreeMap::new();
    let kind = match value {
        Geometry::Event {
            default_x,
            default_y,
            ..
        } => {
            for (name, number) in [("default_x", default_x), ("default_y", default_y)] {
                if let Some(number) = number {
                    attributes.insert(name.into(), Scalar::String(number.to_string()));
                }
            }
            EVENT
        }
        Geometry::Dot {
            index,
            default_x,
            default_y,
            ..
        } => {
            attributes.insert("index".into(), Scalar::Integer(i64::from(*index)));
            for (name, number) in [("default_x", default_x), ("default_y", default_y)] {
                if let Some(number) = number {
                    attributes.insert(name.into(), Scalar::String(number.to_string()));
                }
            }
            DOT
        }
        Geometry::Measure { width, .. } => {
            attributes.insert("width".into(), Scalar::String(width.to_string()));
            attributes.insert(
                "part".into(),
                Scalar::String(part.expect("mapped measure part")),
            );
            MEASURE
        }
    };
    Mark {
        target: format!("#{host}"),
        kind: kind.into(),
        attributes,
        source: SourceRange::default(),
    }
}
pub fn decode(doc: &Document) -> Result<Vec<Geometry>, Vec<Diagnostic>> {
    let error = |message: &str, source| vec![Diagnostic::error("geometry.wire", message, source)];
    let enabled = doc.required_extensions.iter().any(|e| e == EXTENSION);
    let mut events = BTreeMap::new();
    let mut measures = BTreeMap::new();
    let mut dots = BTreeMap::new();
    for frame in &doc.frames {
        for record in &frame.records {
            match record {
                Record::Event(e) => {
                    if let Some(id) = e.id() {
                        events.insert(id, &frame.time);
                        if e.attributes.get("w").and_then(Scalar::as_str).is_some() {
                            if let Some(count) = e
                                .attributes
                                .get("dots")
                                .and_then(Scalar::as_i64)
                                .and_then(|n| u32::try_from(n).ok())
                            {
                                dots.insert(id, count);
                            }
                        }
                    }
                }
                Record::Measure(m) => {
                    measures.insert(m.id.as_str(), (&frame.time, m.target.as_str()));
                }
                _ => {}
            }
        }
    }
    if enabled && doc.document_kind != crate::DocumentKind::Score {
        return Err(error("Geometry requires a complete score", doc.source));
    }
    let mut result = vec![];
    let mut seen = BTreeSet::new();
    for frame in &doc.frames {
        for record in &frame.records {
            let Record::Mark(m) = record else { continue };
            if !is_kind(&m.kind) {
                continue;
            }
            if !enabled {
                return Err(error("Geometry requires scorestream.geometry@1", m.source));
            }
            let host = m
                .target
                .strip_prefix('#')
                .ok_or_else(|| error("Geometry requires a host identity", m.source))?;
            let index = if m.kind == DOT {
                Some(
                    m.attributes
                        .get("index")
                        .and_then(Scalar::as_i64)
                        .and_then(|n| u32::try_from(n).ok())
                        .filter(|i| dots.get(host).is_some_and(|count| i < count))
                        .ok_or_else(|| {
                            error(
                                "Dot geometry requires an existing written dot index",
                                m.source,
                            )
                        })?,
                )
            } else {
                None
            };
            if !seen.insert((m.kind.as_str(), host, index)) {
                return Err(error("Duplicate geometry host", m.source));
            }
            let number = |key: &str| -> Result<Option<Rational>, Vec<Diagnostic>> {
                let value = match m.attributes.get(key) {
                    None => return Ok(None),
                    Some(Scalar::String(r)) => r
                        .parse::<Rational>()
                        .map_err(|_| error("Invalid exact tenths", m.source))?,
                    Some(Scalar::Integer(i)) => Rational::from_integer(*i),
                    _ => return Err(error("Expected exact decimal tenths", m.source)),
                };
                if !decimal(&value) {
                    return Err(error("Tenths must be finite decimals", m.source));
                }
                Ok(Some(value))
            };
            let value = match m.kind.as_str() {
                EVENT | DOT => {
                    if m.attributes.keys().any(|k| {
                        !(["default_x", "default_y"].contains(&k.as_str())
                            || m.kind == DOT && k == "index")
                    }) || events.get(host) != Some(&&frame.time)
                    {
                        return Err(error(
                            "Invalid geometry event fields, host, or onset",
                            m.source,
                        ));
                    }
                    let default_x = number("default_x")?;
                    let default_y = number("default_y")?;
                    if default_x.is_none() && default_y.is_none() {
                        return Err(error("Empty geometry event", m.source));
                    }
                    if let Some(index) = index {
                        Geometry::Dot {
                            event: host.into(),
                            index,
                            default_x,
                            default_y,
                        }
                    } else {
                        Geometry::Event {
                            event: host.into(),
                            default_x,
                            default_y,
                        }
                    }
                }
                MEASURE => {
                    let part = m
                        .attributes
                        .get("part")
                        .and_then(Scalar::as_str)
                        .ok_or_else(|| error("Missing measure part", m.source))?;
                    if m.attributes
                        .keys()
                        .any(|k| !["width", "part"].contains(&k.as_str()))
                        || measures.get(host) != Some(&(&frame.time, part))
                    {
                        return Err(error(
                            "Invalid geometry measure fields, ownership, or onset",
                            m.source,
                        ));
                    }
                    Geometry::Measure {
                        part: part.into(),
                        measure: host.into(),
                        width: number("width")?
                            .ok_or_else(|| error("Missing measure width", m.source))?,
                    }
                }
                _ => return Err(error("Unknown geometry record", m.source)),
            };
            result.push(value);
        }
    }
    if enabled && result.is_empty() {
        return Err(error(
            "Geometry extension requires geometry records",
            doc.source,
        ));
    }
    Ok(result)
}
pub fn validate(doc: &Document, diagnostics: &mut Vec<Diagnostic>) {
    if let Err(mut errors) = decode(doc) {
        diagnostics.append(&mut errors);
    }
}
