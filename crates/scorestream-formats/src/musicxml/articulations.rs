//! Exact empty-symbol articulations. Alternate shapes/attributes fail schema checks.
use super::{
    read::{error, unsupported},
    xml::Node,
};
use crate::MappingReport;
use scorestream_core::{
    notation::{EventId, Placement},
    score::*,
};
use std::{collections::BTreeMap, fmt::Write};

pub(super) fn read(note: &Node, event: &EventId) -> Result<Vec<ScoreArticulation>, MappingReport> {
    let mut result = vec![];
    for group in note
        .children_named("notations")
        .flat_map(|n| n.children_named("articulations"))
    {
        if note.child("pitch").is_none() {
            return Err(unsupported(
                note,
                "articulation.event",
                "Simple articulations require a measured pitched event; no rest or unpitched interpretation is inferred",
            ));
        }
        if group.children.is_empty() {
            return Err(unsupported(
                group,
                "articulation.empty",
                "An empty articulation container has no mapped meaning",
            ));
        }
        for node in &group.children {
            let value = SimpleArticulation::from_name(&node.name).ok_or_else(|| {
                unsupported(
                    node,
                    "articulation.value",
                    "This articulation shape has no complete typed mapping",
                )
            })?;
            let placement = match node.attr("placement") {
                None => None,
                Some("above") => Some(Placement::Above),
                Some("below") => Some(Placement::Below),
                _ => {
                    return Err(error(
                        node,
                        "articulation.placement",
                        "Placement must be above or below",
                    ));
                }
            };
            if result.iter().any(|a: &ScoreArticulation| a.value == value) {
                return Err(unsupported(
                    node,
                    "articulation.duplicate",
                    "Repeated identical signs on one note cannot be silently collapsed",
                ));
            }
            result.push(ScoreArticulation {
                event: event.clone(),
                value,
                placement,
            });
        }
    }
    Ok(result)
}
pub(super) struct Export<'a> {
    by_event: BTreeMap<&'a EventId, Vec<&'a ScoreArticulation>>,
}
impl<'a> Export<'a> {
    pub(super) fn new(score: &'a ScoreDocument) -> Self {
        let mut by_event = BTreeMap::<_, Vec<_>>::new();
        for a in &score.articulations {
            by_event.entry(&a.event).or_default().push(a);
        }
        Self { by_event }
    }
    pub(super) fn contains(&self, event: &EventId) -> bool {
        self.by_event.contains_key(event)
    }
    pub(super) fn write(&self, event: &EventId, out: &mut String) {
        let Some(marks) = self.by_event.get(event) else {
            return;
        };
        out.push_str("<articulations>");
        for a in marks {
            write!(out, "<{}", a.value.name()).unwrap();
            if let Some(place) = a.placement {
                write!(
                    out,
                    " placement=\"{}\"",
                    match place {
                        Placement::Above => "above",
                        Placement::Below => "below",
                    }
                )
                .unwrap();
            }
            out.push_str("/>");
        }
        out.push_str("</articulations>");
    }
}
