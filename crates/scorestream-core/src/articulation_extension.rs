//! Lossless, scalar-only SFS transport for the authored articulation v1 family.
#![allow(clippy::result_large_err)]
use crate::{
    Diagnostic, Document, DocumentKind, Mark, Pitch, Rational, Record, Scalar, SourceRange,
    articulation_model::*,
    notation::{EventId, Placement},
};
use std::collections::{BTreeMap, BTreeSet};
pub const EXTENSION: &str = "scorestream.articulation@1";
pub const EVENT: &str = "scorestream.articulation.event";
pub const NOTATIONS: &str = "scorestream.articulation.notations";
pub const GROUP: &str = "scorestream.articulation.group";
pub const ITEM: &str = "scorestream.articulation.item";
/// Whether a mark belongs to the reserved authored-articulation namespace.
pub fn is_kind(kind: &str) -> bool {
    kind == "scorestream.articulation" || kind.starts_with("scorestream.articulation.")
}
fn err(m: &Mark, msg: &str) -> Diagnostic {
    Diagnostic::error("articulation.wire", msg, m.source)
}
fn string<'a>(m: &'a Mark, key: &str) -> Result<Option<&'a str>, Diagnostic> {
    match m.attributes.get(key) {
        None => Ok(None),
        Some(Scalar::String(s)) => Ok(Some(s)),
        _ => Err(err(m, "Expected a string scalar")),
    }
}
fn num(m: &Mark, key: &str) -> Result<Option<Rational>, Diagnostic> {
    string(m, key)?
        .map(|s| {
            s.parse()
                .map_err(|_| err(m, "Expected an exact rational string"))
        })
        .transpose()
}
fn index(m: &Mark, key: &str) -> Result<usize, Diagnostic> {
    m.attributes
        .get(key)
        .and_then(Scalar::as_i64)
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| err(m, "Missing or invalid nonnegative index"))
}
fn choice<T>(
    m: &Mark,
    key: &str,
    parse: impl FnOnce(&str) -> Option<T>,
) -> Result<Option<T>, Diagnostic> {
    string(m, key)?
        .map(|s| parse(s).ok_or_else(|| err(m, "Unknown enumeration value")))
        .transpose()
}
fn text(m: &Mark, key: &str) -> Result<Option<String>, Diagnostic> {
    Ok(string(m, key)?.map(str::to_owned))
}
fn fields(m: &Mark, allowed: &[&str]) -> Result<(), Diagnostic> {
    if m.attributes.keys().any(|k| !allowed.contains(&k.as_str())) {
        Err(err(
            m,
            "Unknown or inapplicable authored-articulation field",
        ))
    } else {
        Ok(())
    }
}
fn decode_item(m: &Mark) -> Result<AuthoredArticulation, Diagnostic> {
    let name = string(m, "sign")?.ok_or_else(|| err(m, "Missing articulation sign"))?;
    let mut allowed = vec![
        "event_index",
        "wrapper_index",
        "group_index",
        "item_index",
        "sign",
        "placement",
        "default_x",
        "default_y",
        "relative_x",
        "relative_y",
        "font_family",
        "font_style",
        "font_size_points",
        "font_size_css",
        "font_weight",
        "color",
    ];
    match name {
        "strong-accent" => allowed.push("orientation"),
        "scoop" | "plop" | "doit" | "falloff" => allowed.extend([
            "line_shape",
            "line_type",
            "line_length",
            "dash_length",
            "space_length",
        ]),
        "breath-mark" | "caesura" => allowed.push("shape"),
        "other-articulation" => allowed.extend(["text", "smufl"]),
        _ => {}
    }
    fields(m, &allowed)?;
    let line = || -> Result<ArticulationLine, Diagnostic> {
        Ok(ArticulationLine {
            shape: choice(m, "line_shape", ArticulationLineShape::from_name)?,
            line_type: choice(m, "line_type", ArticulationLineType::from_name)?,
            length: choice(m, "line_length", ArticulationLineLength::from_name)?,
            dash_length: num(m, "dash_length")?,
            space_length: num(m, "space_length")?,
        })
    };
    let sign = match name {
        "accent" => ArticulationSign::Accent {},
        "staccato" => ArticulationSign::Staccato {},
        "tenuto" => ArticulationSign::Tenuto {},
        "detached-legato" => ArticulationSign::DetachedLegato {},
        "staccatissimo" => ArticulationSign::Staccatissimo {},
        "spiccato" => ArticulationSign::Spiccato {},
        "stress" => ArticulationSign::Stress {},
        "unstress" => ArticulationSign::Unstress {},
        "soft-accent" => ArticulationSign::SoftAccent {},
        "strong-accent" => ArticulationSign::StrongAccent {
            orientation: choice(m, "orientation", ArticulationOrientation::from_name)?,
        },
        "scoop" => ArticulationSign::Scoop { line: line()? },
        "plop" => ArticulationSign::Plop { line: line()? },
        "doit" => ArticulationSign::Doit { line: line()? },
        "falloff" => ArticulationSign::Falloff { line: line()? },
        "breath-mark" => ArticulationSign::BreathMark {
            shape: choice(m, "shape", BreathMarkShape::from_name)?,
        },
        "caesura" => ArticulationSign::Caesura {
            shape: choice(m, "shape", CaesuraShape::from_name)?,
        },
        "other-articulation" => ArticulationSign::OtherArticulation {
            text: text(m, "text")?.ok_or_else(|| {
                err(
                    m,
                    "Other articulation requires text, including explicit empty text",
                )
            })?,
            smufl: text(m, "smufl")?,
        },
        _ => return Err(err(m, "Unknown articulation sign")),
    };
    let points = num(m, "font_size_points")?;
    let css = choice(m, "font_size_css", ArticulationCssFontSize::from_name)?;
    if points.is_some() && css.is_some() {
        return Err(err(m, "Font size cannot be both points and CSS"));
    }
    Ok(AuthoredArticulation {
        sign,
        placement: choice(m, "placement", |s| match s {
            "above" => Some(Placement::Above),
            "below" => Some(Placement::Below),
            _ => None,
        })?,
        print_style: ArticulationPrintStyle {
            default_x: num(m, "default_x")?,
            default_y: num(m, "default_y")?,
            relative_x: num(m, "relative_x")?,
            relative_y: num(m, "relative_y")?,
            font_family: text(m, "font_family")?,
            font_style: choice(m, "font_style", ArticulationFontStyle::from_name)?,
            font_size: points
                .map(ArticulationFontSize::Points)
                .or_else(|| css.map(ArticulationFontSize::Css)),
            font_weight: choice(m, "font_weight", ArticulationFontWeight::from_name)?,
            color: text(m, "color")?,
        },
    })
}
fn put(m: &mut Mark, key: &str, value: impl ToString) {
    m.attributes
        .insert(key.into(), Scalar::String(value.to_string()));
}
fn base(
    kind: &str,
    id: &str,
    e: usize,
    w: Option<usize>,
    g: Option<usize>,
    i: Option<usize>,
) -> Mark {
    let mut m = Mark {
        kind: kind.into(),
        target: format!("#{id}"),
        attributes: BTreeMap::new(),
        source: SourceRange::default(),
    };
    for (key, n) in [
        ("event_index", Some(e)),
        ("wrapper_index", w),
        ("group_index", g),
        ("item_index", i),
    ] {
        if let Some(n) = n {
            m.attributes.insert(key.into(), Scalar::Integer(n as i64));
        }
    }
    m
}
/// Encode one ordered event with a caller-mapped native event ID (without `#`).
/// Indices are zero based and preserve empty wrappers/groups and repeated signs.
/// Callers must validate the typed score before encoding and the final document.
pub fn encode_event(event: &ArticulationEvent, event_index: usize, mapped_id: &str) -> Vec<Mark> {
    let mut out = vec![base(EVENT, mapped_id, event_index, None, None, None)];
    for (w, wrapper) in event.notations.iter().enumerate() {
        let mut m = base(NOTATIONS, mapped_id, event_index, Some(w), None, None);
        if let Some(id) = &wrapper.source_id {
            put(&mut m, "source_id", id);
        }
        if let Some(v) = wrapper.print_object {
            m.attributes
                .insert("print_object".into(), Scalar::Boolean(v));
        }
        out.push(m);
        for (g, group) in wrapper.groups.iter().enumerate() {
            let mut m = base(GROUP, mapped_id, event_index, Some(w), Some(g), None);
            if let Some(id) = &group.source_id {
                put(&mut m, "source_id", id);
            }
            out.push(m);
            for (i, item) in group.items.iter().enumerate() {
                let mut m = base(ITEM, mapped_id, event_index, Some(w), Some(g), Some(i));
                put(&mut m, "sign", item.sign.name());
                if let Some(v) = item.placement {
                    put(
                        &mut m,
                        "placement",
                        match v {
                            Placement::Above => "above",
                            Placement::Below => "below",
                        },
                    );
                }
                let s = &item.print_style;
                if let Some(v) = &s.default_x {
                    put(&mut m, "default_x", v);
                }
                if let Some(v) = &s.default_y {
                    put(&mut m, "default_y", v);
                }
                if let Some(v) = &s.relative_x {
                    put(&mut m, "relative_x", v);
                }
                if let Some(v) = &s.relative_y {
                    put(&mut m, "relative_y", v);
                }
                if let Some(v) = &s.font_family {
                    put(&mut m, "font_family", v);
                }
                if let Some(v) = &s.color {
                    put(&mut m, "color", v);
                }
                if let Some(v) = s.font_style {
                    put(&mut m, "font_style", v.name());
                }
                if let Some(v) = s.font_weight {
                    put(&mut m, "font_weight", v.name());
                }
                if let Some(v) = &s.font_size {
                    match v {
                        ArticulationFontSize::Points(v) => put(&mut m, "font_size_points", v),
                        ArticulationFontSize::Css(v) => put(&mut m, "font_size_css", v.name()),
                    }
                }
                match &item.sign {
                    ArticulationSign::StrongAccent {
                        orientation: Some(v),
                    } => put(&mut m, "orientation", v.name()),
                    ArticulationSign::BreathMark { shape: Some(v) } => {
                        put(&mut m, "shape", v.name())
                    }
                    ArticulationSign::Caesura { shape: Some(v) } => put(&mut m, "shape", v.name()),
                    ArticulationSign::OtherArticulation { text, smufl } => {
                        put(&mut m, "text", text);
                        if let Some(v) = smufl {
                            put(&mut m, "smufl", v);
                        }
                    }
                    _ => {}
                }
                if let Some(l) = item.sign.line() {
                    if let Some(v) = l.shape {
                        put(&mut m, "line_shape", v.name());
                    }
                    if let Some(v) = l.line_type {
                        put(&mut m, "line_type", v.name());
                    }
                    if let Some(v) = l.length {
                        put(&mut m, "line_length", v.name());
                    }
                    if let Some(v) = &l.dash_length {
                        put(&mut m, "dash_length", v);
                    }
                    if let Some(v) = &l.space_length {
                        put(&mut m, "space_length", v);
                    }
                }
                out.push(m);
            }
        }
    }
    out
}
/// Decode and validate the complete authored wire family. This checks capability,
/// closed fields, contiguous hierarchy, source identities, measured pitched
/// ownership and exact onset, including directly constructed native ASTs.
pub fn decode(doc: &Document) -> Result<Option<AuthoredArticulations>, Vec<Diagnostic>> {
    let enabled = doc.required_extensions.iter().any(|s| s == EXTENSION);
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
    if !enabled && marks.is_empty() {
        return Ok(None);
    }
    let fail = |message: &str| vec![Diagnostic::error("articulation.wire", message, doc.source)];
    if !enabled {
        return Err(fail(
            "Authored articulations require scorestream.articulation@1",
        ));
    }
    if marks.is_empty() {
        return Err(fail(
            "Authored articulation capability requires event records",
        ));
    }
    if doc.document_kind != DocumentKind::Score {
        return Err(fail("Authored articulations require a complete score"));
    }
    let mut rows = BTreeMap::new();
    for (time, m) in marks {
        let result = (|| -> Result<_, Diagnostic> {
            let e = index(m, "event_index")?;
            let (w, g, i) = match m.kind.as_str() {
                EVENT => {
                    fields(m, &["event_index"])?;
                    (None, None, None)
                }
                NOTATIONS => {
                    fields(
                        m,
                        &["event_index", "wrapper_index", "source_id", "print_object"],
                    )?;
                    (Some(index(m, "wrapper_index")?), None, None)
                }
                GROUP => {
                    fields(
                        m,
                        &["event_index", "wrapper_index", "group_index", "source_id"],
                    )?;
                    (
                        Some(index(m, "wrapper_index")?),
                        Some(index(m, "group_index")?),
                        None,
                    )
                }
                ITEM => (
                    Some(index(m, "wrapper_index")?),
                    Some(index(m, "group_index")?),
                    Some(index(m, "item_index")?),
                ),
                _ => return Err(err(m, "Unregistered authored-articulation kind")),
            };
            if !m.target.strip_prefix('#').is_some_and(crate::is_identifier) {
                return Err(err(m, "Expected native event reference"));
            }
            Ok((e, w, g, i))
        })();
        let key = result.map_err(|e| vec![e])?;
        if rows.insert(key, (time, m)).is_some() {
            return Err(fail("Duplicate authored hierarchy index"));
        }
    }
    let mut result = AuthoredArticulations {
        version: ArticulationVersion::V1,
        events: vec![],
    };
    let mut event_ids = BTreeSet::new();
    let mut source_ids = BTreeSet::new();
    let mut native_ids = BTreeSet::new();
    for p in &doc.parts {
        native_ids.insert(p.id.as_str());
    }
    for v in &doc.voices {
        native_ids.insert(v.id.as_str());
    }
    for f in &doc.frames {
        for r in &f.records {
            match r {
                Record::Event(e) => {
                    if let Some(id) = e.id() {
                        native_ids.insert(id);
                    }
                    if let Some(id) = e.chord() {
                        native_ids.insert(id);
                    }
                }
                Record::Relation(r) => {
                    native_ids.insert(r.id.as_str());
                }
                Record::Measure(m) => {
                    native_ids.insert(m.id.as_str());
                }
                Record::Mark(m) => {
                    if let Some(id) = m.attributes.get("id").and_then(Scalar::as_str) {
                        native_ids.insert(id);
                    }
                    if [
                        crate::instrument_extension::KIND,
                        crate::navigation_extension::KIND,
                    ]
                    .contains(&m.kind.as_str())
                    {
                        if let Some(id) = m.attributes.get("source_id").and_then(Scalar::as_str) {
                            native_ids.insert(id);
                        }
                    }
                }
                _ => {}
            }
        }
    }
    // Resolve all hosts and incompatible families once: O(records log records),
    // independent of the number of authored wrappers/items on any event.
    let parts: BTreeMap<_, _> = doc
        .parts
        .iter()
        .map(|p| (p.id.as_str(), p.staves))
        .collect();
    let voices: BTreeMap<_, _> = doc
        .voices
        .iter()
        .map(|v| (v.id.as_str(), v.part.as_str()))
        .collect();
    let mut staffs: BTreeMap<_, _> = doc
        .voices
        .iter()
        .map(|v| (v.id.as_str(), Some(i64::from(v.staff))))
        .collect();
    let mut hosts = BTreeMap::new();
    let mut incompatible = BTreeSet::new();
    let mixed_enabled = doc
        .required_extensions
        .iter()
        .any(|r| r == crate::notation_extension::EXTENSION);
    for frame in &doc.frames {
        for r in &frame.records {
            if let Record::State(state) = r {
                if let Some(value) = state.attributes.get("staff") {
                    if let Some(staff) = staffs.get_mut(state.target.as_str()) {
                        *staff = value.as_i64();
                    }
                }
            }
        }
        for r in &frame.records {
            match r {
                Record::Event(event) => {
                    if let Some(id) = event.id() {
                        let staff = match event.attributes.get("staff") {
                            Some(value) => value.as_i64(),
                            None => staffs.get(event.voice.as_str()).copied().flatten(),
                        };
                        let owned = voices
                            .get(event.voice.as_str())
                            .and_then(|p| parts.get(p))
                            .is_some_and(|count| {
                                staff.is_some_and(|s| s > 0 && s <= i64::from(*count))
                            });
                        let value = (&frame.time, event, owned);
                        if hosts.contains_key(id) {
                            hosts.insert(id, None);
                        } else {
                            hosts.insert(id, Some(value));
                        }
                    }
                }
                Record::Mark(mark)
                    if mark.kind == "articulation"
                        || (!mixed_enabled && mark.kind == crate::fermata_extension::KIND) =>
                {
                    incompatible.insert(mark.target.as_str());
                }
                Record::Relation(relation)
                    if !mixed_enabled && (relation.kind == "tie" || relation.kind == "slur") =>
                {
                    incompatible.insert(relation.anchor.as_str());
                }
                _ => {}
            }
        }
    }
    for ((e, w, g, i), (time, m)) in &rows {
        let attempt = (|| -> Result<(), Diagnostic> {
            if m.kind == EVENT {
                if *e != result.events.len() {
                    return Err(err(m, "Event indices must be contiguous from zero"));
                }
                let id = m.target.strip_prefix('#').unwrap();
                if !event_ids.insert(id) {
                    return Err(err(m, "Repeated authored event anchor"));
                }
                let &(onset, event, owned) = hosts
                    .get(id)
                    .and_then(Option::as_ref)
                    .ok_or_else(|| err(m, "Anchor must identify exactly one native event"))?;
                if !owned
                    || onset != *time
                    || onset.is_negative()
                    || !event
                        .duration
                        .measured()
                        .is_some_and(|d| d.is_positive() && onset + d <= doc.end)
                    || !matches!(event.pitch, Pitch::Pitched { .. })
                    || event
                        .attributes
                        .get("kind")
                        .is_some_and(|s| s.as_str() != Some("note"))
                    || event
                        .attributes
                        .get("measure_rest")
                        .and_then(Scalar::as_bool)
                        == Some(true)
                {
                    return Err(err(
                        m,
                        "Anchor must be an owned measured pitched event at its exact onset",
                    ));
                }
                if incompatible.contains(m.target.as_str()) {
                    return Err(err(
                        m,
                        "Authored family cannot overlap legacy articulations or mixed notation families",
                    ));
                }
                result.events.push(ArticulationEvent {
                    event: EventId(id.into()),
                    notations: vec![],
                });
                return Ok(());
            }
            let parent = rows
                .get(&(*e, None, None, None))
                .ok_or_else(|| err(m, "Missing event parent"))?;
            if parent.0 != *time || parent.1.target != m.target {
                return Err(err(
                    m,
                    "Hierarchy records must share their event target and onset",
                ));
            }
            let event = result
                .events
                .get_mut(*e)
                .ok_or_else(|| err(m, "Missing event parent"))?;
            if m.kind == NOTATIONS {
                if *w != Some(event.notations.len()) {
                    return Err(err(m, "Wrapper indices must be contiguous"));
                }
                let source_id = text(m, "source_id")?;
                let print_object = match m.attributes.get("print_object") {
                    None => None,
                    Some(Scalar::Boolean(v)) => Some(*v),
                    _ => return Err(err(m, "print_object must be boolean")),
                };
                event.notations.push(ArticulationNotations {
                    source_id,
                    print_object,
                    groups: vec![],
                });
            } else {
                let wrapper = event
                    .notations
                    .get_mut(w.unwrap())
                    .ok_or_else(|| err(m, "Missing wrapper parent"))?;
                if m.kind == GROUP {
                    if *g != Some(wrapper.groups.len()) {
                        return Err(err(m, "Group indices must be contiguous"));
                    }
                    wrapper.groups.push(ArticulationGroup {
                        source_id: text(m, "source_id")?,
                        items: vec![],
                    });
                } else {
                    let group = wrapper
                        .groups
                        .get_mut(g.unwrap())
                        .ok_or_else(|| err(m, "Missing group parent"))?;
                    if *i != Some(group.items.len()) {
                        return Err(err(m, "Item indices must be contiguous"));
                    }
                    group.items.push(decode_item(m)?);
                }
            }
            if let Some(id) = string(m, "source_id")? {
                if native_ids.contains(id) || !source_ids.insert(id.to_owned()) {
                    return Err(err(
                        m,
                        "Source IDs must be unique and disjoint from native identities",
                    ));
                }
            }
            Ok(())
        })();
        attempt.map_err(|e| vec![e])?;
    }
    let errors: Vec<_> = result
        .events
        .iter()
        .flat_map(crate::articulation_model::validate_event)
        .collect();
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
    fn source() -> (Document, AuthoredArticulations) {
        let signs = vec![
            ArticulationSign::Accent {},
            ArticulationSign::StrongAccent {
                orientation: Some(ArticulationOrientation::Down),
            },
            ArticulationSign::Staccato {},
            ArticulationSign::Tenuto {},
            ArticulationSign::DetachedLegato {},
            ArticulationSign::Staccatissimo {},
            ArticulationSign::Spiccato {},
            ArticulationSign::Scoop {
                line: ArticulationLine::default(),
            },
            ArticulationSign::Plop {
                line: ArticulationLine::default(),
            },
            ArticulationSign::Doit {
                line: ArticulationLine::default(),
            },
            ArticulationSign::Falloff {
                line: ArticulationLine {
                    shape: Some(ArticulationLineShape::Curved),
                    line_type: Some(ArticulationLineType::Dashed),
                    length: Some(ArticulationLineLength::Long),
                    dash_length: Some("3/2".parse().unwrap()),
                    space_length: Some("1/2".parse().unwrap()),
                },
            },
            ArticulationSign::BreathMark {
                shape: Some(BreathMarkShape::Salzedo),
            },
            ArticulationSign::Caesura {
                shape: Some(CaesuraShape::Single),
            },
            ArticulationSign::Stress {},
            ArticulationSign::Unstress {},
            ArticulationSign::SoftAccent {},
            ArticulationSign::OtherArticulation {
                text: "literal <accent/> & 私".into(),
                smufl: Some("articAccentAbove".into()),
            },
        ];
        let authored = AuthoredArticulations {
            version: ArticulationVersion::V1,
            events: vec![ArticulationEvent {
                event: EventId("e".into()),
                notations: vec![
                    ArticulationNotations {
                        source_id: Some("wrapper".into()),
                        print_object: Some(false),
                        groups: vec![],
                    },
                    ArticulationNotations {
                        source_id: None,
                        print_object: Some(true),
                        groups: vec![
                            ArticulationGroup::default(),
                            ArticulationGroup {
                                source_id: Some("group".into()),
                                items: signs
                                    .into_iter()
                                    .map(|sign| AuthoredArticulation {
                                        sign,
                                        placement: Some(Placement::Below),
                                        print_style: ArticulationPrintStyle {
                                            default_x: Some("-1/2".parse().unwrap()),
                                            default_y: Some("2/3".parse().unwrap()),
                                            relative_x: Some(Rational::zero()),
                                            relative_y: Some(Rational::one()),
                                            font_family: Some("Times New Roman".into()),
                                            font_style: Some(ArticulationFontStyle::Italic),
                                            font_weight: Some(ArticulationFontWeight::Bold),
                                            font_size: Some(ArticulationFontSize::Points(
                                                "19/2".parse().unwrap(),
                                            )),
                                            color: Some("#80AABBCC".into()),
                                        },
                                    })
                                    .collect(),
                            },
                        ],
                    },
                ],
            }],
        };
        let mut doc = Document::new(DocumentKind::Score);
        doc.required_extensions.push(EXTENSION.into());
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
        let mut records = vec![Record::Event(crate::Event {
            voice: "v".into(),
            pitch: "C4".parse().unwrap(),
            duration: crate::Duration::Measured(Rational::one()),
            attributes: [("id".into(), Scalar::String("e".into()))].into(),
            source: SourceRange::default(),
        })];
        records.extend(
            encode_event(&authored.events[0], 0, "e")
                .into_iter()
                .map(Record::Mark),
        );
        doc.frames.push(crate::Frame {
            time: Rational::zero(),
            records,
            source: SourceRange::default(),
        });
        (doc, authored)
    }
    #[test]
    fn all_seventeen_signs_and_empty_hierarchy_roundtrip() {
        let (doc, expected) = source();
        assert_eq!(decode(&doc).unwrap(), Some(expected));
    }
    #[test]
    fn global_event_order_is_independent_of_frame_order() {
        let (mut doc, mut expected) = source();
        let mut later = expected.events[0].clone();
        later.event = EventId("later".into());
        for w in &mut later.notations {
            w.source_id = None;
            for g in &mut w.groups {
                g.source_id = None;
            }
        }
        expected.events.insert(0, later.clone());
        for r in &mut doc.frames[0].records {
            if let Record::Mark(m) = r {
                m.attributes
                    .insert("event_index".into(), Scalar::Integer(1));
            }
        }
        let mut note = doc.frames[0].records[0].clone();
        if let Record::Event(e) = &mut note {
            e.attributes
                .insert("id".into(), Scalar::String("later".into()));
        }
        let mut records = vec![note];
        records.extend(
            encode_event(&later, 0, "later")
                .into_iter()
                .map(Record::Mark),
        );
        doc.end = Rational::from_integer(2);
        doc.frames.push(crate::Frame {
            time: Rational::one(),
            records,
            source: SourceRange::default(),
        });
        assert_eq!(decode(&doc).unwrap(), Some(expected));
    }
    #[test]
    fn reject_header_bypass_empty_unknown_version_and_duplicate_indices() {
        let (doc, _) = source();
        let mut bad = doc.clone();
        bad.required_extensions.clear();
        assert!(decode(&bad).is_err());
        let mut bad = doc.clone();
        bad.required_extensions = vec!["scorestream.articulation@2".into()];
        assert!(decode(&bad).is_err());
        let mut bad = doc.clone();
        bad.frames[0]
            .records
            .retain(|r| !matches!(r, Record::Mark(_)));
        assert!(decode(&bad).is_err());
        let mut bad = doc.clone();
        let duplicate = bad.frames[0].records[1].clone();
        bad.frames[0].records.push(duplicate);
        assert!(decode(&bad).is_err());
    }
    #[test]
    fn reject_gaps_inapplicable_fields_wrong_types_and_source_collisions() {
        for (kind, key, value) in [
            (ITEM, "item_index", Scalar::Integer(100)),
            (ITEM, "orientation", Scalar::String("up".into())),
            (ITEM, "default_x", Scalar::Integer(1)),
            (NOTATIONS, "source_id", Scalar::String("e".into())),
            (GROUP, "wrapper_index", Scalar::Integer(100)),
            (NOTATIONS, "print_object", Scalar::String("false".into())),
        ] {
            let (mut doc, _) = source();
            let m = doc.frames[0]
                .records
                .iter_mut()
                .find_map(|r| match r {
                    Record::Mark(m) if m.kind == kind => Some(m),
                    _ => None,
                })
                .unwrap();
            m.attributes.insert(key.into(), value);
            assert!(decode(&doc).is_err(), "{kind}: {key}");
        }
    }
    #[test]
    fn reject_instrument_and_navigation_source_identity_collisions() {
        for kind in [
            crate::instrument_extension::KIND,
            crate::navigation_extension::KIND,
        ] {
            let (mut doc, _) = source();
            doc.frames[0].records.push(Record::Mark(Mark {
                target: "p".into(),
                kind: kind.into(),
                attributes: [("source_id".into(), Scalar::String("wrapper".into()))].into(),
                source: SourceRange::default(),
            }));
            assert!(decode(&doc).is_err());
        }
    }
    #[test]
    fn closed_shape_and_enum_domains_do_not_cross_signs() {
        for (sign, key, value) in [
            ("breath-mark", "shape", "single"),
            ("caesura", "shape", "salzedo"),
            ("strong-accent", "orientation", "above"),
            ("falloff", "line_shape", "wavy"),
            ("accent", "font_size_css", "12"),
            ("accent", "placement", "up"),
        ] {
            let (mut doc, _) = source();
            let m = doc.frames[0]
                .records
                .iter_mut()
                .find_map(|r| match r {
                    Record::Mark(m)
                        if m.kind == ITEM
                            && m.attributes.get("sign").and_then(Scalar::as_str) == Some(sign) =>
                    {
                        Some(m)
                    }
                    _ => None,
                })
                .unwrap();
            m.attributes
                .insert(key.into(), Scalar::String(value.into()));
            assert!(decode(&doc).is_err(), "{sign}: {key}");
        }
    }
    #[test]
    fn reject_anchor_onset_rest_and_legacy_overlap() {
        let (mut doc, _) = source();
        let event = match &mut doc.frames[0].records[0] {
            Record::Event(e) => e,
            _ => unreachable!(),
        };
        event.pitch = Pitch::Rest;
        assert!(decode(&doc).is_err());
        let (mut doc, _) = source();
        let moved = doc.frames[0].records.pop().unwrap();
        doc.frames.push(crate::Frame {
            time: "1/2".parse().unwrap(),
            records: vec![moved],
            source: SourceRange::default(),
        });
        assert!(decode(&doc).is_err());
        let (mut doc, _) = source();
        doc.frames[0].records.push(Record::Mark(Mark {
            target: "#e".into(),
            kind: "articulation".into(),
            attributes: [("value".into(), Scalar::String("accent".into()))].into(),
            source: SourceRange::default(),
        }));
        assert!(decode(&doc).is_err());
    }
}
