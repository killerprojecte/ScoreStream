//! Bounded performed projection. The written document remains the sole source of
//! authored notes, controls and identities; this output is never serialized as SFS.
// Keep the existing lowerer's structured fail-fast Diagnostic convention.
#![allow(clippy::result_large_err)]
use super::{lower, *};
use crate::{
    navigation as nav, navigation_extension as ext, repeat_traversal as route, Diagnostic,
    Document, Rational, Record, Scalar, Severity, SourceRange,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug)]
pub struct PerformanceLimits {
    pub maximum_occurrences: usize,
    pub maximum_events: usize,
    pub maximum_controls: usize,
}
impl Default for PerformanceLimits {
    fn default() -> Self {
        Self {
            maximum_occurrences: usize::MAX,
            maximum_events: usize::MAX,
            maximum_controls: usize::MAX,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventOccurrence {
    pub id: EventId,
    pub written_id: EventId,
    pub measure_occurrence: usize,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MeasureOccurrence {
    pub written_id: String,
    pub part: PartId,
    pub label: String,
    pub occurrence_index: usize,
    pub start: Rational,
    pub length: Rational,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PerformedScore {
    pub score: ResolvedScore,
    pub events: Vec<EventOccurrence>,
    pub measures: Vec<MeasureOccurrence>,
    pub diagnostics: Vec<Diagnostic>,
}
fn error(code: &str, message: impl Into<String>) -> Diagnostic {
    Diagnostic::error(code, message, SourceRange::default())
}
type Result<T> = std::result::Result<T, Diagnostic>;
#[derive(Clone)]
struct State {
    settings: lower::PersistentState,
    tempo: Option<TempoChange>,
    numeric: BTreeMap<Scope, Direction>,
}
struct Occurrence {
    measure: usize,
    index: usize,
    start: Rational,
    run: usize,
}

/// Closed ordinary repeats only. Open endings, unsupported spans, conditional
/// controls and relations crossing a jump fail instead of being guessed.
pub fn perform_document(
    doc: &Document,
    limits: PerformanceLimits,
) -> std::result::Result<PerformedScore, Vec<Diagnostic>> {
    let errors: Vec<_> = crate::validate(doc, None)
        .into_iter()
        .filter(|d| d.severity == Severity::Error)
        .collect();
    if !errors.is_empty() {
        return Err(errors);
    }
    perform(doc, limits, lower::LoweringProfile::Navigation, None, None).map_err(|e| vec![e])
}
pub(super) fn perform(
    doc: &Document,
    limits: PerformanceLimits,
    profile: lower::LoweringProfile,
    maximum_timing_bits: Option<u64>,
    grace_budgets: Option<(usize, usize)>,
) -> Result<PerformedScore> {
    let articulations = profile == lower::LoweringProfile::ExplicitArticulations;
    if !articulations {
        lower::reject_authored_articulations(doc)?;
    }
    let combined = profile == lower::LoweringProfile::ExplicitGracesAndFermatas;
    let fermatas = combined || profile == lower::LoweringProfile::ExplicitFermatas;
    let graces = combined || profile == lower::LoweringProfile::ExplicitGraces;
    if !fermatas
        && doc
            .required_extensions
            .iter()
            .any(|e| e == crate::fermata_extension::EXTENSION)
    {
        let mut d = error("fermata.performance_unsupported", "Authored fermatas require an explicit hold-duration policy; performed repeats cannot drop their holds");
        d.source = doc.source;
        return Err(d);
    }
    let supported = if articulations {
        doc.required_extensions
            .iter()
            .any(|e| e == crate::articulation_extension::EXTENSION)
            && doc.required_extensions.iter().all(|e| {
                e == crate::articulation_extension::EXTENSION
                    || e == crate::notation_extension::EXTENSION
                    || e == ext::EXTENSION
                    || e == crate::instrument_extension::EXTENSION
                    || e == crate::beam_extension::EXTENSION
            })
    } else if combined {
        [
            crate::fermata_extension::EXTENSION,
            crate::grace_extension::EXTENSION,
        ]
        .iter()
        .all(|required| doc.required_extensions.iter().any(|e| e == required))
            && doc.required_extensions.iter().all(|e| {
                e == crate::fermata_extension::EXTENSION
                    || e == crate::grace_extension::EXTENSION
                    || e == ext::EXTENSION
                    || e == crate::instrument_extension::EXTENSION
                    || e == crate::beam_extension::EXTENSION
            })
    } else if fermatas {
        doc.required_extensions
            .iter()
            .any(|e| e == crate::fermata_extension::EXTENSION)
            && doc.required_extensions.iter().all(|e| {
                e == crate::fermata_extension::EXTENSION
                    || e == ext::EXTENSION
                    || e == crate::instrument_extension::EXTENSION
                    || e == crate::beam_extension::EXTENSION
            })
    } else if graces {
        doc.required_extensions
            .iter()
            .any(|e| e == crate::grace_extension::EXTENSION)
            && doc.required_extensions.iter().all(|e| {
                e == crate::grace_extension::EXTENSION
                    || e == ext::EXTENSION
                    || e == crate::instrument_extension::EXTENSION
                    || e == crate::beam_extension::EXTENSION
            })
    } else {
        doc.required_extensions.iter().any(|e| e == ext::EXTENSION)
            && doc.required_extensions.iter().all(|e| {
                e == ext::EXTENSION
                    || e == crate::instrument_extension::EXTENSION
                    || e == crate::beam_extension::EXTENSION
            })
    };
    if !supported {
        return Err(if articulations {
            error("articulation.performance_capability", "Explicit articulation route supports ordinary/repeated measured music; grace, fermata and other required interpretation policies must be combined explicitly")
        } else if combined {
            error("combined.performance_capability", "Combined projection requires grace@1 and fermata@1, optionally navigation@1; other required semantics cannot be ignored")
        } else if fermatas {
            error("fermata.performance_capability", "Qualified fermata projection requires fermata@1 and optionally navigation@1; other required semantics cannot be ignored")
        } else if graces {
            error("grace.performance_capability", "Qualified grace projection requires grace@1 and optionally navigation@1; other required semantics cannot be ignored")
        } else {
            error("navigation.performance_capability", "Performed navigation requires exactly the supported navigation capability; other required semantics cannot be ignored")
        });
    }
    let mut parts: Vec<_> = doc
        .parts
        .iter()
        .map(|p| nav::Part {
            id: PartId(p.id.clone()),
            measures: vec![],
        })
        .collect();
    let part_indices: BTreeMap<_, _> = parts
        .iter()
        .enumerate()
        .map(|(i, p)| (p.id.0.clone(), i))
        .collect();
    let mut bars = vec![];
    for frame in &doc.frames {
        for record in &frame.records {
            match record {
                Record::Measure(m) => parts[*part_indices.get(&m.target).ok_or_else(|| error("navigation.measure", "Measure target must be a declared part"))?].measures.push(nav::Measure {
                    id: m.id.clone(), label: m.attributes.get("label").and_then(Scalar::as_str).unwrap_or(&m.id).into(),
                    start: frame.time.clone(), length: m.length.clone(),
                }),
                Record::Mark(m) if m.kind == ext::KIND => bars.push(ext::decode(m)?),
                Record::State(_) | Record::Mark(_) | Record::Event(_) if frame.time == doc.end => return Err(error("navigation.final_boundary_control", "A sounding/display control or event at the final boundary has no measure ownership in this performed profile")),
                _ => (),
            }
        }
    }
    for p in &parts {
        let mut at = Rational::zero();
        for m in &p.measures {
            if m.start != at || !m.length.is_positive() {
                return Err(error(
                    "navigation.measure_coverage",
                    "Every part requires positive, contiguous, explicit measures from time zero",
                ));
            }
            at = &at + &m.length;
        }
        if at != doc.end {
            return Err(error(
                "navigation.measure_coverage",
                "Every part's measures must cover the exact written end",
            ));
        }
    }
    let navigation =
        nav::reconcile(&parts, &bars, "document").map_err(|e| error(&e.code, e.message))?;
    let traversal = route::traverse(
        &navigation.movement,
        route::Policy {
            maximum_occurrences: limits.maximum_occurrences,
            ..Default::default()
        },
    )
    .map_err(|e| {
        error(
            "navigation.route_policy",
            format!("Closed ordinary route rejected: {e:?}; no missing ending is inferred"),
        )
    })?;
    let written = lower::lower(doc, profile)?;
    if !graces
        && written
            .events
            .iter()
            .any(|e| e.grace.is_some() || e.role == EventRole::Grace)
    {
        return Err(error(
            "navigation.grace_unsupported",
            "Grace timing across navigation is not yet represented",
        ));
    }
    if written
        .directions
        .iter()
        .any(|d| matches!(d.kind, DirectionKind::Dynamic { sound: true, .. }))
    {
        return Err(error("navigation.symbolic_dynamics", "Performed navigation currently supports explicit numeric dynamics and display-only dynamic signs; symbolic sound policy needs separate checkpoint support"));
    }
    if written
        .relations
        .iter()
        .any(|r| !matches!(r.kind, RelationKind::Tie { .. }))
    {
        return Err(error("navigation.span_unsupported", "Only ordinary ties are supported in performed navigation; other spans must not silently cross or reset at jumps"));
    }
    let measures = &parts[0].measures;
    let measure_of = |at: &Rational| -> Result<usize> {
        let i = measures
            .partition_point(|m| &m.start <= at)
            .checked_sub(1)
            .ok_or_else(|| {
                error(
                    "navigation.measure_coverage",
                    "Object precedes the first measure",
                )
            })?;
        if at >= &(&measures[i].start + &measures[i].length) {
            return Err(error(
                "navigation.measure_coverage",
                "Object lies outside the written measures",
            ));
        }
        Ok(i)
    };
    let mut events = vec![vec![]; measures.len()];
    let mut event_measure = BTreeMap::new();
    for e in &written.events {
        let i = measure_of(&e.start)?;
        if e.end() > &measures[i].start + &measures[i].length {
            return Err(error("navigation.event_boundary", "Measured events must end within their authored measure; use explicit tied segments"));
        }
        events[i].push(e);
        event_measure.insert(e.id.clone(), i);
    }
    let grace_counts: Vec<_> = if grace_budgets.is_some() {
        events
            .iter()
            .map(|events| {
                let graces: Vec<_> = events.iter().filter_map(|e| e.grace.as_ref()).collect();
                let principals: BTreeSet<_> = graces.iter().map(|g| &g.anchor).collect();
                (graces.len(), principals.len())
            })
            .collect()
    } else {
        vec![]
    };
    let mut projected_graces = 0usize;
    let mut projected_grace_groups = 0usize;
    let mut frames = vec![vec![]; measures.len()];
    for f in &doc.frames {
        if f.time < doc.end {
            frames[measure_of(&f.time)?].push(f);
        }
    }
    let mut directions = vec![vec![]; measures.len()];
    for d in &written.directions {
        if d.end.is_some() {
            return Err(error(
                "navigation.direction_interval",
                "Explicit finite written direction intervals need a later navigation profile",
            ));
        }
        let i = measure_of(&d.at)?;
        if let Scope::Event(id) = &d.scope {
            if event_measure.get(id) != Some(&i) {
                return Err(error(
                    "navigation.event_scope",
                    "Event-scoped directions must be in their target event's measure",
                ));
            }
        }
        directions[i].push(d);
    }
    let mut tempos = vec![vec![]; measures.len()];
    for t in &written.tempos {
        tempos[measure_of(&t.at)?].push(t);
    }
    let explicit_staff: BTreeMap<_, _> = doc
        .events()
        .map(|(_, e)| {
            (
                e.id().unwrap().to_owned(),
                e.attributes
                    .get("staff")
                    .and_then(Scalar::as_i64)
                    .map(|n| n as u32),
            )
        })
        .collect();
    let mut at = Rational::zero();
    let mut state = State {
        settings: lower::PersistentState::new(doc),
        tempo: None,
        numeric: BTreeMap::new(),
    };
    let mut checkpoints = BTreeMap::new();
    let mut result = PerformedScore {
        score: ResolvedScore {
            end: Rational::zero(),
            events: vec![],
            directions: vec![],
            relations: vec![],
            tempos: vec![],
        },
        events: vec![],
        measures: vec![],
        diagnostics: vec![],
    };
    let mut occurrences = vec![];
    let mut previous_measure = None;
    let mut run = 0;
    let mut controls = 0usize;
    for step in &traversal.steps {
        match step {
            route::Step::CaptureCheckpoint { repeat_index, .. } => {
                checkpoints.insert(*repeat_index, state.clone());
            }
            route::Step::RestoreCheckpoint { repeat_index, .. } => {
                state = checkpoints[repeat_index].clone();
            }
            route::Step::ReleaseCheckpoint { repeat_index } => {
                checkpoints.remove(repeat_index);
            }
            route::Step::ReplayMeasure {
                measure_index: i,
                occurrence_index,
                ..
            } => {
                if previous_measure.is_some_and(|p| *i != p + 1) {
                    run += 1;
                }
                previous_measure = Some(*i);
                let m = &measures[*i];
                if let Some((maximum_members, maximum_groups)) = grace_budgets {
                    projected_graces = projected_graces
                        .checked_add(grace_counts[*i].0)
                        .ok_or_else(|| {
                            error("grace.timing_budget", "Projected grace count overflow")
                        })?;
                    projected_grace_groups = projected_grace_groups
                        .checked_add(grace_counts[*i].1)
                        .ok_or_else(|| {
                            error(
                                "grace.timing_budget",
                                "Projected grace group count overflow",
                            )
                        })?;
                    if projected_graces > maximum_members || projected_grace_groups > maximum_groups
                    {
                        return Err(error(
                            "grace.timing_budget",
                            "Grace occurrence/group budget exceeded before occurrence allocation",
                        ));
                    }
                }
                // Reject rational growth on the qualified route before allocating
                // this occurrence. Discovery shares this guard without a clock.
                check_timing_bits(&at, maximum_timing_bits)?;
                let end = &at + &m.length;
                check_timing_bits(&end, maximum_timing_bits)?;
                if result
                    .measures
                    .len()
                    .checked_add(parts.len())
                    .is_none_or(|n| n > limits.maximum_controls)
                {
                    return Err(error(
                        "navigation.control_budget",
                        "Performed measure-reference budget exceeded before allocation",
                    ));
                }
                if result
                    .score
                    .events
                    .len()
                    .checked_add(events[*i].len())
                    .is_none_or(|n| n > limits.maximum_events)
                {
                    return Err(error(
                        "navigation.event_budget",
                        "Performed event budget exceeded before allocation",
                    ));
                }
                controls = controls
                    .checked_add(state.numeric.len() + directions[*i].len() + tempos[*i].len() + 1)
                    .ok_or_else(|| {
                        error(
                            "navigation.control_budget",
                            "Performed control count overflow",
                        )
                    })?;
                if controls > limits.maximum_controls {
                    return Err(error(
                        "navigation.control_budget",
                        "Performed control budget exceeded before allocation",
                    ));
                }
                let shift = |t: &Rational| -> Result<Rational> {
                    let shifted = &at + &(t - &m.start);
                    check_timing_bits(&shifted, maximum_timing_bits)?;
                    Ok(shifted)
                };
                // Controls at a boundary belong to the following measure. Checkpoint
                // capture occurs before these writes; a skipped interval executes none.
                for frame in &frames[*i] {
                    if frame.time == m.start {
                        for record in &frame.records {
                            if let Record::State(s) = record {
                                state.settings.apply(s);
                            }
                        }
                    }
                }
                for t in &tempos[*i] {
                    if t.at == m.start {
                        state.tempo = Some((*t).clone());
                    }
                }
                for d in &directions[*i] {
                    if d.at == m.start && matches!(d.kind, DirectionKind::SoundDynamics { .. }) {
                        state.numeric.insert(d.scope.clone(), (*d).clone());
                    }
                }
                let mut tempo = state.tempo.clone().unwrap_or_else(|| TempoChange {
                    at: Rational::zero(),
                    value: Tempo {
                        beat: Beat {
                            unit: BeatUnit::Quarter,
                            dotted: false,
                        },
                        bpm: Rational::from_integer(120),
                    },
                    origin: Origin {
                        object_id: "performance.default_tempo".into(),
                        source: doc.source,
                    },
                });
                tempo.at = at.clone();
                result.score.tempos.push(tempo);
                for d in state.numeric.values() {
                    let mut d = d.clone();
                    d.at = at.clone();
                    d.end = Some(end.clone());
                    result.score.directions.push(d);
                }
                // A measure's time-ordered frames apply settings before all same-time
                // notes, matching the ordinary lowerer's frame-wide state semantics.
                let mut ei = 0;
                for frame in &frames[*i] {
                    if frame.time > m.start {
                        for record in &frame.records {
                            if let Record::State(s) = record {
                                state.settings.apply(s);
                            }
                        }
                    }
                    while ei < events[*i].len() && events[*i][ei].start == frame.time {
                        let original = events[*i][ei];
                        let mut e = original.clone();
                        state
                            .settings
                            .resolve_event(&mut e, explicit_staff[&original.id.0]);
                        e.id = occurrence_id(*occurrence_index, &original.id);
                        if let Some(grace) = &mut e.grace {
                            grace.anchor = occurrence_id(*occurrence_index, &grace.anchor);
                        }
                        e.start = shift(&original.start)?;
                        check_timing_bits(&e.end(), maximum_timing_bits)?;
                        result.events.push(EventOccurrence {
                            id: e.id.clone(),
                            written_id: original.id.clone(),
                            measure_occurrence: *occurrence_index,
                        });
                        result.score.events.push(e);
                        ei += 1;
                    }
                }
                for t in &tempos[*i] {
                    if t.at > m.start {
                        state.tempo = Some((*t).clone());
                        let mut t = (*t).clone();
                        t.at = shift(&t.at)?;
                        result.score.tempos.push(t);
                    }
                }
                for d in &directions[*i] {
                    let numeric = matches!(d.kind, DirectionKind::SoundDynamics { .. });
                    if numeric {
                        state.numeric.insert(d.scope.clone(), (*d).clone());
                        if d.at == m.start {
                            continue;
                        }
                    }
                    let mut d = (*d).clone();
                    d.at = shift(&d.at)?;
                    if numeric {
                        d.end = Some(end.clone());
                    }
                    if let Scope::Event(id) = &mut d.scope {
                        *id = occurrence_id(*occurrence_index, id);
                    }
                    result.score.directions.push(d);
                }
                for p in &parts {
                    let m = &p.measures[*i];
                    result.measures.push(MeasureOccurrence {
                        written_id: m.id.clone(),
                        part: p.id.clone(),
                        label: m.label.clone(),
                        occurrence_index: *occurrence_index,
                        start: at.clone(),
                        length: m.length.clone(),
                    });
                }
                occurrences.push(Occurrence {
                    measure: *i,
                    index: *occurrence_index,
                    start: at.clone(),
                    run,
                });
                at = end;
            }
            _ => (),
        }
    }
    result.score.end = at;
    project_ties(
        &written,
        &occurrences,
        measures,
        &event_measure,
        &mut result,
        limits.maximum_controls,
        maximum_timing_bits,
    )?;
    for d in traversal.diagnostics {
        let (code,message) = match d {
            route::Diagnostic::ConventionalTwoPasses { repeat_id } => ("navigation.conventional_two_passes", format!("{repeat_id}: absent authored count uses the conventional two passes; source remains unchanged")),
            route::Diagnostic::ImplicitStartAtMovementStart { repeat_id } => ("navigation.implicit_start", format!("{repeat_id}: absent forward sign uses movement entry; source remains unchanged")),
            route::Diagnostic::UnclosedFinalEndingContinues { .. } => unreachable!("Default policy rejects this"),
        };
        result
            .diagnostics
            .push(Diagnostic::warning(code, message, doc.source));
    }
    Ok(result)
}
fn check_timing_bits(at: &Rational, maximum: Option<u64>) -> Result<()> {
    if maximum.is_some_and(|bits| at.numerator().bits() > bits || at.denominator().bits() > bits) {
        return Err(error(
            "fermata.rational_budget",
            "Qualified performed time exceeds the rational bit budget before occurrence allocation",
        ));
    }
    Ok(())
}
fn occurrence_id(index: usize, written: &EventId) -> EventId {
    EventId(format!("occ{index}:{}", written.0))
}
fn project_ties(
    written: &ResolvedScore,
    occurrences: &[Occurrence],
    measures: &[nav::Measure],
    event_measure: &BTreeMap<EventId, usize>,
    out: &mut PerformedScore,
    budget: usize,
    maximum_timing_bits: Option<u64>,
) -> Result<()> {
    let mut runs = BTreeMap::<usize, BTreeMap<usize, &Occurrence>>::new();
    for o in occurrences {
        runs.entry(o.run).or_default().insert(o.measure, o);
    }
    let mut by_measure = BTreeMap::<usize, BTreeSet<usize>>::new();
    for (ri, r) in written.relations.iter().enumerate() {
        for a in &r.anchors {
            by_measure
                .entry(event_measure[&a.event])
                .or_default()
                .insert(ri);
        }
    }
    let mut emitted = out
        .score
        .directions
        .len()
        .saturating_add(out.score.tempos.len());
    for (run, map) in runs {
        let candidates: BTreeSet<_> = map
            .keys()
            .flat_map(|mi| by_measure.get(mi).into_iter().flatten().copied())
            .collect();
        for ri in candidates {
            let relation = &written.relations[ri];
            let present = relation
                .anchors
                .iter()
                .filter(|a| map.contains_key(&event_measure[&a.event]))
                .count();
            if present == 0 {
                continue;
            }
            if present != relation.anchors.len() {
                return Err(error("navigation.relation_jump", format!("Tie {} crosses a repeat/ending jump; conditional tie realization is unsupported",relation.id)));
            }
            emitted = emitted
                .checked_add(relation.anchors.len() + 1)
                .ok_or_else(|| {
                    error(
                        "navigation.control_budget",
                        "Performed relation count overflow",
                    )
                })?;
            if emitted > budget {
                return Err(error(
                    "navigation.control_budget",
                    "Performed relation budget exceeded before allocation",
                ));
            }
            let mut relation = relation.clone();
            relation.id = RelationId(format!("run{run}:{}", relation.id));
            for a in &mut relation.anchors {
                let mi = event_measure[&a.event];
                let o = map[&mi];
                a.event = occurrence_id(o.index, &a.event);
                a.at = &o.start + &(&a.at - &measures[mi].start);
                check_timing_bits(&a.at, maximum_timing_bits)?;
            }
            out.score.relations.push(relation);
        }
    }
    // Written validation cannot prove these invariants after a skipped state
    // change. Validate the effective performed endpoints before suppressing any
    // continuation attack in the playback compiler.
    let events: BTreeMap<_, _> = out.score.events.iter().map(|e| (&e.id, e)).collect();
    let mut incoming = BTreeSet::new();
    let mut outgoing = BTreeSet::new();
    for relation in &out.score.relations {
        if !matches!(relation.kind, RelationKind::Tie { sound: true, .. }) {
            continue;
        }
        for pair in relation.anchors.windows(2) {
            let a = events[&pair[0].event];
            let b = events[&pair[1].event];
            let same_pitch = match (&a.written_pitch, &b.written_pitch) {
                (crate::Pitch::Unpitched(a), crate::Pitch::Unpitched(b)) => a == b,
                (crate::Pitch::Pitched { .. }, crate::Pitch::Pitched { .. }) => {
                    a.sounding_pitch.is_some() && a.sounding_pitch == b.sounding_pitch
                }
                _ => false,
            };
            let issue = if !same_pitch {
                Some(("navigation.tie_pitch", "Projected sounding tie endpoints have unequal effective pitches after repeat state replay"))
            } else if a.part != b.part || a.voice != b.voice || a.instrument != b.instrument {
                Some((
                    "navigation.tie_scope",
                    "Projected sounding ties must retain their part, voice and instrument route",
                ))
            } else if a.end() != b.start
                || a.id == b.id
                || pair[0].at != a.start
                || pair[1].at != b.start
            {
                Some((
                    "navigation.tie_continuity",
                    "Projected sounding tie endpoints must be distinct and exactly adjacent",
                ))
            } else if !incoming.insert(&b.id) || !outgoing.insert(&a.id) {
                Some((
                    "navigation.tie_branch",
                    "Projected sounding ties cannot branch or duplicate a continuation",
                ))
            } else {
                None
            };
            if let Some((code, message)) = issue {
                let mut d = error(code, message);
                d.source = relation.origin.source;
                d.object_id = Some(relation.origin.object_id.clone());
                return Err(d);
            }
        }
    }
    // The projected namespace is independent of written identities, but must be
    // internally unique even for unusual user-authored event strings.
    let unique: BTreeSet<_> = out.score.events.iter().map(|e| &e.id).collect();
    if unique.len() != out.score.events.len() {
        return Err(error(
            "navigation.occurrence_identity",
            "Duplicate performed occurrence identity",
        ));
    }
    Ok(())
}
