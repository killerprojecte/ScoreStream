//! Shared score-wide navigation reconciliation, independent of format adapters.
use crate::{notation::PartId, repeat_traversal as route, score::*, Rational};
use std::collections::{BTreeMap, BTreeSet};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Error {
    pub code: String,
    pub message: String,
}
fn unsupported(code: &str, message: &str) -> Error {
    Error {
        code: code.into(),
        message: message.into(),
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Measure {
    pub id: String,
    pub label: String,
    pub start: Rational,
    pub length: Rational,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Part {
    pub id: PartId,
    pub measures: Vec<Measure>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Navigation {
    pub movement: route::Movement,
    /// Parts containing the complete agreeing repeat-sign sequence.
    pub repeat_parts: Vec<PartId>,
    /// Parts containing the complete agreeing ending-sign sequence.
    pub ending_parts: Vec<PartId>,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct RepeatMark {
    at: usize,
    direction: RepeatDirection,
    times: Option<u32>,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct EndingMark {
    at: usize,
    phase: EndingPhase,
    passes: Vec<u32>,
}

fn reconcile_family<T: Eq>(
    canonical: &mut Option<Vec<T>>,
    source: Vec<T>,
    feature: &str,
) -> Result<bool, Error> {
    if source.is_empty() {
        return Ok(false);
    }
    if canonical.as_ref().is_some_and(|c| *c != source) {
        return Err(unsupported(
            feature,
            "Nonempty part navigation sequences disagree; no conductor part is guessed",
        ));
    }
    if canonical.is_none() {
        *canonical = Some(source);
    }
    Ok(true)
}
/// Preserve omitted per-part markings, while requiring every nonempty sequence
/// within each navigation family to agree. Display labels never locate boundaries.
pub fn reconcile(
    parts: &[Part],
    barlines: &[ScoreBarline],
    movement_id: &str,
) -> Result<Navigation, Error> {
    let Some(first) = parts.first().filter(|p| !p.measures.is_empty()) else {
        return Err(unsupported(
            "navigation.empty",
            "Navigation requires measures and parts",
        ));
    };
    if parts.iter().skip(1).any(|p| {
        p.measures.len() != first.measures.len()
            || p.measures
                .iter()
                .zip(&first.measures)
                .any(|(a, b)| a.start != b.start || a.length != b.length)
    }) {
        return Err(unsupported(
            "navigation.part_alignment",
            "Navigation requires exactly aligned partwise measure times and lengths",
        ));
    }
    let ids: BTreeSet<_> = parts.iter().map(|p| &p.id).collect();
    if ids.len() != parts.len() || barlines.iter().any(|b| !ids.contains(&b.part)) {
        return Err(unsupported(
            "navigation.part",
            "Duplicate part or unresolved barline part",
        ));
    }
    for p in parts {
        let ids: BTreeSet<_> = p.measures.iter().map(|m| &m.id).collect();
        if ids.len() != p.measures.len() {
            return Err(unsupported(
                "navigation.measure",
                "Duplicate measure identity",
            ));
        }
    }
    let mut grouped: BTreeMap<&PartId, Vec<&ScoreBarline>> = BTreeMap::new();
    for b in barlines {
        grouped.entry(&b.part).or_default().push(b);
    }
    let (mut repeats, mut endings) = (None, None);
    let (mut repeat_parts, mut ending_parts) = (vec![], vec![]);
    for part in parts {
        let indices: BTreeMap<_, _> = part
            .measures
            .iter()
            .enumerate()
            .map(|(i, m)| (m.id.as_str(), i))
            .collect();
        let (mut rs, mut es) = (vec![], vec![]);
        for b in grouped.get(&part.id).into_iter().flatten() {
            let at = *indices
                .get(b.measure.as_str())
                .ok_or_else(|| unsupported("navigation.measure", "Unresolved barline measure"))?
                + usize::from(b.location == BarlineLocation::Right);
            if let Some(r) = &b.repeat {
                rs.push(RepeatMark {
                    at,
                    direction: r.direction,
                    times: r.times,
                });
            }
            if let Some(e) = &b.ending {
                let mut passes = e.passes.clone();
                passes.sort_unstable();
                es.push(EndingMark {
                    at,
                    phase: e.phase,
                    passes,
                });
            }
        }
        rs.sort();
        es.sort();
        if reconcile_family(&mut repeats, rs, "navigation.repeat_conflict")? {
            repeat_parts.push(part.id.clone());
        }
        if reconcile_family(&mut endings, es, "navigation.ending_conflict")? {
            ending_parts.push(part.id.clone());
        }
    }
    let mut regions = vec![];
    let mut opened = None;
    for mark in repeats.unwrap_or_default() {
        match mark.direction {
            RepeatDirection::Forward => {
                if opened.replace(mark.at).is_some() {
                    return Err(unsupported(
                        "navigation.nested_repeat",
                        "Nested forward repeats require a later profile",
                    ));
                }
            }
            RepeatDirection::Backward => {
                regions.push(route::RepeatRegion {
                    id: format!("repeat{}", regions.len() + 1),
                    start: opened
                        .take()
                        .map(|at| route::RepeatStart::Explicit(route::Boundary(at)))
                        .unwrap_or(route::RepeatStart::ImplicitMovementStart),
                    backward: route::Boundary(mark.at),
                    times: mark.times,
                    endings: vec![],
                });
            }
        }
    }
    if opened.is_some() {
        return Err(unsupported(
            "navigation.unmatched_forward",
            "Forward repeat has no backward endpoint",
        ));
    }
    let mut spans = vec![];
    let mut active: Option<route::Ending> = None;
    for mark in endings.unwrap_or_default() {
        match mark.phase {
            EndingPhase::Start => {
                if active.is_some() {
                    return Err(unsupported(
                        "navigation.ending_overlap",
                        "Ending starts before the preceding ending closes",
                    ));
                }
                active = Some(route::Ending {
                    passes: mark.passes,
                    start: route::Boundary(mark.at),
                    end: route::EndingEnd::Unclosed,
                });
            }
            EndingPhase::Stop | EndingPhase::Discontinue => {
                let Some(mut e) = active.take() else {
                    return Err(unsupported(
                        "navigation.ending_endpoint",
                        "Ending endpoint has no matching start",
                    ));
                };
                if e.passes != mark.passes || e.start.0 >= mark.at {
                    return Err(unsupported(
                        "navigation.ending_endpoint",
                        "Ending endpoint pass numbers or ordering conflict with its start",
                    ));
                }
                e.end = route::EndingEnd::Closed {
                    at: route::Boundary(mark.at),
                    closure: if mark.phase == EndingPhase::Stop {
                        route::EndingClosure::Stop
                    } else {
                        route::EndingClosure::Discontinue
                    },
                };
                spans.push(e);
            }
        }
    }
    if let Some(e) = active {
        spans.push(e);
    }
    let mut closed_at = BTreeMap::new();
    for (index, ending) in spans.iter().enumerate() {
        if let route::EndingEnd::Closed { at, .. } = ending.end {
            if closed_at.insert(at, index).is_some() {
                return Err(unsupported(
                    "navigation.ending_shape",
                    "Multiple endings meet the same boundary",
                ));
            }
        }
    }
    let mut used = BTreeSet::new();
    for r in &mut regions {
        if let Some(index) = closed_at.get(&r.backward) {
            let first = &spans[*index];
            let next = index + 1;
            let Some(last) = spans.get(next).filter(|e| e.start == r.backward) else {
                return Err(unsupported(
                    "navigation.ending_shape",
                    "A first ending requires an adjacent final ending",
                ));
            };
            if !used.insert(*index) || !used.insert(next) {
                return Err(unsupported(
                    "navigation.ending_shape",
                    "Ending is shared by conflicting repeats",
                ));
            }
            r.endings = vec![first.clone(), last.clone()];
        }
    }
    if used.len() != spans.len() {
        return Err(unsupported(
            "navigation.ending_shape",
            "Ending cannot be assigned to an ordinary two-ending repeat",
        ));
    }
    Ok(Navigation {
        movement: route::Movement {
            id: movement_id.into(),
            measure_labels: first.measures.iter().map(|m| m.label.clone()).collect(),
            repeats: regions,
        },
        repeat_parts,
        ending_parts,
    })
}
