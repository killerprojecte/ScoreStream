//! Explicit v1 articulation-only wrapper profile. This is authored interchange,
//! never an implicit upgrade of the four-sign renderer or sound policy.
use super::{
    read::{decimal, error, unsupported},
    xml::{Node, escape},
};
use crate::{Format, MappingEffect, MappingReport};
use scorestream_core::{
    Rational, articulation_model,
    notation::{EventId, Placement},
    score::*,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write,
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ArticulationProfile {
    #[default]
    LegacySimple,
    AuthoredV1,
    /// Preserve existing family payloads and explicit mixed wrapper topology.
    AuthoredMixedV1,
    /// Select legacy semantics only for canonical simple articulation hosts;
    /// otherwise retain the supported authored wrapper structure per note.
    /// This does not imply renderer or playback support for the resulting model.
    Automatic,
}
impl ArticulationProfile {
    pub(super) fn for_note(self, note: &Node) -> Self {
        if self != Self::Automatic {
            return self;
        }
        if super::automatic::is_canonical_simple(note) {
            Self::LegacySimple
        } else {
            Self::AuthoredMixedV1
        }
    }
    pub(crate) fn is_legacy(&self) -> bool {
        *self == Self::LegacySimple
    }
}
pub(super) const CHOICES: &[&str] = &[
    "accent",
    "strong-accent",
    "staccato",
    "tenuto",
    "detached-legato",
    "staccatissimo",
    "spiccato",
    "scoop",
    "plop",
    "doit",
    "falloff",
    "breath-mark",
    "caesura",
    "stress",
    "unstress",
    "soft-accent",
    "other-articulation",
];
const COMMON: &[&str] = &[
    "default-x",
    "default-y",
    "relative-x",
    "relative-y",
    "font-family",
    "font-style",
    "font-size",
    "font-weight",
    "color",
    "placement",
];
pub(super) fn attributes(name: &str) -> &'static [&'static str] {
    match name {
        "strong-accent" => &[
            "default-x",
            "default-y",
            "relative-x",
            "relative-y",
            "font-family",
            "font-style",
            "font-size",
            "font-weight",
            "color",
            "placement",
            "type",
        ],
        "scoop" | "plop" | "doit" | "falloff" => &[
            "default-x",
            "default-y",
            "relative-x",
            "relative-y",
            "font-family",
            "font-style",
            "font-size",
            "font-weight",
            "color",
            "placement",
            "line-shape",
            "line-type",
            "line-length",
            "dash-length",
            "space-length",
        ],
        "other-articulation" => &[
            "default-x",
            "default-y",
            "relative-x",
            "relative-y",
            "font-family",
            "font-style",
            "font-size",
            "font-weight",
            "color",
            "placement",
            "smufl",
        ],
        _ => COMMON,
    }
}
fn enum_attribute<T>(
    node: &Node,
    key: &str,
    parse: impl Fn(&str) -> Option<T>,
) -> Result<Option<T>, MappingReport> {
    node.attr(key)
        .map(|raw| {
            parse(&articulation_model::token(raw)).ok_or_else(|| {
                error(
                    node,
                    &format!("articulation.{key}"),
                    format!("Invalid {key} value"),
                )
            })
        })
        .transpose()
}
fn number(node: &Node, key: &str) -> Result<Option<Rational>, MappingReport> {
    node.attr(key)
        .map(|raw| {
            // XML decimal whitespace is only #x9/#xA/#xD/#x20; the generic decimal
            // helper uses Rust trim, so refuse any broader Unicode trimming first.
            if raw
                .chars()
                .any(|c| c.is_whitespace() && !matches!(c, ' ' | '\t' | '\r' | '\n'))
            {
                return Err(error(
                    node,
                    "articulation.decimal",
                    "Only XML whitespace is allowed around articulation decimals",
                ));
            }
            decimal(&node.scalar_attribute(key))
        })
        .transpose()
}
fn no_content(node: &Node) -> Result<(), MappingReport> {
    if !node
        .text
        .chars()
        .all(|c| matches!(c, ' ' | '\t' | '\r' | '\n'))
    {
        return Err(error(
            node,
            "articulation.content",
            "Empty articulation elements and containers permit only XML whitespace",
        ));
    }
    Ok(())
}
fn item(node: &Node) -> Result<AuthoredArticulation, MappingReport> {
    let line = || -> Result<ArticulationLine, MappingReport> {
        Ok(ArticulationLine {
            shape: enum_attribute(node, "line-shape", ArticulationLineShape::from_name)?,
            line_type: enum_attribute(node, "line-type", ArticulationLineType::from_name)?,
            length: enum_attribute(node, "line-length", ArticulationLineLength::from_name)?,
            dash_length: number(node, "dash-length")?,
            space_length: number(node, "space-length")?,
        })
    };
    if !["breath-mark", "caesura", "other-articulation"].contains(&node.name.as_str()) {
        no_content(node)?;
    }
    let sign = match node.name.as_str() {
        "accent" => ArticulationSign::Accent {},
        "strong-accent" => ArticulationSign::StrongAccent {
            orientation: enum_attribute(node, "type", ArticulationOrientation::from_name)?,
        },
        "staccato" => ArticulationSign::Staccato {},
        "tenuto" => ArticulationSign::Tenuto {},
        "detached-legato" => ArticulationSign::DetachedLegato {},
        "staccatissimo" => ArticulationSign::Staccatissimo {},
        "spiccato" => ArticulationSign::Spiccato {},
        "scoop" => ArticulationSign::Scoop { line: line()? },
        "plop" => ArticulationSign::Plop { line: line()? },
        "doit" => ArticulationSign::Doit { line: line()? },
        "falloff" => ArticulationSign::Falloff { line: line()? },
        "breath-mark" => ArticulationSign::BreathMark {
            shape: if node.text.is_empty() {
                None
            } else {
                Some(BreathMarkShape::from_name(&node.text).ok_or_else(|| error(node,"articulation.breath_shape","Breath shape must be exact empty/comma/tick/upbow/salzedo text, without trimming"))?)
            },
        },
        "caesura" => ArticulationSign::Caesura {
            shape: if node.text.is_empty() {
                None
            } else {
                Some(CaesuraShape::from_name(&node.text).ok_or_else(|| error(node,"articulation.caesura_shape","Caesura shape must be exact empty/normal/thick/short/curved/single text, without trimming"))?)
            },
        },
        "stress" => ArticulationSign::Stress {},
        "unstress" => ArticulationSign::Unstress {},
        "soft-accent" => ArticulationSign::SoftAccent {},
        "other-articulation" => ArticulationSign::OtherArticulation {
            text: node.text.clone(),
            smufl: node.attr("smufl").map(articulation_model::token),
        },
        _ => {
            return Err(unsupported(
                node,
                "articulation.choice",
                "Unknown authored articulation choice",
            ));
        }
    };
    let font_size = if let Some(raw) = node.attr("font-size") {
        if let Some(size) = ArticulationCssFontSize::from_name(&articulation_model::token(raw)) {
            Some(ArticulationFontSize::Css(size))
        } else {
            Some(ArticulationFontSize::Points(
                number(node, "font-size")?.expect("attribute exists"),
            ))
        }
    } else {
        None
    };
    Ok(AuthoredArticulation {
        sign,
        placement: enum_attribute(node, "placement", |s| match s {
            "above" => Some(Placement::Above),
            "below" => Some(Placement::Below),
            _ => None,
        })?,
        print_style: ArticulationPrintStyle {
            default_x: number(node, "default-x")?,
            default_y: number(node, "default-y")?,
            relative_x: number(node, "relative-x")?,
            relative_y: number(node, "relative-y")?,
            font_family: node.attr("font-family").map(articulation_model::token),
            font_style: enum_attribute(node, "font-style", ArticulationFontStyle::from_name)?,
            font_size,
            font_weight: enum_attribute(node, "font-weight", ArticulationFontWeight::from_name)?,
            color: node.attr("color").map(articulation_model::token),
        },
    })
}
pub(super) fn read(
    note: &Node,
    event: &EventId,
    mixed: bool,
) -> Result<Option<ArticulationEvent>, MappingReport> {
    let wrappers: Vec<_> = note.children_named("notations").collect();
    let any = wrappers
        .iter()
        .any(|w| w.children.iter().any(|g| g.name == "articulations"));
    if !any {
        // Broader schema admission must never let the old notation readers
        // discard newly admitted wrapper identity or print visibility.
        if let Some(wrapper) = wrappers.iter().find(|w| !w.attrs.is_empty()) {
            return Err(unsupported(
                wrapper,
                "articulation.wrapper_scope",
                "Wrapper attributes without an articulation group require a different authored notation profile",
            ));
        }
        return Ok(None);
    }
    if note.child("pitch").is_none() || note.child("grace").is_some() {
        return Err(unsupported(
            note,
            "articulation.event",
            "Authored v1 requires a measured pitched event; rest, unpitched and grace hosts are not inferred",
        ));
    }
    if !mixed
        && (note.child("tie").is_some()
            || wrappers
                .iter()
                .any(|w| w.children.iter().any(|g| g.name != "articulations")))
    {
        return Err(unsupported(
            note,
            "articulation.mixed_notations",
            "Authored v1 preserves articulation-only wrappers. Other note notations on the same event require an ordered mixed-family model; no wrappers are flattened",
        ));
    }
    let mut value = ArticulationEvent {
        event: event.clone(),
        notations: vec![],
    };
    for wrapper in wrappers {
        no_content(wrapper)?;
        let mut notation = ArticulationNotations {
            source_id: wrapper.attr("id").map(str::to_owned),
            print_object: enum_attribute(wrapper, "print-object", |s| match s {
                "yes" => Some(true),
                "no" => Some(false),
                _ => None,
            })?,
            groups: vec![],
        };
        for group in wrapper.children_named("articulations") {
            no_content(group)?;
            notation.groups.push(ArticulationGroup {
                source_id: group.attr("id").map(str::to_owned),
                items: group.children.iter().map(item).collect::<Result<_, _>>()?,
            });
        }
        value.notations.push(notation);
    }
    let diagnostics = articulation_model::validate_event(&value);
    if let Some(d) = diagnostics.first() {
        return Err(error(note, &d.code, &d.message));
    }
    Ok(Some(value))
}

pub(super) struct Export<'a> {
    by_event: BTreeMap<&'a EventId, &'a ArticulationEvent>,
}
impl<'a> Export<'a> {
    pub(super) fn new(
        score: &'a ScoreDocument,
        ids: &mut BTreeSet<String>,
        report: &mut MappingReport,
    ) -> Self {
        let mut by_event = BTreeMap::new();
        if let Some(authored) = &score.authored_articulations {
            for event in &authored.events {
                by_event.insert(&event.event, event);
                for wrapper in &event.notations {
                    for id in wrapper
                        .source_id
                        .iter()
                        .chain(wrapper.groups.iter().filter_map(|g| g.source_id.as_ref()))
                    {
                        if !super::write::xml_id(id) || !ids.insert(id.clone()) {
                            report.push(
                                Format::MusicXml,
                                MappingEffect::Invalid,
                                "articulation.source_id",
                                &event.event.0,
                                "Invalid or duplicate authored wrapper/group XML identity",
                            );
                        }
                    }
                    for item in wrapper.groups.iter().flat_map(|g| &g.items) {
                        let s = &item.print_style;
                        let mut values = vec![
                            s.default_x.as_ref(),
                            s.default_y.as_ref(),
                            s.relative_x.as_ref(),
                            s.relative_y.as_ref(),
                        ];
                        if let Some(ArticulationFontSize::Points(n)) = &s.font_size {
                            values.push(Some(n));
                        }
                        if let Some(line) = item.sign.line() {
                            values.extend([line.dash_length.as_ref(), line.space_length.as_ref()]);
                        }
                        if values
                            .into_iter()
                            .flatten()
                            .any(|n| super::write::decimal(n).is_none_or(|text| text.len() > 64))
                        {
                            report.push(Format::MusicXml,MappingEffect::Unsupported,"articulation.decimal",&event.event.0,"Exact articulation values require finite MusicXML decimals within the existing 32-place and 64-character bounds; no rounding is performed");
                        }
                    }
                }
            }
        }
        if let Some(authored) = &score.authored_articulations {
            let emitted: Vec<_> = score
                .parts
                .iter()
                .flat_map(|p| {
                    p.measures.iter().flat_map(move |m| {
                        p.voices
                            .iter()
                            .flat_map(move |v| super::write::ordered_events(m, &v.id))
                    })
                })
                .filter(|e| by_event.contains_key(&e.id))
                .map(|e| &e.id)
                .collect();
            let associations: Vec<_> = authored.events.iter().map(|e| &e.event).collect();
            if associations != emitted {
                report.push(Format::MusicXml, MappingEffect::Normalized, "articulation.event_order", "/authored_articulations/events",
                    "Event association order follows MusicXML's existing voice/note serialization. Each event's authored wrapper, group and item order, identities and attributes remain unchanged.");
            }
        }
        if let Some(order) = &score.notation_order {
            let hosts: BTreeSet<_> = order.events.iter().map(|e| &e.event).collect();
            let emitted: Vec<_> = score
                .parts
                .iter()
                .flat_map(|p| {
                    p.measures.iter().flat_map(move |m| {
                        p.voices
                            .iter()
                            .flat_map(move |v| super::write::ordered_events(m, &v.id))
                    })
                })
                .filter(|e| hosts.contains(&e.id))
                .map(|e| &e.id)
                .collect();
            if emitted != order.events.iter().map(|e| &e.event).collect::<Vec<_>>() {
                report.push(Format::MusicXml, MappingEffect::Normalized, "notations.event_order", "/notation_order/events",
                    "Mixed host association order follows XML voice/note serialization; wrapper child order and semantic attachments remain exact.");
            }
        }
        Self { by_event }
    }
    pub(super) fn write(
        &self,
        event: &EventId,
        out: &mut String,
        order: Option<&NotationEventOrder>,
        mut write_other: impl FnMut(&NotationChild, &mut String),
    ) {
        let Some(authored) = self.by_event.get(event) else {
            return;
        };
        for (index, wrapper) in authored.notations.iter().enumerate() {
            out.push_str("        <notations");
            attr(out, "id", wrapper.source_id.as_deref());
            attr(
                out,
                "print-object",
                wrapper.print_object.map(|v| if v { "yes" } else { "no" }),
            );
            out.push('>');
            if let Some(order) = order {
                for child in &order.wrappers[index] {
                    if let NotationChild::Articulations { group_index } = child {
                        write_group(&wrapper.groups[*group_index], out);
                    } else {
                        write_other(child, out);
                    }
                }
            } else {
                for group in &wrapper.groups {
                    write_group(group, out);
                }
            }
            out.push_str("</notations>\n");
        }
    }
}
fn write_group(group: &ArticulationGroup, out: &mut String) {
    out.push_str("<articulations");
    attr(out, "id", group.source_id.as_deref());
    out.push('>');
    for value in &group.items {
        write_item(value, out);
    }
    out.push_str("</articulations>");
}
fn attr(out: &mut String, key: &str, value: Option<&str>) {
    if let Some(value) = value {
        write!(out, " {key}=\"{}\"", escape(value)).unwrap();
    }
}
fn number_attr(out: &mut String, key: &str, value: Option<&Rational>) {
    if let Some(value) = value {
        attr(
            out,
            key,
            Some(&super::write::decimal(value).expect("validated finite exact decimal")),
        );
    }
}
fn write_item(item: &AuthoredArticulation, out: &mut String) {
    let name = item.sign.name();
    write!(out, "<{name}").unwrap();
    attr(
        out,
        "placement",
        item.placement.map(|p| match p {
            Placement::Above => "above",
            Placement::Below => "below",
        }),
    );
    let s = &item.print_style;
    for (key, value) in [
        ("default-x", &s.default_x),
        ("default-y", &s.default_y),
        ("relative-x", &s.relative_x),
        ("relative-y", &s.relative_y),
    ] {
        number_attr(out, key, value.as_ref());
    }
    attr(out, "font-family", s.font_family.as_deref());
    attr(out, "font-style", s.font_style.map(|v| v.name()));
    attr(out, "font-weight", s.font_weight.map(|v| v.name()));
    attr(out, "color", s.color.as_deref());
    if let Some(size) = &s.font_size {
        match size {
            ArticulationFontSize::Points(v) => number_attr(out, "font-size", Some(v)),
            ArticulationFontSize::Css(v) => attr(out, "font-size", Some(v.name())),
        }
    }
    if let ArticulationSign::StrongAccent { orientation } = &item.sign {
        attr(out, "type", orientation.map(|v| v.name()));
    }
    if let Some(line) = item.sign.line() {
        attr(out, "line-shape", line.shape.map(|v| v.name()));
        attr(out, "line-type", line.line_type.map(|v| v.name()));
        attr(out, "line-length", line.length.map(|v| v.name()));
        number_attr(out, "dash-length", line.dash_length.as_ref());
        number_attr(out, "space-length", line.space_length.as_ref());
    }
    let text = match &item.sign {
        ArticulationSign::BreathMark { shape } => shape.map(|s| s.name()).unwrap_or(""),
        ArticulationSign::Caesura { shape } => shape.map(|s| s.name()).unwrap_or(""),
        ArticulationSign::OtherArticulation { text, smufl } => {
            attr(out, "smufl", smufl.as_deref());
            text
        }
        _ => "",
    };
    if text.is_empty() {
        out.push_str("/>");
    } else {
        write!(out, ">{}</{name}>", escape(text)).unwrap();
    }
}

/// XML has no stable voice/chord/relation IDs for these generated internal
/// objects. In the authored profile, never let such an implementation name
/// steal a retained wrapper/group source ID. Authored XML IDs stay untouched.
pub(super) fn protect_generated_identities(
    score: &mut ScoreDocument,
    used: &mut BTreeSet<String>,
    report: &mut MappingReport,
) {
    let Some(authored) = &score.authored_articulations else {
        return;
    };
    let reserved: BTreeSet<_> = authored
        .events
        .iter()
        .flat_map(|e| &e.notations)
        .flat_map(|w| {
            w.source_id
                .iter()
                .chain(w.groups.iter().filter_map(|g| g.source_id.as_ref()))
        })
        .cloned()
        .collect();
    used.extend(
        score
            .parts
            .iter()
            .flat_map(|p| p.voices.iter().map(|v| v.id.0.clone())),
    );
    used.extend(
        score
            .parts
            .iter()
            .flat_map(|p| &p.measures)
            .flat_map(|m| &m.events)
            .filter_map(|e| e.chord.clone()),
    );
    used.extend(score.ties.iter().map(|t| t.id.0.clone()));
    used.extend(score.slurs.iter().map(|s| s.id.0.clone()));
    let mut mapping = BTreeMap::new();
    for old in &reserved {
        let mut new = format!("{old}_");
        while !used.insert(new.clone()) {
            new.push('_');
        }
        mapping.insert(old.clone(), new);
    }
    let mut changed = BTreeSet::new();
    for part in &mut score.parts {
        for voice in &mut part.voices {
            if let Some(new) = mapping.get(&voice.id.0) {
                changed.insert(voice.id.0.clone());
                voice.id.0 = new.clone();
            }
        }
        for event in part.measures.iter_mut().flat_map(|m| &mut m.events) {
            if let Some(new) = mapping.get(&event.voice.0) {
                event.voice.0 = new.clone();
            }
            for chord in event.chord.iter_mut() {
                if let Some(new) = mapping.get(chord) {
                    changed.insert(chord.clone());
                    *chord = new.clone();
                }
            }
        }
    }
    for id in score
        .ties
        .iter_mut()
        .map(|t| &mut t.id.0)
        .chain(score.slurs.iter_mut().map(|s| &mut s.id.0))
    {
        if let Some(new) = mapping.get(id) {
            changed.insert(id.clone());
            *id = new.clone();
        }
    }
    if let Some(order) = &mut score.notation_order {
        for child in order
            .events
            .iter_mut()
            .flat_map(|e| &mut e.wrappers)
            .flatten()
        {
            let NotationChild::Slur { relation, .. } = child else {
                continue;
            };
            if let Some(new) = mapping.get(&relation.0) {
                relation.0 = new.clone();
            }
        }
    }
    for id in changed {
        report.push(Format::MusicXml,MappingEffect::Normalized,"articulation.generated_identity",&id,
            format!("Generated internal identity normalized to {} to preserve the authored wrapper/group source ID",mapping[&id]));
    }
}
