//! Explicit, inspectable occurrence scheduling only. Never constructs gains or executable state.
use super::{perform_document, MeasureOccurrence, PartId, PerformanceLimits};
use crate::{
    authored_dynamics::{self, AuthoredDynamic},
    Diagnostic, Document, Rational, Record, Severity, SourceRange,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthoredDynamicSchedulePolicy {
    AdjacentWrittenOwnersV1,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoredDynamicOccurrence {
    pub source_index: usize,
    pub source: AuthoredDynamic,
    pub measure_occurrence: usize,
    pub part_visit: usize,
    pub visit_start: Rational,
    pub visit_length: Rational,
    pub performed_visual_at: Rational,
    pub performed_sounding_at: Rational,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoundaryOrdering {
    /// No simultaneous authored statements in this exact scope/occurrence time.
    IsolatedExact,
    /// Only this qualified adjacent visit proves terminal before opening.
    AdjacentTerminalBeforeOpening,
    /// Retained without any execution precedence assignment.
    Unresolved,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoredDynamicBoundary {
    pub part: String,
    pub staff: Option<u32>,
    pub at: Rational,
    pub statement_indices: Vec<usize>,
    pub ordering: BoundaryOrdering,
    /// Present only when a narrow two-statement ordering was proved.
    pub terminal_then_opening: Option<[usize; 2]>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScheduleDisclosure {
    pub code: String,
    pub message: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoredDynamicSchedule {
    pub policy: AuthoredDynamicSchedulePolicy,
    /// This result is not an executable PlaybackPlan, even when all pairs qualify.
    pub execution_withheld: bool,
    /// Complete written lane, including statements skipped by this traversal.
    pub written_statements: Vec<AuthoredDynamic>,
    pub statements: Vec<AuthoredDynamicOccurrence>,
    pub boundaries: Vec<AuthoredDynamicBoundary>,
    pub diagnostics: Vec<ScheduleDisclosure>,
}
fn fail(message: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "authored_dynamics.schedule",
        message,
        SourceRange::default(),
    )]
}

/// Inspect a selected ordering policy without mutating the written document, events,
/// ties, legacy numeric states or held-note envelopes. Normal playback still refuses
/// authored_dynamics@1. Repeated visits use the existing validated traversal.
pub fn inspect_authored_dynamic_schedule(
    doc: &Document,
    policy: AuthoredDynamicSchedulePolicy,
    limits: PerformanceLimits,
) -> Result<AuthoredDynamicSchedule, Vec<Diagnostic>> {
    let errors: Vec<_> = crate::validate(doc, None)
        .into_iter()
        .filter(|d| d.severity == Severity::Error)
        .collect();
    if !errors.is_empty() {
        return Err(errors);
    }
    let source = authored_dynamics::decode(doc)?;
    if source.is_empty() {
        return Err(fail(
            "An authored dynamics lane is required for explicit occurrence inspection",
        ));
    }
    let mut written = BTreeMap::<String, Vec<(String, Rational, Rational)>>::new();
    for f in &doc.frames {
        for r in &f.records {
            if let Record::Measure(m) = r {
                written.entry(m.target.clone()).or_default().push((
                    m.id.clone(),
                    f.time.clone(),
                    m.length.clone(),
                ));
            }
        }
    }
    for measures in written.values_mut() {
        measures.sort_by(|a, b| a.1.cmp(&b.1));
    }
    let mut owners = BTreeMap::new();
    for (part, measures) in &written {
        for (ordinal, (id, start, len)) in measures.iter().enumerate() {
            owners.insert((part.as_str(), id.as_str()), (ordinal, start, len));
        }
    }
    let repeated = doc
        .required_extensions
        .iter()
        .any(|e| e == crate::navigation_extension::EXTENSION);
    let itinerary: Vec<MeasureOccurrence> = if repeated {
        // Discovery copy only: remove this one quarantined lane so the existing
        // route machinery can report visits. No projected score escapes this API.
        // Every other capability and traversal restriction remains enforced.
        let mut route_doc = doc.clone();
        route_doc
            .required_extensions
            .retain(|e| e != authored_dynamics::EXTENSION);
        for frame in &mut route_doc.frames {
            frame
                .records
                .retain(|r| !matches!(r,Record::Mark(m) if m.kind==authored_dynamics::KIND));
        }
        perform_document(&route_doc, limits)?.measures
    } else {
        let mut visits = Vec::new();
        for (part, measures) in &written {
            for (index, (id, start, len)) in measures.iter().enumerate() {
                if index >= limits.maximum_occurrences {
                    return Err(fail("Selected measure occurrence budget exceeded"));
                }
                visits.push(MeasureOccurrence {
                    written_id: id.clone(),
                    part: PartId(part.clone()),
                    label: String::new(),
                    occurrence_index: index,
                    start: start.clone(),
                    length: len.clone(),
                });
            }
        }
        visits
    };
    let mut by_owner = BTreeMap::<(&str, &str), Vec<(usize, &AuthoredDynamic)>>::new();
    for (index, e) in source.iter().enumerate() {
        by_owner
            .entry((&e.part, &e.measure))
            .or_default()
            .push((index, e));
    }
    let mut visits = BTreeMap::<String, Vec<MeasureOccurrence>>::new();
    for visit in itinerary {
        visits.entry(visit.part.0.clone()).or_default().push(visit);
    }
    let mut statements = Vec::new();
    for (part, part_visits) in &mut visits {
        part_visits.sort_by(|a, b| {
            a.start
                .cmp(&b.start)
                .then(a.occurrence_index.cmp(&b.occurrence_index))
        });
        for (part_visit, visit) in part_visits.iter().enumerate() {
            let (_, written_start, written_len) = owners
                .get(&(part.as_str(), visit.written_id.as_str()))
                .ok_or_else(|| fail("Traversal visit has no written owner"))?;
            if visit.length != **written_len {
                return Err(fail("Traversal altered the written owner extent"));
            }
            for (source_index, e) in by_owner
                .get(&(part.as_str(), visit.written_id.as_str()))
                .into_iter()
                .flatten()
            {
                if statements.len() >= limits.maximum_controls {
                    return Err(fail(
                        "Selected authored statement occurrence budget exceeded",
                    ));
                }
                statements.push(AuthoredDynamicOccurrence {
                    source_index: *source_index,
                    source: (*e).clone(),
                    measure_occurrence: visit.occurrence_index,
                    part_visit,
                    visit_start: visit.start.clone(),
                    visit_length: visit.length.clone(),
                    performed_visual_at: &visit.start + &(&e.visual_at - *written_start),
                    performed_sounding_at: &visit.start + &(&e.sounding_at - *written_start),
                });
            }
        }
    }
    let mut groups = BTreeMap::<(String, Option<u32>, Rational), Vec<usize>>::new();
    for (i, e) in statements.iter().enumerate() {
        groups
            .entry((
                e.source.part.clone(),
                e.source.staff,
                e.performed_sounding_at.clone(),
            ))
            .or_default()
            .push(i);
    }
    let terminal = |e: &AuthoredDynamicOccurrence| {
        e.source.cursor == e.visit_length
            && e.performed_sounding_at == &e.visit_start + &e.visit_length
    };
    let opening = |e: &AuthoredDynamicOccurrence| {
        e.source.cursor == Rational::zero() && e.performed_sounding_at == e.visit_start
    };
    let mut diagnostics=vec![ScheduleDisclosure {code:"authored_dynamics.schedule_only".into(),message:"Occurrence schedule only: no gains, attack velocities, held-note updates or tie changes are produced. Normal playback remains withheld until an explicit attack-only realization consumes this evidence.".into()}];
    let visited: std::collections::BTreeSet<_> =
        statements.iter().map(|e| e.source_index).collect();
    for (index, e) in source.iter().enumerate() {
        if !visited.contains(&index) {
            diagnostics.push(ScheduleDisclosure {code:"authored_dynamics.unvisited_written_statement".into(),message:format!("Written statement {index}, owner {}, is retained but has no occurrence on this route.",e.measure)});
        }
    }
    let mut boundaries = Vec::new();
    for ((part, staff, at), indices) in groups {
        let pair = if indices.len() == 2 {
            let a = indices[0];
            let b = indices[1];
            let candidate = if terminal(&statements[a]) && opening(&statements[b]) {
                Some([a, b])
            } else if terminal(&statements[b]) && opening(&statements[a]) {
                Some([b, a])
            } else {
                None
            };
            candidate.filter(|[a, b]| {
                let left = &statements[*a];
                let right = &statements[*b];
                let lo = owners[&(part.as_str(), left.source.measure.as_str())].0;
                let ro = owners[&(part.as_str(), right.source.measure.as_str())].0;
                lo.checked_add(1) == Some(ro)
                    && left.part_visit.checked_add(1) == Some(right.part_visit)
                    && &left.visit_start + &left.visit_length == right.visit_start
            })
        } else {
            None
        };
        let ordering = if pair.is_some() {
            BoundaryOrdering::AdjacentTerminalBeforeOpening
        } else if indices.len() == 1 {
            BoundaryOrdering::IsolatedExact
        } else {
            BoundaryOrdering::Unresolved
        };
        let code = match ordering {
            BoundaryOrdering::AdjacentTerminalBeforeOpening => {
                "authored_dynamics.adjacent_visit_order"
            }
            BoundaryOrdering::IsolatedExact => "authored_dynamics.isolated_visit",
            BoundaryOrdering::Unresolved => "authored_dynamics.unresolved_visit",
        };
        diagnostics.push(ScheduleDisclosure {code:code.into(),message:format!("Part {part}, staff {staff:?}, performed time {at}: {} retained statement(s); ordering {ordering:?}. No value is coalesced or applied.",indices.len())});
        if pair.is_none() {
            for i in &indices {
                let e = &statements[*i];
                if terminal(e) || opening(e) {
                    diagnostics.push(ScheduleDisclosure {code:"authored_dynamics.unpaired_owner_visit".into(),message:format!("Written owner {} visit {} has no proven adjacent terminal/opening pair on this traversal. A skipped ending, jump, isolated boundary or final endpoint is not paired by written time alone.",e.source.measure,e.measure_occurrence)});
                }
            }
        }
        boundaries.push(AuthoredDynamicBoundary {
            part,
            staff,
            at,
            statement_indices: indices,
            ordering,
            terminal_then_opening: pair,
        });
    }
    Ok(AuthoredDynamicSchedule {
        policy,
        execution_withheld: true,
        written_statements: source,
        statements,
        boundaries,
        diagnostics,
    })
}
