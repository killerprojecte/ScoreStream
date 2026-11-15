//! Authored-only SFS navigation capability. This is not performed note lowering.
// Match the inline fail-fast diagnostic convention of the codec and notation lowerer.
#![allow(clippy::result_large_err)]
use crate::{
    notation::PartId, score::*, Diagnostic, Document, DocumentKind, Mark, Rational, Record, Scalar,
    SourceRange,
};
use std::collections::{BTreeMap, BTreeSet};
pub const EXTENSION: &str = "scorestream.navigation@1";
pub const KIND: &str = "scorestream.barline";
fn error(m: &Mark, code: &str, message: &str) -> Diagnostic {
    Diagnostic::error(code, message, m.source)
}
fn string<'a>(m: &'a Mark, key: &str) -> Result<Option<&'a str>, Diagnostic> {
    match m.attributes.get(key) {
        None => Ok(None),
        Some(Scalar::String(s)) => Ok(Some(s)),
        _ => Err(error(
            m,
            "navigation.field",
            "Expected a string-valued navigation field",
        )),
    }
}
fn required<'a>(m: &'a Mark, key: &str) -> Result<&'a str, Diagnostic> {
    string(m, key)?.ok_or_else(|| error(m, "navigation.field", "Missing required navigation field"))
}
fn count(m: &Mark, key: &str) -> Result<Option<u32>, Diagnostic> {
    match m.attributes.get(key) {
        None => Ok(None),
        Some(Scalar::Integer(n)) => u32::try_from(*n).map(Some).map_err(|_| {
            error(
                m,
                "navigation.count",
                "Count must fit an unsigned 32-bit integer",
            )
        }),
        _ => Err(error(m, "navigation.count", "Count requires an integer")),
    }
}
pub fn style_name(value: BarlineStyle) -> &'static str {
    match value {
        BarlineStyle::Regular => "regular",
        BarlineStyle::Dotted => "dotted",
        BarlineStyle::Dashed => "dashed",
        BarlineStyle::Heavy => "heavy",
        BarlineStyle::LightLight => "light-light",
        BarlineStyle::LightHeavy => "light-heavy",
        BarlineStyle::HeavyHeavy => "heavy-heavy",
        BarlineStyle::HeavyLight => "heavy-light",
        BarlineStyle::Tick => "tick",
        BarlineStyle::Short => "short",
        BarlineStyle::None => "none",
    }
}
/// Decode only the typed fields; complete document validation additionally checks
/// the required capability, part/measure ownership and exact boundary time.
pub fn decode(mark: &Mark) -> Result<ScoreBarline, Diagnostic> {
    if mark.kind != KIND {
        return Err(error(
            mark,
            "navigation.kind",
            "Expected the registered barline kind",
        ));
    }
    let allowed = [
        "measure",
        "side",
        "style",
        "repeat",
        "times",
        "ending",
        "passes",
        "ending_text",
        "ending_y",
        "source_id",
    ];
    if mark
        .attributes
        .keys()
        .any(|k| !allowed.contains(&k.as_str()))
    {
        return Err(error(
            mark,
            "navigation.field",
            "Field has no lossless authored-barline mapping",
        ));
    }
    let location = match required(mark, "side")? {
        "left" => BarlineLocation::Left,
        "right" => BarlineLocation::Right,
        _ => {
            return Err(error(
                mark,
                "navigation.side",
                "Only left/right boundaries are supported",
            ));
        }
    };
    let style = string(mark, "style")?
        .map(|s| match s {
            "regular" => Ok(BarlineStyle::Regular),
            "dotted" => Ok(BarlineStyle::Dotted),
            "dashed" => Ok(BarlineStyle::Dashed),
            "heavy" => Ok(BarlineStyle::Heavy),
            "light-light" => Ok(BarlineStyle::LightLight),
            "light-heavy" => Ok(BarlineStyle::LightHeavy),
            "heavy-light" => Ok(BarlineStyle::HeavyLight),
            "heavy-heavy" => Ok(BarlineStyle::HeavyHeavy),
            "tick" => Ok(BarlineStyle::Tick),
            "short" => Ok(BarlineStyle::Short),
            "none" => Ok(BarlineStyle::None),
            _ => Err(error(mark, "navigation.style", "Unknown barline style")),
        })
        .transpose()?;
    let times = count(mark, "times")?;
    let repeat = string(mark, "repeat")?
        .map(|s| {
            let direction = match s {
                "forward" => RepeatDirection::Forward,
                "backward" => RepeatDirection::Backward,
                _ => return Err(error(mark, "navigation.repeat", "Unknown repeat direction")),
            };
            if direction == RepeatDirection::Forward && times.is_some() {
                return Err(error(
                    mark,
                    "navigation.count",
                    "Forward repeat cannot carry times",
                ));
            }
            Ok(RepeatSign { direction, times })
        })
        .transpose()?;
    if repeat.is_none() && times.is_some() {
        return Err(error(
            mark,
            "navigation.count",
            "times requires a backward repeat",
        ));
    }
    let ending = string(mark, "ending")?
        .map(|s| {
            let phase = match s {
                "start" => EndingPhase::Start,
                "stop" => EndingPhase::Stop,
                "discontinue" => EndingPhase::Discontinue,
                _ => return Err(error(mark, "navigation.ending", "Unknown ending phase")),
            };
            let raw = required(mark, "passes")?;
            if raw.len() > 16384 {
                return Err(error(
                    mark,
                    "navigation.passes",
                    "Pass list exceeds supported size",
                ));
            }
            let passes = raw
                .split(',')
                .map(|s| {
                    if crate::registry::positive_integer(s) {
                        s.parse::<u32>().ok()
                    } else {
                        None
                    }
                })
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| {
                    error(
                        mark,
                        "navigation.passes",
                        "Passes require comma-separated positive integers",
                    )
                })?;
            if passes.len() > 1024 || passes.iter().collect::<BTreeSet<_>>().len() != passes.len() {
                return Err(error(
                    mark,
                    "navigation.passes",
                    "At most 1024 distinct positive passes",
                ));
            }
            let default_y = string(mark, "ending_y")?
                .map(|s| {
                    s.parse::<Rational>().map_err(|_| {
                        error(
                            mark,
                            "navigation.ending_y",
                            "Ending placement requires an exact rational",
                        )
                    })
                })
                .transpose()?;
            Ok(EndingSign {
                passes,
                phase,
                text: string(mark, "ending_text")?.unwrap_or("").into(),
                default_y,
            })
        })
        .transpose()?;
    if ending.is_none()
        && ["passes", "ending_text", "ending_y"]
            .iter()
            .any(|k| mark.attributes.contains_key(*k))
    {
        return Err(error(
            mark,
            "navigation.ending",
            "Ending fields require an explicit ending phase",
        ));
    }
    let id = string(mark, "source_id")?.map(str::to_owned);
    if id.as_ref().is_some_and(|s| s.is_empty()) {
        return Err(error(
            mark,
            "navigation.identity",
            "Authored source ID cannot be empty",
        ));
    }
    Ok(ScoreBarline {
        part: PartId(mark.target.clone()),
        measure: required(mark, "measure")?.into(),
        location,
        id,
        style,
        repeat,
        ending,
    })
}
/// Structural capability validation only. Route closure is checked by the
/// common-score conversion profile; performance lowering always rejects here.
pub(crate) fn validate(doc: &Document, out: &mut Vec<Diagnostic>) {
    let enabled = doc.required_extensions.iter().any(|s| s == EXTENSION);
    let bars: Vec<_> = doc
        .frames
        .iter()
        .flat_map(|f| {
            f.records.iter().filter_map(move |r| match r {
                Record::Mark(m) if m.kind == KIND => Some((&f.time, m)),
                _ => None,
            })
        })
        .collect();
    if enabled && bars.is_empty() {
        out.push(Diagnostic::error(
            "navigation.empty_requirement",
            "Navigation capability requires at least one authored boundary",
            doc.source,
        ));
    }
    if bars.is_empty() {
        return;
    }
    if !enabled {
        out.push(Diagnostic::error(
            "navigation.requirement",
            "Authored barlines require !requires scorestream.navigation@1",
            doc.source,
        ));
    }
    if doc.document_kind != DocumentKind::Score {
        out.push(Diagnostic::error(
            "fragment_global_state",
            "Navigation cannot be introduced by a fragment",
            doc.source,
        ));
    }
    let parts: BTreeSet<_> = doc.parts.iter().map(|p| p.id.as_str()).collect();
    let measures: BTreeMap<_, _> = doc
        .frames
        .iter()
        .flat_map(|f| {
            f.records.iter().filter_map(move |r| match r {
                Record::Measure(m) => Some((m.id.as_str(), (&f.time, m))),
                _ => None,
            })
        })
        .collect();
    let mut ids: BTreeSet<&str> = parts.clone();
    ids.extend(measures.keys().copied());
    ids.extend(doc.events().filter_map(|(_, e)| e.id()));
    let mut positions = BTreeSet::new();
    let mut ending_marks = Vec::new();
    for (time, mark) in bars {
        let b = match decode(mark) {
            Ok(b) => b,
            Err(e) => {
                out.push(e);
                continue;
            }
        };
        if !parts.contains(b.part.0.as_str()) {
            out.push(error(
                mark,
                "navigation.part",
                "Barline target must be a declared part, not score/staff/voice/event",
            ));
        }
        match measures.get(b.measure.as_str()) {
            Some((start, m)) if m.target == b.part.0 => {
                let at = if b.location == BarlineLocation::Left {
                    (*start).clone()
                } else {
                    *start + &m.length
                };
                if *time != at {
                    out.push(error(mark,"navigation.boundary","Barline frame must equal the referenced measure's exact left/right boundary"));
                }
            }
            _ => out.push(error(
                mark,
                "navigation.measure",
                "Barline measure must belong to the targeted part",
            )),
        }
        if let Some(e) = &b.ending {
            ending_marks.push((
                time.clone(),
                e.phase,
                b.part.clone(),
                e.passes.clone(),
                mark.source,
            ));
        }
        if !positions.insert((b.part, b.measure, b.location)) {
            out.push(error(
                mark,
                "navigation.duplicate",
                "Duplicate authored part/measure side",
            ));
        }
        if mark
            .attributes
            .get("source_id")
            .and_then(Scalar::as_str)
            .is_some_and(|id| !ids.insert(id))
        {
            out.push(error(
                mark,
                "navigation.identity",
                "Authored barline ID collides with another source object",
            ));
        }
    }
    ending_marks.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)));
    let mut open = BTreeMap::new();
    for (_, phase, part, mut passes, source) in ending_marks {
        passes.sort_unstable();
        let key = (part, passes);
        if phase == EndingPhase::Start {
            open.insert(key, source);
        } else {
            open.remove(&key);
        }
    }
    for ((part, passes), source) in open {
        out.push(Diagnostic::warning("navigation.unclosed_ending", format!("Part {part} ending {passes:?} has no authored endpoint; no final-pass continuation policy is inferred"), source));
    }
    out.push(Diagnostic::warning("navigation.realization_unavailable","Authored repeat notation is preserved; repeat engraving and performed playback are not implemented",SourceRange::default()));
}
