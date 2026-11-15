//! Reconcile authored per-part signs into one bounded score-wide replay route.
//! This is not an audio renderer: controls and relations remain in the source score.
use crate::{Format, MappingEffect, MappingReport, validate_score};
use scorestream_core::{Rational, notation::PartId, repeat_traversal as route, score::*};
use std::collections::{BTreeMap, BTreeSet};

fn issue(report: &mut MappingReport, feature: &str, path: &str, message: &str) {
    report.push(
        Format::ScoreModel,
        MappingEffect::Invalid,
        feature,
        path,
        message,
    );
}
fn unsupported(feature: &str, message: &str) -> MappingReport {
    let mut report = MappingReport::default();
    report.push(
        Format::ScoreModel,
        MappingEffect::Unsupported,
        feature,
        "/barlines",
        message,
    );
    report
}
/// Validate public typed input without inferring or repairing performance routes.
pub(crate) fn validate_barlines(score: &ScoreDocument, report: &mut MappingReport) {
    let measures: BTreeSet<_> = score
        .parts
        .iter()
        .flat_map(|p| p.measures.iter().map(move |m| (&p.id, m.id.as_str())))
        .collect();
    let mut ids: BTreeSet<&str> = score
        .parts
        .iter()
        .flat_map(|p| {
            std::iter::once(p.id.0.as_str()).chain(p.measures.iter().flat_map(|m| {
                std::iter::once(m.id.as_str()).chain(m.events.iter().map(|e| e.id.0.as_str()))
            }))
        })
        .collect();
    let mut positions = BTreeSet::new();
    for b in &score.barlines {
        let path = format!("/barlines/{}/{}", b.part, b.measure);
        if !measures.contains(&(&b.part, b.measure.as_str())) {
            issue(
                report,
                "barline.measure_reference",
                &path,
                "Barline must reference a measure in its own part",
            );
        }
        if !positions.insert((&b.part, &b.measure, b.location)) {
            issue(
                report,
                "barline.duplicate_boundary",
                &path,
                "At most one authored barline at each side of a part measure",
            );
        }
        if let Some(id) = &b.id
            && (id.is_empty() || !ids.insert(id.as_str()))
        {
            issue(
                report,
                "barline.identity",
                &path,
                "Barline ID must be nonempty and globally distinct",
            );
        }
        if b.repeat
            .as_ref()
            .is_some_and(|r| r.direction == RepeatDirection::Forward && r.times.is_some())
        {
            issue(
                report,
                "repeat.times_direction",
                &path,
                "Only a backward repeat may carry an authored play count",
            );
        }
        if let Some(e) = &b.ending {
            let passes: BTreeSet<_> = e.passes.iter().collect();
            if e.passes.is_empty() || e.passes.contains(&0) || passes.len() != e.passes.len() {
                issue(
                    report,
                    "ending.passes",
                    &path,
                    "Ending requires distinct positive pass numbers",
                );
            }
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Navigation {
    pub movement: route::Movement,
    /// Parts containing the complete agreeing repeat-sign sequence.
    pub repeat_parts: Vec<PartId>,
    /// Parts containing the complete agreeing ending-sign sequence.
    pub ending_parts: Vec<PartId>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplayOccurrence {
    pub measure_index: usize,
    pub occurrence_index: usize,
    pub written_start: Rational,
    pub performed_start: Rational,
    pub length: Rational,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScoreReplayPlan {
    pub navigation: Navigation,
    /// Checkpoints restore BEFORE authored entry controls. Skips execute no controls.
    pub traversal: route::MovementPlan,
    /// References the original aligned measures, without copying notes or state.
    pub occurrences: Vec<ReplayOccurrence>,
    pub duration_quarters: Rational,
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

fn reconcile<T: Eq>(
    canonical: &mut Option<Vec<T>>,
    source: Vec<T>,
    feature: &str,
) -> Result<bool, MappingReport> {
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
pub fn from_score(score: &ScoreDocument, movement_id: &str) -> Result<Navigation, MappingReport> {
    let report = validate_score(score);
    if report.has_errors() {
        return Err(report);
    }
    let first = &score.parts[0];
    if score.parts.iter().skip(1).any(|p| {
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
    let mut grouped: BTreeMap<&PartId, Vec<&ScoreBarline>> = BTreeMap::new();
    for b in &score.barlines {
        grouped.entry(&b.part).or_default().push(b);
    }
    let (mut repeats, mut endings) = (None, None);
    let (mut repeat_parts, mut ending_parts) = (vec![], vec![]);
    for part in &score.parts {
        let indices: BTreeMap<_, _> = part
            .measures
            .iter()
            .enumerate()
            .map(|(i, m)| (m.id.as_str(), i))
            .collect();
        let (mut rs, mut es) = (vec![], vec![]);
        for b in grouped.get(&part.id).into_iter().flatten() {
            let at =
                indices[b.measure.as_str()] + usize::from(b.location == BarlineLocation::Right);
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
        if reconcile(&mut repeats, rs, "navigation.repeat_conflict")? {
            repeat_parts.push(part.id.clone());
        }
        if reconcile(&mut endings, es, "navigation.ending_conflict")? {
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
        if let route::EndingEnd::Closed { at, .. } = ending.end
            && closed_at.insert(at, index).is_some()
        {
            return Err(unsupported(
                "navigation.ending_shape",
                "Multiple endings meet the same boundary",
            ));
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
/// Compile exact written-quarter references plus checkpoint/replay instructions.
/// No seconds, tempo policy, note duplication or sounding relation realization occurs.
pub fn plan(
    score: &ScoreDocument,
    movement_id: &str,
    policy: route::Policy,
) -> Result<ScoreReplayPlan, MappingReport> {
    let navigation = from_score(score, movement_id)?;
    let traversal = route::traverse(&navigation.movement, policy).map_err(|e| {
        unsupported(
            "navigation.traversal",
            &format!("Ordinary-repeat route rejected: {e:?}"),
        )
    })?;
    let mut occurrences = Vec::with_capacity(traversal.occurrences);
    let mut at = Rational::zero();
    for step in &traversal.steps {
        if let route::Step::ReplayMeasure {
            measure_index,
            occurrence_index,
            ..
        } = step
        {
            let m = &score.parts[0].measures[*measure_index];
            occurrences.push(ReplayOccurrence {
                measure_index: *measure_index,
                occurrence_index: *occurrence_index,
                written_start: m.start.clone(),
                performed_start: at.clone(),
                length: m.length.clone(),
            });
            at = &at + &m.length;
        }
    }
    Ok(ScoreReplayPlan {
        navigation,
        traversal,
        occurrences,
        duration_quarters: at,
    })
}
