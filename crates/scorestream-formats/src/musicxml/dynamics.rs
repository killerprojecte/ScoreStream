//! Numeric MusicXML sound and printed dynamics: no label, text or glyph inference.
use super::{
    read::{decimal, error, unsupported},
    tempo::locations,
    xml::{Node, escape},
};
use crate::{Format, MappingEffect, MappingReport};
use scorestream_core::{
    Rational,
    notation::{PartId, Placement},
    score::*,
};
use std::{collections::BTreeMap, fmt::Write};
type Result<T> = std::result::Result<T, MappingReport>;
#[allow(clippy::too_many_arguments)]
pub(super) fn read(
    node: &Node,
    part: &ScorePart,
    start: &Rational,
    cursor: &Rational,
    divisions: Option<&Rational>,
    marks: &mut Vec<ScoreDynamic>,
    sounds: &mut Vec<ScoreSoundDynamics>,
) -> Result<()> {
    let (visual, sounding) = locations(node, start, cursor, divisions)?;
    let staff = node
        .child("staff")
        .map(|n| {
            n.text
                .trim()
                .parse::<u32>()
                .ok()
                .filter(|s| *s > 0 && *s <= part.staves)
                .ok_or_else(|| error(n, "staff.number", "Direction staff is outside its part"))
        })
        .transpose()?;
    let scope = ScorePartScope {
        part: part.id.clone(),
        staff,
    };
    if let Some(d) = node
        .child("direction-type")
        .and_then(|t| t.child("dynamics"))
    {
        let symbol = d.children.first().ok_or_else(|| {
            unsupported(
                d,
                "dynamics.empty",
                "An explicit dynamic symbol is required",
            )
        })?;
        let sign = crate::dynamics::symbol(&symbol.name).ok_or_else(|| {
            unsupported(
                symbol,
                "dynamics.symbol",
                "Unmapped dynamic symbol; spelling is not substituted",
            )
        })?;
        let placement = match node.attr("placement") {
            None => None,
            Some("above") => Some(Placement::Above),
            Some("below") => Some(Placement::Below),
            _ => return Err(error(node, "dynamics.placement", "Expected above or below")),
        };
        marks.push(ScoreDynamic {
            at: visual,
            scope: scope.clone(),
            sign,
            placement,
            sound: false,
        });
    }
    let sound = if node.name == "sound" {
        Some(node)
    } else {
        node.child("sound")
    };
    if let Some(sound) = sound
        && let Some(raw) = sound.attr("dynamics")
    {
        let mut number = sound.clone();
        number.text = raw.into();
        let percent = decimal(&number)?;
        if percent.is_negative() {
            return Err(error(
                sound,
                "sound_dynamics.percent",
                "MusicXML numeric dynamics must be nonnegative",
            ));
        }
        sounds.push(ScoreSoundDynamics {
            at: sounding,
            scope,
            percent,
        });
    }
    Ok(())
}
/// Preserve absence rather than inventing an authored assignment. A caller can
/// select a renderer policy separately; this adapter does not change old SFS80.
pub(super) fn report_unassigned_initial(score: &ScoreDocument, r: &mut MappingReport) {
    if score.sound_dynamics.is_empty() && score.dynamics.is_empty() {
        return;
    }
    for part in &score.parts {
        if !(1..=part.staves).all(|staff| {
            score.sound_dynamics.iter().any(|d| {
                d.scope.part == part.id
                    && d.at == Rational::zero()
                    && (d.scope.staff.is_none() || d.scope.staff == Some(staff))
            })
        }) {
            r.push(Format::MusicXml, MappingEffect::Normalized,
                "sound_dynamics.initial_unassigned", &part.id.0,
                "No numeric initial sound is invented for an unassigned staff. The MusicXML MIDI tutorial suggests default velocity90; the existing SFS renderer retains80 until an explicit control applies. Absence is preserved, not playback-default equivalence.");
        }
    }
}
#[derive(Default)]
struct Points<'a> {
    marks: Vec<&'a ScoreDynamic>,
    sounds: Vec<&'a ScoreSoundDynamics>,
}
pub(super) struct Export<'a> {
    by_part: BTreeMap<&'a PartId, BTreeMap<Rational, Points<'a>>>,
}
impl<'a> Export<'a> {
    pub(super) fn new(score: &'a ScoreDocument, r: &mut MappingReport) -> Self {
        report_unassigned_initial(score, r);
        let mut by_part: BTreeMap<_, BTreeMap<Rational, Points<'a>>> = BTreeMap::new();
        for m in &score.dynamics {
            by_part
                .entry(&m.scope.part)
                .or_default()
                .entry(m.at.clone())
                .or_default()
                .marks
                .push(m);
            if m.sound {
                r.push(Format::MusicXml,MappingEffect::Unsupported,"dynamics.symbolic_sound",m.at.to_string(),"A performer-policy symbolic dynamic is not equivalent to explicit MusicXML numeric sound; export requires a display-only mark");
            }
        }
        for d in &score.sound_dynamics {
            by_part
                .entry(&d.scope.part)
                .or_default()
                .entry(d.at.clone())
                .or_default()
                .sounds
                .push(d);
            if super::write::decimal(&d.percent).is_none_or(|v| v.len() > 64) {
                r.push(Format::MusicXml,MappingEffect::Unsupported,"sound_dynamics.decimal",d.at.to_string(),"Exact percentage must have a finite decimal within the supported precision; it is not rounded");
            }
        }
        for points in by_part.values().flat_map(|p| p.values()) {
            for d in &points.sounds {
                if d.scope.staff.is_some() && !points.marks.iter().any(|m| m.scope == d.scope) {
                    r.push(Format::MusicXml,MappingEffect::Unsupported,"sound_dynamics.staff_export",d.at.to_string(),"Staff-scoped numeric export needs a display dynamic at exactly the same position; no invisible or fake direction is invented");
                }
            }
        }
        Self { by_part }
    }
    pub(super) fn offsets(&self, part: &ScorePart, m: &ScoreMeasure) -> Vec<Rational> {
        self.by_part
            .get(&part.id)
            .into_iter()
            .flat_map(|p| {
                p.range(m.start.clone()..&m.start + &m.length)
                    .map(|(at, _)| at - &m.start)
            })
            .collect()
    }
    pub(super) fn write_measure(
        &self,
        part: &ScorePart,
        m: &ScoreMeasure,
        div: u64,
        out: &mut String,
    ) {
        let Some(points) = self.by_part.get(&part.id) else {
            return;
        };
        for (at, values) in points.range(m.start.clone()..&m.start + &m.length) {
            let offset = (&(at - &m.start) * &Rational::from_big_integer(div.into())).to_string();
            let mut staff_written = std::collections::BTreeSet::new();
            for mark in &values.marks {
                let placement = mark
                    .placement
                    .map(|p| {
                        format!(
                            " placement=\"{}\"",
                            match p {
                                Placement::Above => "above",
                                Placement::Below => "below",
                            }
                        )
                    })
                    .unwrap_or_default();
                let symbol = crate::dynamics::symbol_text(mark.sign);
                write!(out,"      <direction{placement}><direction-type><dynamics><{symbol}/></dynamics></direction-type><offset sound=\"no\">{offset}</offset>").unwrap();
                if let Some(staff) = mark.scope.staff {
                    write!(out, "<staff>{staff}</staff>").unwrap();
                    if staff_written.insert(staff)
                        && let Some(d) = values.sounds.iter().find(|d| d.scope == mark.scope)
                    {
                        write!(
                            out,
                            "<sound dynamics=\"{}\"><offset>{offset}</offset></sound>",
                            escape(&super::write::decimal(&d.percent).unwrap())
                        )
                        .unwrap();
                    }
                }
                out.push_str("</direction>\n");
            }
            for sound in values.sounds.iter().filter(|d| d.scope.staff.is_none()) {
                writeln!(
                    out,
                    "      <sound dynamics=\"{}\"><offset>{offset}</offset></sound>",
                    super::write::decimal(&sound.percent).unwrap()
                )
                .unwrap();
            }
        }
    }
}
