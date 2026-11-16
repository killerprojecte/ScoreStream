//! Ordered mixed-family wrapper topology. References existing typed semantics;
//! it neither duplicates sound data nor broadens the ordinary tie profile.
use crate::{
    Diagnostic, SourceRange,
    articulation_model::ArticulationEvent,
    notation::{EventId, RelationId},
    score::ScoreDocument,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum NotationOrderVersion {
    #[serde(rename = "1")]
    V1,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotationOrder {
    pub version: NotationOrderVersion,
    pub events: Vec<NotationEventOrder>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NotationEventOrder {
    pub event: EventId,
    pub wrappers: Vec<Vec<NotationChild>>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NotationPhase {
    Start,
    Stop,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum NotationChild {
    Articulations {
        group_index: usize,
    },
    Tied {
        phase: NotationPhase,
    },
    Slur {
        relation: RelationId,
        phase: NotationPhase,
    },
    Fermata {},
}
fn error(message: &str) -> Diagnostic {
    Diagnostic::error("notations.order", message, SourceRange::default())
}

/// Validate exact topology-to-semantic bijection for one host. Empty wrappers
/// belong to the articulation hierarchy, not an inferred second wrapper model.
pub(crate) fn validate_event(
    order: &NotationEventOrder,
    authored: &ArticulationEvent,
    expected: &BTreeSet<NotationChild>,
) -> Vec<Diagnostic> {
    let mut out = vec![];
    if order.event != authored.event || order.wrappers.len() != authored.notations.len() {
        return vec![error(
            "Topology must reference the exact authored host and all its wrappers",
        )];
    }
    if expected.is_empty() {
        out.push(error(
            "Mixed topology requires a non-articulation notation family",
        ));
    }
    let mut seen = BTreeSet::new();
    let mut tied_start = false;
    for (children, wrapper) in order.wrappers.iter().zip(&authored.notations) {
        let mut next_group = 0;
        for child in children {
            if let NotationChild::Articulations { group_index } = child {
                if *group_index != next_group {
                    out.push(error("Articulation group references must be contiguous in authored order within their wrapper"));
                }
                next_group += 1;
            } else {
                if !expected.contains(child) || !seen.insert(child.clone()) {
                    out.push(error(
                        "Notation child is duplicated, foreign, or has no matching typed endpoint",
                    ));
                }
                match child {
                    NotationChild::Tied {
                        phase: NotationPhase::Start,
                    } => tied_start = true,
                    NotationChild::Tied {
                        phase: NotationPhase::Stop,
                    } if tied_start => out.push(error(
                        "Ordinary tied stop must precede start; repeat shapes are not inferred",
                    )),
                    _ => {}
                }
            }
        }
        if next_group != wrapper.groups.len() {
            out.push(error(
                "Every articulation group must be referenced exactly once in its own wrapper",
            ));
        }
    }
    if &seen != expected {
        out.push(error(
            "Topology must include every supported semantic endpoint exactly once",
        ));
    }
    out
}

pub(crate) fn validate_orders(
    order: &NotationOrder,
    authored: &[ArticulationEvent],
    expected: &BTreeMap<EventId, BTreeSet<NotationChild>>,
) -> Vec<Diagnostic> {
    let mut out = vec![];
    if order.events.is_empty() {
        out.push(error("Notation order capability requires mixed events"));
    }
    let authored_by_event: BTreeMap<_, _> = authored.iter().map(|a| (&a.event, a)).collect();
    let mut seen = BTreeSet::new();
    for event in &order.events {
        if !seen.insert(event.event.clone()) {
            out.push(error("Repeated notation-order host"));
        }
        match authored_by_event.get(&event.event) {
            Some(a) => out.extend(validate_event(
                event,
                a,
                expected.get(&event.event).unwrap_or(&BTreeSet::new()),
            )),
            None => out.push(error(
                "Mixed topology requires an existing measured pitched authored-articulation host",
            )),
        }
    }
    for a in authored {
        if expected.get(&a.event).is_some_and(|e| !e.is_empty()) && !seen.contains(&a.event) {
            out.push(error(
                "Every mixed authored host requires explicit topology",
            ));
        }
    }
    out
}
/// Cross-check the topology against existing typed tie/slur/fermata semantics.
/// Ordinary family validators remain responsible for musical validity.
pub fn validate_score(score: &ScoreDocument) -> Vec<Diagnostic> {
    let Some(order) = &score.notation_order else {
        return vec![];
    };
    let Some(authored) = &score.authored_articulations else {
        return vec![error("Notation order requires authored articulation data")];
    };
    let mut expected: BTreeMap<EventId, BTreeSet<NotationChild>> = BTreeMap::new();
    for tie in &score.ties {
        for (i, event) in tie.events.iter().enumerate() {
            let children = expected.entry(event.clone()).or_default();
            if i > 0 {
                children.insert(NotationChild::Tied {
                    phase: NotationPhase::Stop,
                });
            }
            if i + 1 < tie.events.len() {
                children.insert(NotationChild::Tied {
                    phase: NotationPhase::Start,
                });
            }
        }
    }
    for slur in &score.slurs {
        for (endpoint, phase) in [
            (&slur.start, NotationPhase::Start),
            (&slur.end, NotationPhase::Stop),
        ] {
            expected
                .entry(endpoint.event.clone())
                .or_default()
                .insert(NotationChild::Slur {
                    relation: slur.id.clone(),
                    phase,
                });
        }
    }
    for fermata in &score.fermatas {
        expected
            .entry(fermata.event.clone())
            .or_default()
            .insert(NotationChild::Fermata {});
    }
    validate_orders(order, &authored.events, &expected)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::articulation_model::{ArticulationGroup, ArticulationNotations};
    #[test]
    fn all_mixed_endpoint_refs_are_bijective_and_tied_stop_precedes_start() {
        let authored = ArticulationEvent {
            event: EventId("e".into()),
            notations: vec![
                ArticulationNotations {
                    groups: vec![ArticulationGroup::default()],
                    ..Default::default()
                },
                ArticulationNotations::default(),
            ],
        };
        let start = NotationChild::Tied {
            phase: NotationPhase::Start,
        };
        let stop = NotationChild::Tied {
            phase: NotationPhase::Stop,
        };
        let slur = NotationChild::Slur {
            relation: RelationId("s".into()),
            phase: NotationPhase::Start,
        };
        let expected = BTreeSet::from([
            start.clone(),
            stop.clone(),
            slur.clone(),
            NotationChild::Fermata {},
        ]);
        let mut order = NotationEventOrder {
            event: authored.event.clone(),
            wrappers: vec![
                vec![
                    stop.clone(),
                    NotationChild::Articulations { group_index: 0 },
                    slur,
                ],
                vec![NotationChild::Fermata {}, start.clone()],
            ],
        };
        assert!(validate_event(&order, &authored, &expected).is_empty());
        order.wrappers[0][0] = start;
        order.wrappers[1][1] = stop;
        assert!(!validate_event(&order, &authored, &expected).is_empty());
        order.wrappers[1].pop();
        assert!(!validate_event(&order, &authored, &expected).is_empty());
    }
    #[test]
    fn orphan_topology_is_not_ignored_without_authored_family() {
        let score = ScoreDocument {
            notation_order: Some(NotationOrder {
                version: NotationOrderVersion::V1,
                events: vec![],
            }),
            ..Default::default()
        };
        assert!(!crate::articulation_model::validate_score(&score).is_empty());
    }
}
