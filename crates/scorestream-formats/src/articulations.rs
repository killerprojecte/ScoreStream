//! Event-local simple signs shared by the loss-aware MusicXML and SFS bridges.
use crate::{Format, MappingEffect, MappingReport};
use scorestream_core::{
    notation::{EventId, Placement},
    score::*,
    *,
};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn validate(score: &ScoreDocument, report: &mut MappingReport) {
    let events: BTreeSet<_> = score
        .parts
        .iter()
        .flat_map(|p| &p.measures)
        .flat_map(|m| &m.events)
        .filter(|e| e.pitch.is_note())
        .map(|e| &e.id)
        .collect();
    let mut seen = BTreeSet::new();
    for a in &score.articulations {
        if !events.contains(&a.event) {
            report.push(Format::ScoreModel, MappingEffect::Invalid, "articulation.event", &a.event.0,
                "Simple articulation must reference one measured pitched note; rest, spacer and grace attachments are not guessed");
        }
        if !seen.insert((&a.event, a.value)) {
            report.push(Format::ScoreModel, MappingEffect::Unsupported, "articulation.duplicate", &a.event.0,
                "Repeated identical signs on one note have no supported engraving meaning; independent signs and chord members stay distinct");
        }
    }
}
pub(crate) fn sort(score: &mut ScoreDocument) {
    let order: BTreeMap<_, _> = score
        .parts
        .iter()
        .flat_map(|p| &p.measures)
        .flat_map(|m| &m.events)
        .enumerate()
        .map(|(i, e)| (e.id.clone(), i))
        .collect();
    score
        .articulations
        .sort_by_key(|a| (order.get(&a.event).copied(), a.value));
}
pub(crate) fn read_mark(mark: &Mark, score: &mut ScoreDocument, report: &mut MappingReport) {
    for key in mark.attributes.keys() {
        if !["value", "placement"].contains(&key.as_str()) {
            report.push_source(
                Format::Sfs,
                MappingEffect::Unsupported,
                "articulation.attribute",
                &mark.target,
                "Unmapped articulation attributes are not discarded",
                Some(mark.source),
            );
        }
    }
    let Some(value) = mark
        .attributes
        .get("value")
        .and_then(Scalar::as_str)
        .and_then(SimpleArticulation::from_name)
    else {
        report.push_source(Format::Sfs, MappingEffect::Unsupported, "articulation.value", &mark.target,
            "Only staccato, staccatissimo, tenuto and accent have a complete simple-symbol conversion mapping", Some(mark.source));
        return;
    };
    let Some(event) = mark.target.strip_prefix('#') else {
        report.push_source(
            Format::Sfs,
            MappingEffect::Unsupported,
            "articulation.scope",
            &mark.target,
            "An articulation requires an explicit event attachment",
            Some(mark.source),
        );
        return;
    };
    let placement = match mark.attributes.get("placement").and_then(Scalar::as_str) {
        Some("above") => Some(Placement::Above),
        Some("below") => Some(Placement::Below),
        _ => None, // The validated SFS AST has already rejected other values.
    };
    score.articulations.push(ScoreArticulation {
        event: EventId(event.into()),
        value,
        placement,
    });
}
pub(crate) fn write_mark(
    a: &ScoreArticulation,
    event: String,
    at: Rational,
    frames: &mut BTreeMap<Rational, Vec<Record>>,
) {
    let mut attributes =
        Attributes::from([("value".into(), Scalar::String(a.value.name().into()))]);
    if let Some(place) = a.placement {
        attributes.insert(
            "placement".into(),
            Scalar::String(
                match place {
                    Placement::Above => "above",
                    Placement::Below => "below",
                }
                .into(),
            ),
        );
    }
    frames.entry(at).or_default().push(Record::Mark(Mark {
        target: format!("#{event}"),
        kind: "articulation".into(),
        attributes,
        source: SourceRange::default(),
    }));
}
