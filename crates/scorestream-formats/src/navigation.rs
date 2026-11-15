//! Reconcile authored per-part signs into one bounded score-wide replay route.
//! This is not an audio renderer: controls and relations remain in the source score.
use crate::{Format, MappingEffect, MappingReport, validate_score};
use scorestream_core::{Rational, repeat_traversal as route, score::*};
use std::collections::BTreeSet;

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
pub use scorestream_core::navigation::Navigation;
/// Preserve omitted part markings; share one reconciliation with performed lowering.
pub fn from_score(score: &ScoreDocument, movement_id: &str) -> Result<Navigation, MappingReport> {
    let report = validate_score(score);
    if report.has_errors() {
        return Err(report);
    }
    let parts = score
        .parts
        .iter()
        .map(|p| scorestream_core::navigation::Part {
            id: p.id.clone(),
            measures: p
                .measures
                .iter()
                .map(|m| scorestream_core::navigation::Measure {
                    id: m.id.clone(),
                    label: m.label.clone(),
                    start: m.start.clone(),
                    length: m.length.clone(),
                })
                .collect(),
        })
        .collect::<Vec<_>>();
    scorestream_core::navigation::reconcile(&parts, &score.barlines, movement_id)
        .map_err(|e| unsupported(&e.code, &e.message))
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
