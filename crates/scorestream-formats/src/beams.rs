//! Authored per-event beam spelling. Never infer grouping from time or rewrite rhythm.
use crate::{Format, MappingEffect, MappingReport};
use scorestream_core::{Rational, Severity, beam_extension, notation::EventId, score::*};
use std::collections::BTreeMap;

pub(crate) fn validate(score: &ScoreDocument, report: &mut MappingReport) {
    for d in beam_extension::validate_score(score) {
        report.push_source(
            Format::ScoreModel,
            if d.severity == Severity::Error
                && (d.code == "beam.profile"
                    || d.code == "beam.written"
                    || d.code == "beam.primary_hook_unsupported"
                    || d.code == "beam.chord_profile"
                    || d.code == "beam.chord_rhythm"
                    || d.code == "beam.chord_stem"
                    || d.code == "beam.chord_member")
            {
                MappingEffect::Unsupported
            } else if d.severity == Severity::Error {
                MappingEffect::Invalid
            } else {
                MappingEffect::Preserved
            },
            d.code,
            d.object_id
                .map(|id| format!("/beams/{id}"))
                .unwrap_or_else(|| "/beams".into()),
            d.message,
            Some(d.source),
        );
    }
}

pub(crate) fn positions(score: &ScoreDocument) -> BTreeMap<EventId, Rational> {
    let mut out: BTreeMap<_, _> = score
        .parts
        .iter()
        .flat_map(|p| &p.measures)
        .flat_map(|m| {
            m.events
                .iter()
                .map(move |e| (e.id.clone(), &m.start + &e.offset))
        })
        .collect();
    for g in &score.graces {
        if let Some(at) = out.get(&g.anchor).cloned() {
            out.insert(g.id.clone(), at);
        }
    }
    out
}

pub(crate) fn sort(score: &mut ScoreDocument) {
    let mut order = BTreeMap::new();
    for (pi, p) in score.parts.iter().enumerate() {
        for (mi, m) in p.measures.iter().enumerate() {
            for e in &m.events {
                order.insert(
                    e.id.clone(),
                    (pi, mi, e.offset.clone(), e.voice.0.clone(), 0),
                );
            }
        }
    }
    for g in &score.graces {
        if let Some((pi, mi, at, voice, _)) = order.get(&g.anchor).cloned() {
            order.insert(g.id.clone(), (pi, mi, at, voice, g.order));
        }
    }
    score
        .beams
        .sort_by_key(|b| (order.get(&b.event).cloned(), b.number.unwrap_or(1)));
}
