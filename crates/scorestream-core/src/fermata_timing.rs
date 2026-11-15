//! Opt-in, exact fermata clock prototype for an already validated performed route.
//!
//! This module does not project source documents, choose routes, infer hold groups,
//! or change note gates/audio. In particular it does not bypass the ordinary
//! lowerer/projector's `fermata.performance_unsupported` guard. The caller must
//! supply a validated source-to-performed projection; the checks here verify its
//! identity/part/timing receipts, not the musical correctness of its route,
//! projected controls/tempo, pitches, voice/staff or relations. Those remain
//! caller preconditions, not a complete performed-projection validation here.
//! `notation::perform_document_with_explicit_fermatas` is the source-qualified
//! path: it privately projects the unchanged source and invokes this clock before
//! returning a playable projection. This bare-snapshot API keeps its preconditions.
//! Authored signs never specify seconds. Only an explicit policy supplies them.
use crate::{
    fermata_extension,
    notation::{EventId, EventRole, PartId, PerformedScore},
    score::ScoreFermata,
    Document, Pitch, Rational, Record, Severity,
};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// There is deliberately no default duration, grouping, or policy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExplicitFermataPolicy {
    pub groups: Vec<ExplicitHoldGroup>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExplicitHoldGroup {
    pub id: String,
    /// Performed event IDs, never written IDs. Every visited sign occurs once.
    pub members: Vec<EventId>,
    pub added_seconds: Rational,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimingLimits {
    pub maximum_events: usize,
    pub maximum_measures: usize,
    pub maximum_authored_fermatas: usize,
    pub maximum_tempos: usize,
    pub maximum_hold_groups: usize,
    pub maximum_group_members: usize,
    pub maximum_source_records: usize,
    /// Applies to input times, durations, tempo values and policy seconds.
    pub maximum_rational_bits: u64,
    /// Positive and at most six hours. Cannot relax the hard six-hour ceiling.
    pub maximum_timeline_seconds: Rational,
}
impl Default for TimingLimits {
    fn default() -> Self {
        Self {
            maximum_events: 100_000,
            maximum_measures: 100_000,
            maximum_authored_fermatas: 100_000,
            maximum_tempos: 100_000,
            maximum_hold_groups: 100_000,
            maximum_group_members: 100_000,
            maximum_source_records: 1_000_000,
            maximum_rational_bits: 4096,
            maximum_timeline_seconds: Rational::from_integer(6 * 60 * 60),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimingError {
    pub code: &'static str,
    pub message: String,
}
impl fmt::Display for TimingError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for TimingError {}
type Result<T> = std::result::Result<T, TimingError>;
fn error(code: &'static str, message: impl Into<String>) -> TimingError {
    TimingError {
        code,
        message: message.into(),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TimingDiagnostic {
    /// Quarter = 120 is a clock convention, not an inferred printed metronome.
    DefaultQuarterTempo {
        quarter_bpm: Rational,
        until_quarter: Rational,
    },
    /// A source-qualified route explicitly restored/emitted the default clock
    /// state. These intervals can recur after repeat checkpoints.
    PerformedDefaultQuarterTempo {
        quarter_bpm: Rational,
        from_quarter: Rational,
        until_quarter: Rational,
    },
    /// Seconds are supplied by policy; the authored signs supply no duration.
    ExplicitPolicyHold {
        group_id: String,
        added_seconds: Rational,
        members: Vec<EventId>,
        duration_source: HoldDurationSource,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HoldDurationSource {
    ExplicitPolicyNotAuthoredSign,
}

/// Every authored sign, including signs in skipped/unvisited source measures.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthoredFermataAudit {
    pub sign: ScoreFermata,
    /// Empty means unvisited, not dropped and not assigned an implicit hold.
    pub occurrence_ids: Vec<EventId>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FermataOccurrence {
    pub sign: ScoreFermata,
    pub authored_index: usize,
    pub id: EventId,
    pub written_id: EventId,
    pub measure_occurrence: usize,
    pub part: PartId,
    pub written_quarter_start: Rational,
    pub written_quarter_end: Rational,
    /// Exact performed-route coordinates, not source coordinates.
    pub quarter_start: Rational,
    pub quarter_end: Rational,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventTiming {
    pub id: EventId,
    pub written_id: EventId,
    pub measure_occurrence: usize,
    pub part: PartId,
    pub written_quarter_start: Rational,
    pub written_quarter_end: Rational,
    /// Exact performed-route coordinates, not source coordinates.
    pub quarter_start: Rational,
    pub quarter_end: Rational,
    /// Both endpoints use `after`. These are markers, not note gates.
    pub seconds_start: Rational,
    pub seconds_end: Rational,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MeasureTiming {
    pub written_id: String,
    pub label: String,
    pub part: PartId,
    pub occurrence_index: usize,
    pub written_quarter_start: Rational,
    pub written_quarter_end: Rational,
    /// Exact performed-route coordinates, not source coordinates.
    pub quarter_start: Rational,
    pub quarter_end: Rational,
    pub seconds_start: Rational,
    pub seconds_end: Rational,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HoldInterval {
    pub id: String,
    pub members: Vec<EventId>,
    /// The minimum member start; used only for rejecting ambiguous overlap.
    pub support_start: Rational,
    pub boundary: Rational,
    pub added_seconds: Rational,
    /// Exact half-open interval [seconds_start, seconds_end).
    pub seconds_start: Rational,
    pub seconds_end: Rational,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SeekPosition {
    Music {
        quarter: Rational,
    },
    Hold {
        group_id: String,
        boundary: Rational,
        elapsed_seconds: Rational,
    },
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct TempoSegment {
    quarter_start: Rational,
    quarter_end: Rational,
    seconds_start: Rational,
    seconds_end: Rational,
    seconds_per_quarter: Rational,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TimingProjection {
    pub authored: Vec<AuthoredFermataAudit>,
    pub fermatas: Vec<FermataOccurrence>,
    pub events: Vec<EventTiming>,
    pub measures: Vec<MeasureTiming>,
    pub diagnostics: Vec<TimingDiagnostic>,
    // Keep the clock immutable: callers can inspect holds, not corrupt seek data.
    holds: Vec<HoldInterval>,
    tempos: Vec<TempoSegment>,
    quarter_end: Rational,
    duration_seconds: Rational,
}
impl TimingProjection {
    pub fn holds(&self) -> &[HoldInterval] {
        &self.holds
    }
    pub fn quarter_end(&self) -> &Rational {
        &self.quarter_end
    }
    pub fn duration_seconds(&self) -> &Rational {
        &self.duration_seconds
    }

    fn clamp_quarter(&self, quarter: &Rational) -> Rational {
        quarter
            .max(&Rational::zero())
            .min(&self.quarter_end)
            .clone()
    }
    fn base(&self, quarter: &Rational) -> Rational {
        let index = self.tempos.partition_point(|s| s.quarter_start <= *quarter);
        match index.checked_sub(1).map(|i| &self.tempos[i]) {
            Some(s) => &s.seconds_start + &(&(quarter - &s.quarter_start) * &s.seconds_per_quarter),
            None => Rational::zero(),
        }
    }
    /// Base tempo time plus holds strictly before q; q clamps to [0, end].
    pub fn before(&self, quarter: &Rational) -> Rational {
        let q = self.clamp_quarter(quarter);
        let count = self.holds.partition_point(|h| h.boundary < q);
        let added = count
            .checked_sub(1)
            .map(|i| &self.holds[i].seconds_end - &self.base(&self.holds[i].boundary))
            .unwrap_or_default();
        &self.base(&q) + &added
    }
    /// Base tempo time plus holds at or before q; q clamps to [0, end].
    pub fn after(&self, quarter: &Rational) -> Rational {
        let q = self.clamp_quarter(quarter);
        let count = self.holds.partition_point(|h| h.boundary <= q);
        let added = count
            .checked_sub(1)
            .map(|i| &self.holds[i].seconds_end - &self.base(&self.holds[i].boundary))
            .unwrap_or_default();
        &self.base(&q) + &added
    }
    /// Exact inverse. At a hold's end music resumes at its boundary. Negative
    /// seconds clamp to q=0; seconds at/after total duration clamp to score end.
    pub fn seek(&self, seconds: &Rational) -> SeekPosition {
        if seconds <= &Rational::zero() {
            return SeekPosition::Music {
                quarter: Rational::zero(),
            };
        }
        if seconds >= &self.duration_seconds {
            return SeekPosition::Music {
                quarter: self.quarter_end.clone(),
            };
        }
        let count = self.holds.partition_point(|h| h.seconds_start <= *seconds);
        let added = if let Some(i) = count.checked_sub(1) {
            let h = &self.holds[i];
            if seconds < &h.seconds_end {
                return SeekPosition::Hold {
                    group_id: h.id.clone(),
                    boundary: h.boundary.clone(),
                    elapsed_seconds: seconds - &h.seconds_start,
                };
            }
            &h.seconds_end - &self.base(&h.boundary)
        } else {
            Rational::zero()
        };
        let base_seconds = seconds - &added;
        let index = self
            .tempos
            .partition_point(|s| s.seconds_start <= base_seconds);
        let quarter = match index.checked_sub(1).map(|i| &self.tempos[i]) {
            Some(s) => {
                &s.quarter_start + &(&(&base_seconds - &s.seconds_start) / &s.seconds_per_quarter)
            }
            None => Rational::zero(),
        };
        SeekPosition::Music { quarter }
    }
}

fn count(actual: usize, limit: usize, subject: &str) -> Result<()> {
    if actual > limit {
        return Err(error(
            "fermata.timing_budget",
            format!("{subject} count {actual} exceeds limit {limit}"),
        ));
    }
    Ok(())
}
fn rational(value: &Rational, limits: &TimingLimits) -> Result<()> {
    if value.numerator().bits() > limits.maximum_rational_bits
        || value.denominator().bits() > limits.maximum_rational_bits
    {
        return Err(error(
            "fermata.rational_budget",
            "Timing rational exceeds the configured bit budget",
        ));
    }
    Ok(())
}
fn time(value: &Rational, end: &Rational, limits: &TimingLimits) -> Result<()> {
    rational(value, limits)?;
    if value.is_negative() || value > end {
        return Err(error(
            "fermata.invalid_time",
            format!("Time {value} lies outside [0, {end}]"),
        ));
    }
    Ok(())
}
fn duration(value: &Rational, limits: &TimingLimits) -> Result<()> {
    rational(value, limits)?;
    if !value.is_positive() {
        return Err(error(
            "fermata.invalid_duration",
            "Timing durations must be strictly positive",
        ));
    }
    Ok(())
}

/// Construct a pure clock from a source document and an independently validated
/// performed route. Does not invoke a source projector or remove any extension.
/// Source validation and full authored-sign extraction prevent detached-policy
/// callers from omitting written fermatas. Every performed receipt must identify
/// a source event, have its duration/part, and retain its within-measure onset.
/// Event/tie identities and quarter intervals are not rewritten.
pub fn interpret(
    source: &Document,
    performed: &PerformedScore,
    policy: &ExplicitFermataPolicy,
    limits: TimingLimits,
) -> Result<TimingProjection> {
    interpret_inner(source, performed, policy, limits, None)
}
/// Internal only: the schedule is privately allocated from this unchanged source
/// and the same qualified route. Raw callers never gain an ignore-grace flag.
pub(crate) fn interpret_qualified_graces(
    source: &Document,
    performed: &PerformedScore,
    policy: &ExplicitFermataPolicy,
    limits: TimingLimits,
    schedule: &crate::notation::GraceSchedule,
) -> Result<TimingProjection> {
    interpret_inner(source, performed, policy, limits, Some(schedule))
}
fn interpret_inner(
    source: &Document,
    performed: &PerformedScore,
    policy: &ExplicitFermataPolicy,
    limits: TimingLimits,
    schedule: Option<&crate::notation::GraceSchedule>,
) -> Result<TimingProjection> {
    let hard_maximum = Rational::from_integer(6 * 60 * 60);
    if !limits.maximum_timeline_seconds.is_positive()
        || limits.maximum_timeline_seconds > hard_maximum
        || limits.maximum_rational_bits == 0
    {
        return Err(error("fermata.invalid_limits", "Timeline limit must be positive and at most six hours; rational bit budget must be positive"));
    }
    count(
        source.frames.len(),
        limits.maximum_source_records,
        "Source frames",
    )?;
    count(
        source.parts.len(),
        limits.maximum_source_records,
        "Source parts",
    )?;
    count(
        source.voices.len(),
        limits.maximum_source_records,
        "Source voices",
    )?;
    let mut source_records = 0usize;
    for frame in &source.frames {
        source_records = source_records
            .checked_add(frame.records.len())
            .ok_or_else(|| error("fermata.timing_budget", "Source record count overflow"))?;
        count(
            source_records,
            limits.maximum_source_records,
            "Source records",
        )?;
    }
    count(source.event_count(), limits.maximum_events, "Source events")?;
    count(
        performed.score.events.len(),
        limits.maximum_events,
        "Performed events",
    )?;
    count(
        performed.events.len(),
        limits.maximum_events,
        "Event receipts",
    )?;
    count(
        performed.measures.len(),
        limits.maximum_measures,
        "Measure receipts",
    )?;
    count(
        performed.score.tempos.len(),
        limits.maximum_tempos,
        "Tempo controls",
    )?;
    count(
        policy.groups.len(),
        limits.maximum_hold_groups,
        "Hold groups",
    )?;
    let mut member_count = 0usize;
    for group in &policy.groups {
        member_count = member_count
            .checked_add(group.members.len())
            .ok_or_else(|| error("fermata.timing_budget", "Policy member count overflow"))?;
        count(member_count, limits.maximum_group_members, "Policy members")?;
    }
    rational(&source.end, &limits)?;
    for frame in &source.frames {
        rational(&frame.time, &limits)?;
        for record in &frame.records {
            match record {
                Record::Event(event) => {
                    if let Some(value) = event.duration.measured() {
                        rational(value, &limits)?;
                    }
                }
                Record::Measure(measure) => rational(&measure.length, &limits)?,
                _ => (),
            }
        }
    }
    if let Some(d) = crate::validate(source, None)
        .into_iter()
        .find(|d| d.severity == Severity::Error)
    {
        return Err(error(
            "fermata.invalid_source",
            format!("{}: {}", d.code, d.message),
        ));
    }
    // Reject unsupported source attacks even if a supplied snapshot omits them.
    // This prototype cannot silently erase grace timing while checking only the
    // measured-event receipts of an otherwise plausible route.
    if schedule.is_none() && source.events().any(|(_, event)| event.is_grace()) {
        return Err(error(
            "fermata.unsupported_grace",
            "Source grace timing is outside this prototype, including unvisited grace notes",
        ));
    }
    if performed
        .diagnostics
        .iter()
        .any(|d| d.severity == Severity::Error)
    {
        return Err(error(
            "fermata.invalid_projection",
            "Performed projection contains error diagnostics",
        ));
    }
    rational(&performed.score.end, &limits)?;
    if performed.score.end.is_negative() {
        return Err(error(
            "fermata.invalid_time",
            "Performed end cannot be negative",
        ));
    }
    let end = &performed.score.end;
    if source.end.is_positive() && !end.is_positive() {
        return Err(error(
            "fermata.invalid_projection",
            "A positive-length source cannot have an empty performed route",
        ));
    }
    let voices: BTreeMap<_, _> = source
        .voices
        .iter()
        .map(|v| (v.id.as_str(), v.part.as_str()))
        .collect();
    let mut source_events = BTreeMap::new();
    for (at, event) in source.events() {
        let id = event.id().ok_or_else(|| {
            error(
                "fermata.source_identity",
                "Timing requires stable IDs for source events",
            )
        })?;
        let part = voices
            .get(event.voice.as_str())
            .ok_or_else(|| error("fermata.source_identity", "Source event voice has no part"))?;
        if source_events
            .insert(EventId(id.into()), (at, event, *part))
            .is_some()
        {
            return Err(error(
                "fermata.source_identity",
                format!("Duplicate source event {id}"),
            ));
        }
    }
    let mut source_measures = BTreeMap::new();
    let mut authored = Vec::new();
    let mut authored_index = BTreeMap::new();
    for frame in &source.frames {
        for record in &frame.records {
            match record {
                Record::Measure(m) => {
                    if source_measures
                        .insert(
                            (m.target.as_str(), m.id.as_str()),
                            (
                                &frame.time,
                                &m.length,
                                m.attributes
                                    .get("label")
                                    .and_then(crate::Scalar::as_str)
                                    .unwrap_or(&m.id),
                            ),
                        )
                        .is_some()
                    {
                        return Err(error(
                            "fermata.measure_receipt",
                            "Duplicate source measure identity",
                        ));
                    }
                }
                Record::Mark(m) if m.kind == fermata_extension::KIND => {
                    count(
                        authored.len() + 1,
                        limits.maximum_authored_fermatas,
                        "Authored fermatas",
                    )?;
                    let sign = fermata_extension::decode(m)
                        .map_err(|d| error("fermata.invalid_source", d.to_string()))?;
                    if authored_index
                        .insert(sign.event.clone(), authored.len())
                        .is_some()
                    {
                        return Err(error(
                            "fermata.source_identity",
                            "Duplicate authored fermata",
                        ));
                    }
                    authored.push(AuthoredFermataAudit {
                        sign,
                        occurrence_ids: vec![],
                    });
                }
                _ => (),
            }
        }
    }
    count(
        source_measures.len(),
        limits.maximum_measures,
        "Source measures",
    )?;
    let mut measure_map = BTreeMap::new();
    let mut occurrence_times = BTreeMap::new();
    let mut part_measures: BTreeMap<&PartId, Vec<_>> = BTreeMap::new();
    for m in &performed.measures {
        time(&m.start, end, &limits)?;
        duration(&m.length, &limits)?;
        time(&(&m.start + &m.length), end, &limits)?;
        let (_, written_length, written_label) = source_measures
            .get(&(m.part.0.as_str(), m.written_id.as_str()))
            .ok_or_else(|| {
                error(
                    "fermata.measure_receipt",
                    "Performed measure references an unknown source measure/part",
                )
            })?;
        if m.length != **written_length
            || m.label != *written_label
            || measure_map
                .insert((m.part.clone(), m.occurrence_index), m)
                .is_some()
        {
            return Err(error(
                "fermata.measure_receipt",
                "Duplicate or inconsistent measure occurrence receipt",
            ));
        }
        if let Some((start, length)) =
            occurrence_times.insert(m.occurrence_index, (&m.start, &m.length))
        {
            if start != &m.start || length != &m.length {
                return Err(error(
                    "fermata.measure_receipt",
                    "Parts disagree about a measure occurrence's exact times",
                ));
            }
        }
        part_measures.entry(&m.part).or_default().push(m);
    }
    if occurrence_times.len().checked_mul(source.parts.len()) != Some(performed.measures.len()) {
        return Err(error(
            "fermata.measure_receipt",
            "Every occurrence index must identify exactly one measure per source part",
        ));
    }
    for measures in part_measures.values_mut() {
        measures.sort_by(|a, b| a.start.cmp(&b.start));
        let mut next = Rational::zero();
        for m in measures {
            if m.start != next {
                return Err(error(
                    "fermata.measure_receipt",
                    "Performed measures must cover contiguous time from zero",
                ));
            }
            next = &m.start + &m.length;
        }
        if &next != end {
            return Err(error(
                "fermata.measure_receipt",
                "Performed measure coverage differs from score end",
            ));
        }
    }
    if end.is_positive() && part_measures.len() != source.parts.len() {
        return Err(error(
            "fermata.measure_receipt",
            "Performed measures must retain every source part",
        ));
    }
    let scheduled_graces: BTreeMap<_, _> = schedule
        .into_iter()
        .flat_map(|s| s.discovery().occurrences.iter().map(|g| (&g.id, g)))
        .collect();
    let mut event_map = BTreeMap::new();
    for event in &performed.score.events {
        time(&event.start, end, &limits)?;
        if let Some(value) = event.duration.measured() {
            duration(value, &limits)?;
            if event.role == EventRole::Grace || event.grace.is_some() {
                return Err(error(
                    "fermata.unsupported_grace",
                    "Grace timing is outside this prototype",
                ));
            }
        } else {
            let qualified = scheduled_graces.get(&event.id).is_some_and(|g| {
                event.role == EventRole::Grace
                    && event
                        .grace
                        .as_ref()
                        .is_some_and(|metadata| metadata.anchor == g.principal)
                    && event.start == g.anchor_quarter
                    && event.part == g.part
            });
            if !qualified {
                return Err(error(
                    "fermata.unsupported_grace",
                    "Unqualified grace marker in performed route",
                ));
            }
        }
        time(&event.end(), end, &limits)?;
        if event_map.insert(event.id.clone(), event).is_some() {
            return Err(error(
                "fermata.event_receipt",
                "Duplicate performed event ID",
            ));
        }
    }
    let mut receipt_ids = BTreeSet::new();
    let mut written_occurrences = BTreeSet::new();
    let mut fermatas = Vec::new();
    for receipt in &performed.events {
        if !receipt_ids.insert(receipt.id.clone())
            || !written_occurrences.insert((receipt.written_id.clone(), receipt.measure_occurrence))
        {
            return Err(error(
                "fermata.event_receipt",
                "Duplicate event receipt or written event within an occurrence",
            ));
        }
        let event = event_map.get(&receipt.id).ok_or_else(|| {
            error(
                "fermata.event_receipt",
                format!("Receipt references unknown performed event {}", receipt.id),
            )
        })?;
        let (source_at, original, part) =
            source_events.get(&receipt.written_id).ok_or_else(|| {
                error(
                    "fermata.event_receipt",
                    format!("Unknown written event {}", receipt.written_id),
                )
            })?;
        if original.is_grace()
            && !scheduled_graces.get(&receipt.id).is_some_and(|g| {
                g.written_id == receipt.written_id
                    && g.measure_occurrence == receipt.measure_occurrence
            })
        {
            return Err(error(
                "fermata.event_receipt",
                "Grace schedule does not match its source occurrence receipt",
            ));
        }
        if original.duration != event.duration
            || event.part.0 != *part
            || event.staff.part != event.part
        {
            return Err(error(
                "fermata.event_receipt",
                format!("Duration/part differs for {}", receipt.id),
            ));
        }
        let measure = measure_map
            .get(&(event.part.clone(), receipt.measure_occurrence))
            .ok_or_else(|| {
                error(
                    "fermata.event_receipt",
                    "Event receipt references an unknown measure occurrence/part",
                )
            })?;
        let (source_start, source_length, _) =
            source_measures[&(measure.part.0.as_str(), measure.written_id.as_str())];
        if *source_at < source_start
            || *source_at >= &(source_start + source_length)
            || event.start != &measure.start + &(*source_at - source_start)
            || event.end() > &measure.start + &measure.length
        {
            return Err(error(
                "fermata.event_receipt",
                format!("Inconsistent source/occurrence times for {}", receipt.id),
            ));
        }
        if let Some(&index) = authored_index.get(&receipt.written_id) {
            if !(event.written_pitch.is_note() || event.written_pitch == Pitch::Rest) {
                return Err(error(
                    "fermata.event_receipt",
                    "A fermata occurrence must remain a note or rest",
                ));
            }
            authored[index].occurrence_ids.push(receipt.id.clone());
            fermatas.push(FermataOccurrence {
                sign: authored[index].sign.clone(),
                authored_index: index,
                id: receipt.id.clone(),
                written_id: receipt.written_id.clone(),
                measure_occurrence: receipt.measure_occurrence,
                part: event.part.clone(),
                written_quarter_start: (*source_at).clone(),
                written_quarter_end: *source_at
                    + original
                        .duration
                        .measured()
                        .expect("matched measured occurrence"),
                quarter_start: event.start.clone(),
                quarter_end: event.end(),
            });
        }
    }
    if scheduled_graces.keys().any(|id| !receipt_ids.contains(*id)) {
        return Err(error(
            "fermata.event_receipt",
            "A scheduled grace is missing its occurrence receipt",
        ));
    }
    if receipt_ids.len() != event_map.len() {
        return Err(error(
            "fermata.event_receipt",
            "A performed event is missing its occurrence receipt",
        ));
    }
    // A visited source event cannot be omitted together with its receipt.
    // Index by part/time, so repeat completeness is bounded by visited events.
    let mut events_by_part: BTreeMap<&str, Vec<(&Rational, &EventId)>> = BTreeMap::new();
    for (id, (at, event, part)) in &source_events {
        if schedule.is_some() || event.duration.measured().is_some() {
            events_by_part.entry(part).or_default().push((at, id));
        }
    }
    for events in events_by_part.values_mut() {
        events.sort_by(|a, b| a.0.cmp(b.0));
    }
    let mut expected_event_count = 0usize;
    for m in &performed.measures {
        if let Some(signs) = events_by_part.get(m.part.0.as_str()) {
            let (start, length, _) = source_measures[&(m.part.0.as_str(), m.written_id.as_str())];
            let stop = start + length;
            let first = signs.partition_point(|(at, _)| *at < start);
            let last = signs.partition_point(|(at, _)| *at < &stop);
            expected_event_count = expected_event_count
                .checked_add(last - first)
                .ok_or_else(|| error("fermata.timing_budget", "Expected event count overflow"))?;
            count(
                expected_event_count,
                limits.maximum_events,
                "Expected performed events",
            )?;
            for (_, id) in &signs[first..last] {
                if !written_occurrences.contains(&((*id).clone(), m.occurrence_index)) {
                    return Err(error(
                        "fermata.event_receipt",
                        format!("Visited source measure omitted event {id}"),
                    ));
                }
            }
        }
    }

    let mut diagnostics = Vec::new();
    let tempos = tempo_segments(performed, &limits, &mut diagnostics)?;
    let mut projection = TimingProjection {
        authored,
        fermatas,
        events: vec![],
        measures: vec![],
        diagnostics,
        holds: vec![],
        tempos,
        quarter_end: end.clone(),
        duration_seconds: Rational::zero(),
    };
    let by_id: BTreeMap<_, _> = projection
        .fermatas
        .iter()
        .enumerate()
        .map(|(i, f)| (f.id.clone(), i))
        .collect();
    let mut group_ids = BTreeSet::new();
    let mut covered = BTreeSet::new();
    for group in &policy.groups {
        if group.id.trim().is_empty() || !group_ids.insert(group.id.clone()) {
            return Err(error(
                "fermata.group_identity",
                "Hold group IDs must be nonempty and unique",
            ));
        }
        if group.members.is_empty() {
            return Err(error(
                "fermata.empty_group",
                format!("Hold group {} has no members", group.id),
            ));
        }
        rational(&group.added_seconds, &limits)?;
        if !group.added_seconds.is_positive() {
            return Err(error(
                "fermata.hold_duration",
                format!("Hold group {} requires explicit positive seconds", group.id),
            ));
        }
        let mut boundary = None;
        let mut support_start = end.clone();
        for id in &group.members {
            let f = &projection.fermatas[*by_id.get(id).ok_or_else(|| {
                error(
                    "fermata.unknown_member",
                    format!(
                        "Group {} references unknown/non-fermata occurrence {id}",
                        group.id
                    ),
                )
            })?];
            if !covered.insert(id.clone()) {
                return Err(error(
                    "fermata.duplicate_member",
                    format!("Occurrence {id} belongs to more than one membership entry"),
                ));
            }
            if boundary.as_ref().is_some_and(|b| b != &f.quarter_end) {
                return Err(error(
                    "fermata.unequal_boundaries",
                    format!(
                        "Group {} has unequal exact member ends; splitting is not inferred",
                        group.id
                    ),
                ));
            }
            boundary = Some(f.quarter_end.clone());
            support_start = support_start.min(f.quarter_start.clone());
        }
        projection.holds.push(HoldInterval {
            id: group.id.clone(),
            members: group.members.clone(),
            support_start,
            boundary: boundary.expect("nonempty validated group"),
            added_seconds: group.added_seconds.clone(),
            seconds_start: Rational::zero(),
            seconds_end: Rational::zero(),
        });
    }
    if covered.len() != projection.fermatas.len() {
        return Err(error(
            "fermata.incomplete_policy",
            "Explicit policy must cover every visited fermata occurrence exactly once",
        ));
    }
    projection.holds.sort_by(|a, b| a.boundary.cmp(&b.boundary));
    for pair in projection.holds.windows(2) {
        if pair[0].boundary == pair[1].boundary {
            return Err(error(
                "fermata.duplicate_boundary",
                "Separate groups at the same boundary are forbidden; specify one global group",
            ));
        }
        if pair[1].support_start < pair[0].boundary {
            return Err(error("fermata.overlapping_support", "Hold groups have overlapping support intervals; grouping/splitting is not inferred"));
        }
    }
    let mut added = Rational::zero();
    for i in 0..projection.holds.len() {
        let start = &projection.base(&projection.holds[i].boundary) + &added;
        let hold = &mut projection.holds[i];
        hold.seconds_start = start;
        hold.seconds_end = &hold.seconds_start + &hold.added_seconds;
        added = &added + &hold.added_seconds;
        rational(&added, &limits)?;
        projection
            .diagnostics
            .push(TimingDiagnostic::ExplicitPolicyHold {
                group_id: hold.id.clone(),
                added_seconds: hold.added_seconds.clone(),
                members: hold.members.clone(),
                duration_source: HoldDurationSource::ExplicitPolicyNotAuthoredSign,
            });
    }
    projection.duration_seconds = &projection.base(end) + &added;
    if projection.duration_seconds > limits.maximum_timeline_seconds {
        return Err(error(
            "fermata.timeline_budget",
            "Exact total timeline exceeds configured limit (hard maximum six hours)",
        ));
    }
    rational(&projection.duration_seconds, &limits)?;
    for receipt in &performed.events {
        let event = event_map[&receipt.id];
        let (source_at, original, _) = source_events[&receipt.written_id];
        projection.events.push(EventTiming {
            id: receipt.id.clone(),
            written_id: receipt.written_id.clone(),
            measure_occurrence: receipt.measure_occurrence,
            part: event.part.clone(),
            written_quarter_start: source_at.clone(),
            written_quarter_end: source_at
                + &original.duration.measured().cloned().unwrap_or_default(),
            quarter_start: event.start.clone(),
            quarter_end: event.end(),
            seconds_start: projection.after(&event.start),
            seconds_end: projection.after(&event.end()),
        });
    }
    for m in &performed.measures {
        let quarter_end = &m.start + &m.length;
        let (written_start, written_length, _) =
            source_measures[&(m.part.0.as_str(), m.written_id.as_str())];
        projection.measures.push(MeasureTiming {
            written_id: m.written_id.clone(),
            label: m.label.clone(),
            part: m.part.clone(),
            occurrence_index: m.occurrence_index,
            written_quarter_start: written_start.clone(),
            written_quarter_end: written_start + written_length,
            quarter_start: m.start.clone(),
            seconds_start: projection.after(&m.start),
            seconds_end: projection.after(&quarter_end),
            quarter_end,
        });
    }
    Ok(projection)
}

fn tempo_segments(
    performed: &PerformedScore,
    limits: &TimingLimits,
    diagnostics: &mut Vec<TimingDiagnostic>,
) -> Result<Vec<TempoSegment>> {
    let end = &performed.score.end;
    let mut controls: Vec<(Rational, Rational)> = Vec::new();
    for tempo in &performed.score.tempos {
        time(&tempo.at, end, limits)?;
        rational(&tempo.value.bpm, limits)?;
        let beat = tempo.value.beat.quarters();
        if !tempo.value.bpm.is_positive() || !beat.is_positive() {
            return Err(error(
                "fermata.invalid_tempo",
                "Tempo BPM and beat duration must be positive",
            ));
        }
        let rate = &Rational::from_integer(60) / &(&tempo.value.bpm * &beat);
        rational(&rate, limits)?;
        if let Some((last_at, last_rate)) = controls.last() {
            if tempo.at < *last_at {
                return Err(error(
                    "fermata.tempo_order",
                    "Tempo controls must be nondecreasing",
                ));
            }
            if tempo.at == *last_at {
                if rate != *last_rate {
                    return Err(error(
                        "fermata.tempo_conflict",
                        "Conflicting tempo controls at the same exact time",
                    ));
                }
                continue;
            }
        }
        controls.push((tempo.at.clone(), rate));
    }
    if controls
        .first()
        .is_none_or(|(at, _)| at > &Rational::zero())
    {
        diagnostics.push(TimingDiagnostic::DefaultQuarterTempo {
            quarter_bpm: Rational::from_integer(120),
            until_quarter: controls
                .first()
                .map(|(at, _)| at.clone())
                .unwrap_or_else(|| end.clone()),
        });
        controls.insert(
            0,
            (
                Rational::zero(),
                &Rational::one() / &Rational::from_integer(2),
            ),
        );
    }
    let mut segments = Vec::new();
    let mut elapsed = Rational::zero();
    for (i, (at, rate)) in controls.iter().enumerate() {
        let stop = controls.get(i + 1).map(|(at, _)| at).unwrap_or(end);
        let next = &elapsed + &(&(stop - at) * rate);
        rational(&next, limits)?;
        if next > limits.maximum_timeline_seconds {
            return Err(error(
                "fermata.timeline_budget",
                "Base tempo timeline exceeds configured limit (hard maximum six hours)",
            ));
        }
        segments.push(TempoSegment {
            quarter_start: at.clone(),
            quarter_end: stop.clone(),
            seconds_start: elapsed,
            seconds_end: next.clone(),
            seconds_per_quarter: rate.clone(),
        });
        elapsed = next;
    }
    Ok(segments)
}
