//! Authored slur fidelity on native `slur` relation IDs. This bounded profile
//! preserves independent start/end coordinates, orientation and placement. It
//! does not infer geometry, phrasing, articulation, or a performance policy.
#![allow(clippy::result_large_err)]
use crate::{
    notation::{EventId, Placement},
    score::{ScoreDocument, SlurEndpoint, SlurOrientation},
    Diagnostic, Document, DocumentKind, Pitch, Rational, Record, Relation, RelationPhase, Scalar,
    SourceRange,
};
use std::collections::{BTreeMap, BTreeSet};

pub const EXTENSION: &str = "scorestream.slur@1";
pub(crate) fn is_endpoint_field(key: &str) -> bool {
    ["bezier_x", "bezier_y", "orientation", "placement"].contains(&key)
}
fn error(source: SourceRange, code: &str, message: &str) -> Diagnostic {
    Diagnostic::error(code, message, source)
}

/// Decode one start or end. This validates field types and event-reference syntax;
/// full validation also checks capability, identity, ownership and temporal order.
pub fn decode_endpoint(relation: &Relation) -> Result<SlurEndpoint, Diagnostic> {
    let fail = |code, message| error(relation.source, code, message);
    if relation.kind != "slur" {
        return Err(fail("slur.kind", "Expected the native slur relation kind"));
    }
    if !crate::is_identifier(&relation.id) {
        return Err(fail(
            "slur.id",
            "Slur relation ID must be a valid identifier",
        ));
    }
    if relation.phase == RelationPhase::Continue {
        return Err(fail(
            "slur.phase",
            "Authored slurs support exactly start and end, not continue",
        ));
    }
    if relation
        .attributes
        .keys()
        .any(|key| !is_endpoint_field(key))
    {
        return Err(fail(
            "slur.field",
            "Unknown or unsupported authored-slur endpoint field",
        ));
    }
    let event = relation
        .anchor
        .strip_prefix('#')
        .filter(|id| crate::is_identifier(id))
        .ok_or_else(|| fail("slur.event", "Slur anchor must be an event reference"))?;
    let number = |key| -> Result<Option<Rational>, Diagnostic> {
        match relation.attributes.get(key) {
            None => Ok(None),
            Some(Scalar::String(value)) => value.parse().map(Some).map_err(|_| {
                fail(
                    "slur.coordinate",
                    "Slur coordinates must be exact rational strings",
                )
            }),
            _ => Err(fail(
                "slur.coordinate",
                "Slur coordinates must be exact rational strings",
            )),
        }
    };
    let orientation = match relation.attributes.get("orientation") {
        None => None,
        Some(Scalar::String(value)) if value == "over" => Some(SlurOrientation::Over),
        Some(Scalar::String(value)) if value == "under" => Some(SlurOrientation::Under),
        _ => {
            return Err(fail(
                "slur.orientation",
                "Slur orientation must be over or under",
            ))
        }
    };
    let placement = match relation.attributes.get("placement") {
        None => None,
        Some(Scalar::String(value)) if value == "above" => Some(Placement::Above),
        Some(Scalar::String(value)) if value == "below" => Some(Placement::Below),
        _ => {
            return Err(fail(
                "slur.placement",
                "Slur placement must be above or below",
            ))
        }
    };
    Ok(SlurEndpoint {
        event: EventId(event.into()),
        bezier_x: number("bezier_x")?,
        bezier_y: number("bezier_y")?,
        orientation,
        placement,
    })
}

/// Encode the caller's mapped relation/event IDs (without `#`). Each absent
/// coordinate remains absent, including a missing partner in an authored pair.
pub fn encode_endpoint(
    relation_id: String,
    event_id: String,
    phase: RelationPhase,
    endpoint: &SlurEndpoint,
) -> Relation {
    let mut attributes = BTreeMap::new();
    for (key, value) in [
        ("bezier_x", &endpoint.bezier_x),
        ("bezier_y", &endpoint.bezier_y),
    ] {
        if let Some(value) = value {
            attributes.insert(key.into(), Scalar::String(value.to_string()));
        }
    }
    if let Some(value) = endpoint.orientation {
        attributes.insert(
            "orientation".into(),
            Scalar::String(
                match value {
                    SlurOrientation::Over => "over",
                    SlurOrientation::Under => "under",
                }
                .into(),
            ),
        );
    }
    if let Some(value) = endpoint.placement {
        attributes.insert(
            "placement".into(),
            Scalar::String(
                match value {
                    Placement::Above => "above",
                    Placement::Below => "below",
                }
                .into(),
            ),
        );
    }
    Relation {
        kind: "slur".into(),
        id: relation_id,
        phase,
        anchor: format!("#{event_id}"),
        attributes,
        source: SourceRange::default(),
    }
}

#[derive(Clone)]
struct Anchor {
    part: String,
    at: Rational,
    eligible: bool,
    // Only genuine before-principal graces have an order, never ordinary notes.
    grace: Option<(String, i64)>,
}
type Anchors = BTreeMap<String, Option<Anchor>>;
fn insert(anchors: &mut Anchors, id: &str, anchor: Anchor) {
    if anchors.contains_key(id) {
        anchors.insert(id.into(), None); // An ambiguous identity must not resolve.
    } else {
        anchors.insert(id.into(), Some(anchor));
    }
}
fn check_pair(
    start: &SlurEndpoint,
    end: &SlurEndpoint,
    anchors: &Anchors,
    source: SourceRange,
    out: &mut Vec<Diagnostic>,
) {
    let resolve = |endpoint: &SlurEndpoint| anchors.get(&endpoint.event.0).and_then(Option::as_ref);
    let (Some(a), Some(b)) = (resolve(start), resolve(end)) else {
        out.push(error(
            source,
            "slur.event",
            "Slur anchors must resolve to unambiguous events",
        ));
        return;
    };
    if start.event.0.is_empty()
        || end.event.0.is_empty()
        || start.event == end.event
        || !a.eligible
        || !b.eligible
    {
        out.push(error(source, "slur.anchor", "Slur endpoints must be two distinct pitched notes or genuine grace notes, not rests, silent cues or unpitched events"));
    }
    if a.part != b.part {
        out.push(error(
            source,
            "slur.part",
            "Slur endpoints must belong to the same part; voices and staves may differ",
        ));
    }
    let same_onset_order = match (&a.grace, &b.grace) {
        (Some((principal, order)), Some((other_principal, other_order))) => {
            principal == other_principal && order < other_order
        }
        (Some((principal, _)), None) => principal == &end.event.0,
        _ => false,
    };
    if a.at > b.at || (a.at == b.at && !same_onset_order) {
        out.push(error(source, "slur.order", "Slur start must precede end; equal onsets require strictly ordered genuine grace notes in one principal group"));
    }
}

/// Validate the authored-slur subset of the common model. Callers must also
/// validate the rest of the score. Distinct IDs may share the same anchor pair.
pub fn validate_score(score: &ScoreDocument) -> Vec<Diagnostic> {
    if score.slurs.is_empty() {
        return vec![];
    }
    let mut anchors = Anchors::new();
    let mut measured = BTreeMap::new();
    for part in &score.parts {
        for measure in &part.measures {
            for event in &measure.events {
                let owned = !part.id.0.is_empty()
                    && part.voices.iter().any(|v| v.id == event.voice)
                    && event.staff > 0
                    && event.staff <= part.staves
                    && !measure.start.is_negative()
                    && measure.length.is_positive()
                    && !event.offset.is_negative()
                    && event.duration.is_positive()
                    && &event.offset + &event.duration <= measure.length;
                insert(
                    &mut anchors,
                    &event.id.0,
                    Anchor {
                        part: part.id.0.clone(),
                        at: &measure.start + &event.offset,
                        eligible: owned
                            && matches!(event.pitch, Pitch::Pitched { .. })
                            && event.measure_rest != Some(true),
                        grace: None,
                    },
                );
                measured.insert(event.id.0.as_str(), event);
            }
        }
    }
    let principals = anchors.clone();
    let mut grace_orders = BTreeMap::new();
    for grace in &score.graces {
        *grace_orders
            .entry((&grace.anchor.0, grace.order))
            .or_insert(0usize) += 1;
    }
    for grace in &score.graces {
        let principal = principals.get(&grace.anchor.0).and_then(Option::as_ref);
        let event = measured.get(grace.anchor.0.as_str());
        let eligible = principal.is_some_and(|p| p.eligible)
            && event.is_some_and(|p| p.chord.is_none())
            && grace.order < 0
            && grace.timing.valid()
            && grace_orders.get(&(&grace.anchor.0, grace.order)) == Some(&1)
            && matches!(grace.pitch, Pitch::Pitched { .. });
        insert(
            &mut anchors,
            &grace.id.0,
            Anchor {
                part: principal.map_or_else(String::new, |p| p.part.clone()),
                at: principal.map_or_else(Rational::zero, |p| p.at.clone()),
                eligible,
                grace: Some((grace.anchor.0.clone(), grace.order)),
            },
        );
    }
    let mut out = Vec::new();
    let mut ids = BTreeSet::new();
    for slur in &score.slurs {
        if slur.id.0.is_empty()
            || !ids.insert(&slur.id.0)
            || score.ties.iter().any(|tie| tie.id == slur.id)
        {
            out.push(error(
                SourceRange::default(),
                "slur.id",
                "Common slur relation IDs must be nonempty, unique and not reused by another relation kind",
            ));
        }
        check_pair(
            &slur.start,
            &slur.end,
            &anchors,
            SourceRange::default(),
            &mut out,
        );
    }
    out
}

fn native_anchors(doc: &Document) -> Anchors {
    // Preserve ambiguity instead of silently selecting the last repeated event ID.
    let mut events = BTreeMap::new();
    for (time, event) in doc.events() {
        if let Some(id) = event.id() {
            if events.contains_key(id) {
                events.insert(id, None);
            } else {
                events.insert(id, Some((time, event)));
            }
        }
    }
    let mut current_staff: BTreeMap<_, _> = doc
        .voices
        .iter()
        .map(|v| (v.id.as_str(), i64::from(v.staff)))
        .collect();
    let mut staffs = BTreeMap::new();
    for frame in &doc.frames {
        for record in &frame.records {
            if let Record::State(state) = record {
                if let Some(staff) = state.attributes.get("staff").and_then(Scalar::as_i64) {
                    if let Some(current) = current_staff.get_mut(state.target.as_str()) {
                        *current = staff;
                    }
                }
            }
        }
        for record in &frame.records {
            if let Record::Event(event) = record {
                if let Some(id) = event.id() {
                    let staff = match event.attributes.get("staff") {
                        None => current_staff.get(event.voice.as_str()).copied(),
                        Some(value) => value.as_i64(),
                    };
                    staffs.insert(id, staff);
                }
            }
        }
    }
    let mut orders = BTreeMap::new();
    for (_, event) in doc.events().filter(|(_, e)| e.is_grace()) {
        if let (Some(anchor), Some(order)) = (
            event.attributes.get("anchor").and_then(Scalar::as_str),
            event.attributes.get("order").and_then(Scalar::as_i64),
        ) {
            *orders.entry((anchor, order)).or_insert(0usize) += 1;
        }
    }
    let measured_note = |event: &crate::Event| {
        event.duration.measured().is_some_and(Rational::is_positive)
            && matches!(event.pitch, Pitch::Pitched { .. })
            && match event.attributes.get("kind") {
                None => true,
                Some(value) => value.as_str() == Some("note"),
            }
            && event
                .attributes
                .get("measure_rest")
                .and_then(Scalar::as_bool)
                != Some(true)
    };
    let mut anchors = Anchors::new();
    for (time, event) in doc.events() {
        let Some(id) = event.id() else {
            continue;
        };
        let voice = doc.voices.iter().find(|v| v.id == event.voice);
        let part = voice.and_then(|v| doc.parts.iter().find(|p| p.id == v.part));
        let owned = part.is_some_and(|p| {
            staffs
                .get(id)
                .copied()
                .flatten()
                .is_some_and(|staff| staff > 0 && staff <= i64::from(p.staves))
        }) && !time.is_negative()
            && time < &doc.end;
        let (eligible, grace) = if event.is_grace() {
            let anchor_ref = event.attributes.get("anchor").and_then(Scalar::as_str);
            let principal_id = anchor_ref.and_then(|s| s.strip_prefix('#'));
            let order = event.attributes.get("order").and_then(Scalar::as_i64);
            let principal = principal_id
                .and_then(|id| events.get(id))
                .and_then(Option::as_ref);
            let valid = event.attributes.get("kind").and_then(Scalar::as_str) == Some("grace")
                && matches!(event.pitch, Pitch::Pitched { .. })
                && event.chord().is_none()
                && order.is_some_and(|n| n < 0)
                && anchor_ref
                    .zip(order)
                    .is_some_and(|key| orders.get(&key) == Some(&1))
                && principal.is_some_and(|(at, p)| {
                    *at == time
                        && measured_note(p)
                        && p.chord().is_none()
                        && p.voice == event.voice
                        && staffs.get(id) == p.id().and_then(|id| staffs.get(id))
                });
            (valid, principal_id.zip(order).map(|(id, n)| (id.into(), n)))
        } else {
            (
                measured_note(event)
                    && event
                        .duration
                        .measured()
                        .is_some_and(|duration| time + duration <= doc.end),
                None,
            )
        };
        insert(
            &mut anchors,
            id,
            Anchor {
                part: part.map_or_else(String::new, |p| p.id.clone()),
                at: time.clone(),
                eligible: owned && eligible,
                grace,
            },
        );
    }
    anchors
}

/// Validate native documents, including directly constructed or JSON-loaded ASTs.
/// Legacy slurs without the capability retain their original grammar and policy.
pub fn validate(doc: &Document, out: &mut Vec<Diagnostic>) {
    let enabled = doc.required_extensions.iter().any(|e| e == EXTENSION);
    let relations: Vec<_> = doc
        .frames
        .iter()
        .flat_map(|f| {
            f.records.iter().filter_map(move |r| match r {
                Record::Relation(r) if r.kind == "slur" => Some((&f.time, r)),
                _ => None,
            })
        })
        .collect();
    if !enabled {
        for (_, relation) in relations {
            if relation
                .attributes
                .keys()
                .any(|key| ["bezier_x", "bezier_y", "orientation"].contains(&key.as_str()))
            {
                out.push(error(
                    relation.source,
                    "slur.requirement",
                    "Authored slur endpoint fields require !requires scorestream.slur@1",
                ));
            }
        }
        return;
    }
    if relations.is_empty() {
        out.push(error(
            doc.source,
            "slur.empty_requirement",
            "Slur capability requires at least one authored slur",
        ));
    }
    if doc.document_kind != DocumentKind::Score {
        out.push(error(
            doc.source,
            "slur.document",
            "Authored slurs require a complete score",
        ));
    }
    let anchors = native_anchors(doc);
    let mut groups = BTreeMap::<&str, Vec<_>>::new();
    for (time, relation) in relations {
        match decode_endpoint(relation) {
            Ok(endpoint) => {
                if anchors
                    .get(&endpoint.event.0)
                    .and_then(Option::as_ref)
                    .is_some_and(|anchor| &anchor.at != time)
                {
                    out.push(error(
                        relation.source,
                        "slur.onset",
                        "Slur endpoint frame must equal its event's exact onset",
                    ));
                }
            }
            Err(diagnostic) => out.push(diagnostic),
        }
        groups.entry(&relation.id).or_default().push(relation);
    }
    let other_relation_ids: BTreeSet<&str> = doc
        .frames
        .iter()
        .flat_map(|f| &f.records)
        .filter_map(|record| match record {
            Record::Relation(relation) if relation.kind != "slur" => Some(relation.id.as_str()),
            _ => None,
        })
        .collect();
    for (id, group) in groups {
        if other_relation_ids.contains(id) {
            out.push(error(
                group[0].source,
                "slur.id",
                "Slur relation ID is reused by another relation kind",
            ));
        }
        if group.len() != 2
            || group
                .iter()
                .filter(|r| r.phase == RelationPhase::Start)
                .count()
                != 1
            || group
                .iter()
                .filter(|r| r.phase == RelationPhase::End)
                .count()
                != 1
        {
            out.push(error(
                group[0].source,
                "slur.phase",
                "Authored slurs require exactly one start and one end per relation ID",
            ));
            continue;
        }
        let start = group
            .iter()
            .find(|r| r.phase == RelationPhase::Start)
            .unwrap();
        let end = group
            .iter()
            .find(|r| r.phase == RelationPhase::End)
            .unwrap();
        if let (Ok(a), Ok(b)) = (decode_endpoint(start), decode_endpoint(end)) {
            check_pair(&a, &b, &anchors, start.source, out);
        }
    }
}
