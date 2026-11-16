//! Exclusive sustained-dynamic intervals and independent transient attacks.
//! This resolves ownership/time ranges; numeric loudness is a performer policy.
use super::*;
use crate::{Diagnostic, Rational};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DynamicAssignment {
    pub scope: Scope,
    pub start: Rational,
    pub end: Rational,
    pub level: DynamicLevel,
    pub origins: Vec<Origin>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DynamicAttack {
    pub scope: Scope,
    pub at: Rational,
    pub sign: DynamicSign,
    pub origin: Origin,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HairpinInterval {
    pub id: RelationId,
    pub scope: Scope,
    pub start: Rational,
    pub end: Rational,
    pub notated_end: Rational,
    pub start_event: EventId,
    pub end_event: EventId,
    pub direction: HairpinKind,
    pub origin: Origin,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExpressionIntervals {
    pub assignments: BTreeMap<Scope, Vec<DynamicAssignment>>,
    pub attacks: BTreeMap<Scope, Vec<DynamicAttack>>,
    pub hairpins: BTreeMap<Scope, Vec<HairpinInterval>>,
    pub diagnostics: Vec<Diagnostic>,
}
impl ExpressionIntervals {
    pub fn scopes(event: &ResolvedEvent) -> [Scope; 5] {
        [
            Scope::Score,
            Scope::Part(event.part.clone()),
            Scope::Staff(event.staff.clone()),
            Scope::Voice(event.voice.clone()),
            Scope::Event(event.id.clone()),
        ]
    }
    /// Half-open intervals mean the following assignment wins at a boundary.
    pub fn assignment_at<'a>(
        &'a self,
        event: &ResolvedEvent,
        at: &Rational,
    ) -> Option<&'a DynamicAssignment> {
        Self::scopes(event)
            .iter()
            .rev()
            .find_map(|scope| self.in_scope(scope, at))
    }
    pub fn in_scope(&self, scope: &Scope, at: &Rational) -> Option<&DynamicAssignment> {
        let values = self.assignments.get(scope)?;
        let index = values.partition_point(|v| v.start <= *at).checked_sub(1)?;
        (at < &values[index].end).then_some(&values[index])
    }
    pub fn hairpin_at<'a>(
        &'a self,
        event: &ResolvedEvent,
        at: &Rational,
    ) -> Option<&'a HairpinInterval> {
        Self::scopes(event).iter().rev().find_map(|scope| {
            let values = self.hairpins.get(scope)?;
            let index = values.partition_point(|v| v.start <= *at).checked_sub(1)?;
            (at < &values[index].end).then_some(&values[index])
        })
    }
}
fn diagnostic(code: &str, message: &str, origin: &Origin) -> Diagnostic {
    let mut d = Diagnostic::error(code, message, origin.source);
    d.object_id = Some(origin.object_id.clone());
    d
}

pub fn resolve_expression(score: &ResolvedScore) -> Result<ExpressionIntervals, Vec<Diagnostic>> {
    let events: BTreeMap<_, _> = score.events.iter().map(|e| (e.id.clone(), e)).collect();
    let mut result = ExpressionIntervals::default();
    let mut errors = Vec::new();
    for direction in &score.directions {
        if matches!(&direction.kind, DirectionKind::Dynamic { sound: true, qualifier: Some(_), .. }) {
            errors.push(diagnostic("dynamics.qualifier_performance_unsupported", "Sounding dynamic qualifier requires an explicit expressive interpretation", &direction.origin));
            continue;
        }
        let DirectionKind::Dynamic { sign, sound: true, .. } = direction.kind else {
            continue;
        };
        let end = match &direction.scope {
            Scope::Event(id) => match events.get(id) {
                Some(e) => e.end(),
                None => {
                    errors.push(diagnostic(
                        "expression_target",
                        "Dynamic event target is unresolved",
                        &direction.origin,
                    ));
                    continue;
                }
            },
            _ => score.end.clone(),
        };
        let end = direction
            .end
            .as_ref()
            .map(|explicit| explicit.clone().min(end.clone()))
            .unwrap_or(end);
        if end == direction.at && direction.end.is_none() {
            let mut d = Diagnostic::warning(
                "expression_empty_scope",
                "Dynamic has no measured interval at this anchor and is not realized",
                direction.origin.source,
            );
            d.object_id = Some(direction.origin.object_id.clone());
            result.diagnostics.push(d);
            continue;
        }
        if direction.at.is_negative() || end > score.end || end <= direction.at {
            errors.push(diagnostic(
                "expression_interval",
                "Expression interval must be positive and within the score",
                &direction.origin,
            ));
            continue;
        }
        let level = match sign {
            DynamicSign::Level(level) => Some(level),
            DynamicSign::FortePiano => Some(DynamicLevel::P),
            DynamicSign::Sforzando | DynamicSign::Sforzato => None,
        };
        if let Some(level) = level {
            result
                .assignments
                .entry(direction.scope.clone())
                .or_default()
                .push(DynamicAssignment {
                    scope: direction.scope.clone(),
                    start: direction.at.clone(),
                    end,
                    level,
                    origins: vec![direction.origin.clone()],
                });
        }
        if !matches!(sign, DynamicSign::Level(_)) {
            result
                .attacks
                .entry(direction.scope.clone())
                .or_default()
                .push(DynamicAttack {
                    scope: direction.scope.clone(),
                    at: direction.at.clone(),
                    sign,
                    origin: direction.origin.clone(),
                });
        }
    }
    for values in result.assignments.values_mut() {
        values.sort_by(|a, b| a.start.cmp(&b.start).then(a.level.cmp(&b.level)));
        let mut merged = Vec::<DynamicAssignment>::new();
        for value in values.drain(..) {
            if let Some(previous) = merged.last_mut().filter(|v| v.start == value.start) {
                if previous.level != value.level {
                    errors.push(diagnostic(
                        "dynamic_conflict",
                        "Conflicting simultaneous sustained dynamics in the same scope",
                        &value.origins[0],
                    ));
                } else {
                    previous.end = previous.end.clone().min(value.end);
                    previous.origins.extend(value.origins);
                }
            } else {
                merged.push(value);
            }
        }
        for i in 0..merged.len().saturating_sub(1) {
            merged[i].end = merged[i].end.clone().min(merged[i + 1].start.clone());
        }
        *values = merged;
    }
    for values in result.attacks.values_mut() {
        values.sort_by(|a, b| a.at.cmp(&b.at).then(a.sign.cmp(&b.sign)));
    }
    for relation in &score.relations {
        let RelationKind::Hairpin { direction } = relation.kind else {
            continue;
        };
        let (Some(first), Some(last)) = (relation.anchors.first(), relation.anchors.last()) else {
            errors.push(diagnostic(
                "hairpin_anchors",
                "Hairpin requires resolved endpoints",
                &relation.origin,
            ));
            continue;
        };
        let (Some(start), Some(end)) = (events.get(&first.event), events.get(&last.event)) else {
            errors.push(diagnostic(
                "hairpin_anchors",
                "Hairpin endpoints are unresolved",
                &relation.origin,
            ));
            continue;
        };
        if start.start >= end.start
            || !relation.scope.applies(start)
            || !relation.scope.applies(end)
        {
            errors.push(diagnostic(
                "hairpin_scope",
                "Hairpin endpoints must share its scope and have increasing time",
                &relation.origin,
            ));
            continue;
        }
        let mut effective_end = end.start.clone();
        if let Some(values) = result.assignments.get(&relation.scope) {
            let i = values.partition_point(|d| d.start <= start.start);
            if let Some(next) = values.get(i).filter(|d| d.start < effective_end) {
                effective_end = next.start.clone();
            }
        }
        result
            .hairpins
            .entry(relation.scope.clone())
            .or_default()
            .push(HairpinInterval {
                id: relation.id.clone(),
                scope: relation.scope.clone(),
                start: start.start.clone(),
                end: effective_end,
                notated_end: end.start.clone(),
                start_event: start.id.clone(),
                end_event: end.id.clone(),
                direction,
                origin: relation.origin.clone(),
            });
    }
    for values in result.hairpins.values_mut() {
        values.sort_by(|a, b| a.start.cmp(&b.start).then(a.id.cmp(&b.id)));
        for pair in values.windows(2) {
            if pair[0].end > pair[1].start {
                errors.push(diagnostic(
                    "hairpin_overlap",
                    "Same-scope hairpins overlap; specify non-overlapping expression intervals",
                    &pair[1].origin,
                ));
            }
        }
    }
    if errors.is_empty() {
        Ok(result)
    } else {
        Err(errors)
    }
}
