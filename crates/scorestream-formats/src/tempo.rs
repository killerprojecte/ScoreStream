//! Shared tempo semantics. Explicit playback state and printed metronomes stay separate.
use crate::{Format, MappingEffect, MappingReport};
use scorestream_core::{
    notation::{Beat, BeatUnit, PartId, Tempo},
    score::*,
    *,
};
use std::collections::BTreeMap;

pub(crate) fn parse_beat(value: &str) -> Option<Beat> {
    let dotted = value.ends_with('.');
    let unit = match value.trim_end_matches('.') {
        "w" => BeatUnit::Whole,
        "h" => BeatUnit::Half,
        "q" => BeatUnit::Quarter,
        "e" => BeatUnit::Eighth,
        "s" => BeatUnit::Sixteenth,
        _ => return None,
    };
    if value.matches('.').count() > 1 {
        return None;
    }
    Some(Beat { unit, dotted })
}
pub(crate) fn beat_text(beat: Beat) -> String {
    format!(
        "{}{}",
        match beat.unit {
            BeatUnit::Whole => "w",
            BeatUnit::Half => "h",
            BeatUnit::Quarter => "q",
            BeatUnit::Eighth => "e",
            BeatUnit::Sixteenth => "s",
        },
        if beat.dotted { "." } else { "" }
    )
}
/// Partwise duplication is not precedence: every simultaneous assignment must agree.
pub(crate) fn normalize(tempos: &mut Vec<ScoreTempo>, report: &mut MappingReport, format: Format) {
    tempos.sort_by(|a, b| a.at.cmp(&b.at));
    for pair in tempos.windows(2) {
        if pair[0].at == pair[1].at {
            let same = pair[0].quarter_bpm == pair[1].quarter_bpm;
            report.push(format, if same { MappingEffect::Normalized } else { MappingEffect::Invalid },
                if same { "tempo.duplicate" } else { "tempo.conflict" }, pair[1].at.to_string(),
                if same { "Equivalent score-wide tempo assignments coalesced" } else { "Conflicting simultaneous score-wide tempos; no part or document-order precedence is guessed" });
        }
    }
    tempos.dedup();
}
pub(crate) fn validate(score: &ScoreDocument, report: &mut MappingReport) {
    let end = score
        .parts
        .iter()
        .flat_map(|p| &p.measures)
        .map(|m| &m.start + &m.length)
        .max()
        .unwrap_or_default();
    let mut seen = BTreeMap::new();
    for t in &score.tempos {
        if t.at.is_negative() || t.at >= end || !t.quarter_bpm.is_positive() {
            report.push(
                Format::ScoreModel,
                MappingEffect::Invalid,
                "tempo.value",
                t.at.to_string(),
                "Tempo needs positive quarter BPM and an onset inside the score",
            );
        }
        if seen.insert(&t.at, &t.quarter_bpm).is_some() {
            report.push(
                Format::ScoreModel,
                MappingEffect::Invalid,
                "tempo.conflict",
                t.at.to_string(),
                "Common tempo lane must have exactly one assignment per position",
            );
        }
    }
    let parts: BTreeMap<_, _> = score
        .parts
        .iter()
        .map(|p| {
            (
                &p.id,
                (
                    p.staves,
                    p.measures
                        .last()
                        .map(|m| &m.start + &m.length)
                        .unwrap_or_default(),
                ),
            )
        })
        .collect();
    for m in &score.metronomes {
        let valid = parts.get(&m.part).is_some_and(|(staves, end)| {
            m.staff.is_none_or(|s| s > 0 && s <= *staves) && !m.at.is_negative() && m.at < *end
        });
        if !valid || !m.value.bpm.is_positive() {
            report.push(Format::ScoreModel, MappingEffect::Invalid, "metronome.value", m.at.to_string(), "Metronome requires a valid part/staff, onset inside that part, and positive displayed BPM");
        }
    }
}
pub(crate) fn read_mark(
    mark: &Mark,
    at: &Rational,
    score: &mut ScoreDocument,
    report: &mut MappingReport,
) {
    for key in mark.attributes.keys() {
        if !["beat", "bpm"].contains(&key.as_str()) {
            report.push(Format::Sfs, MappingEffect::Unsupported, format!("metronome.{key}"), &mark.target, "Metronome text and identity need a richer shared display model; they are not discarded");
        }
    }
    let (part, staff) = if score.parts.iter().any(|p| p.id.0 == mark.target) {
        (mark.target.as_str(), None)
    } else if let Some((p, s)) = mark
        .target
        .rsplit_once(".s")
        .and_then(|(p, s)| s.parse::<u32>().ok().map(|s| (p, s)))
    {
        (p, Some(s))
    } else {
        report.push(
            Format::Sfs,
            MappingEffect::Unsupported,
            "metronome.scope",
            &mark.target,
            "Only explicit part and staff metronome display scopes are mapped",
        );
        return;
    };
    let beat = parse_beat(mark.attributes["beat"].as_str().unwrap()).unwrap();
    let bpm = mark.attributes["bpm"].as_str().unwrap().parse().unwrap();
    score.metronomes.push(ScoreMetronome {
        at: at.clone(),
        part: PartId(part.into()),
        staff,
        value: Tempo { beat, bpm },
    });
}
pub(crate) fn write_tempos(score: &ScoreDocument, frames: &mut BTreeMap<Rational, Vec<Record>>) {
    for t in &score.tempos {
        frames
            .entry(t.at.clone())
            .or_default()
            .push(Record::State(State {
                target: "score".into(),
                attributes: [(
                    "tempo".into(),
                    Scalar::String(format!("q:{}", t.quarter_bpm)),
                )]
                .into(),
                source: SourceRange::default(),
            }));
    }
}
pub(crate) fn write_mark(
    m: &ScoreMetronome,
    part: &str,
    frames: &mut BTreeMap<Rational, Vec<Record>>,
) {
    frames
        .entry(m.at.clone())
        .or_default()
        .push(Record::Mark(Mark {
            target: m
                .staff
                .map(|s| format!("{part}.s{s}"))
                .unwrap_or_else(|| part.into()),
            kind: "metronome".into(),
            attributes: [
                ("beat".into(), Scalar::String(beat_text(m.value.beat))),
                ("bpm".into(), Scalar::String(m.value.bpm.to_string())),
            ]
            .into(),
            source: SourceRange::default(),
        }));
}
