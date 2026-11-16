//! Ordered typed references to the existing bounded notation families. XML paths
//! exist only while resolving import; none enter the public model or native SFS.
use super::xml::Node;
use scorestream_core::{
    notation::{EventId, RelationId},
    score::*,
};
use std::collections::BTreeMap;

pub(super) struct Pending {
    event: EventId,
    wrappers: Vec<Vec<PendingChild>>,
}
enum PendingChild {
    Ready(NotationChild),
    Slur { path: String, phase: NotationPhase },
}
pub(super) fn read(note: &Node, event: &EventId) -> Option<Pending> {
    let mut mixed = false;
    let wrappers = note
        .children_named("notations")
        .map(|wrapper| {
            let mut group_index = 0;
            wrapper
                .children
                .iter()
                .filter(|child| child.name != "tuplet")
                .map(|child| {
                    let phase = if child.attr("type") == Some("start") {
                        NotationPhase::Start
                    } else {
                        NotationPhase::Stop
                    };
                    match child.name.as_str() {
                        "articulations" => {
                            let value = NotationChild::Articulations { group_index };
                            group_index += 1;
                            PendingChild::Ready(value)
                        }
                        "tied" => {
                            mixed = true;
                            PendingChild::Ready(NotationChild::Tied { phase })
                        }
                        "slur" => {
                            mixed = true;
                            PendingChild::Slur {
                                path: child.path.clone(),
                                phase,
                            }
                        }
                        "fermata" => {
                            mixed = true;
                            PendingChild::Ready(NotationChild::Fermata {})
                        }
                        _ => unreachable!("closed XML schema checked before mapping"),
                    }
                })
                .collect()
        })
        .collect();
    mixed.then_some(Pending {
        event: event.clone(),
        wrappers,
    })
}
pub(super) fn resolve(
    pending: Vec<Pending>,
    slurs: &BTreeMap<String, RelationId>,
) -> Option<NotationOrder> {
    if pending.is_empty() {
        return None;
    }
    Some(NotationOrder {
        version: NotationOrderVersion::V1,
        events: pending
            .into_iter()
            .map(|event| NotationEventOrder {
                event: event.event,
                wrappers: event
                    .wrappers
                    .into_iter()
                    .map(|wrapper| {
                        wrapper
                            .into_iter()
                            .map(|child| match child {
                                PendingChild::Ready(value) => value,
                                PendingChild::Slur { path, phase } => NotationChild::Slur {
                                    relation: slurs[&path].clone(),
                                    phase,
                                },
                            })
                            .collect()
                    })
                    .collect(),
            })
            .collect(),
    })
}
