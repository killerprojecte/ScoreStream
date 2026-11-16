//! Closed scalar transport for mixed-family wrapper child order.
#![allow(clippy::result_large_err)]
use crate::{
    Diagnostic, Document, Mark, Record, RelationPhase, Scalar, SourceRange,
    notation::{EventId, RelationId},
    notation_order::*,
};
use std::collections::{BTreeMap, BTreeSet};
pub const EXTENSION: &str = "scorestream.notations@1";
pub const EVENT: &str = "scorestream.notations.event";
pub const CHILD: &str = "scorestream.notations.child";
pub fn is_kind(kind: &str) -> bool {
    kind == "scorestream.notations" || kind.starts_with("scorestream.notations.")
}
fn error(source: SourceRange, message: &str) -> Diagnostic {
    Diagnostic::error("notations.wire", message, source)
}
fn index(mark: &Mark, key: &str) -> Result<usize, Diagnostic> {
    mark.attributes
        .get(key)
        .and_then(Scalar::as_i64)
        .and_then(|i| usize::try_from(i).ok())
        .ok_or_else(|| error(mark.source, "Expected a nonnegative integer index"))
}
fn text<'a>(mark: &'a Mark, key: &str) -> Result<&'a str, Diagnostic> {
    mark.attributes
        .get(key)
        .and_then(Scalar::as_str)
        .ok_or_else(|| error(mark.source, "Expected a string field"))
}
fn fields(mark: &Mark, allowed: &[&str]) -> Result<(), Diagnostic> {
    if mark
        .attributes
        .keys()
        .any(|key| !allowed.contains(&key.as_str()))
    {
        Err(error(
            mark.source,
            "Unknown or inapplicable notation-order field",
        ))
    } else {
        Ok(())
    }
}
fn phase(mark: &Mark) -> Result<NotationPhase, Diagnostic> {
    match text(mark, "phase")? {
        "start" => Ok(NotationPhase::Start),
        "stop" => Ok(NotationPhase::Stop),
        _ => Err(error(mark.source, "Expected start or stop notation phase")),
    }
}
fn decode_child(mark: &Mark) -> Result<NotationChild, Diagnostic> {
    let mut allowed = vec!["event_index", "wrapper_index", "child_index", "kind"];
    let child = match text(mark, "kind")? {
        "articulations" => {
            allowed.push("group_index");
            NotationChild::Articulations {
                group_index: index(mark, "group_index")?,
            }
        }
        "tied" => {
            allowed.push("phase");
            NotationChild::Tied {
                phase: phase(mark)?,
            }
        }
        "slur" => {
            allowed.extend(["phase", "relation"]);
            let relation = text(mark, "relation")?;
            if !crate::is_identifier(relation) {
                return Err(error(mark.source, "Invalid slur relation identity"));
            }
            NotationChild::Slur {
                relation: RelationId(relation.into()),
                phase: phase(mark)?,
            }
        }
        "fermata" => NotationChild::Fermata {},
        _ => return Err(error(mark.source, "Unknown notation child kind")),
    };
    fields(mark, &allowed)?;
    Ok(child)
}
/// Caller maps event and slur relation identities before encoding, then validates
/// the resulting complete native document. Indices retain association order.
pub fn encode_event(event: &NotationEventOrder, event_index: usize, mapped_id: &str) -> Vec<Mark> {
    let base = |kind: &str| Mark {
        kind: kind.into(),
        target: format!("#{mapped_id}"),
        attributes: BTreeMap::from([("event_index".into(), Scalar::Integer(event_index as i64))]),
        source: SourceRange::default(),
    };
    let mut out = vec![base(EVENT)];
    for (w, children) in event.wrappers.iter().enumerate() {
        for (i, child) in children.iter().enumerate() {
            let mut mark = base(CHILD);
            mark.attributes
                .insert("wrapper_index".into(), Scalar::Integer(w as i64));
            mark.attributes
                .insert("child_index".into(), Scalar::Integer(i as i64));
            let kind = match child {
                NotationChild::Articulations { group_index } => {
                    mark.attributes
                        .insert("group_index".into(), Scalar::Integer(*group_index as i64));
                    "articulations"
                }
                NotationChild::Tied { .. } => "tied",
                NotationChild::Slur { relation, .. } => {
                    mark.attributes
                        .insert("relation".into(), Scalar::String(relation.0.clone()));
                    "slur"
                }
                NotationChild::Fermata {} => "fermata",
            };
            mark.attributes
                .insert("kind".into(), Scalar::String(kind.into()));
            if let NotationChild::Tied { phase } | NotationChild::Slur { phase, .. } = child {
                mark.attributes.insert(
                    "phase".into(),
                    Scalar::String(
                        match phase {
                            NotationPhase::Start => "start",
                            NotationPhase::Stop => "stop",
                        }
                        .into(),
                    ),
                );
            }
            out.push(mark);
        }
    }
    out
}
/// Decode independently of record/frame ordering, checking exact source onset,
/// ownership, contiguous child indices and all semantic endpoint references.
pub fn decode(doc: &Document) -> Result<Option<NotationOrder>, Vec<Diagnostic>> {
    let enabled = doc.required_extensions.iter().any(|r| r == EXTENSION);
    let marks: Vec<_> = doc
        .frames
        .iter()
        .flat_map(|f| {
            f.records.iter().filter_map(move |r| match r {
                Record::Mark(m) if is_kind(&m.kind) => Some((&f.time, m)),
                _ => None,
            })
        })
        .collect();
    let fail = |message: &str| vec![error(doc.source, message)];
    if !enabled && marks.is_empty() {
        return Ok(None);
    }
    if !enabled {
        return Err(fail(
            "Mixed notation order requires scorestream.notations@1",
        ));
    }
    if marks.is_empty() {
        return Err(fail("Mixed notation capability requires event records"));
    }
    let authored = crate::articulation_extension::decode(doc)?
        .ok_or_else(|| fail("Mixed notation order requires scorestream.articulation@1"))?;
    let mut rows = BTreeMap::new();
    for (time, mark) in marks {
        let key = (|| -> Result<_, Diagnostic> {
            let e = index(mark, "event_index")?;
            let child = match mark.kind.as_str() {
                EVENT => {
                    fields(mark, &["event_index"])?;
                    None
                }
                CHILD => Some((index(mark, "wrapper_index")?, index(mark, "child_index")?)),
                _ => return Err(error(mark.source, "Unknown notation-order record kind")),
            };
            Ok((e, child))
        })()
        .map_err(|e| vec![e])?;
        if rows.insert(key, (time, mark)).is_some() {
            return Err(fail("Duplicate notation-order index"));
        }
    }
    let authored_by_event: BTreeMap<_, _> = authored
        .events
        .iter()
        .map(|a| (a.event.0.as_str(), a))
        .collect();
    let onsets: BTreeMap<_, _> = doc
        .frames
        .iter()
        .flat_map(|f| {
            f.records.iter().filter_map(move |r| match r {
                Record::Mark(m) if m.kind == crate::articulation_extension::EVENT => {
                    Some((m.target.as_str(), &f.time))
                }
                _ => None,
            })
        })
        .collect();
    let mut result = NotationOrder {
        version: NotationOrderVersion::V1,
        events: vec![],
    };
    for ((e, child), (time, mark)) in &rows {
        if child.is_none() {
            if *e != result.events.len() {
                return Err(fail("Notation event indices must be contiguous"));
            }
            let host = mark
                .target
                .strip_prefix('#')
                .ok_or_else(|| fail("Expected event reference"))?;
            let a = authored_by_event
                .get(host)
                .ok_or_else(|| fail("Topology has no authored-articulation host"))?;
            let onset = onsets
                .get(mark.target.as_str())
                .copied()
                .expect("decoded articulation host has event record");
            if onset != *time {
                return Err(fail("Notation topology must be at the exact host onset"));
            }
            result.events.push(NotationEventOrder {
                event: a.event.clone(),
                wrappers: vec![vec![]; a.notations.len()],
            });
        } else {
            let parent = rows
                .get(&(*e, None))
                .ok_or_else(|| fail("Missing notation event parent"))?;
            if parent.0 != *time || parent.1.target != mark.target {
                return Err(fail("Notation children must share host identity and onset"));
            }
            let (w, i) = child.unwrap();
            let children = result
                .events
                .get_mut(*e)
                .and_then(|event| event.wrappers.get_mut(w))
                .ok_or_else(|| fail("Missing authored wrapper"))?;
            if i != children.len() {
                return Err(fail("Notation child indices must be contiguous"));
            }
            children.push(decode_child(mark).map_err(|e| vec![e])?);
        }
    }
    let mut expected: BTreeMap<EventId, BTreeSet<NotationChild>> = BTreeMap::new();
    let hosts: BTreeSet<_> = authored
        .events
        .iter()
        .map(|e| format!("#{}", e.event.0))
        .collect();
    for record in doc.frames.iter().flat_map(|f| &f.records) {
        match record {
            Record::Mark(mark)
                if mark.kind == crate::fermata_extension::KIND && hosts.contains(&mark.target) =>
            {
                expected
                    .entry(EventId(mark.target[1..].into()))
                    .or_default()
                    .insert(NotationChild::Fermata {});
            }
            Record::Relation(r)
                if hosts.contains(&r.anchor) && (r.kind == "tie" || r.kind == "slur") =>
            {
                let children = expected.entry(EventId(r.anchor[1..].into())).or_default();
                if r.kind == "tie" {
                    if !r.attributes.is_empty() {
                        return Err(fail(
                            "Mixed notation supports only ordinary sound-and-print ties without overrides",
                        ));
                    }
                    for phase in [NotationPhase::Stop, NotationPhase::Start] {
                        let present = (phase == NotationPhase::Stop
                            && r.phase != RelationPhase::Start)
                            || (phase == NotationPhase::Start && r.phase != RelationPhase::End);
                        if present && !children.insert(NotationChild::Tied { phase }) {
                            return Err(fail("Ambiguous repeated tied phase on mixed host"));
                        }
                    }
                } else {
                    if !doc
                        .required_extensions
                        .iter()
                        .any(|r| r == crate::slur_extension::EXTENSION)
                    {
                        return Err(fail("Mixed slurs require the authored slur capability"));
                    }
                    crate::slur_extension::decode_endpoint(r).map_err(|e| vec![e])?;
                    children.insert(NotationChild::Slur {
                        relation: RelationId(r.id.clone()),
                        phase: if r.phase == RelationPhase::Start {
                            NotationPhase::Start
                        } else {
                            NotationPhase::Stop
                        },
                    });
                }
            }
            _ => {}
        }
    }
    let errors = crate::notation_order::validate_orders(&result, &authored.events, &expected);
    if !errors.is_empty() {
        return Err(errors);
    }
    Ok(Some(result))
}
pub(crate) fn validate(doc: &Document, out: &mut Vec<Diagnostic>) {
    if let Err(errors) = decode(doc) {
        out.extend(errors);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Rational, articulation_model::*};
    fn fixture() -> (Document, NotationOrder) {
        let authored = ArticulationEvent {
            event: EventId("e".into()),
            notations: vec![
                ArticulationNotations::default(),
                ArticulationNotations {
                    source_id: Some("wrapper".into()),
                    print_object: Some(false),
                    groups: vec![ArticulationGroup::default(), ArticulationGroup::default()],
                },
            ],
        };
        let order = NotationOrder {
            version: NotationOrderVersion::V1,
            events: vec![NotationEventOrder {
                event: EventId("e".into()),
                wrappers: vec![
                    vec![],
                    vec![
                        NotationChild::Articulations { group_index: 0 },
                        NotationChild::Fermata {},
                        NotationChild::Articulations { group_index: 1 },
                    ],
                ],
            }],
        };
        let mut doc = Document::new(crate::DocumentKind::Score);
        doc.required_extensions = vec![
            EXTENSION.into(),
            crate::articulation_extension::EXTENSION.into(),
            crate::fermata_extension::EXTENSION.into(),
        ];
        doc.end = Rational::one();
        doc.parts.push(crate::Part {
            id: "p".into(),
            instrument: "piano".into(),
            staves: 1,
            display_name: None,
            source: SourceRange::default(),
        });
        doc.voices.push(crate::Voice {
            id: "v".into(),
            part: "p".into(),
            staff: 1,
            source: SourceRange::default(),
        });
        let mut records = vec![
            Record::Event(crate::Event {
                voice: "v".into(),
                pitch: "C4".parse().unwrap(),
                duration: crate::Duration::Measured(Rational::one()),
                attributes: BTreeMap::from([("id".into(), Scalar::String("e".into()))]),
                source: SourceRange::default(),
            }),
            Record::Mark(Mark {
                kind: crate::fermata_extension::KIND.into(),
                target: "#e".into(),
                attributes: BTreeMap::new(),
                source: SourceRange::default(),
            }),
        ];
        records.extend(
            crate::articulation_extension::encode_event(&authored, 0, "e")
                .into_iter()
                .map(Record::Mark),
        );
        records.extend(
            encode_event(&order.events[0], 0, "e")
                .into_iter()
                .map(Record::Mark),
        );
        doc.frames.push(crate::Frame {
            time: Rational::zero(),
            records,
            source: SourceRange::default(),
        });
        (doc, order)
    }
    fn edit_child(doc: &mut Document, i: i64, edit: impl FnOnce(&mut Mark)) {
        let mark = doc
            .frames
            .iter_mut()
            .flat_map(|f| &mut f.records)
            .find_map(|r| match r {
                Record::Mark(m)
                    if m.kind == CHILD
                        && m.attributes.get("child_index").and_then(Scalar::as_i64) == Some(i) =>
                {
                    Some(m)
                }
                _ => None,
            })
            .unwrap();
        edit(mark);
    }
    #[test]
    fn mixed_topology_roundtrip_preserves_empty_wrappers_and_interleaving() {
        let (mut doc, order) = fixture();
        assert_eq!(decode(&doc).unwrap(), Some(order.clone()));
        doc.frames[0].records.reverse();
        assert_eq!(decode(&doc).unwrap(), Some(order));
        assert!(
            crate::validate(&doc, None)
                .iter()
                .all(|d| d.severity != crate::Severity::Error),
            "{:?}",
            crate::validate(&doc, None)
        );
    }
    #[test]
    fn mixed_topology_closed_json() {
        let (_, order) = fixture();
        let mut json = serde_json::to_value(&order).unwrap();
        json["version"] = "2".into();
        assert!(serde_json::from_value::<NotationOrder>(json).is_err());
        let mut json = serde_json::to_value(&order).unwrap();
        json["events"][0]["wrappers"][1][1]["sound"] = true.into();
        assert!(serde_json::from_value::<NotationOrder>(json).is_err());
        assert!(
            serde_json::from_str::<NotationChild>(r#"{"kind":"tied","phase":"continue"}"#).is_err()
        );
    }
    #[test]
    fn mixed_topology_rejects_missing_capability_and_header_only() {
        let (mut doc, _) = fixture();
        doc.required_extensions.retain(|r| r != EXTENSION);
        assert!(decode(&doc).is_err());
        let (mut doc, _) = fixture();
        doc.frames[0]
            .records
            .retain(|r| !matches!(r, Record::Mark(m) if is_kind(&m.kind)));
        assert!(decode(&doc).is_err());
        let (mut doc, _) = fixture();
        doc.required_extensions
            .retain(|r| r != crate::articulation_extension::EXTENSION);
        assert!(decode(&doc).is_err());
    }
    #[test]
    fn mixed_topology_rejects_gaps_duplicates_foreign_fields_and_bad_references() {
        for case in 0..7 {
            let (mut doc, _) = fixture();
            edit_child(&mut doc, 1, |m| match case {
                0 => {
                    m.attributes
                        .insert("child_index".into(), Scalar::Integer(7));
                }
                1 => {
                    m.attributes
                        .insert("child_index".into(), Scalar::Integer(0));
                }
                2 => {
                    m.attributes.insert("sound".into(), Scalar::Boolean(true));
                }
                3 => {
                    m.target = "#missing".into();
                }
                4 => {
                    m.attributes
                        .insert("kind".into(), Scalar::String("slur".into()));
                    m.attributes
                        .insert("relation".into(), Scalar::String("missing".into()));
                    m.attributes
                        .insert("phase".into(), Scalar::String("start".into()));
                }
                5 => {
                    m.attributes
                        .insert("wrapper_index".into(), Scalar::Integer(99));
                }
                _ => {
                    m.attributes
                        .insert("kind".into(), Scalar::String("articulations".into()));
                    m.attributes
                        .insert("group_index".into(), Scalar::Integer(0));
                }
            });
            assert!(decode(&doc).is_err(), "case {case}");
        }
    }
    #[test]
    fn mixed_topology_never_aliases_graphic_and_sounding_ties() {
        for (field, value) in [
            ("sound", false),
            ("print", false),
            ("sound", true),
            ("print", true),
        ] {
            let (mut doc, _) = fixture();
            doc.frames[0]
                .records
                .push(Record::Relation(crate::Relation {
                    kind: "tie".into(),
                    id: "t".into(),
                    phase: RelationPhase::Start,
                    anchor: "#e".into(),
                    attributes: BTreeMap::from([(field.into(), Scalar::Boolean(value))]),
                    source: SourceRange::default(),
                }));
            let failure = decode(&doc).unwrap_err();
            assert!(
                failure
                    .iter()
                    .any(|d| d.message.contains("sound-and-print ties without overrides"))
            );
        }
    }
    #[test]
    fn mixed_topology_runtime_remains_fail_closed() {
        let (doc, _) = fixture();
        assert!(crate::notation::lower_document(&doc).is_err());
        let mut missing_header = doc.clone();
        missing_header.required_extensions.clear();
        assert!(crate::notation::lower_document(&missing_header).is_err());
    }
}
