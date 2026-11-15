//! Common-model grace validation. Graces never masquerade as measured notes.
use crate::{Format, MappingEffect, MappingReport};
use scorestream_core::{Pitch, score::*};
use std::collections::{BTreeMap, BTreeSet};
pub(crate) fn authored_only(score: &ScoreDocument, format: Format, r: &mut MappingReport) {
    if !score.graces.is_empty() {
        r.push(format,MappingEffect::Preserved,"grace.authored_only","/graces","Grace pitch, written rhythm, principal placement, slash absence and explicit timing intent are retained. No performed duration is inferred; exact playback requires a supported grace policy.");
    }
}
pub(crate) fn validate(score: &ScoreDocument, r: &mut MappingReport) {
    let index = crate::ties::event_index(score);
    let mut ids: BTreeSet<_> = index.keys().map(|id| &id.0).collect();
    let mut groups = BTreeMap::new();
    for g in &score.graces {
        let mut invalid = |feature: &str, message: &str| {
            r.push(
                Format::ScoreModel,
                MappingEffect::Invalid,
                feature,
                &g.id.0,
                message,
            )
        };
        if g.id.0.is_empty() || !ids.insert(&g.id.0) {
            invalid(
                "grace.identity",
                "Grace identity must be nonempty and unique across all musical events",
            );
        }
        if index.get(&g.anchor).is_none_or(|a| {
            !matches!(a.event.pitch, Pitch::Pitched { .. }) || a.event.chord.is_some()
        }) {
            invalid(
                "grace.anchor",
                "Grace principal must be a measured pitched non-chord event",
            );
        }
        if !matches!(g.pitch,Pitch::Pitched{step,..}if ('A'..='G').contains(&step)) {
            invalid("grace.pitch", "Only pitched grace notes are mapped");
        }
        if g.order >= 0 {
            invalid(
                "grace.order",
                "Only before-principal grace placement is mapped",
            );
        }
        if !g.timing.valid() {
            invalid(
                "grace.timing",
                "Steal percentages must be 0 through 100; make-time must be nonnegative",
            );
        }
        if g.written.as_ref().is_some_and(|w| {
            w.dots > 32
                || w.time_modification.as_ref().is_some_and(|t| {
                    t.actual == 0
                        || t.normal == 0
                        || t.normal_dots > 32
                        || (t.normal_type.is_none() && t.normal_dots > 0)
                })
        }) {
            invalid("grace.written", "Invalid written grace rhythm");
        }
        groups
            .entry(&g.anchor)
            .or_insert_with(Vec::new)
            .push(g.order);
    }
    for (anchor, orders) in &mut groups {
        orders.sort();
        let n = orders.len() as i64;
        if orders.iter().enumerate().any(|(i, o)| *o != i as i64 - n) {
            r.push(
                Format::ScoreModel,
                MappingEffect::Invalid,
                "grace.order",
                &anchor.0,
                "Grace orders must be consecutive -N through -1 per principal",
            );
        }
    }
}

pub(crate) fn sort(score: &mut ScoreDocument) {
    let order: BTreeMap<_, _> = score
        .parts
        .iter()
        .enumerate()
        .flat_map(|(pi, p)| {
            p.measures.iter().enumerate().flat_map(move |(mi, m)| {
                m.events
                    .iter()
                    .enumerate()
                    .map(move |(ei, e)| (e.id.clone(), (pi, mi, ei)))
            })
        })
        .collect();
    score
        .graces
        .sort_by_key(|g| (order.get(&g.anchor).copied(), g.order));
}
