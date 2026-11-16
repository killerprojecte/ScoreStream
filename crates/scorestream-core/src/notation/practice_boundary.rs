//! Sealed source/policy/route witness for the explicitly selected boundary practice path.
use super::*;
use crate::{Diagnostic, Document, Record, Severity};
#[derive(Clone, Debug)]
pub struct QualifiedBoundaryPractice {
    source: Document,
    projection: Document,
    base: QualifiedPracticePerformance,
    schedule: AuthoredDynamicSchedule,
    policy: PracticeBoundaryAttackPolicy,
}
impl QualifiedBoundaryPractice {
    pub fn source(&self) -> &Document {
        &self.source
    }
    pub fn interpretation_projection(&self) -> &Document {
        &self.projection
    }
    pub fn joint(&self) -> &QualifiedCombinedPerformance {
        self.base.joint()
    }
    pub fn schedule(&self) -> &AuthoredDynamicSchedule {
        &self.schedule
    }
    pub fn resolve_attacks(
        &self,
        attacks: &[QualifiedPracticeAttack],
    ) -> Result<PracticeAttackResolution, Vec<Diagnostic>> {
        super::authored_dynamic_attacks::resolve_on_schedule(
            &self.source,
            attacks,
            self.policy,
            self.schedule.clone(),
        )
    }
}
/// Only this explicit source-retaining policy can quarantine the authored lane.
/// Every actual occurrence must be consumed by a proven adjacent pair; extra lifted
/// controls, skipped/unpaired statements and unresolved groups are not discarded.
#[allow(clippy::too_many_arguments)]
pub fn perform_document_practice_with_boundary_attacks(
    doc: &Document,
    boundary: PracticeBoundaryAttackPolicy,
    practice: PracticeDiscoveryPolicy,
    routes: &[PracticeMidiSelection],
    articulation: ExplicitArticulationPolicy,
    grace: &ExplicitGracePolicy,
    grace_sound: GraceSoundPolicy,
    holds: &crate::fermata_timing::ExplicitSynchronizedFermataPolicy,
) -> Result<QualifiedBoundaryPractice, Vec<Diagnostic>> {
    let errors: Vec<_> = crate::validate(doc, None)
        .into_iter()
        .filter(|d| d.severity == Severity::Error)
        .collect();
    if !errors.is_empty() {
        return Err(errors);
    }
    let source = crate::authored_dynamics::decode(doc)?;
    let fail = |m: &str| {
        vec![Diagnostic::error(
            "practice.boundary_coverage",
            m,
            doc.source,
        )]
    };
    if source.is_empty() {
        return Err(fail("Explicit boundary practice requires an authored lane"));
    }
    let mut projection = doc.clone();
    projection
        .required_extensions
        .retain(|e| e != crate::authored_dynamics::EXTENSION);
    for f in &mut projection.frames {
        f.records
            .retain(|r| !matches!(r,Record::Mark(m) if m.kind==crate::authored_dynamics::KIND));
    }
    let base = perform_document_practice_joint(
        &projection,
        practice,
        routes,
        articulation,
        grace,
        grace_sound,
        holds,
    )?;
    let schedule = super::authored_dynamic_schedule::inspect_on_visits(
        doc,
        AuthoredDynamicSchedulePolicy::AdjacentWrittenOwnersV1,
        PerformanceLimits::default(),
        Some(base.joint().performed().measures.clone()),
    )?;
    let mut covered = std::collections::BTreeSet::new();
    for b in &schedule.boundaries {
        let Some(pair) = b.terminal_then_opening else {
            return Err(fail(
                "Lifted statement needs separate semantics: an occurrence is not a proven adjacent terminal/opening pair",
            ));
        };
        for i in pair {
            if !covered.insert(i) {
                return Err(fail("Repeated coverage of one authored occurrence"));
            }
        }
    }
    if covered.len() != schedule.statements.len()
        || source
            .iter()
            .enumerate()
            .any(|(i, _)| !schedule.statements.iter().any(|s| s.source_index == i))
    {
        return Err(fail(
            "Every retained source statement and occurrence requires explicit paired coverage",
        ));
    }
    Ok(QualifiedBoundaryPractice {
        source: doc.clone(),
        projection,
        base,
        schedule,
        policy: boundary,
    })
}
