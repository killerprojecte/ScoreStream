//! Authored fermata annotations, not durations or performer timing policy.
use crate::{Format, MappingEffect, MappingReport};
use scorestream_core::{
    Attributes, Mark, Rational, Record, Scalar, SourceRange, fermata_extension as ext,
    notation::EventId, score::*,
};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) fn authored_only(score: &ScoreDocument, format: Format, report: &mut MappingReport) {
    if !score.fermatas.is_empty() {
        report.push(format, MappingEffect::Normalized, "fermata.authored_only", "/fermatas",
            "Fermata event anchors, shapes, orientations and supported placement roundtrip as authored notation. No hold duration, synchronization, engraving or performance policy is supplied; playback rejects this capability.");
    }
}
pub(crate) fn validate(score: &ScoreDocument, report: &mut MappingReport) {
    let events: BTreeSet<_> = score
        .parts
        .iter()
        .flat_map(|p| &p.measures)
        .flat_map(|m| &m.events)
        .filter(|e| e.pitch.is_note() || e.pitch == scorestream_core::Pitch::Rest)
        .map(|e| &e.id)
        .collect();
    let mut seen = BTreeSet::new();
    for f in &score.fermatas {
        if !events.contains(&f.event) {
            report.push(
                Format::ScoreModel,
                MappingEffect::Invalid,
                "fermata.event",
                &f.event.0,
                "Fermata must reference a measured note or rest in this score",
            );
        }
        if !seen.insert(&f.event) {
            report.push(Format::ScoreModel, MappingEffect::Unsupported, "fermata.duplicate", &f.event.0,
                "Multiple fermata signs on one event are outside this bounded profile; signs on distinct part/events remain distinct");
        }
    }
}
pub(crate) fn sort(score: &mut ScoreDocument) {
    let order: BTreeMap<_, _> = score
        .parts
        .iter()
        .enumerate()
        .flat_map(|(p, part)| {
            part.measures
                .iter()
                .enumerate()
                .flat_map(move |(m, measure)| {
                    measure
                        .events
                        .iter()
                        .enumerate()
                        .map(move |(e, event)| (event.id.clone(), (p, m, e)))
                })
        })
        .collect();
    score.fermatas.sort_by_key(|f| order.get(&f.event).copied());
}
pub(crate) fn read_mark(mark: &Mark, score: &mut ScoreDocument, report: &mut MappingReport) {
    match ext::decode(mark) {
        Ok(f) => score.fermatas.push(f),
        Err(e) => report.push_source(
            Format::Sfs,
            MappingEffect::Invalid,
            e.code,
            &mark.target,
            e.message,
            Some(e.source),
        ),
    }
}
pub(crate) fn write_mark(
    f: &ScoreFermata,
    event: String,
    at: Rational,
    frames: &mut BTreeMap<Rational, Vec<Record>>,
) {
    let mut attributes = Attributes::new();
    if let Some(s) = f.shape {
        attributes.insert("shape".into(), Scalar::String(ext::shape_name(s).into()));
    }
    if let Some(o) = f.orientation {
        attributes.insert(
            "orientation".into(),
            Scalar::String(ext::orientation_name(o).into()),
        );
    }
    for (name, value) in [("default_y", &f.default_y), ("relative_y", &f.relative_y)] {
        if let Some(v) = value {
            attributes.insert(name.into(), Scalar::String(v.to_string()));
        }
    }
    frames.entry(at).or_default().push(Record::Mark(Mark {
        target: format!("#{event}"),
        kind: ext::KIND.into(),
        attributes,
        source: SourceRange::default(),
    }));
}
pub(crate) fn positions(score: &ScoreDocument) -> BTreeMap<&EventId, Rational> {
    score
        .parts
        .iter()
        .flat_map(|p| &p.measures)
        .flat_map(|m| m.events.iter().map(|e| (&e.id, &m.start + &e.offset)))
        .collect()
}
