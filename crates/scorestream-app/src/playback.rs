//! Compile the shared typed notation model into an immutable output plan.
//! SFS parsing/lowering, expression policy, continuity, and tempo conversion are
//! distinct modules. No UI, audio devices, markup or wire attributes enter here.
mod continuity;
mod diagnostic;
mod expression;
mod numeric;
mod plan;
mod tempo;
use continuity::Continuity;
use diagnostic::Diagnostics;
pub use diagnostic::{PerformanceDiagnostic, RealizationEffect};
use expression::Expression;
pub use plan::*;
use scorestream_codec::{Document, Duration, Rational, Record, Scalar, notation::*};
use std::collections::{BTreeMap, BTreeSet};
use tempo::TempoMap;

/// Invalid drafts never play a last-good snapshot. This has no workspace effects.
pub fn plan_source(source: &str) -> Result<PlaybackPlan, String> {
    if source.len() > 4 * 1024 * 1024 {
        return Err("Playback accepts score sources up to 4 MiB".into());
    }
    let doc = scorestream_codec::parse(source).map_err(format_diagnostics)?;
    if doc
        .required_extensions
        .iter()
        .any(|e| e == scorestream_codec::navigation_extension::EXTENSION)
    {
        let performed =
            perform_document(&doc, PerformanceLimits::default()).map_err(format_diagnostics)?;
        compile_performed(&performed.score, &doc, Some(&performed))
    } else {
        let score = lower_document(&doc).map_err(format_diagnostics)?;
        compile(&score, &doc)
    }
}
fn format_diagnostics(diagnostics: Vec<scorestream_codec::Diagnostic>) -> String {
    diagnostics
        .iter()
        .take(8)
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}
fn compile(score: &ResolvedScore, doc: &Document) -> Result<PlaybackPlan, String> {
    compile_performed(score, doc, None)
}
fn compile_performed(
    score: &ResolvedScore,
    doc: &Document,
    performed: Option<&PerformedScore>,
) -> Result<PlaybackPlan, String> {
    if score.events.len() > 100_000 {
        return Err("Playback is limited to 100,000 events per score".into());
    }
    let tempo = TempoMap::new(score);
    let mut duration_seconds = tempo.seconds(&score.end)?;
    let written_ids: BTreeMap<_, _> = performed
        .into_iter()
        .flat_map(|p| p.events.iter().map(|e| (&e.id, &e.written_id)))
        .collect();
    // Map every written event before continuity/audio policy can omit an attack.
    // In particular, a tied continuation still owns its own score interval.
    let event_positions = score
        .events
        .iter()
        .map(|event| {
            Ok(PlaybackEventPosition {
                id: written_ids
                    .get(&event.id)
                    .map(|id| id.0.clone())
                    .unwrap_or_else(|| event.id.0.clone()),
                occurrence_id: performed.map(|_| event.id.0.clone()),
                start_seconds: tempo.seconds(&event.start)?,
                end_seconds: tempo.seconds(&event.end())?,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    // Measures live in the validated document rather than the performance model.
    // Convert their explicit boundaries, never infer uniform lengths or BPM.
    let mut measures = Vec::new();
    if let Some(performed) = performed {
        for m in &performed.measures {
            measures.push(PlaybackMeasure {
                id: m.written_id.clone(),
                occurrence_index: Some(m.occurrence_index),
                part: m.part.0.clone(),
                label: Some(m.label.clone()),
                start_seconds: tempo.seconds(&m.start)?,
                end_seconds: tempo.seconds(&(&m.start + &m.length))?,
            });
        }
    } else {
        for frame in &doc.frames {
            for record in &frame.records {
                if let Record::Measure(measure) = record {
                    measures.push(PlaybackMeasure {
                        id: measure.id.clone(),
                        occurrence_index: None,
                        part: measure.target.clone(),
                        label: measure
                            .attributes
                            .get("label")
                            .and_then(Scalar::as_str)
                            .map(str::to_owned),
                        start_seconds: tempo.seconds(&frame.time)?,
                        end_seconds: tempo.seconds(&(&frame.time + &measure.length))?,
                    });
                }
            }
        }
    }
    let mut warnings = Diagnostics::default();
    if let Some(performed) = performed {
        for d in &performed.diagnostics {
            warnings.add(
                &d.code,
                "navigation.repeat",
                RealizationEffect::Policy,
                &Origin {
                    object_id: "document".into(),
                    source: d.source,
                },
                &d.message,
            );
        }
    }
    let expression = Expression::new(score, &mut warnings)?;
    let continuity = Continuity::new(score, &expression, &mut warnings)?;
    for direction in &score.directions {
        match &direction.kind {
            DirectionKind::Words {..} => warnings.add("words_not_realized", "direction.words", RealizationEffect::NotationOnly, &direction.origin, "Text is represented in the shared model; no musical effect is guessed from arbitrary words."),
            DirectionKind::Metronome {..} => warnings.add("metronome_visual_only", "tempo.metronome", RealizationEffect::NotationOnly, &direction.origin, "Printed metronome notation does not replace explicit playback tempo state."),
            _ => (),
        }
    }
    let mut notes = Vec::new();
    let mut routes = BTreeSet::new();
    for (index, event) in score.events.iter().enumerate() {
        if continuity.targets.contains(&index) {
            continue;
        }
        if event.duration == Duration::Grace {
            warnings.add(
                "grace_unrealized",
                "rhythm.grace",
                RealizationEffect::Omitted,
                &event.origin,
                "No grace timing policy is selected; grace note omitted.",
            );
            continue;
        }
        if event.role == EventRole::Cue {
            warnings.add(
                "cue_silent",
                "rhythm.cue",
                RealizationEffect::Policy,
                &event.origin,
                "Cue notes are silent under this policy.",
            );
            continue;
        }
        let Some(pitch) = &continuity.pitch[index] else {
            if event.written_pitch.is_note() {
                warnings.add(
                    "percussion_unmapped",
                    "pitch.unpitched",
                    RealizationEffect::Omitted,
                    &event.origin,
                    "No percussion instrument map; note omitted.",
                );
            }
            continue;
        };
        let pitch = (pitch + &Rational::from_integer(12)).to_f64();
        if !pitch.is_finite() || !(0.0..=127.0).contains(&pitch) {
            warnings.add(
                "pitch_range",
                "pitch.range",
                RealizationEffect::Omitted,
                &event.origin,
                "Pitch outside MIDI 0–127; note omitted.",
            );
            continue;
        }
        let chain = continuity.chain(index)?;
        let tail = *chain.last().unwrap();
        let release = continuity.release(&score.events[tail], tail);
        let start = tempo.seconds(&event.start)?;
        let end = tempo.seconds(&release)?;
        let duration = end - start;
        if duration < 0.000_001 {
            return Err("A note is shorter than the playback clock can represent".into());
        }
        let velocity = expression.attack(event).round().clamp(1., 127.) as u8;
        let segments: Vec<_> = chain.iter().map(|i| &score.events[*i]).collect();
        let envelope = expression.envelope(&segments, &release, velocity, &tempo)?;
        routes.insert(PlaybackRoute {
            part: event.part.0.clone(),
            instrument: event.instrument.0.clone(),
        });
        duration_seconds = duration_seconds.max(end);
        notes.push(PlaybackNote {
            id: event.id.0.clone(),
            written_id: written_ids.get(&event.id).map(|id| id.0.clone()),
            part: event.part.0.clone(),
            instrument: event.instrument.0.clone(),
            voice: event.voice.0.clone(),
            start_seconds: start,
            duration_seconds: duration,
            midi_note: pitch,
            frequency_hz: 440. * 2_f64.powf((pitch - 69.) / 12.),
            velocity,
            gain_envelope: envelope,
        });
    }
    notes.sort_by(|a, b| {
        a.start_seconds
            .total_cmp(&b.start_seconds)
            .then(a.part.cmp(&b.part))
            .then(a.voice.cmp(&b.voice))
            .then(a.id.cmp(&b.id))
    });
    let (warnings, diagnostics) = warnings.finish();
    Ok(PlaybackPlan {
        duration_seconds,
        notes,
        event_positions,
        measures,
        routes: routes.into_iter().collect(),
        warnings,
        diagnostics,
    })
}
#[cfg(test)]
mod tests;
