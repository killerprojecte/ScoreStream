//! Authored-only note/rest fermatas. Shapes are printed signs, never hold durations.
// Match the inline fail-fast diagnostic convention of other typed decoders.
#![allow(clippy::result_large_err)]
use crate::{
    Diagnostic, Document, DocumentKind, Mark, Pitch, Rational, Record, Scalar,
    notation::EventId,
    score::{FermataOrientation, FermataShape, ScoreFermata},
};
use std::collections::{BTreeMap, BTreeSet};

pub const EXTENSION: &str = "scorestream.fermata@1";
pub const KIND: &str = "scorestream.fermata";

fn error(mark: &Mark, code: &str, message: &str) -> Diagnostic {
    Diagnostic::error(code, message, mark.source)
}

fn string<'a>(mark: &'a Mark, key: &str) -> Result<Option<&'a str>, Diagnostic> {
    match mark.attributes.get(key) {
        None => Ok(None),
        Some(Scalar::String(value)) => Ok(Some(value)),
        _ => Err(error(
            mark,
            "fermata.field",
            "Fermata fields require string values",
        )),
    }
}

fn rational(mark: &Mark, key: &str) -> Result<Option<Rational>, Diagnostic> {
    string(mark, key)?
        .map(|value| {
            value.parse().map_err(|_| {
                error(
                    mark,
                    "fermata.placement",
                    "Fermata placement requires a canonical exact rational string",
                )
            })
        })
        .transpose()
}

pub fn shape_name(value: FermataShape) -> &'static str {
    match value {
        FermataShape::Normal => "normal",
        FermataShape::Angled => "angled",
        FermataShape::Square => "square",
        FermataShape::DoubleAngled => "double-angled",
        FermataShape::DoubleSquare => "double-square",
        FermataShape::DoubleDot => "double-dot",
        FermataShape::HalfCurve => "half-curve",
        FermataShape::Curlew => "curlew",
    }
}

pub fn orientation_name(value: FermataOrientation) -> &'static str {
    match value {
        FermataOrientation::Upright => "upright",
        FermataOrientation::Inverted => "inverted",
    }
}

/// Decode only the typed authored fields. Document validation additionally checks
/// capability registration, an actual measured note/rest, and its exact onset.
pub fn decode(mark: &Mark) -> Result<ScoreFermata, Diagnostic> {
    if mark.kind != KIND {
        return Err(error(
            mark,
            "fermata.kind",
            "Expected the registered fermata kind",
        ));
    }
    let allowed = ["shape", "orientation", "default_y", "relative_y"];
    if mark
        .attributes
        .keys()
        .any(|key| !allowed.contains(&key.as_str()))
    {
        return Err(error(
            mark,
            "fermata.field",
            "Field has no lossless authored-fermata mapping",
        ));
    }
    let event = mark
        .target
        .strip_prefix('#')
        .filter(|id| crate::is_identifier(id))
        .ok_or_else(|| {
            error(
                mark,
                "fermata.event",
                "Fermata target must be an event reference",
            )
        })?;
    let shape = string(mark, "shape")?
        .map(|value| match value {
            "normal" => Ok(FermataShape::Normal),
            "angled" => Ok(FermataShape::Angled),
            "square" => Ok(FermataShape::Square),
            "double-angled" => Ok(FermataShape::DoubleAngled),
            "double-square" => Ok(FermataShape::DoubleSquare),
            "double-dot" => Ok(FermataShape::DoubleDot),
            "half-curve" => Ok(FermataShape::HalfCurve),
            "curlew" => Ok(FermataShape::Curlew),
            _ => Err(error(mark, "fermata.shape", "Unknown fermata shape")),
        })
        .transpose()?;
    let orientation = string(mark, "orientation")?
        .map(|value| match value {
            "upright" => Ok(FermataOrientation::Upright),
            "inverted" => Ok(FermataOrientation::Inverted),
            _ => Err(error(
                mark,
                "fermata.orientation",
                "Unknown fermata orientation",
            )),
        })
        .transpose()?;
    Ok(ScoreFermata {
        event: EventId(event.into()),
        shape,
        orientation,
        default_y: rational(mark, "default_y")?,
        relative_y: rational(mark, "relative_y")?,
    })
}

pub(crate) fn validate(doc: &Document, out: &mut Vec<Diagnostic>) {
    let enabled = doc.required_extensions.iter().any(|s| s == EXTENSION);
    let marks: Vec<_> = doc
        .frames
        .iter()
        .flat_map(|frame| {
            frame.records.iter().filter_map(move |record| match record {
                Record::Mark(mark) if mark.kind == KIND => Some((&frame.time, mark)),
                _ => None,
            })
        })
        .collect();
    if enabled && marks.is_empty() {
        out.push(Diagnostic::error(
            "fermata.empty_requirement",
            "Fermata capability requires at least one authored sign",
            doc.source,
        ));
    }
    if marks.is_empty() {
        return;
    }
    if !enabled {
        out.push(Diagnostic::error(
            "fermata.requirement",
            "Authored fermatas require !requires scorestream.fermata@1",
            doc.source,
        ));
    }
    if doc.document_kind != DocumentKind::Score {
        out.push(Diagnostic::error(
            "fragment_global_state",
            "Fermatas cannot be introduced by a fragment",
            doc.source,
        ));
    }
    let events: BTreeMap<_, _> = doc
        .events()
        .filter_map(|(time, event)| event.id().map(|id| (id, (time, event))))
        .collect();
    let mut seen = BTreeSet::new();
    for (time, mark) in marks {
        let fermata = match decode(mark) {
            Ok(fermata) => fermata,
            Err(diagnostic) => {
                out.push(diagnostic);
                continue;
            }
        };
        if !seen.insert(fermata.event.clone()) {
            out.push(error(
                mark,
                "fermata.duplicate",
                "Multiple authored fermatas on one event are not supported",
            ));
        }
        match events.get(fermata.event.0.as_str()) {
            Some((onset, event))
                if event.duration.measured().is_some()
                    && (event.pitch.is_note() || event.pitch == Pitch::Rest) =>
            {
                if time != *onset {
                    out.push(error(
                        mark,
                        "fermata.onset",
                        "Fermata frame must equal the referenced event's exact onset",
                    ));
                }
            }
            _ => out.push(error(
                mark,
                "fermata.event",
                "Fermata target must resolve to a measured note or rest",
            )),
        }
    }
    out.push(Diagnostic::warning(
        "fermata.realization_unavailable",
        "Authored fermata signs are preserved; hold duration and performed playback are not implemented",
        doc.source,
    ));
}
