//! Bounded authored slurs. Local XML numbers are pairing keys, never identities.
use super::{
    read::{decimal, error, unsupported},
    write,
    xml::Node,
};
use crate::{Format, MappingEffect, MappingReport};
use scorestream_core::{
    SourceRange,
    notation::{EventId, PartId, Placement, RelationId},
    score::{ScoreDocument, ScoreSlur, SlurEndpoint, SlurOrientation},
};
use std::{collections::BTreeMap, fmt::Write};

type Result<T> = std::result::Result<T, MappingReport>;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Phase {
    // Prefer stops before starts when both endpoints close or both open numbers.
    Stop,
    Start,
}
impl Phase {
    fn name(self) -> &'static str {
        match self {
            Self::Start => "start",
            Self::Stop => "stop",
        }
    }
}

pub(super) struct Encounter {
    part: PartId,
    number: u8,
    phase: Phase,
    endpoint: SlurEndpoint,
    source: SourceRange,
    path: String,
}
impl Encounter {
    fn error(&self, feature: &str, message: &str) -> MappingReport {
        let mut report = MappingReport::default();
        report.push_source(
            Format::MusicXml,
            MappingEffect::Invalid,
            feature,
            &self.path,
            message,
            Some(self.source),
        );
        report
    }
}

/// Preserve literal source encounter order, including multiple notations blocks.
/// No voice, staff, pitch, or onset is part of the MusicXML local-number key.
pub(super) fn read(note: &Node, part: &PartId, event: &EventId) -> Result<Vec<Encounter>> {
    let mut endpoints = Vec::new();
    for n in note
        .children_named("notations")
        .flat_map(|notations| notations.children_named("slur"))
    {
        let mut report = MappingReport::default();
        super::xml::schema_with_geometry(
            n,
            super::GeometryPolicy::Strict,
            &mut super::geometry::Audit::default(),
            &mut report,
        );
        if report.has_errors() {
            return Err(report);
        }
        let phase = match n.attr("type") {
            Some("start") => Phase::Start,
            Some("stop") => Phase::Stop,
            _ => {
                return Err(unsupported(
                    n,
                    "slur.type",
                    "Only authored start/stop slur endpoints are mapped; continue is not discarded",
                ));
            }
        };
        let number = n
            .attr("number")
            .unwrap_or("1")
            .parse::<u8>()
            .ok()
            .filter(|number| (1..=16).contains(number))
            .ok_or_else(|| error(n, "slur.number", "Slur number must be 1 through 16"))?;
        let coordinate = |key: &str| {
            n.attr(key)
                .map(|value| {
                    let mut scalar = n.scalar_attribute(key);
                    scalar.path.push_str(&format!("/@{key}"));
                    if value
                        .chars()
                        .any(|c| c.is_whitespace() && !matches!(c, ' ' | '\t' | '\n' | '\r'))
                    {
                        return Err(error(
                            &scalar,
                            "slur.number_whitespace",
                            "Slur decimals allow only XML whitespace",
                        ));
                    }
                    decimal(&scalar)
                })
                .transpose()
        };
        let orientation = match n.attr("orientation") {
            None => None,
            Some("over") => Some(SlurOrientation::Over),
            Some("under") => Some(SlurOrientation::Under),
            _ => {
                return Err(error(
                    n,
                    "slur.orientation",
                    "Slur orientation must be over or under",
                ));
            }
        };
        let placement = match n.attr("placement") {
            None => None,
            Some("above") => Some(Placement::Above),
            Some("below") => Some(Placement::Below),
            _ => {
                return Err(error(
                    n,
                    "slur.placement",
                    "Slur placement must be above or below",
                ));
            }
        };
        endpoints.push(Encounter {
            part: part.clone(),
            number,
            phase,
            endpoint: SlurEndpoint {
                event: event.clone(),
                bezier_x: coordinate("bezier-x")?,
                bezier_y: coordinate("bezier-y")?,
                orientation,
                placement,
            },
            source: n.source,
            path: n.path.clone(),
        });
    }
    Ok(endpoints)
}

/// One open phase per part/local number. The complementary endpoint may appear
/// first in XML due to backups and voice ordering. Musical order is validated
/// separately, after measured events and grace anchors have all been resolved.
pub(super) fn resolve(
    endpoints: Vec<Encounter>,
    report: &mut MappingReport,
) -> Result<Vec<ScoreSlur>> {
    let mut pending = BTreeMap::<(PartId, u8), Encounter>::new();
    let mut slurs = Vec::new();
    for current in endpoints {
        let key = (current.part.clone(), current.number);
        if let Some(previous) = pending.remove(&key) {
            if previous.phase == current.phase {
                return Err(current.error(
                    "slur.duplicate",
                    "Duplicate open slur phase for the same part-local number is ambiguous; endpoints are not guessed or merged",
                ));
            }
            let (start, end) = if previous.phase == Phase::Start {
                (previous.endpoint, current.endpoint)
            } else {
                (current.endpoint, previous.endpoint)
            };
            slurs.push(ScoreSlur {
                id: RelationId(format!("mx_slur{}", slurs.len() + 1)),
                start,
                end,
            });
        } else {
            pending.insert(key, current);
        }
    }
    if let Some(endpoint) = pending.into_values().next() {
        return Err(endpoint.error(
            "slur.unmatched",
            "Slur endpoint has no complementary endpoint with the same part-local number",
        ));
    }
    if !slurs.is_empty() {
        report.push(Format::MusicXml, MappingEffect::Normalized, "slur.identity", "/slurs",
            "Part-local slur numbers are resolved in source encounter order into distinct relation/event identities; XML numbering is regenerated on export");
    }
    slurs.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(slurs)
}

#[derive(Clone, Copy)]
struct OutputEndpoint<'a> {
    relation: usize,
    phase: Phase,
    endpoint: &'a SlurEndpoint,
    number: u8,
}
pub(super) struct Export<'a> {
    by_event: BTreeMap<&'a EventId, Vec<OutputEndpoint<'a>>>,
}
impl<'a> Export<'a> {
    pub(super) fn new(
        score: &'a ScoreDocument,
        graces: &super::graces::Export<'_>,
        report: &mut MappingReport,
    ) -> Result<Self> {
        let mut by_event = BTreeMap::<_, Vec<_>>::new();
        for (relation, slur) in score.slurs.iter().enumerate() {
            for (phase, endpoint) in [(Phase::Start, &slur.start), (Phase::Stop, &slur.end)] {
                for value in [&endpoint.bezier_x, &endpoint.bezier_y]
                    .into_iter()
                    .flatten()
                {
                    if write::decimal(value).is_none_or(|s| s.len() > 64) {
                        let mut error = MappingReport::default();
                        error.push(Format::MusicXml, MappingEffect::Unsupported,
                            "slur.coordinate_decimal", &slur.id.0,
                            "Slur coordinates require exact finite decimal output within the bounded XML numeric profile; no rounding or regeneration");
                        return Err(error);
                    }
                }
                by_event
                    .entry(&endpoint.event)
                    .or_default()
                    .push(OutputEndpoint {
                        relation,
                        phase,
                        endpoint,
                        number: 0,
                    });
            }
        }
        // This is XML encounter order, not musical-time order. In particular a
        // voice backup can cause the first encountered endpoint to be a stop.
        for part in &score.parts {
            let mut open = BTreeMap::<usize, u8>::new();
            let mut slots = [false; 16];
            for measure in &part.measures {
                for voice in &part.voices {
                    for event in write::ordered_events(measure, &voice.id) {
                        for id in graces
                            .before(&event.id)
                            .map(|g| &g.id)
                            .chain(std::iter::once(&event.id))
                        {
                            let Some(endpoints) = by_event.get_mut(id) else {
                                continue;
                            };
                            // Close existing numbers before opening any others,
                            // even when a stop-first relation closes with a start.
                            // Phase is secondary, so ordinary stop/start reuse
                            // still emits its stop first. Emit this exact order.
                            endpoints.sort_by(|a, b| {
                                open.contains_key(&b.relation)
                                    .cmp(&open.contains_key(&a.relation))
                                    .then_with(|| a.phase.cmp(&b.phase))
                                    .then_with(|| {
                                        score.slurs[a.relation].id.cmp(&score.slurs[b.relation].id)
                                    })
                            });
                            for endpoint in endpoints {
                                if let Some(number) = open.remove(&endpoint.relation) {
                                    endpoint.number = number;
                                    slots[usize::from(number - 1)] = false;
                                } else {
                                    let Some(slot) = slots.iter().position(|used| !used) else {
                                        let mut error = MappingReport::default();
                                        error.push(Format::MusicXml, MappingEffect::Unsupported,
                                            "slur.number_capacity", &endpoint.endpoint.event.0,
                                            "More than 16 simultaneously open part-local slurs in emitted XML order cannot be numbered exactly");
                                        return Err(error);
                                    };
                                    let number = (slot + 1) as u8;
                                    slots[slot] = true;
                                    open.insert(endpoint.relation, number);
                                    endpoint.number = number;
                                }
                            }
                        }
                    }
                }
            }
            // Core validation has already established both endpoints in one part.
            debug_assert!(open.is_empty());
        }
        if !by_event.is_empty() {
            report.push(Format::MusicXml, MappingEffect::Normalized, "slur.identity", "/slurs",
                "MusicXML has local numbered endpoints, not SFS relation identities; independent slurs and endpoint data are preserved while relation IDs are regenerated on import");
        }
        crate::slurs::authored_only(score, Format::MusicXml, report);
        Ok(Self { by_event })
    }

    pub(super) fn contains(&self, event: &EventId) -> bool {
        self.by_event.contains_key(event)
    }

    pub(super) fn write(&self, event: &EventId, out: &mut String) {
        for endpoint in self.by_event.get(event).into_iter().flatten() {
            write!(
                out,
                "<slur type=\"{}\" number=\"{}\"",
                endpoint.phase.name(),
                endpoint.number
            )
            .unwrap();
            for (key, value) in [
                ("bezier-x", &endpoint.endpoint.bezier_x),
                ("bezier-y", &endpoint.endpoint.bezier_y),
            ] {
                if let Some(value) = value {
                    write!(out, " {key}=\"{}\"", write::decimal(value).unwrap()).unwrap();
                }
            }
            if let Some(orientation) = endpoint.endpoint.orientation {
                let value = match orientation {
                    SlurOrientation::Over => "over",
                    SlurOrientation::Under => "under",
                };
                write!(out, " orientation=\"{value}\"").unwrap();
            }
            if let Some(placement) = endpoint.endpoint.placement {
                let value = match placement {
                    Placement::Above => "above",
                    Placement::Below => "below",
                };
                write!(out, " placement=\"{value}\"").unwrap();
            }
            out.push_str("/>");
        }
    }
}
