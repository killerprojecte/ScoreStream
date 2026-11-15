//! Shared tie validation uses exact event identity, time, and effective staff pitch.
use crate::{Format, MappingEffect, MappingReport};
use scorestream_core::{
    Rational,
    notation::{EventId, PartId},
    score::*,
};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) struct EventContext<'a> {
    pub part: &'a PartId,
    pub event: &'a ScoreEvent,
    pub start: Rational,
    pub end: Rational,
    pub pitch: Option<Rational>,
}
pub(crate) fn event_index(score: &ScoreDocument) -> BTreeMap<&EventId, EventContext<'_>> {
    let mut index = BTreeMap::new();
    for part in &score.parts {
        let mut transpositions = BTreeMap::new();
        for measure in &part.measures {
            for state in &measure.states {
                if let Some(t) = &state.transpose {
                    transpositions.insert(
                        state.staff,
                        &t.chromatic
                            + &(&Rational::from_integer(t.octaves) * &Rational::from_integer(12)),
                    );
                }
            }
            for event in &measure.events {
                let start = &measure.start + &event.offset;
                index.insert(
                    &event.id,
                    EventContext {
                        part: &part.id,
                        event,
                        end: &start + &event.duration,
                        start,
                        pitch: event.pitch.semitones().map(|p| {
                            &p + transpositions
                                .get(&event.staff)
                                .unwrap_or(&Rational::zero())
                        }),
                    },
                );
            }
        }
    }
    index
}
pub(crate) fn validate(score: &ScoreDocument, report: &mut MappingReport) {
    if score.ties.is_empty() {
        return;
    }
    let index = event_index(score);
    let mut ids = BTreeSet::new();
    let mut outgoing = BTreeSet::new();
    let mut incoming = BTreeSet::new();
    let mut invalid = |tie: &ScoreTie, feature: &str, message: &str| {
        report.push(
            Format::ScoreModel,
            MappingEffect::Invalid,
            feature,
            format!("/ties/{}", tie.id),
            message,
        );
    };
    for tie in &score.ties {
        if tie.id.0.is_empty() || !ids.insert(&tie.id) {
            invalid(
                tie,
                "tie.identity",
                "Tie identity must be nonempty and unique",
            );
        }
        if tie.events.len() < 2 {
            invalid(
                tie,
                "tie.anchors",
                "A tie must link at least two distinct measured notes",
            );
        }
        for id in &tie.events {
            if !index.contains_key(id) {
                invalid(
                    tie,
                    "tie.reference",
                    "Tie anchor does not resolve to a score event",
                );
            }
        }
        for pair in tie.events.windows(2) {
            let (Some(a), Some(b)) = (index.get(&pair[0]), index.get(&pair[1])) else {
                continue;
            };
            if a.event.id == b.event.id || a.pitch.is_none() || b.pitch.is_none() {
                invalid(
                    tie,
                    "tie.anchors",
                    "Tie anchors must be distinct pitched measured notes",
                );
            }
            if a.part != b.part || a.event.voice != b.event.voice {
                invalid(
                    tie,
                    "tie.scope",
                    "This tie profile requires the same part and voice; staff changes are permitted",
                );
            }
            if a.pitch != b.pitch {
                invalid(
                    tie,
                    "tie.pitch",
                    "Tie endpoints have different effective sounding pitches",
                );
            }
            if a.end != b.start {
                invalid(
                    tie,
                    "tie.continuity",
                    "Each tie continuation must begin exactly at the preceding note tail",
                );
            }
            if !outgoing.insert(&pair[0]) || !incoming.insert(&pair[1]) {
                invalid(
                    tie,
                    "tie.branch",
                    "Duplicate or branching sounding ties are ambiguous",
                );
            }
        }
    }
}
