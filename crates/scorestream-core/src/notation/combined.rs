//! Source-qualified composition of explicit quarter-based graces and additive
//! fermata seconds. No independent score/schedule pair or ignore flag is public.
use super::{fermata, grace, lower::LoweringProfile, perform, *};
use crate::{
    Diagnostic, Document, Rational, Record, Severity, fermata_extension,
    fermata_timing::{self, ExplicitFermataPolicy, SeekPosition, TimingLimits, TimingProjection},
};
use std::collections::BTreeSet;

/// Policy-entry metadata from one unchanged source and one performed route.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CombinedPerformanceDiscovery {
    pub graces: GraceDiscovery,
    pub fermatas: FermataDiscovery,
}
/// Read-only proof of complete source consumption under both explicit policies.
/// Grace seconds here are FINAL hold-aware times, never the base grace clock.
#[derive(Clone, Debug)]
pub struct QualifiedCombinedPerformance {
    performed: PerformedScore,
    timing: TimingProjection,
    grace_discovery: GraceDiscovery,
    grace_windows: Vec<GraceSoundWindow>,
    grace_groups: Vec<GraceGroupRealization>,
    grace_sound_policy: GraceSoundPolicy,
    default_tempo_intervals: Vec<GraceDefaultTempoInterval>,
}
impl QualifiedCombinedPerformance {
    pub fn performed(&self) -> &PerformedScore {
        &self.performed
    }
    pub fn timing(&self) -> &TimingProjection {
        &self.timing
    }
    pub fn grace_discovery(&self) -> &GraceDiscovery {
        &self.grace_discovery
    }
    pub fn grace_windows(&self) -> &[GraceSoundWindow] {
        &self.grace_windows
    }
    pub fn grace_groups(&self) -> &[GraceGroupRealization] {
        &self.grace_groups
    }
    pub fn grace_sound_policy(&self) -> GraceSoundPolicy {
        self.grace_sound_policy
    }
    pub fn default_tempo_intervals(&self) -> &[GraceDefaultTempoInterval] {
        &self.default_tempo_intervals
    }
    pub fn seek(&self, seconds: &Rational) -> SeekPosition {
        self.timing.seek(seconds)
    }
}
fn fail(doc: &Document, code: &str, message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::error(code, message, doc.source)]
}
fn bounded(r: &Rational, bits: u64) -> bool {
    r.numerator().bits() <= bits && r.denominator().bits() <= bits
}
fn limits(
    doc: &Document,
    grace: &GraceTimingLimits,
    timing: &TimingLimits,
) -> Result<(GraceTimingLimits, TimingLimits), Vec<Diagnostic>> {
    for (bits, seconds) in [
        (grace.maximum_rational_bits, &grace.maximum_timeline_seconds),
        (
            timing.maximum_rational_bits,
            &timing.maximum_timeline_seconds,
        ),
    ] {
        if bits == 0 || seconds.as_ref().is_some_and(|limit| !limit.is_positive()) {
            return Err(fail(
                doc,
                "combined.invalid_limits",
                "Both profiles require a positive bit budget and positive optional timeline limit",
            ));
        }
    }
    let mut grace = grace.clone();
    let mut timing = timing.clone();
    let bits = grace
        .maximum_rational_bits
        .min(timing.maximum_rational_bits);
    let seconds = match (
        &grace.maximum_timeline_seconds,
        &timing.maximum_timeline_seconds,
    ) {
        (Some(a), Some(b)) => Some(a.min(b).clone()),
        (Some(a), None) | (None, Some(a)) => Some(a.clone()),
        (None, None) => None,
    };
    let events = grace.maximum_events.min(timing.maximum_events);
    let records = grace
        .maximum_source_records
        .min(timing.maximum_source_records);
    grace.maximum_rational_bits = bits;
    timing.maximum_rational_bits = bits;
    grace.maximum_timeline_seconds = seconds.clone();
    timing.maximum_timeline_seconds = seconds;
    grace.maximum_events = events;
    timing.maximum_events = events;
    grace.maximum_source_records = records;
    timing.maximum_source_records = records;
    // Conservatively cap every projected control, including restored defaults,
    // by both budgets before route allocation rather than counting tempos later.
    grace.maximum_controls = grace.maximum_controls.min(timing.maximum_tempos);
    Ok((grace, timing))
}
/// Read-only source preflight for codec use before semantic validation. It bounds
/// both interpretations, including authored controls and grace timing scalars.
pub fn preflight_combined_source(
    doc: &Document,
    grace_limits: &GraceTimingLimits,
    timing_limits: &TimingLimits,
) -> Result<(), Vec<Diagnostic>> {
    let (grace_limits, timing_limits) = limits(doc, grace_limits, timing_limits)?;
    grace::preflight_grace_source(doc, &grace_limits)?;
    let mut fermatas = 0usize;
    let mut controls = 0usize;
    let mut measures = 0usize;
    for record in doc.frames.iter().flat_map(|f| &f.records) {
        match record {
            Record::Measure(_) => measures += 1,
            Record::State(_) | Record::Mark(_) | Record::Relation(_) => controls += 1,
            _ => (),
        }
        if matches!(record, Record::Mark(m) if m.kind == fermata_extension::KIND) {
            fermatas += 1;
        }
    }
    let mut members = 0usize;
    let mut principals = BTreeSet::new();
    for (_, event) in doc.events().filter(|(_, e)| e.is_grace()) {
        members += 1;
        if let Some(anchor) = event
            .attributes
            .get("anchor")
            .and_then(crate::Scalar::as_str)
        {
            principals.insert(anchor);
        }
    }
    if fermatas > timing_limits.maximum_authored_fermatas
        || measures > timing_limits.maximum_measures
        || controls > grace_limits.maximum_controls
        || members > grace_limits.maximum_members
        || principals.len() > grace_limits.maximum_groups
    {
        return Err(fail(
            doc,
            "combined.timing_budget",
            "Source signs, measures, controls or grace groups exceed the combined budget before validation",
        ));
    }
    Ok(())
}
fn preflight_fermata_policy(
    doc: &Document,
    policy: &ExplicitFermataPolicy,
    limits: &TimingLimits,
) -> Result<(), Vec<Diagnostic>> {
    if policy.groups.len() > limits.maximum_hold_groups {
        return Err(fail(
            doc,
            "fermata.timing_budget",
            "Hold group budget exceeded before route allocation",
        ));
    }
    let mut members = 0usize;
    for group in &policy.groups {
        members = members
            .checked_add(group.members.len())
            .ok_or_else(|| fail(doc, "fermata.timing_budget", "Policy member count overflow"))?;
        if members > limits.maximum_group_members {
            return Err(fail(
                doc,
                "fermata.timing_budget",
                "Hold member budget exceeded before route allocation",
            ));
        }
        if !bounded(&group.added_seconds, limits.maximum_rational_bits) {
            return Err(fail(
                doc,
                "fermata.rational_budget",
                "Policy hold seconds exceed bit budget before route allocation",
            ));
        }
    }
    Ok(())
}
fn prepare(
    doc: &Document,
    mut performance: PerformanceLimits,
    grace: &GraceTimingLimits,
    timing: &TimingLimits,
) -> Result<PerformedScore, Vec<Diagnostic>> {
    preflight_combined_source(doc, grace, timing)?;
    let errors: Vec<_> = crate::validate(doc, None)
        .into_iter()
        .filter(|d| d.severity == Severity::Error)
        .collect();
    if !errors.is_empty() {
        return Err(errors);
    }
    if doc.events().any(|(_, e)| e.id().is_none()) {
        return Err(fail(
            doc,
            "combined.source_identity",
            "Every source event requires a stable ID",
        ));
    }
    performance.maximum_events = performance.maximum_events.min(grace.maximum_events);
    performance.maximum_controls = performance.maximum_controls.min(grace.maximum_controls);
    // Each route occurrence emits one measure receipt per part.
    performance.maximum_occurrences = performance
        .maximum_occurrences
        .min(timing.maximum_measures / doc.parts.len().max(1))
        .min(grace.maximum_controls);
    perform::perform(
        doc,
        performance,
        LoweringProfile::ExplicitGracesAndFermatas,
        Some(grace.maximum_rational_bits),
        Some((grace.maximum_members, grace.maximum_groups)),
    )
    .map_err(|e| vec![e])
}
pub fn discover_document_graces_and_fermatas(
    doc: &Document,
) -> Result<CombinedPerformanceDiscovery, Vec<Diagnostic>> {
    let (grace_limits, timing_limits) =
        limits(doc, &GraceTimingLimits::default(), &TimingLimits::default())?;
    let performed = prepare(
        doc,
        PerformanceLimits::default(),
        &grace_limits,
        &timing_limits,
    )?;
    Ok(CombinedPerformanceDiscovery {
        graces: grace::discover(doc, &performed)?,
        fermatas: fermata::discover(doc, &performed, &timing_limits)?,
    })
}
/// First allocate quarters from each principal, then apply global additive holds.
/// A hold at group onset is preceding time; a hold strictly inside or at the
/// nominal principal endpoint rejects, even if its sign belongs to another part.
#[allow(clippy::too_many_arguments)]
pub fn perform_document_with_explicit_graces_and_fermatas(
    doc: &Document,
    grace_policy: &ExplicitGracePolicy,
    grace_sound_policy: GraceSoundPolicy,
    fermata_policy: &ExplicitFermataPolicy,
    performance_limits: PerformanceLimits,
    grace_limits: GraceTimingLimits,
    timing_limits: TimingLimits,
) -> Result<QualifiedCombinedPerformance, Vec<Diagnostic>> {
    perform_combined(
        doc,
        grace_policy,
        grace_sound_policy,
        fermata_policy,
        None,
        performance_limits,
        grace_limits,
        timing_limits,
    )
}

/// The same source-qualified combined schedule, with caller-selected latest
/// releases for earlier rest anchors. No policy or grace schedule is inferred.
#[allow(clippy::too_many_arguments)]
pub fn perform_document_with_explicit_graces_and_synchronized_fermatas(
    doc: &Document,
    grace_policy: &ExplicitGracePolicy,
    grace_sound_policy: GraceSoundPolicy,
    fermata_policy: &fermata_timing::ExplicitSynchronizedFermataPolicy,
    performance_limits: PerformanceLimits,
    grace_limits: GraceTimingLimits,
    timing_limits: TimingLimits,
) -> Result<QualifiedCombinedPerformance, Vec<Diagnostic>> {
    let policy = ExplicitFermataPolicy {
        groups: fermata_policy
            .groups
            .iter()
            .map(|g| g.hold.clone())
            .collect(),
    };
    let boundaries: Vec<_> = fermata_policy
        .groups
        .iter()
        .map(|g| g.boundary.clone())
        .collect();
    perform_combined(
        doc,
        grace_policy,
        grace_sound_policy,
        &policy,
        Some(&boundaries),
        performance_limits,
        grace_limits,
        timing_limits,
    )
}

#[allow(clippy::too_many_arguments)]
fn perform_combined(
    doc: &Document,
    grace_policy: &ExplicitGracePolicy,
    grace_sound_policy: GraceSoundPolicy,
    fermata_policy: &ExplicitFermataPolicy,
    synchronization: Option<&[Rational]>,
    performance_limits: PerformanceLimits,
    grace_limits: GraceTimingLimits,
    timing_limits: TimingLimits,
) -> Result<QualifiedCombinedPerformance, Vec<Diagnostic>> {
    let (grace_limits, timing_limits) = limits(doc, &grace_limits, &timing_limits)?;
    if synchronization.is_some_and(|boundaries| {
        boundaries
            .iter()
            .any(|b| !bounded(b, timing_limits.maximum_rational_bits))
    }) {
        return Err(fail(
            doc,
            "combined.rational_budget",
            "Explicit synchronized boundaries exceed the configured exact arithmetic budget",
        ));
    }
    grace::preflight_policy(doc, grace_policy, &grace_limits)?;
    preflight_fermata_policy(doc, fermata_policy, &timing_limits)?;
    let performed = prepare(doc, performance_limits, &grace_limits, &timing_limits)?;
    let schedule = grace::allocate(
        doc,
        &performed,
        grace_policy,
        grace_sound_policy,
        &grace_limits,
    )?;
    let mut timing = match synchronization {
        Some(boundaries) => fermata_timing::interpret_qualified_graces_synchronized(
            doc,
            &performed,
            fermata_policy,
            timing_limits,
            &schedule,
            boundaries,
        ),
        None => fermata_timing::interpret_qualified_graces(
            doc,
            &performed,
            fermata_policy,
            timing_limits,
            &schedule,
        ),
    }
    .map_err(|e| fail(doc, e.code, e.message))?;
    let affected: BTreeSet<_> = schedule.windows().iter().map(|w| &w.id).collect();
    for sign in &timing.fermatas {
        if affected.contains(&sign.id) {
            return Err(fail(
                doc,
                "combined.affected_fermata_anchor",
                format!(
                    "Fermata anchor {} is a scheduled grace/principal at boundary {}",
                    sign.id, sign.quarter_end
                ),
            ));
        }
    }
    for group in schedule
        .discovery()
        .occurrences
        .iter()
        .filter(|g| g.order == -1)
    {
        let lo = timing
            .holds()
            .partition_point(|h| h.boundary <= group.principal_start);
        if let Some(hold) = timing
            .holds()
            .get(lo)
            .filter(|h| h.boundary <= group.principal_end)
        {
            return Err(fail(
                doc,
                "combined.grace_hold_overlap",
                format!(
                    "Grace principal {} nominal interval [{}, {}] contains hold {} at boundary {} (members {:?}); only a hold at the principal onset is supported",
                    group.principal,
                    group.principal_start,
                    group.principal_end,
                    hold.id,
                    hold.boundary,
                    hold.members
                ),
            ));
        }
    }
    let mut windows = schedule.windows().to_vec();
    for window in &mut windows {
        window.seconds_start = timing.after(&window.quarter_start);
        window.seconds_end = timing.after(&window.quarter_end);
        let duration = &window.seconds_end - &window.seconds_start;
        if [&window.seconds_start, &window.seconds_end, &duration]
            .iter()
            .any(|r| !bounded(r, grace_limits.maximum_rational_bits))
        {
            return Err(fail(
                doc,
                "combined.rational_budget",
                format!(
                    "Final hold-aware grace/principal window {} exceeds exact bit budget",
                    window.id
                ),
            ));
        }
        let start = window.seconds_start.to_f64();
        let end = window.seconds_end.to_f64();
        if duration < "1/1000000".parse().unwrap()
            || !start.is_finite()
            || !end.is_finite()
            || end - start < 0.000_001
        {
            return Err(fail(
                doc,
                "combined.clock_precision",
                format!(
                    "Final hold-aware window {} collapses or is shorter than the one-microsecond audio minimum",
                    window.id
                ),
            ));
        }
    }
    fermata::retain_default_tempo_provenance(&performed, &mut timing);
    Ok(QualifiedCombinedPerformance {
        performed,
        timing,
        grace_discovery: schedule.discovery().clone(),
        grace_windows: windows,
        grace_groups: schedule.groups().to_vec(),
        grace_sound_policy,
        default_tempo_intervals: schedule.default_tempo_intervals().to_vec(),
    })
}

#[cfg(test)]
mod tests;
