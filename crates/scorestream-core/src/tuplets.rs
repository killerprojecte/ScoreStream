//! Shared single-wrapper topology for authored tuplets and ornaments. No pairing or clock inference.
use crate::{
    Diagnostic, Document, DocumentKind, Mark, Record, RelationPhase, Scalar, SourceRange,
    notation::{EventId, RelationId},
    notation_order::{NotationChild, NotationPhase},
    score::ScoreDocument,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
pub const EXTENSION: &str = "scorestream.tuplets@1";
pub const ORNAMENT_EXTENSION: &str = "scorestream.ornaments@1";
pub const ORNAMENT_KIND: &str = "scorestream.ornaments.child";
pub const KIND: &str = "scorestream.tuplets.child";
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TupletNotation {
    pub event: EventId,
    pub children: Vec<TupletChild>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum TupletChild {
    Tuplet {
        phase: NotationPhase,
        bracket: Option<bool>,
    },
    Ornament {
        group_index: usize,
        ornament: Ornament,
    },
    Existing {
        notation: NotationChild,
    },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Ornament {
    TrillMark {},
    Tremolo {
        strokes: u8,
    },
    WavyLine {
        phase: NotationPhase,
        number: Option<String>,
        default_y: Option<crate::Rational>,
    },
}
impl Ornament {
    pub fn valid(&self) -> bool {
        match self {
            Self::TrillMark {} => true,
            Self::Tremolo { strokes } => (1..=4).contains(strokes),
            Self::WavyLine {
                number, default_y, ..
            } => {
                number.as_ref().is_none_or(|n| n == "1")
                    && default_y.as_ref().is_none_or(crate::geometry::decimal)
            }
        }
    }
}
pub fn has_ornaments(g: &TupletNotation) -> bool {
    g.children
        .iter()
        .any(|c| matches!(c, TupletChild::Ornament { .. }))
}
pub fn has_tuplets(g: &TupletNotation) -> bool {
    g.children
        .iter()
        .any(|c| matches!(c, TupletChild::Tuplet { .. }))
}
pub fn is_kind(k: &str) -> bool {
    k == "scorestream.tuplets"
        || k.starts_with("scorestream.tuplets.")
        || k == "scorestream.ornaments"
        || k.starts_with("scorestream.ornaments.")
}
fn error(s: SourceRange, message: &str) -> Vec<Diagnostic> {
    vec![Diagnostic::error("tuplets.wire", message, s)]
}
fn valid(group: &TupletNotation, expected: &BTreeSet<NotationChild>, groups: usize) -> bool {
    let mut seen = BTreeSet::new();
    let mut next_group = 0;
    let mut endpoints = 0;
    let mut start = false;
    let mut ornament_group = None;
    let mut next_ornament_group = 0;
    for c in &group.children {
        if !matches!(c, TupletChild::Ornament { .. }) {
            ornament_group = None;
        }
        match c {
            TupletChild::Ornament {
                group_index,
                ornament,
            } => {
                if !ornament.valid() {
                    return false;
                }
                if ornament_group != Some(*group_index) {
                    if *group_index != next_ornament_group {
                        return false;
                    }
                    next_ornament_group += 1;
                    ornament_group = Some(*group_index);
                }
                endpoints += 1;
            }
            TupletChild::Tuplet { phase, bracket } => {
                if *phase == NotationPhase::Stop && bracket.is_some() {
                    return false;
                }
                endpoints += 1;
            }
            TupletChild::Existing {
                notation: NotationChild::Articulations { group_index },
            } => {
                if *group_index != next_group {
                    return false;
                }
                next_group += 1;
            }
            TupletChild::Existing { notation } => {
                if !expected.contains(notation) || !seen.insert(notation.clone()) {
                    return false;
                }
                if matches!(
                    notation,
                    NotationChild::Tied {
                        phase: NotationPhase::Start
                    }
                ) {
                    start = true;
                }
                if start
                    && matches!(
                        notation,
                        NotationChild::Tied {
                            phase: NotationPhase::Stop
                        }
                    )
                {
                    return false;
                }
            }
        }
    }
    endpoints > 0 && next_group == groups && seen == *expected
}
pub fn encode(group: &TupletNotation, id: &str) -> Vec<Mark> {
    let wire_kind = if has_ornaments(group) {
        ORNAMENT_KIND
    } else {
        KIND
    };
    group
        .children
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let mut a = BTreeMap::from([("index".into(), Scalar::Integer(i as i64))]);
            let (kind, phase) = match c {
                TupletChild::Ornament {
                    group_index,
                    ornament,
                } => {
                    a.insert(
                        "ornament_group".into(),
                        Scalar::Integer(*group_index as i64),
                    );
                    match ornament {
                        Ornament::TrillMark {} => ("trill_mark", None),
                        Ornament::Tremolo { strokes } => {
                            a.insert("strokes".into(), Scalar::Integer(i64::from(*strokes)));
                            ("tremolo", None)
                        }
                        Ornament::WavyLine {
                            phase,
                            number,
                            default_y,
                        } => {
                            if let Some(n) = number {
                                a.insert("number".into(), Scalar::String(n.clone()));
                            }
                            if let Some(y) = default_y {
                                a.insert("default_y".into(), Scalar::String(y.to_string()));
                            }
                            ("wavy_line", Some(*phase))
                        }
                    }
                }
                TupletChild::Tuplet { phase, bracket } => {
                    if let Some(v) = bracket {
                        a.insert("bracket".into(), Scalar::Boolean(*v));
                    }
                    ("tuplet", Some(*phase))
                }
                TupletChild::Existing { notation } => match notation {
                    NotationChild::Articulations { group_index } => {
                        a.insert("group_index".into(), Scalar::Integer(*group_index as i64));
                        ("articulations", None)
                    }
                    NotationChild::Tied { phase } => ("tied", Some(*phase)),
                    NotationChild::Slur { phase, relation } => {
                        a.insert("relation".into(), Scalar::String(relation.0.clone()));
                        ("slur", Some(*phase))
                    }
                    NotationChild::Fermata {} => ("fermata", None),
                },
            };
            a.insert("kind".into(), Scalar::String(kind.into()));
            if let Some(p) = phase {
                a.insert(
                    "phase".into(),
                    Scalar::String(
                        if p == NotationPhase::Start {
                            "start"
                        } else {
                            "stop"
                        }
                        .into(),
                    ),
                );
            }
            Mark {
                target: format!("#{id}"),
                kind: wire_kind.into(),
                attributes: a,
                source: SourceRange::default(),
            }
        })
        .collect()
}
pub fn decode(doc: &Document) -> Result<Vec<TupletNotation>, Vec<Diagnostic>> {
    let enabled = doc.required_extensions.iter().any(|e| e == EXTENSION);
    let ornaments_enabled = doc
        .required_extensions
        .iter()
        .any(|e| e == ORNAMENT_EXTENSION);
    let mut wire_kinds = BTreeMap::new();
    let hosts: BTreeMap<_, _> = doc
        .events()
        .filter(|(_, e)| !e.is_grace())
        .filter_map(|(t, e)| e.id().map(|id| (id, t)))
        .collect();
    let mut parsed = BTreeMap::<EventId, BTreeMap<usize, TupletChild>>::new();
    for f in &doc.frames {
        for r in &f.records {
            let Record::Mark(m) = r else { continue };
            if !is_kind(&m.kind) {
                continue;
            }
            let fail = || {
                error(
                    m.source,
                    "Invalid tuplet capability, host/onset, index, or closed child fields",
                )
            };
            if doc.document_kind != DocumentKind::Score
                || !((enabled && m.kind == KIND) || (ornaments_enabled && m.kind == ORNAMENT_KIND))
            {
                return Err(fail());
            }
            let id = m.target.strip_prefix('#').ok_or_else(fail)?;
            if hosts.get(id) != Some(&&f.time) {
                return Err(fail());
            }
            if wire_kinds
                .insert(id.to_string(), m.kind.clone())
                .is_some_and(|k| k != m.kind)
            {
                return Err(fail());
            }
            let a = &m.attributes;
            let index = |k: &str| {
                a.get(k)
                    .and_then(Scalar::as_i64)
                    .and_then(|v| usize::try_from(v).ok())
                    .ok_or_else(fail)
            };
            let phase = || match a.get("phase").and_then(Scalar::as_str) {
                Some("start") => Ok(NotationPhase::Start),
                Some("stop") => Ok(NotationPhase::Stop),
                _ => Err(fail()),
            };
            let mut allowed = vec!["index", "kind"];
            let child = match a.get("kind").and_then(Scalar::as_str) {
                Some("trill_mark" | "tremolo" | "wavy_line")
                    if ornaments_enabled && m.kind == ORNAMENT_KIND =>
                {
                    allowed.push("ornament_group");
                    let ornament = match a.get("kind").and_then(Scalar::as_str).unwrap() {
                        "trill_mark" => Ornament::TrillMark {},
                        "tremolo" => {
                            allowed.push("strokes");
                            Ornament::Tremolo {
                                strokes: u8::try_from(index("strokes")?).map_err(|_| fail())?,
                            }
                        }
                        _ => {
                            allowed.extend(["phase", "number", "default_y"]);
                            Ornament::WavyLine {
                                phase: phase()?,
                                number: a
                                    .get("number")
                                    .map(|s| s.as_str().map(str::to_owned).ok_or_else(fail))
                                    .transpose()?,
                                default_y: a
                                    .get("default_y")
                                    .map(|s| {
                                        s.as_str().ok_or_else(fail)?.parse().map_err(|_| fail())
                                    })
                                    .transpose()?,
                            }
                        }
                    };
                    TupletChild::Ornament {
                        group_index: index("ornament_group")?,
                        ornament,
                    }
                }
                Some("tuplet") if enabled => {
                    allowed.extend(["phase", "bracket"]);
                    TupletChild::Tuplet {
                        phase: phase()?,
                        bracket: a
                            .get("bracket")
                            .map(|v| v.as_bool().ok_or_else(fail))
                            .transpose()?,
                    }
                }
                Some("articulations") => {
                    allowed.push("group_index");
                    TupletChild::Existing {
                        notation: NotationChild::Articulations {
                            group_index: index("group_index")?,
                        },
                    }
                }
                Some("tied") => {
                    allowed.push("phase");
                    TupletChild::Existing {
                        notation: NotationChild::Tied { phase: phase()? },
                    }
                }
                Some("slur") => {
                    allowed.extend(["phase", "relation"]);
                    let relation = a
                        .get("relation")
                        .and_then(Scalar::as_str)
                        .filter(|s| crate::is_identifier(s))
                        .ok_or_else(fail)?;
                    TupletChild::Existing {
                        notation: NotationChild::Slur {
                            phase: phase()?,
                            relation: RelationId(relation.into()),
                        },
                    }
                }
                Some("fermata") => TupletChild::Existing {
                    notation: NotationChild::Fermata {},
                },
                _ => return Err(fail()),
            };
            if a.keys().any(|k| !allowed.contains(&k.as_str()))
                || parsed
                    .entry(EventId(id.into()))
                    .or_default()
                    .insert(index("index")?, child)
                    .is_some()
            {
                return Err(fail());
            }
        }
    }
    if enabled && parsed.is_empty() {
        return Err(error(
            doc.source,
            "Tuplet capability needs endpoint records",
        ));
    }
    let mut out = vec![];
    for (event, children) in parsed {
        if children.keys().copied().ne(0..children.len()) {
            return Err(error(doc.source, "Tuplet child indices must be contiguous"));
        }
        out.push(TupletNotation {
            event,
            children: children.into_values().collect(),
        });
    }
    if enabled != out.iter().any(has_tuplets)
        || ornaments_enabled != out.iter().any(has_ornaments)
        || out
            .iter()
            .any(|g| (wire_kinds[&g.event.0] == ORNAMENT_KIND) != has_ornaments(g))
    {
        return Err(error(
            doc.source,
            "Topology capability and canonical host kind must match retained endpoint/ornament families",
        ));
    }
    let authored = if doc
        .required_extensions
        .iter()
        .any(|e| e == crate::articulation_extension::EXTENSION)
    {
        crate::articulation_extension::decode(doc)?
    } else {
        None
    };
    let orders = crate::notation_extension::decode(doc)?;
    let mut expected = BTreeMap::<EventId, BTreeSet<NotationChild>>::new();
    let selected: BTreeSet<_> = out.iter().map(|g| format!("#{}", g.event.0)).collect();
    for r in doc.frames.iter().flat_map(|f| &f.records) {
        match r {
            Record::Mark(m)
                if selected.contains(&m.target) && m.kind == crate::fermata_extension::KIND =>
            {
                expected
                    .entry(EventId(m.target[1..].into()))
                    .or_default()
                    .insert(NotationChild::Fermata {});
            }
            Record::Relation(r)
                if selected.contains(&r.anchor) && (r.kind == "tie" || r.kind == "slur") =>
            {
                let set = expected.entry(EventId(r.anchor[1..].into())).or_default();
                if r.kind == "tie" {
                    if !r.attributes.is_empty() {
                        return Err(error(
                            r.source,
                            "Tuplet topology supports only ordinary ties",
                        ));
                    }
                    for p in [NotationPhase::Stop, NotationPhase::Start] {
                        if (p == NotationPhase::Stop && r.phase != RelationPhase::Start)
                            || (p == NotationPhase::Start && r.phase != RelationPhase::End)
                        {
                            set.insert(NotationChild::Tied { phase: p });
                        }
                    }
                } else {
                    if !doc
                        .required_extensions
                        .iter()
                        .any(|e| e == crate::slur_extension::EXTENSION)
                    {
                        return Err(error(
                            r.source,
                            "Tuplet slur references require authored slur metadata",
                        ));
                    }
                    crate::slur_extension::decode_endpoint(r).map_err(|e| vec![e])?;
                    set.insert(NotationChild::Slur {
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
    let authored_by_event: BTreeMap<_, _> = authored
        .iter()
        .flat_map(|a| &a.events)
        .map(|a| (&a.event, a))
        .collect();
    let legacy_hosts: BTreeSet<_> = doc
        .frames
        .iter()
        .flat_map(|f| &f.records)
        .filter_map(|r| {
            if let Record::Mark(m) = r {
                if m.kind == "articulation" {
                    m.target.strip_prefix('#').map(|s| EventId(s.into()))
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect();
    let orders_by_event: BTreeMap<_, _> = orders
        .iter()
        .flat_map(|o| &o.events)
        .map(|o| (&o.event, o))
        .collect();
    for g in &out {
        let a = authored_by_event.get(&g.event).copied();
        let groups = match a {
            Some(a)
                if a.notations.len() == 1
                    && a.notations[0].source_id.is_none()
                    && a.notations[0].print_object.is_none() =>
            {
                a.notations[0].groups.len()
            }
            Some(_) => {
                return Err(error(
                    doc.source,
                    "Tuplet profile needs one unadorned wrapper",
                ));
            }
            None => usize::from(legacy_hosts.contains(&g.event)),
        };
        if !valid(
            g,
            expected.get(&g.event).unwrap_or(&BTreeSet::new()),
            groups,
        ) {
            return Err(error(
                doc.source,
                "Tuplet topology must include every existing sibling exactly once",
            ));
        }
        if let Some(order) = orders_by_event.get(&g.event).copied() {
            let filtered: Vec<_> = g
                .children
                .iter()
                .filter_map(|c| {
                    if let TupletChild::Existing { notation } = c {
                        Some(notation.clone())
                    } else {
                        None
                    }
                })
                .collect();
            if order.wrappers != vec![filtered] {
                return Err(error(doc.source, "Conflicting existing notation topology"));
            }
        }
    }
    Ok(out)
}
pub fn validate(doc: &Document, out: &mut Vec<Diagnostic>) {
    if let Err(mut errors) = decode(doc) {
        out.append(&mut errors);
    }
}
pub fn validate_score(score: &ScoreDocument) -> Vec<Diagnostic> {
    if score.tuplets.is_empty() {
        return vec![];
    }
    let mut expected = BTreeMap::<EventId, BTreeSet<NotationChild>>::new();
    for t in &score.ties {
        for (i, e) in t.events.iter().enumerate() {
            let set = expected.entry(e.clone()).or_default();
            if i > 0 {
                set.insert(NotationChild::Tied {
                    phase: NotationPhase::Stop,
                });
            }
            if i + 1 < t.events.len() {
                set.insert(NotationChild::Tied {
                    phase: NotationPhase::Start,
                });
            }
        }
    }
    for s in &score.slurs {
        for (e, phase) in [
            (&s.start.event, NotationPhase::Start),
            (&s.end.event, NotationPhase::Stop),
        ] {
            expected
                .entry(e.clone())
                .or_default()
                .insert(NotationChild::Slur {
                    relation: s.id.clone(),
                    phase,
                });
        }
    }
    for f in &score.fermatas {
        expected
            .entry(f.event.clone())
            .or_default()
            .insert(NotationChild::Fermata {});
    }
    let events: BTreeSet<_> = score
        .parts
        .iter()
        .flat_map(|p| &p.measures)
        .flat_map(|m| &m.events)
        .map(|e| &e.id)
        .collect();
    let mut seen = BTreeSet::new();
    let authored_by_event: BTreeMap<_, _> = score
        .authored_articulations
        .iter()
        .flat_map(|a| &a.events)
        .map(|a| (&a.event, a))
        .collect();
    let legacy_hosts: BTreeSet<_> = score.articulations.iter().map(|a| &a.event).collect();
    let orders_by_event: BTreeMap<_, _> = score
        .notation_order
        .iter()
        .flat_map(|o| &o.events)
        .map(|o| (&o.event, o))
        .collect();
    for g in &score.tuplets {
        let a = authored_by_event.get(&g.event).copied();
        let groups = match a {
            Some(a)
                if a.notations.len() == 1
                    && a.notations[0].source_id.is_none()
                    && a.notations[0].print_object.is_none() =>
            {
                a.notations[0].groups.len()
            }
            Some(_) => {
                return error(
                    SourceRange::default(),
                    "Tuplet profile needs one unadorned wrapper",
                );
            }
            None => usize::from(legacy_hosts.contains(&g.event)),
        };
        if !events.contains(&g.event)
            || !seen.insert(&g.event)
            || !valid(
                g,
                expected.get(&g.event).unwrap_or(&BTreeSet::new()),
                groups,
            )
        {
            return error(
                SourceRange::default(),
                "Invalid tuplet owner or incomplete ordered sibling references",
            );
        }
        if let Some(order) = orders_by_event.get(&g.event).copied() {
            let filtered: Vec<_> = g
                .children
                .iter()
                .filter_map(|c| {
                    if let TupletChild::Existing { notation } = c {
                        Some(notation.clone())
                    } else {
                        None
                    }
                })
                .collect();
            if order.wrappers != vec![filtered] {
                return error(
                    SourceRange::default(),
                    "Conflicting existing notation topology",
                );
            }
        }
    }
    vec![]
}
