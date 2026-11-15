//! MusicXML 4.0 authored articulation family, version 1. Typed data only: no
//! foreign markup, arbitrary maps, inferred defaults, or performance recipes.
use crate::{
    notation::{EventId, Placement},
    Rational,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArticulationVersion {
    #[serde(rename = "1")]
    V1,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoredArticulations {
    pub version: ArticulationVersion,
    pub events: Vec<ArticulationEvent>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArticulationEvent {
    pub event: EventId,
    /// Ordered wrappers. This checkpoint admits only articulation-only wrappers;
    /// mixed-family wrapper topology is refused rather than flattened.
    pub notations: Vec<ArticulationNotations>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArticulationNotations {
    pub source_id: Option<String>,
    pub print_object: Option<bool>,
    /// Ordered groups, including empty groups. No source IDs are synthesized.
    pub groups: Vec<ArticulationGroup>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArticulationGroup {
    pub source_id: Option<String>,
    /// Duplicates and stacking order are authored data, never a set.
    pub items: Vec<AuthoredArticulation>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AuthoredArticulation {
    pub sign: ArticulationSign,
    pub placement: Option<Placement>,
    pub print_style: ArticulationPrintStyle,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArticulationPrintStyle {
    pub default_x: Option<Rational>,
    pub default_y: Option<Rational>,
    pub relative_x: Option<Rational>,
    pub relative_y: Option<Rational>,
    pub font_family: Option<String>,
    pub font_style: Option<ArticulationFontStyle>,
    pub font_size: Option<ArticulationFontSize>,
    pub font_weight: Option<ArticulationFontWeight>,
    /// Authored RGB versus ARGB spelling is retained, including explicit alpha.
    pub color: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum ArticulationSign {
    Accent {},
    StrongAccent {
        orientation: Option<ArticulationOrientation>,
    },
    Staccato {},
    Tenuto {},
    DetachedLegato {},
    Staccatissimo {},
    Spiccato {},
    Scoop {
        line: ArticulationLine,
    },
    Plop {
        line: ArticulationLine,
    },
    Doit {
        line: ArticulationLine,
    },
    Falloff {
        line: ArticulationLine,
    },
    BreathMark {
        shape: Option<BreathMarkShape>,
    },
    Caesura {
        shape: Option<CaesuraShape>,
    },
    Stress {},
    Unstress {},
    SoftAccent {},
    /// Passive authored extension. Neither text nor a glyph name grants a
    /// performance interpretation, markup execution or rendering acceptance.
    OtherArticulation {
        text: String,
        smufl: Option<String>,
    },
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ArticulationLine {
    pub shape: Option<ArticulationLineShape>,
    pub line_type: Option<ArticulationLineType>,
    pub length: Option<ArticulationLineLength>,
    pub dash_length: Option<Rational>,
    pub space_length: Option<Rational>,
}
macro_rules! vocabulary {
    ($name:ident { $($variant:ident => $token:literal),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
        pub enum $name { $(#[serde(rename = $token)] $variant),+ }
        impl $name {
            pub fn from_name(value: &str) -> Option<Self> { match value { $($token => Some(Self::$variant),)+ _ => None } }
            pub fn name(self) -> &'static str { match self { $(Self::$variant => $token),+ } }
        }
    }
}
vocabulary!(ArticulationOrientation { Up => "up", Down => "down" });
vocabulary!(ArticulationFontStyle { Normal => "normal", Italic => "italic" });
vocabulary!(ArticulationFontWeight { Normal => "normal", Bold => "bold" });
vocabulary!(ArticulationCssFontSize { XxSmall => "xx-small", XSmall => "x-small", Small => "small", Medium => "medium", Large => "large", XLarge => "x-large", XxLarge => "xx-large" });
vocabulary!(ArticulationLineShape { Straight => "straight", Curved => "curved" });
vocabulary!(ArticulationLineType { Solid => "solid", Dashed => "dashed", Dotted => "dotted", Wavy => "wavy" });
vocabulary!(ArticulationLineLength { Short => "short", Medium => "medium", Long => "long" });
vocabulary!(BreathMarkShape { Comma => "comma", Tick => "tick", Upbow => "upbow", Salzedo => "salzedo" });
vocabulary!(CaesuraShape { Normal => "normal", Thick => "thick", Short => "short", Curved => "curved", Single => "single" });
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub enum ArticulationFontSize {
    Points(Rational),
    Css(ArticulationCssFontSize),
}
impl ArticulationSign {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Accent {} => "accent",
            Self::StrongAccent { .. } => "strong-accent",
            Self::Staccato {} => "staccato",
            Self::Tenuto {} => "tenuto",
            Self::DetachedLegato {} => "detached-legato",
            Self::Staccatissimo {} => "staccatissimo",
            Self::Spiccato {} => "spiccato",
            Self::Scoop { .. } => "scoop",
            Self::Plop { .. } => "plop",
            Self::Doit { .. } => "doit",
            Self::Falloff { .. } => "falloff",
            Self::BreathMark { .. } => "breath-mark",
            Self::Caesura { .. } => "caesura",
            Self::Stress {} => "stress",
            Self::Unstress {} => "unstress",
            Self::SoftAccent {} => "soft-accent",
            Self::OtherArticulation { .. } => "other-articulation",
        }
    }
    pub fn line(&self) -> Option<&ArticulationLine> {
        match self {
            Self::Scoop { line }
            | Self::Plop { line }
            | Self::Doit { line }
            | Self::Falloff { line } => Some(line),
            _ => None,
        }
    }
}

fn diagnostic(code: &str, message: &str) -> crate::Diagnostic {
    crate::Diagnostic::error(code, message, crate::SourceRange::default())
}
fn xml_char(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\r' | '\u{20}'..='\u{d7ff}' | '\u{e000}'..='\u{fffd}' | '\u{10000}'..='\u{10ffff}')
}
/// The existing interchange profile uses ASCII XML IDs, not the full Unicode
/// NCName space. Kept separate from SFS object IDs (these are source identities).
pub fn source_id(value: &str) -> bool {
    value
        .bytes()
        .next()
        .is_some_and(|b| b.is_ascii_alphabetic() || b == b'_')
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
}
/// XML Schema token whitespace normalization. Unlike Rust trim/split_whitespace,
/// Unicode spaces such as NBSP remain significant.
pub fn token(value: &str) -> String {
    value
        .split([' ', '\t', '\n', '\r'])
        .filter(|v| !v.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}
/// Validate one ordered authored event's structure and display fields. Anchor
/// resolution and whole-document identity are checked by the score/native caller.
pub fn validate_event(event: &ArticulationEvent) -> Vec<crate::Diagnostic> {
    let mut out = Vec::new();
    if event.notations.is_empty() || event.notations.iter().all(|n| n.groups.is_empty()) {
        out.push(diagnostic("articulation.structure", "Authored articulation event requires at least one group; empty groups and additional empty wrappers are preserved"));
    }
    for wrapper in &event.notations {
        for id in wrapper
            .source_id
            .iter()
            .chain(wrapper.groups.iter().filter_map(|g| g.source_id.as_ref()))
        {
            if !source_id(id) {
                out.push(diagnostic(
                    "articulation.source_id",
                    "Source identity is outside the supported ASCII XML ID profile",
                ));
            }
        }
        for item in wrapper.groups.iter().flat_map(|g| &g.items) {
            let style = &item.print_style;
            if let Some(family) = &style.font_family {
                if family.is_empty()
                    || token(family) != *family
                    || family.split(',').any(str::is_empty)
                    || family.chars().any(|c| !xml_char(c))
                {
                    out.push(diagnostic("articulation.font_family", "Font family must be normalized XML token text with nonempty comma-separated entries"));
                }
            }
            if let Some(color) = &style.color {
                if !matches!(color.len(), 7 | 9)
                    || !color.starts_with('#')
                    || !color[1..]
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'A'..=b'F').contains(&b))
                {
                    out.push(diagnostic(
                        "articulation.color",
                        "Color must preserve an uppercase #RRGGBB or #AARRGGBB value",
                    ));
                }
            }
            if let ArticulationSign::OtherArticulation { text, smufl } = &item.sign {
                if text.chars().any(|c| !xml_char(c)) {
                    out.push(diagnostic(
                        "articulation.text",
                        "Custom articulation text contains an XML-forbidden character",
                    ));
                }
                if let Some(name) = smufl {
                    if name.is_empty()
                        || !name
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b"_.:-".contains(&b))
                    {
                        out.push(diagnostic("articulation.smufl", "SMuFL reference is outside this profile's ASCII NMTOKEN subset; glyph existence or rendering is not implied"));
                    }
                }
            }
        }
    }
    out
}
/// Validate authored articulation anchors, ordered structure, legacy overlap and
/// the bounded family-only wrapper contract. Caller also validates other domains.
pub fn validate_score(score: &crate::score::ScoreDocument) -> Vec<crate::Diagnostic> {
    use std::collections::{BTreeMap, BTreeSet};
    let Some(authored) = &score.authored_articulations else {
        return vec![];
    };
    let mut out = Vec::new();
    if authored.events.is_empty() {
        out.push(diagnostic(
            "articulation.empty_requirement",
            "Authored articulation v1 requires at least one event",
        ));
    }
    let mut hosts = BTreeMap::new();
    for event in score
        .parts
        .iter()
        .flat_map(|p| &p.measures)
        .flat_map(|m| &m.events)
    {
        let value =
            matches!(event.pitch, crate::Pitch::Pitched { .. }) && event.duration.is_positive();
        hosts
            .entry(&event.id)
            .and_modify(|v| *v = false)
            .or_insert(value);
    }
    let mut seen = BTreeSet::new();
    let mut source_ids = BTreeSet::new();
    // Source identities cannot alias an existing object when later transported
    // into native SFS. Reserve before any adapter-generated ID normalization.
    for part in &score.parts {
        source_ids.insert(&part.id.0);
        for voice in &part.voices {
            source_ids.insert(&voice.id.0);
        }
        if let Some(metadata) = &part.instrument_metadata {
            source_ids.insert(&metadata.source_id);
        }
        for measure in &part.measures {
            source_ids.insert(&measure.id);
            for event in &measure.events {
                source_ids.insert(&event.id.0);
                if let Some(chord) = &event.chord {
                    source_ids.insert(chord);
                }
            }
        }
    }
    for grace in &score.graces {
        source_ids.insert(&grace.id.0);
    }
    for slur in &score.slurs {
        source_ids.insert(&slur.id.0);
    }
    for tie in &score.ties {
        source_ids.insert(&tie.id.0);
    }
    for id in score.barlines.iter().filter_map(|b| b.id.as_ref()) {
        source_ids.insert(id);
    }
    for authored_event in &authored.events {
        out.extend(validate_event(authored_event));
        let event = &authored_event.event;
        if !seen.insert(event) || hosts.get(event) != Some(&true) {
            out.push(diagnostic("articulation.event", "Authored articulation host must be one unique measured pitched event, not rest, spacer, grace or missing identity"));
        }
        if score.articulations.iter().any(|a| &a.event == event) {
            out.push(diagnostic(
                "articulation.legacy_conflict",
                "Legacy simple marks and authored articulation v1 cannot overlap on the same event",
            ));
        }
        if score.ties.iter().any(|t| t.events.contains(event))
            || score
                .slurs
                .iter()
                .any(|s| &s.start.event == event || &s.end.event == event)
            || score.fermatas.iter().any(|f| &f.event == event)
        {
            out.push(diagnostic("articulation.mixed_notations", "Authored v1 preserves articulation-only wrappers; tied/slur/fermata on the same event need a future ordered mixed-notation model"));
        }
        for id in authored_event.notations.iter().flat_map(|w| {
            w.source_id
                .iter()
                .chain(w.groups.iter().filter_map(|g| g.source_id.as_ref()))
        }) {
            if !source_ids.insert(id) {
                out.push(diagnostic("articulation.source_id", "Authored wrapper/group source IDs must be unique and disjoint from existing object/source identities"));
            }
        }
    }
    out
}
