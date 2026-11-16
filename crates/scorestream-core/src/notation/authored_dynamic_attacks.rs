//! Small practice-only attack percentage resolver. No note, tie, gain or route mutation.
use super::{
    inspect_authored_dynamic_schedule, AuthoredDynamicSchedule, AuthoredDynamicSchedulePolicy,
    BoundaryOrdering, PerformanceLimits,
};
use crate::{Diagnostic, Document, Rational, Record, SourceRange};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PracticeBoundaryAttackPolicy {
    AdjacentOpeningVisitAttackPercentV1,
}
/// Facts supplied by the existing qualified route/continuity layer, not raw XML.
/// onset identifies this segment; chain_attack identifies its true sounding attack.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct QualifiedPracticeAttack {
    pub occurrence_id: String,
    pub written_event_id: String,
    pub part: String,
    pub staff: u32,
    pub written_measure: String,
    pub measure_occurrence: usize,
    pub onset: Rational,
    pub chain_attack: Rational,
    /// Existing explicit per-note attack policy is never replaced.
    pub existing_attack_percent: Option<Rational>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PracticeAttackDisposition {
    Selected,
    ContinuationOrAlreadySounding,
    NoProvenOpeningVisit,
    UnresolvedBoundary,
    CompetingStatement,
    ExistingAttackPercentage,
    LegacyControlConflict,
    AmbiguousScope,
    UnverifiedAttackFacts,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PracticeAttackDecision {
    pub occurrence_id: String,
    pub disposition: PracticeAttackDisposition,
    pub percent: Option<Rational>,
    /// Retain both distinct values and their visit proof through schedule indices.
    pub terminal_then_opening: Option<[usize; 2]>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PracticeAttackResolution {
    pub policy: PracticeBoundaryAttackPolicy,
    pub schedule: AuthoredDynamicSchedule,
    pub decisions: Vec<PracticeAttackDecision>,
}
fn failure(message: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error(
        "authored_dynamics.practice_attack",
        message,
        SourceRange::default(),
    )]
}
fn scope_matches(part: &str, staff: Option<u32>, a: &QualifiedPracticeAttack) -> bool {
    part == a.part && staff.is_none_or(|s| s == a.staff)
}
/// Explicitly selected, bounded-to-opening-visit percentage suggestions for new attacks.
/// Never calls NumericDynamics, constructs an envelope, changes a held note, updates a
/// persistent state, modifies a route, or unlocks normal/strict playback.
pub fn resolve_practice_boundary_attacks(
    doc: &Document,
    attacks: &[QualifiedPracticeAttack],
    policy: PracticeBoundaryAttackPolicy,
    limits: PerformanceLimits,
) -> Result<PracticeAttackResolution, Vec<Diagnostic>> {
    let mut ids = BTreeSet::new();
    for a in attacks {
        if a.occurrence_id.is_empty()
            || !ids.insert(&a.occurrence_id)
            || a.staff == 0
            || a.onset.is_negative()
            || a.chain_attack.is_negative()
            || a.chain_attack > a.onset
            || a.existing_attack_percent
                .as_ref()
                .is_some_and(Rational::is_negative)
        {
            return Err(failure("Attack facts require unique occurrence IDs, positive staff, exact nonnegative onset/chain times and nonnegative existing percentage"));
        }
    }
    let schedule = inspect_authored_dynamic_schedule(
        doc,
        AuthoredDynamicSchedulePolicy::AdjacentWrittenOwnersV1,
        limits,
    )?;
    let hosts = crate::authored_dynamics::hosts(doc);
    let mut decisions = Vec::new();
    for a in attacks {
        let mut decision = PracticeAttackDecision {
            occurrence_id: a.occurrence_id.clone(),
            disposition: PracticeAttackDisposition::NoProvenOpeningVisit,
            percent: None,
            terminal_then_opening: None,
        };
        if a.chain_attack < a.onset {
            decision.disposition = PracticeAttackDisposition::ContinuationOrAlreadySounding;
            decisions.push(decision);
            continue;
        }
        if a.existing_attack_percent.is_some() {
            decision.disposition = PracticeAttackDisposition::ExistingAttackPercentage;
            decisions.push(decision);
            continue;
        }
        let candidates: Vec<_> = schedule
            .boundaries
            .iter()
            .filter_map(|b| {
                let pair = b.terminal_then_opening?;
                let opening = &schedule.statements[pair[1]];
                (b.ordering == BoundaryOrdering::AdjacentTerminalBeforeOpening
                    && scope_matches(&b.part, b.staff, a)
                    && opening.source.measure == a.written_measure
                    && opening.measure_occurrence == a.measure_occurrence
                    && a.onset >= b.at
                    && a.onset < &opening.visit_start + &opening.visit_length)
                    .then_some((b, pair))
            })
            .collect();
        if candidates.len() > 1 {
            decision.disposition = PracticeAttackDisposition::AmbiguousScope;
            decisions.push(decision);
            continue;
        }
        let Some((boundary, pair)) = candidates.first().copied() else {
            if schedule.boundaries.iter().any(|b| {
                b.ordering == BoundaryOrdering::Unresolved
                    && b.at == a.onset
                    && scope_matches(&b.part, b.staff, a)
            }) {
                decision.disposition = PracticeAttackDisposition::UnresolvedBoundary;
            }
            decisions.push(decision);
            continue;
        };
        // Do not carry a selected value through another explicit authored assignment,
        // including a different overlapping part/staff scope. No generic last-wins.
        let competing = schedule.statements.iter().enumerate().any(|(i, e)| {
            i != pair[0]
                && i != pair[1]
                && scope_matches(&e.source.part, e.source.staff, a)
                && e.performed_sounding_at >= boundary.at
                && e.performed_sounding_at <= a.onset
        });
        if competing {
            decision.disposition = PracticeAttackDisposition::CompetingStatement;
            decisions.push(decision);
            continue;
        }
        let opening = &schedule.statements[pair[1]];
        let (written_start, _, _) = hosts
            .get(&(a.part.clone(), a.written_measure.clone()))
            .ok_or_else(|| failure("Qualified opening owner missing"))?;
        let written_onset = written_start + &(&a.onset - &opening.visit_start);
        let event_voice = doc
            .frames
            .iter()
            .flat_map(|f| &f.records)
            .find_map(|r| match r {
                Record::Event(e) if e.id() == Some(a.written_event_id.as_str()) => {
                    Some(e.voice.as_str())
                }
                _ => None,
            });
        let exact_target = |target: &str| {
            event_voice == Some(target)
                || target == a.part
                || target == format!("{}.s{}", a.part, a.staff)
                || target == format!("#{}", a.written_event_id)
        };
        let source_event = doc.frames.iter().find_map(|f| {
            f.records.iter().find_map(|r| match r {
                Record::Event(e) if e.id() == Some(a.written_event_id.as_str()) => {
                    Some((&f.time, e))
                }
                _ => None,
            })
        });
        let verified = source_event.is_some_and(|(time, e)| {
            let voice = doc.voices.iter().find(|v| v.id == e.voice);
            let staff = e
                .attributes
                .get("staff")
                .and_then(crate::Scalar::as_i64)
                .and_then(|n| u32::try_from(n).ok())
                .or_else(|| voice.map(|v| v.staff));
            *time == written_onset
                && voice.is_some_and(|v| v.part == a.part)
                && staff == Some(a.staff)
                && e.duration.measured().is_some()
                && e.pitch != crate::Pitch::Rest
        });
        if !verified {
            decision.disposition = PracticeAttackDisposition::UnverifiedAttackFacts;
            decisions.push(decision);
            continue;
        }
        let incoming_tie=doc.frames.iter().flat_map(|f|&f.records).any(|r|matches!(r,Record::Relation(rel) if rel.kind=="tie" && rel.anchor==format!("#{}",a.written_event_id) && matches!(rel.phase,crate::RelationPhase::Continue|crate::RelationPhase::End) && rel.attributes.get("sound").and_then(crate::Scalar::as_bool).unwrap_or(true)));
        if incoming_tie {
            decision.disposition = PracticeAttackDisposition::ContinuationOrAlreadySounding;
            decisions.push(decision);
            continue;
        }
        let legacy_conflict = doc
            .frames
            .iter()
            .filter(|f| f.time >= *written_start && f.time <= written_onset)
            .flat_map(|f| &f.records)
            .any(|r| match r {
                Record::State(s) => {
                    s.attributes.contains_key("sound_dynamics") && exact_target(&s.target)
                }
                Record::Mark(m) => {
                    m.kind == "dynamic"
                        && m.attributes
                            .get("sound")
                            .and_then(crate::Scalar::as_bool)
                            .unwrap_or(true)
                        && exact_target(&m.target)
                }
                Record::Event(e) => {
                    e.id() == Some(a.written_event_id.as_str())
                        && e.attributes.contains_key("attack_dynamics")
                }
                _ => false,
            });
        if legacy_conflict {
            decision.disposition = PracticeAttackDisposition::LegacyControlConflict;
            decisions.push(decision);
            continue;
        }
        decision.disposition = PracticeAttackDisposition::Selected;
        decision.percent = Some(opening.source.percent.clone());
        decision.terminal_then_opening = Some(pair);
        decisions.push(decision);
    }
    Ok(PracticeAttackResolution {
        policy,
        schedule,
        decisions,
    })
}
