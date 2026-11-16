//! Source-qualified following-principal grace timing. Authored events remain
//! unmeasured; the separately typed schedule is never an editable score.
use super::{lower::LoweringProfile, perform, *};
use crate::{
    Diagnostic, Document, Rational, Record, Scalar, Severity, grace_extension, score::GraceTiming,
    tempo_clock::ExactTempoMap,
};
use std::collections::{BTreeMap, BTreeSet};

/// No default allocation, duration, grouping or sound policy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExplicitGracePolicy {
    pub groups: Vec<ExplicitGraceGroup>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExplicitGraceGroup {
    /// Unique performed principal ID, never merely its written ID.
    pub principal: EventId,
    pub allocation: GraceAllocation,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GraceAllocation {
    /// Exactly one grace with only a positive, less-than-100 authored following
    /// percentage, over a constant effective tempo interval.
    AuthoredFollowingPercent,
    /// Every authored timing field must be absent. Members may be supplied in any
    /// order; playback always follows the source's consecutive negative order.
    ExplicitFollowingQuarters {
        members: Vec<GraceQuarterAllocation>,
    },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraceQuarterAllocation {
    pub event: EventId,
    pub quarters: Rational,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GraceSoundPolicy {
    /// Full grace windows, then a principal gate of 23/25 of the REMAINING time.
    /// This order/gate is playback policy, not a rule supplied by MusicXML.
    FullGraceThenGatedRemainder,
    /// Practice-only explicit composition: unchanged full grace slots; a
    /// staccato principal gates half its remaining interval, accent affects its
    /// own fresh attack. Unmarked principal keeps 23/25. No grace-note marks.
    PracticePrincipalStaccatoAccentInAllocatedRemainderV1,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraceTimingLimits {
    pub maximum_source_records: usize,
    pub maximum_events: usize,
    pub maximum_controls: usize,
    pub maximum_groups: usize,
    pub maximum_members: usize,
    pub maximum_rational_bits: u64,
    /// Optional positive caller-selected duration budget; None imposes no fixed ceiling.
    pub maximum_timeline_seconds: Option<Rational>,
}
impl Default for GraceTimingLimits {
    fn default() -> Self {
        Self {
            maximum_source_records: usize::MAX,
            maximum_events: usize::MAX,
            maximum_controls: usize::MAX,
            maximum_groups: usize::MAX,
            maximum_members: usize::MAX,
            maximum_rational_bits: 4096,
            maximum_timeline_seconds: None,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthoredGraceAudit {
    pub written_id: EventId,
    pub written_principal: EventId,
    pub order: i64,
    /// Retains absent/false/true directly from the source Event.
    pub slash: Option<bool>,
    pub timing: GraceTiming,
    pub occurrence_ids: Vec<EventId>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraceOccurrence {
    pub id: EventId,
    pub written_id: EventId,
    pub principal: EventId,
    pub written_principal: EventId,
    pub order: i64,
    pub slash: Option<bool>,
    pub timing: GraceTiming,
    pub measure_occurrence: usize,
    pub part: PartId,
    pub written_anchor_quarter: Rational,
    pub anchor_quarter: Rational,
    /// The principal's nominal anchor interval, not a measured grace duration.
    pub written_principal_start: Rational,
    pub written_principal_end: Rational,
    pub principal_start: Rational,
    pub principal_end: Rational,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraceDiscovery {
    pub authored: Vec<AuthoredGraceAudit>,
    pub occurrences: Vec<GraceOccurrence>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraceSoundWindow {
    pub id: EventId,
    pub principal: EventId,
    pub role: EventRole,
    pub quarter_start: Rational,
    pub quarter_end: Rational,
    pub seconds_start: Rational,
    pub seconds_end: Rational,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraceGroupRealization {
    pub principal: EventId,
    pub allocation: GraceAllocation,
    pub allocated_quarters: Rational,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraceDefaultTempoInterval {
    pub from_quarter: Rational,
    pub until_quarter: Rational,
    pub quarter_bpm: Rational,
}
/// Private construction, no mutable access. Contains all source audits, actual
/// sound windows and the same exact clock used by the audio compiler.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraceSchedule {
    default_tempo_intervals: Vec<GraceDefaultTempoInterval>,
    discovery: GraceDiscovery,
    windows: Vec<GraceSoundWindow>,
    groups: Vec<GraceGroupRealization>,
    sound_policy: GraceSoundPolicy,
    clock: ExactTempoMap,
    maximum_rational_bits: u64,
}
impl GraceSchedule {
    pub fn default_tempo_intervals(&self) -> &[GraceDefaultTempoInterval] {
        &self.default_tempo_intervals
    }
    pub fn discovery(&self) -> &GraceDiscovery {
        &self.discovery
    }
    pub fn windows(&self) -> &[GraceSoundWindow] {
        &self.windows
    }
    pub fn groups(&self) -> &[GraceGroupRealization] {
        &self.groups
    }
    pub fn sound_policy(&self) -> GraceSoundPolicy {
        self.sound_policy
    }
    pub fn quarter_end(&self) -> &Rational {
        self.clock.quarter_end()
    }
    pub fn duration_seconds(&self) -> Rational {
        self.clock.seconds(self.clock.quarter_end())
    }
    /// Unlike seek, compiler queries must not silently clamp an invalid release.
    pub fn seconds(&self, quarter: &Rational) -> Result<Rational, String> {
        if quarter.is_negative() || quarter > self.quarter_end() {
            return Err("grace.clock_range: Query lies outside the performed score".into());
        }
        let seconds = self
            .clock
            .checked_seconds(quarter)
            .map_err(|e| format!("grace.rational_budget: {e}"))?;
        if !bounded(quarter, self.maximum_rational_bits)
            || !bounded(&seconds, self.maximum_rational_bits)
        {
            return Err("grace.rational_budget: Clock query exceeds exact bit budget".into());
        }
        Ok(seconds)
    }
    /// Exact musical seek; inspect windows() for sounding graces/principals.
    /// Original grace marker intervals remain zero-length at principal onset.
    pub fn seek(&self, seconds: &Rational) -> Rational {
        self.clock.seek(seconds)
    }
    pub fn boundaries<'a>(
        &'a self,
        start: &Rational,
        end: &Rational,
    ) -> impl Iterator<Item = &'a Rational> {
        self.clock.boundaries(start, end)
    }
}
#[derive(Clone, Debug)]
pub struct QualifiedGracePerformance {
    performed: PerformedScore,
    schedule: GraceSchedule,
}
impl QualifiedGracePerformance {
    pub fn performed(&self) -> &PerformedScore {
        &self.performed
    }
    pub fn schedule(&self) -> &GraceSchedule {
        &self.schedule
    }
    pub fn into_schedule(self) -> GraceSchedule {
        self.schedule
    }
}
fn fail(doc: &Document, code: &str, message: impl Into<String>) -> Vec<Diagnostic> {
    vec![Diagnostic::error(code, message, doc.source)]
}
fn bounded(r: &Rational, bits: u64) -> bool {
    r.numerator().bits() <= bits && r.denominator().bits() <= bits
}
fn check(doc: &Document, r: &Rational, limits: &GraceTimingLimits) -> Result<(), Vec<Diagnostic>> {
    if bounded(r, limits.maximum_rational_bits) {
        Ok(())
    } else {
        Err(fail(
            doc,
            "grace.rational_budget",
            "Exact grace timing exceeds rational bit budget",
        ))
    }
}
/// Resource preflight only, suitable for a codec callback before semantic
/// validation. This returns no score, performs no projection, and never replaces
/// validation in discovery or qualification. Parsing itself precedes this check.
pub fn preflight_grace_source(
    doc: &Document,
    limits: &GraceTimingLimits,
) -> Result<(), Vec<Diagnostic>> {
    if limits.maximum_rational_bits == 0
        || limits
            .maximum_timeline_seconds
            .as_ref()
            .is_some_and(|limit| !limit.is_positive())
    {
        return Err(fail(
            doc,
            "grace.invalid_limits",
            "Positive bit budget and positive optional timeline limit required",
        ));
    }
    let records = doc
        .frames
        .iter()
        .try_fold(0usize, |n, f| n.checked_add(f.records.len()));
    if records.is_none_or(|n| n > limits.maximum_source_records)
        || doc.frames.len() > limits.maximum_source_records
        || doc.parts.len() > limits.maximum_source_records
        || doc.voices.len() > limits.maximum_source_records
        || doc.event_count() > limits.maximum_events
    {
        return Err(fail(
            doc,
            "grace.timing_budget",
            "Source record/event budget exceeded before validation",
        ));
    }
    check(doc, &doc.end, limits)?;
    let max_chars = limits
        .maximum_rational_bits
        .saturating_mul(2)
        .saturating_add(64);
    for frame in &doc.frames {
        check(doc, &frame.time, limits)?;
        for record in &frame.records {
            match record {
                Record::Measure(m) => check(doc, &m.length, limits)?,
                Record::Event(e) => {
                    if let Some(d) = e.duration.measured() {
                        check(doc, d, limits)?;
                    }
                }
                _ => (),
            }
            let attributes = match record {
                Record::State(s) => Some((&s.attributes, false)),
                Record::Mark(m) if m.kind == grace_extension::KIND => Some((&m.attributes, true)),
                _ => None,
            };
            if let Some((attributes, grace)) = attributes {
                let keys: &[&str] = if grace {
                    &["steal_previous", "steal_following", "make_time"]
                } else {
                    &["tempo", "sound_dynamics", "transpose_chromatic"]
                };
                for key in keys {
                    if let Some(value) = attributes.get(*key).and_then(Scalar::as_i64) {
                        check(doc, &Rational::from_integer(value), limits)?;
                    }
                    if let Some(value) = attributes.get(*key).and_then(Scalar::as_str) {
                        let value = if *key == "tempo" {
                            value.split_once(':').map(|(_, v)| v).unwrap_or(value)
                        } else {
                            value
                        };
                        if value.len() as u64 > max_chars {
                            return Err(fail(
                                doc,
                                "grace.rational_budget",
                                "Source timing scalar exceeds length budget before validation",
                            ));
                        }
                        if let Ok(value) = value.parse::<Rational>() {
                            check(doc, &value, limits)?;
                        }
                    }
                }
            }
        }
    }
    Ok(())
}
fn prepare(
    doc: &Document,
    mut performance: PerformanceLimits,
    limits: &GraceTimingLimits,
) -> Result<PerformedScore, Vec<Diagnostic>> {
    preflight_grace_source(doc, limits)?;
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
            "grace.source_identity",
            "Every source event requires a stable ID",
        ));
    }
    // These limits constrain allocation, not just a completed route afterward.
    performance.maximum_events = performance.maximum_events.min(limits.maximum_events);
    performance.maximum_controls = performance.maximum_controls.min(limits.maximum_controls);
    performance.maximum_occurrences = performance.maximum_occurrences.min(limits.maximum_controls);
    perform::perform(
        doc,
        performance,
        LoweringProfile::ExplicitGraces,
        Some(limits.maximum_rational_bits),
        Some((limits.maximum_members, limits.maximum_groups)),
    )
    .map_err(|mut e| {
        if e.code == "fermata.rational_budget" {
            e.code = "grace.rational_budget".into();
        }
        vec![e]
    })
}
pub(super) fn discover(
    doc: &Document,
    performed: &PerformedScore,
) -> Result<GraceDiscovery, Vec<Diagnostic>> {
    let written: BTreeMap<_, _> = doc
        .events()
        .map(|(at, e)| (e.id().unwrap(), (at, e)))
        .collect();
    let mut metadata = BTreeMap::new();
    for frame in &doc.frames {
        for record in &frame.records {
            if let Record::Mark(m) = record {
                if m.kind == grace_extension::KIND {
                    metadata.insert(
                        m.target.trim_start_matches('#'),
                        grace_extension::decode(m).map_err(|e| vec![e])?,
                    );
                }
            }
        }
    }
    let mut authored = Vec::new();
    let mut by_written = BTreeMap::new();
    for (_, event) in doc.events().filter(|(_, e)| e.is_grace()) {
        let id = event.id().unwrap();
        let principal = event.attributes["anchor"]
            .as_str()
            .unwrap()
            .trim_start_matches('#');
        by_written.insert(id, authored.len());
        authored.push(AuthoredGraceAudit {
            written_id: EventId(id.into()),
            written_principal: EventId(principal.into()),
            order: event.attributes["order"].as_i64().unwrap(),
            slash: event.attributes.get("slash").and_then(Scalar::as_bool),
            timing: metadata[id].clone(),
            occurrence_ids: vec![],
        });
    }
    let events: BTreeMap<_, _> = performed.score.events.iter().map(|e| (&e.id, e)).collect();
    let mut occurrences = Vec::new();
    for receipt in &performed.events {
        if let Some(&index) = by_written.get(receipt.written_id.0.as_str()) {
            let source = &mut authored[index];
            let event = events[&receipt.id];
            let grace = event.grace.as_ref().expect("validated grace");
            let principal = events[&grace.anchor];
            // Source validation uses declared staff defaults; performed state
            // can change those defaults. Recheck the actual occurrence pairing.
            if event.part != principal.part
                || event.voice != principal.voice
                || event.staff != principal.staff
                || event.start != principal.start
            {
                return Err(fail(
                    doc,
                    "grace.anchor_scope",
                    "Grace and principal must share the effective performed part, voice, staff and anchor onset",
                ));
            }
            let (written_at, written_principal) = written[source.written_principal.0.as_str()];
            source.occurrence_ids.push(receipt.id.clone());
            occurrences.push(GraceOccurrence {
                id: receipt.id.clone(),
                written_id: receipt.written_id.clone(),
                principal: principal.id.clone(),
                written_principal: source.written_principal.clone(),
                order: source.order,
                slash: source.slash,
                timing: source.timing.clone(),
                measure_occurrence: receipt.measure_occurrence,
                part: event.part.clone(),
                written_anchor_quarter: written_at.clone(),
                anchor_quarter: event.start.clone(),
                written_principal_start: written_at.clone(),
                written_principal_end: written_at
                    + written_principal
                        .duration
                        .measured()
                        .expect("validated principal"),
                principal_start: principal.start.clone(),
                principal_end: principal.end(),
            });
        }
    }
    // Canonical source order, not event ID or caller member order.
    occurrences.sort_by(|a, b| {
        a.measure_occurrence
            .cmp(&b.measure_occurrence)
            .then(a.principal_start.cmp(&b.principal_start))
            .then(a.principal.cmp(&b.principal))
            .then(a.order.cmp(&b.order))
    });
    Ok(GraceDiscovery {
        authored,
        occurrences,
    })
}
/// Policy-entry metadata only; no detached playable ResolvedScore escapes.
pub fn discover_document_graces(doc: &Document) -> Result<GraceDiscovery, Vec<Diagnostic>> {
    let performed = prepare(
        doc,
        PerformanceLimits::default(),
        &GraceTimingLimits::default(),
    )?;
    discover(doc, &performed)
}
fn sound_structure(
    doc: &Document,
    score: &ResolvedScore,
    affected: &BTreeSet<EventId>,
    practice_principals: bool,
) -> Result<(), Vec<Diagnostic>> {
    let events: BTreeMap<_, _> = score.events.iter().map(|e| (&e.id, e)).collect();
    for relation in &score.relations {
        if relation.anchors.iter().any(|a| affected.contains(&a.event)) {
            return Err(fail(
                doc,
                "grace.relation_unsupported",
                "Grace members and their principals must be untied and have no relation anchors",
            ));
        }
        if matches!(relation.kind, RelationKind::Tie { sound: true, .. }) {
            let has_cue = relation
                .anchors
                .iter()
                .any(|a| events[&a.event].role == EventRole::Cue);
            let has_note = relation
                .anchors
                .iter()
                .any(|a| events[&a.event].role != EventRole::Cue);
            if has_cue && has_note {
                return Err(fail(
                    doc,
                    "grace.mixed_cue_tie",
                    "Sounding tie chain mixes silent cue and ordinary note roles",
                ));
            }
        }
    }
    for direction in &score.directions {
        if matches!(direction.kind, DirectionKind::Articulation { .. }) {
            if let Scope::Event(id) = &direction.scope {
                if affected.contains(id) && !(practice_principals
                    && events[id].duration != crate::Duration::Grace
                    && matches!(direction.kind, DirectionKind::Articulation { value: Articulation::Staccato | Articulation::Accent })) {
                    return Err(fail(
                        doc,
                        "grace.articulation_unsupported",
                        "Grace and principal articulations need a separate sound policy",
                    ));
                }
            }
        }
    }
    for id in affected {
        let event = events[id];
        let audible = event.sounding_pitch.as_ref().is_some_and(|p| {
            let pitch = (p + &Rational::from_integer(12)).to_f64();
            pitch.is_finite() && (0.0..=127.0).contains(&pitch)
        });
        if event.role == EventRole::Cue || !audible {
            return Err(fail(
                doc,
                "grace.unsupported_sound_anchor",
                "Grace and principal must be audible pitched notes within MIDI 0–127",
            ));
        }
    }
    Ok(())
}
/// Qualify unchanged source and consume all visited graces with explicit policy.
/// No external performed snapshot or caller-manufactured schedule is accepted.
pub fn perform_document_with_explicit_graces(
    doc: &Document,
    policy: &ExplicitGracePolicy,
    sound_policy: GraceSoundPolicy,
    performance_limits: PerformanceLimits,
    limits: GraceTimingLimits,
) -> Result<QualifiedGracePerformance, Vec<Diagnostic>> {
    preflight_policy(doc, policy, &limits)?;
    let performed = prepare(doc, performance_limits, &limits)?;
    let schedule = allocate(doc, &performed, policy, sound_policy, &limits)?;
    Ok(QualifiedGracePerformance {
        performed,
        schedule,
    })
}
pub(super) fn preflight_policy(
    doc: &Document,
    policy: &ExplicitGracePolicy,
    limits: &GraceTimingLimits,
) -> Result<(), Vec<Diagnostic>> {
    if policy.groups.len() > limits.maximum_groups {
        return Err(fail(
            doc,
            "grace.timing_budget",
            "Policy group budget exceeded",
        ));
    }
    let mut members = 0usize;
    for group in &policy.groups {
        if let GraceAllocation::ExplicitFollowingQuarters { members: values } = &group.allocation {
            members = members
                .checked_add(values.len())
                .ok_or_else(|| fail(doc, "grace.timing_budget", "Policy member count overflow"))?;
            if members > limits.maximum_members {
                return Err(fail(
                    doc,
                    "grace.timing_budget",
                    "Policy member budget exceeded",
                ));
            }
            for member in values {
                check(doc, &member.quarters, limits)?;
            }
        }
    }
    Ok(())
}
/// The route is created exactly once by a source-qualified constructor.
pub(super) fn allocate(
    doc: &Document,
    performed: &PerformedScore,
    policy: &ExplicitGracePolicy,
    sound_policy: GraceSoundPolicy,
    limits: &GraceTimingLimits,
) -> Result<GraceSchedule, Vec<Diagnostic>> {
    allocate_profile(doc,performed,policy,sound_policy,limits,false)
}
pub(super) fn allocate_practice(
    doc:&Document, performed:&PerformedScore, policy:&ExplicitGracePolicy,
    sound_policy:GraceSoundPolicy, limits:&GraceTimingLimits,
)->Result<GraceSchedule,Vec<Diagnostic>> {
    allocate_profile(doc,performed,policy,sound_policy,limits,true)
}
fn allocate_profile(
    doc:&Document, performed:&PerformedScore, policy:&ExplicitGracePolicy,
    sound_policy:GraceSoundPolicy, limits:&GraceTimingLimits, practice:bool,
)->Result<GraceSchedule,Vec<Diagnostic>> {
    let principal_articulations=matches!(sound_policy,GraceSoundPolicy::PracticePrincipalStaccatoAccentInAllocatedRemainderV1);
    if principal_articulations && !practice {
        return Err(fail(doc,"grace.practice_policy_scope","Principal articulation composition requires the explicit practice joint profile"));
    }
    let discovery = discover(doc, performed)?;
    let clock = ExactTempoMap::new(&performed.score, Some(limits.maximum_rational_bits))
        .map_err(|e| fail(doc, "grace.rational_budget", e))?;
    if limits
        .maximum_timeline_seconds
        .as_ref()
        .is_some_and(|limit| clock.seconds(&performed.score.end) > *limit)
    {
        return Err(fail(
            doc,
            "grace.timeline_budget",
            "Grace performance exceeds the configured timeline",
        ));
    }
    let mut by_principal = BTreeMap::<EventId, Vec<&GraceOccurrence>>::new();
    for occurrence in &discovery.occurrences {
        by_principal
            .entry(occurrence.principal.clone())
            .or_default()
            .push(occurrence);
    }
    if by_principal.len() > limits.maximum_groups
        || discovery.occurrences.len() > limits.maximum_members
    {
        return Err(fail(
            doc,
            "grace.timing_budget",
            "Grace occurrence/group budget exceeded",
        ));
    }
    let affected = discovery
        .occurrences
        .iter()
        .flat_map(|o| [o.id.clone(), o.principal.clone()])
        .collect();
    sound_structure(doc, &performed.score, &affected, principal_articulations)?;
    let mut consumed = BTreeSet::new();
    let mut windows = Vec::new();
    let mut groups = Vec::new();
    for group in &policy.groups {
        let occurrences = by_principal.get(&group.principal).ok_or_else(|| {
            fail(
                doc,
                "grace.policy_principal",
                format!("Unknown performed principal {}", group.principal),
            )
        })?;
        if !consumed.insert(&group.principal) {
            return Err(fail(
                doc,
                "grace.policy_duplicate",
                "Principal occurrence assigned more than once",
            ));
        }
        let first = occurrences[0];
        let duration = &first.principal_end - &first.principal_start;
        check(doc, &duration, limits)?;
        let allocations = match &group.allocation {
            GraceAllocation::AuthoredFollowingPercent => {
                if occurrences.len() != 1
                    || first.timing.steal_previous.is_some()
                    || first.timing.make_time_quarters.is_some()
                    || !first
                        .timing
                        .steal_following
                        .as_ref()
                        .is_some_and(|p| p.is_positive() && p < &Rational::from_integer(100))
                {
                    return Err(fail(
                        doc,
                        "grace.authored_following_unsupported",
                        "Authored percentage requires a singleton with only 0 < steal-following < 100",
                    ));
                }
                if !clock.constant_between(&first.principal_start, &first.principal_end) {
                    return Err(fail(
                        doc,
                        "grace.percentage_tempo_ambiguous",
                        "Authored following percentage across changing effective tempo is ambiguous",
                    ));
                }
                let product = &duration * first.timing.steal_following.as_ref().unwrap();
                check(doc, &product, limits)?;
                vec![&product / &Rational::from_integer(100)]
            }
            GraceAllocation::ExplicitFollowingQuarters { members } => {
                if occurrences
                    .iter()
                    .any(|o| o.timing != GraceTiming::default())
                {
                    return Err(fail(
                        doc,
                        "grace.authored_timing_conflict",
                        "Explicit quarter allocations require every authored timing field to be absent, including explicit zero",
                    ));
                }
                let expected: BTreeSet<_> = occurrences.iter().map(|o| &o.id).collect();
                let mut assigned = BTreeMap::new();
                for member in members {
                    if !expected.contains(&member.event) {
                        return Err(fail(
                            doc,
                            "grace.policy_member",
                            "Allocation references an unknown/written/other-group grace ID",
                        ));
                    }
                    if assigned.insert(&member.event, &member.quarters).is_some() {
                        return Err(fail(
                            doc,
                            "grace.policy_duplicate",
                            "Grace occurrence assigned more than once",
                        ));
                    }
                }
                if assigned.len() != occurrences.len() {
                    return Err(fail(
                        doc,
                        "grace.policy_missing",
                        "Every grace in the principal occurrence group must be allocated",
                    ));
                }
                occurrences
                    .iter()
                    .map(|o| assigned[&o.id].clone())
                    .collect()
            }
        };
        let mut cursor = first.principal_start.clone();
        let mut total = Rational::zero();
        for (occurrence, allocation) in occurrences.iter().zip(allocations) {
            check(doc, &allocation, limits)?;
            if !allocation.is_positive() {
                return Err(fail(
                    doc,
                    "grace.allocation_positive",
                    "Each grace allocation must be positive",
                ));
            }
            total = &total + &allocation;
            check(doc, &total, limits)?;
            if total >= duration {
                return Err(fail(
                    doc,
                    "grace.principal_remainder",
                    "Total grace allocation must be strictly shorter than the nominal principal duration",
                ));
            }
            let end = &cursor + &allocation;
            windows.push(window(
                doc,
                occurrence.id.clone(),
                group.principal.clone(),
                EventRole::Grace,
                (cursor, end.clone()),
                &clock,
                limits,
            )?);
            cursor = end;
        }
        let remainder = &duration - &total;
        check(doc, &remainder, limits)?;
        let staccato = principal_articulations && performed.score.directions.iter().any(|d|
            matches!(&d.scope,Scope::Event(id) if id==&group.principal)
                && matches!(d.kind,DirectionKind::Articulation {value:Articulation::Staccato}));
        let gate:Rational=if staccato { "1/2" } else { "23/25" }.parse().unwrap();
        let gated = &remainder * &gate;
        check(doc, &gated, limits)?;
        let release = &cursor + &gated;
        windows.push(window(
            doc,
            group.principal.clone(),
            group.principal.clone(),
            EventRole::Note,
            (cursor, release),
            &clock,
            limits,
        )?);
        groups.push(GraceGroupRealization {
            principal: group.principal.clone(),
            allocation: group.allocation.clone(),
            allocated_quarters: total,
        });
    }
    if consumed.len() != by_principal.len() {
        return Err(fail(
            doc,
            "grace.policy_missing",
            "Every visited principal grace group requires exactly one allocation policy",
        ));
    }
    let default_tempo_intervals = performed
        .score
        .tempos
        .iter()
        .enumerate()
        .filter_map(|(i, tempo)| {
            let end = performed
                .score
                .tempos
                .get(i + 1)
                .map(|t| &t.at)
                .unwrap_or(&performed.score.end);
            (tempo.origin.object_id == "performance.default_tempo" && end > &tempo.at).then(|| {
                GraceDefaultTempoInterval {
                    from_quarter: tempo.at.clone(),
                    until_quarter: end.clone(),
                    quarter_bpm: Rational::from_integer(120),
                }
            })
        })
        .collect();
    windows.sort_by(|a, b| a.quarter_start.cmp(&b.quarter_start).then(a.id.cmp(&b.id)));
    Ok(GraceSchedule {
        default_tempo_intervals,
        discovery,
        windows,
        groups,
        sound_policy,
        clock,
        maximum_rational_bits: limits.maximum_rational_bits,
    })
}
fn window(
    doc: &Document,
    id: EventId,
    principal: EventId,
    role: EventRole,
    quarter_range: (Rational, Rational),
    clock: &ExactTempoMap,
    limits: &GraceTimingLimits,
) -> Result<GraceSoundWindow, Vec<Diagnostic>> {
    let (start, end) = quarter_range;
    check(doc, &start, limits)?;
    check(doc, &end, limits)?;
    let seconds_start = clock
        .checked_seconds(&start)
        .map_err(|e| fail(doc, "grace.rational_budget", e))?;
    let seconds_end = clock
        .checked_seconds(&end)
        .map_err(|e| fail(doc, "grace.rational_budget", e))?;
    check(doc, &seconds_start, limits)?;
    check(doc, &seconds_end, limits)?;
    let duration = &seconds_end - &seconds_start;
    check(doc, &duration, limits)?;
    let a = seconds_start.to_f64();
    let b = seconds_end.to_f64();
    if !duration.is_positive()
        || duration < "1/1000000".parse().unwrap()
        || !a.is_finite()
        || !b.is_finite()
        || b - a < 0.000_001
    {
        return Err(fail(
            doc,
            "grace.clock_precision",
            format!(
                "Positive grace/principal window {id} collapses or is shorter than the one-microsecond audio minimum"
            ),
        ));
    }
    Ok(GraceSoundWindow {
        id,
        principal,
        role,
        quarter_start: start,
        quarter_end: end,
        seconds_start,
        seconds_end,
    })
}
