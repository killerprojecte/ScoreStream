//! A deliberately narrow, loss-aware MusicXML tempo profile.
//! W3C MusicXML 4.0 sound/offset/metronome semantics: no tempo guessed from prose.
use super::{
    read::{bool_attr, child, decimal, error, unsupported},
    xml::Node,
};
use crate::{MappingEffect, MappingReport};
use scorestream_core::{
    Rational,
    notation::{Beat, BeatUnit, Tempo},
    score::*,
};

type Result<T> = std::result::Result<T, MappingReport>;
fn offset(node: &Node, divisions: Option<&Rational>) -> Result<Rational> {
    let value = decimal(node)?;
    if value == Rational::zero() {
        return Ok(value);
    }
    let divisions = divisions.ok_or_else(|| {
        error(
            node,
            "divisions.missing",
            "A nonzero tempo offset requires explicit divisions",
        )
    })?;
    Ok(&value / divisions)
}
fn positive(node: &Node, value: &str) -> Result<Rational> {
    let mut number = node.clone();
    number.text = value.into();
    let rate = decimal(&number)?;
    if !rate.is_positive() {
        return Err(unsupported(
            node,
            "tempo.rate",
            "A positive tempo is required; zero asks for an interactive tempo choice and is not guessed",
        ));
    }
    Ok(rate)
}
/// Exact direction display/sound positions. Nested sound offset overrides direction.
pub(super) fn locations(
    node: &Node,
    start: &Rational,
    cursor: &Rational,
    divisions: Option<&Rational>,
) -> Result<(Rational, Rational)> {
    let current = start + cursor;
    let mut visual = current.clone();
    let mut sounding = current.clone();
    if node.name == "direction"
        && let Some(o) = node.child("offset")
    {
        visual = &current + &offset(o, divisions)?;
        if bool_attr(o, "sound")? {
            sounding = visual.clone();
        }
    }
    let sound = if node.name == "sound" {
        Some(node)
    } else {
        node.child("sound")
    };
    if let Some(o) = sound.and_then(|s| s.child("offset")) {
        if o.attr("sound").is_some() {
            return Err(unsupported(
                o,
                "sound.offset_attribute",
                "The sound flag on an offset nested in sound is outside this profile",
            ));
        }
        sounding = &current + &offset(o, divisions)?;
    }
    Ok((visual, sounding))
}
#[allow(clippy::too_many_arguments)]
pub(super) fn read(
    node: &Node,
    part: &ScorePart,
    measure: &str,
    start: &Rational,
    cursor: &Rational,
    divisions: Option<&Rational>,
    tempos: &mut Vec<ScoreTempo>,
    metronomes: &mut Vec<ScoreMetronome>,
    report: &mut MappingReport,
) -> Result<()> {
    let (visual_at, sounding_at) = locations(node, start, cursor, divisions)?;
    if let Some(m) = node.children.iter().filter(|d|d.name=="direction-type").find_map(|d|d.child("metronome")) {
        let presentation=super::metronome_display::read(node,measure)?;
        if presentation.is_some() {m.error(report,MappingEffect::Preserved,"metronome_display.authored_only","Ordered metronome presentation retained; additional text/parentheses/geometry are not realized by preview, and numeric sound remains independent");}
        let unit = match child(m, "beat-unit")?.text.trim() {
            "whole" => BeatUnit::Whole,
            "half" => BeatUnit::Half,
            "quarter" => BeatUnit::Quarter,
            "eighth" => BeatUnit::Eighth,
            "16th" => BeatUnit::Sixteenth,
            _ => {
                return Err(unsupported(
                    m,
                    "metronome.beat",
                    "This profile supports whole through sixteenth metronome beats",
                ));
            }
        };
        let dots = m.children_named("beat-unit-dot").count();
        if dots > 1 {
            return Err(unsupported(
                m,
                "metronome.dots",
                "SFS currently represents at most one metronome beat dot",
            ));
        }
        let rate = child(m, "per-minute")?;
        let bpm = positive(rate, rate.text.trim()).map_err(|_| unsupported(rate, "metronome.per_minute", "Only a positive, exact decimal display rate is mapped; ranges and descriptive rates are not guessed"))?;
        let staff = node
            .child("staff")
            .map(|s| {
                s.text
                    .trim()
                    .parse::<u32>()
                    .ok()
                    .filter(|s| *s > 0 && *s <= part.staves)
                    .ok_or_else(|| error(s, "staff.number", "Direction staff is outside the part"))
            })
            .transpose()?;
        metronomes.push(ScoreMetronome {
            presentation,
            at: visual_at,
            part: part.id.clone(),
            staff,
            value: Tempo {
                beat: Beat {
                    unit,
                    dotted: dots == 1,
                },
                bpm,
            },
        });
        m.error(report, MappingEffect::Normalized, "metronome.numeric", "Numeric display rate canonicalized exactly; printed metronome does not assign playback tempo");
    }
    let sound = if node.name == "sound" {
        Some(node)
    } else {
        node.child("sound")
    };
    if let Some(sound) = sound {
        if let Some(tempo) = sound.attr("tempo") {
            tempos.push(ScoreTempo {
                at: sounding_at,
                quarter_bpm: positive(sound, tempo)?,
            });
        } else if sound.attr("dynamics").is_none() && sound.attr("pizzicato").is_none() {
            return Err(unsupported(
                sound,
                "sound.empty",
                "Sound requires an explicit mapped tempo or numeric dynamics",
            ));
        }
    }
    Ok(())
}

/// Indexed once: serialization must not scan every tempo for every measure.
pub(super) struct Export<'a> {
    first: Option<&'a scorestream_core::notation::PartId>,
    tempos: std::collections::BTreeMap<Rational, &'a ScoreTempo>,
    marks: std::collections::BTreeMap<
        &'a scorestream_core::notation::PartId,
        std::collections::BTreeMap<Rational, Vec<&'a ScoreMetronome>>,
    >,
}
impl<'a> Export<'a> {
    pub(super) fn new(score: &'a ScoreDocument) -> Self {
        let mut marks: std::collections::BTreeMap<_, std::collections::BTreeMap<_, Vec<_>>> =
            std::collections::BTreeMap::new();
        for m in &score.metronomes {
            marks
                .entry(&m.part)
                .or_default()
                .entry(m.at.clone())
                .or_default()
                .push(m);
        }
        Self {
            first: score.parts.first().map(|p| &p.id),
            tempos: score.tempos.iter().map(|t| (t.at.clone(), t)).collect(),
            marks,
        }
    }
    pub(super) fn positions(&self, part: &ScorePart, measure: &ScoreMeasure) -> Vec<Rational> {
        let end = &measure.start + &measure.length;
        let mut result = Vec::new();
        if self.first == Some(&part.id) {
            result.extend(
                self.tempos
                    .range(measure.start.clone()..end.clone())
                    .map(|(at, _)| at - &measure.start),
            );
        }
        if let Some(marks) = self.marks.get(&part.id) {
            result.extend(
                marks
                    .range(measure.start.clone()..end)
                    .map(|(at, _)| at - &measure.start),
            );
        }
        result
    }
}
pub(super) fn validate_export(score: &ScoreDocument, report: &mut MappingReport) {
    for (at, rate) in score
        .tempos
        .iter()
        .map(|t| (&t.at, &t.quarter_bpm))
        .chain(score.metronomes.iter().map(|m| (&m.at, &m.value.bpm)))
    {
        if super::write::decimal(rate).is_none_or(|v| v.len() > 64) {
            report.push(crate::Format::MusicXml, MappingEffect::Unsupported, "tempo.decimal", at.to_string(),
                "MusicXML tempo needs an exact finite decimal of at most 32 places and 64 characters; it is not rounded");
        }
    }
    if let Some(first) = score.parts.first() {
        let end = first
            .measures
            .last()
            .map(|m| &m.start + &m.length)
            .unwrap_or_default();
        if score.tempos.iter().any(|t| t.at >= end) {
            report.push(
                crate::Format::MusicXml,
                MappingEffect::Unsupported,
                "tempo.global_extent",
                "/tempos",
                "First part must cover every score-wide tempo when serializing global changes",
            );
        }
    }
}
pub(super) fn write_measure(
    index: &Export<'_>,
    part: &ScorePart,
    measure: &ScoreMeasure,
    divisions: u64,
    out: &mut String,
) {
    use std::fmt::Write;
    let end = &measure.start + &measure.length;
    let units = |at: &Rational| {
        (&(at - &measure.start) * &Rational::from_big_integer(divisions.into())).to_string()
    };
    if index.first == Some(&part.id) {
        for tempo in index
            .tempos
            .range(measure.start.clone()..end.clone())
            .map(|(_, t)| t)
        {
            writeln!(
                out,
                "      <sound tempo=\"{}\"><offset>{}</offset></sound>",
                super::write::decimal(&tempo.quarter_bpm).unwrap(),
                units(&tempo.at)
            )
            .unwrap();
        }
    }
    for mark in index.marks.get(&part.id).into_iter().flat_map(|marks| {
        marks
            .range(measure.start.clone()..end.clone())
            .flat_map(|(_, marks)| marks)
    }) {
        let unit = match mark.value.beat.unit {
            BeatUnit::Whole => "whole",
            BeatUnit::Half => "half",
            BeatUnit::Quarter => "quarter",
            BeatUnit::Eighth => "eighth",
            BeatUnit::Sixteenth => "16th",
        };
        if let Some(p)=&mark.presentation {super::metronome_display::start(p,out);} else {out.push_str("      <direction><direction-type><metronome>");}
        write!(out,"<beat-unit>{unit}</beat-unit>{}<per-minute>{}</per-minute></metronome></direction-type>",if mark.value.beat.dotted {"<beat-unit-dot/>"}else{""},super::write::decimal(&mark.value.bpm).unwrap()).unwrap();
        if let Some(w)=mark.presentation.as_ref().and_then(|p|p.trailing_words.as_ref()) {super::words::write_text(w,out);}
        write!(out,"<offset sound=\"no\">{}</offset>",units(&mark.at)).unwrap();
        if let Some(staff) = mark.staff {
            write!(out, "<staff>{staff}</staff>").unwrap();
        }
        out.push_str("</direction>\n");
    }
}
