//! Compile the shared typed notation model into an immutable output plan.
//! SFS parsing/lowering, expression policy, continuity, and tempo conversion are
//! distinct modules. No UI, audio devices, markup or wire attributes enter here.
mod articulations;
pub use articulations::*;
mod clock;
mod combined;
mod continuity;
mod attack_dynamics;
mod diagnostic;
mod expression;
mod fermata;
mod grace;
mod numeric;
mod pizzicato;
mod plan;
mod tempo;
use clock::PlaybackClock;
pub use combined::*;
use continuity::Continuity;
use diagnostic::Diagnostics;
pub use diagnostic::{PerformanceDiagnostic, RealizationEffect};
use expression::Expression;
pub use fermata::*;
pub use grace::*;
pub use plan::*;
use scorestream_codec::fermata_timing::TimingProjection;
use scorestream_codec::{Document, Duration, Rational, Record, Scalar, notation::*};
use std::collections::{BTreeMap, BTreeSet};

/// Invalid drafts never play a last-good snapshot. This has no workspace effects.
pub fn plan_source(source: &str) -> Result<PlaybackPlan, String> {
    let doc = scorestream_codec::parse(source).map_err(format_diagnostics)?;
    if doc
        .required_extensions
        .iter()
        .any(|e| e == scorestream_codec::navigation_extension::EXTENSION)
    {
        let performed =
            perform_document(&doc, PerformanceLimits::default()).map_err(format_diagnostics)?;
        compile_performed(&performed.score, &doc, Some(&performed), None)
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
    compile_performed(score, doc, None, None)
}
fn compile_performed(
    score: &ResolvedScore,
    doc: &Document,
    performed: Option<&PerformedScore>,
    timing: Option<&TimingProjection>,
) -> Result<PlaybackPlan, String> {
    compile_scheduled(score, doc, performed, timing, None)
}
fn compile_scheduled(
    score: &ResolvedScore,
    doc: &Document,
    performed: Option<&PerformedScore>,
    timing: Option<&TimingProjection>,
    grace: Option<&GraceSchedule>,
) -> Result<PlaybackPlan, String> {
    if timing.is_some() && grace.is_some() {
        return Err("Combined fermata and grace sound policies are unsupported".into());
    }
    compile_with_graces(
        score,
        doc,
        performed,
        timing,
        grace.map(GraceView::Standalone),
    )
}
/// Only a complete source-qualified proof may enter the joint clock path.
fn compile_combined(
    doc: &Document,
    qualified: &QualifiedCombinedPerformance,
) -> Result<PlaybackPlan, String> {
    let performed = qualified.performed();
    compile_with_graces(
        &performed.score,
        doc,
        Some(performed),
        Some(qualified.timing()),
        Some(GraceView::Combined(qualified)),
    )
}
#[derive(Clone, Copy)]
enum GraceView<'a> {
    Standalone(&'a GraceSchedule),
    Combined(&'a QualifiedCombinedPerformance),
}
impl<'a> GraceView<'a> {
    fn windows(self) -> &'a [GraceSoundWindow] {
        match self {
            Self::Standalone(s) => s.windows(),
            Self::Combined(q) => q.grace_windows(),
        }
    }
    fn groups(self) -> &'a [GraceGroupRealization] {
        match self {
            Self::Standalone(s) => s.groups(),
            Self::Combined(q) => q.grace_groups(),
        }
    }
    fn default_tempo_intervals(self) -> &'a [GraceDefaultTempoInterval] {
        match self {
            Self::Standalone(s) => s.default_tempo_intervals(),
            Self::Combined(q) => q.default_tempo_intervals(),
        }
    }
}
fn compile_with_graces(
    score: &ResolvedScore,
    doc: &Document,
    performed: Option<&PerformedScore>,
    timing: Option<&TimingProjection>,
    grace: Option<GraceView<'_>>,
) -> Result<PlaybackPlan, String> {
    let tempo = match grace {
        Some(GraceView::Standalone(schedule)) => PlaybackClock::Grace(schedule),
        _ => PlaybackClock::new(score, timing)?,
    };
    let grace_windows: BTreeMap<_, _> = grace
        .into_iter()
        .flat_map(|g| g.windows().iter().map(|w| (&w.id, w)))
        .collect();
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
    for d in doc.diagnostics.iter().filter(|d|d.code.starts_with("metronome_display.")) {warnings.add(&d.code,"engraving.metronome_display",RealizationEffect::NotationOnly,&Origin{object_id:"document".into(),source:d.source},&d.message);}

    for d in doc.diagnostics.iter().filter(|d|d.code.starts_with("part_labels.")) {warnings.add(&d.code,"engraving.part_labels",RealizationEffect::NotationOnly,&Origin {object_id:"document".into(),source:d.source},&d.message);}

    for d in doc.diagnostics.iter().filter(|d| d.code.starts_with("wedge.")) {
        warnings.add(&d.code,"engraving.wedge",RealizationEffect::NotationOnly,&Origin {object_id:"document".into(),source:d.source},&d.message);
    }
    for d in doc.diagnostics.iter().filter(|d| d.code.starts_with("dashes.")) {
        warnings.add(&d.code,"engraving.dashes",RealizationEffect::NotationOnly,&Origin {object_id:"document".into(),source:d.source},&d.message);
    }
    for d in doc.diagnostics.iter().filter(|d| d.code.starts_with("words.")) {
        warnings.add(&d.code,"engraving.words",RealizationEffect::NotationOnly,&Origin {object_id:"document".into(),source:d.source},&d.message);
    }
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
    if matches!(grace, Some(GraceView::Combined(_))) {
        warnings.add("combined.explicit_order_boundary", "performance.grace_fermata", RealizationEffect::Policy,
            &Origin { object_id: "document".into(), source: doc.source },
            "Explicit quarter allocation precedes additive global fermata seconds. Grace and principal attacks use the final after-boundary clock: a hold at group onset finishes before grace attacks and boundary controls; holds inside or at the nominal principal endpoint are unsupported. Written grace markers remain zero-length.");
    }
    let expression = Expression::new(score, &mut warnings)?;
    let continuity = Continuity::new(score, &expression, &mut warnings)?;
    for (index, event) in score.events.iter().enumerate() {
        if event.attack_dynamics.is_some() {
            if continuity.targets.contains(&index) {
                return Err(format!("note_dynamics.tie_continuation_unsupported: {} is not a new attack", event.id));
            }
            if event.grace.is_some() || grace_windows.contains_key(&event.id) || event.role == EventRole::Cue || !matches!(event.written_pitch, scorestream_codec::Pitch::Pitched { .. }) {
                return Err(format!("note_dynamics.owner_unsupported: {} requires a qualified pitched measured attack",event.id));
            }
        }
    }
    if let Some(grace) = grace {
        let events: BTreeMap<_, _> = score.events.iter().map(|e| (&e.id, e)).collect();
        for window in grace.windows() {
            let event = events
                .get(&window.id)
                .ok_or("grace.schedule_identity: Missing scheduled event")?;
            if expression.value(event, &window.quarter_start) <= 0. {
                return Err(format!(
                    "grace.inaudible_anchor: Scheduled event {} has zero sound dynamics at its actual attack",
                    event.id
                ));
            }
            warnings.add("grace.explicit_sound_window", "rhythm.grace", RealizationEffect::Policy, &event.origin,
                "Explicit following-principal allocation: graces sound in source order for their full allocation; principal sounds for 23/25 of the remaining nominal duration. Written markers and source events remain unchanged. Pitch, staff and instrument stay resolved at the written anchor; numeric dynamics use actual attacks and absolute control times.");
        }
        for interval in grace.default_tempo_intervals() {
            warnings.add("grace.default_quarter_tempo", "tempo.default", RealizationEffect::Policy,
                &Origin { object_id: "performance.default_tempo".into(), source: doc.source },
                &format!("Existing quarter=120 clock convention applies from {} through {} quarters; this is not an authored tempo.", interval.from_quarter, interval.until_quarter));
        }
        for group in grace.groups() {
            let event = events.get(&group.principal).expect("qualified principal");
            let (code, message) = match group.allocation {
                GraceAllocation::AuthoredFollowingPercent => (
                    "grace.authored_following_percent",
                    "Duration derives from the sole authored following percentage at constant effective tempo; grace-first gate order is explicit playback policy, not a MusicXML rule.",
                ),
                GraceAllocation::ExplicitFollowingQuarters { .. } => (
                    "grace.explicit_following_quarters",
                    "Every grace duration derives from the caller's exact quarter allocation; no slash, written-value or equal-division duration was inferred.",
                ),
            };
            warnings.add(
                code,
                "rhythm.grace",
                RealizationEffect::Policy,
                &event.origin,
                message,
            );
        }
    }
    let held_tails = if let Some(timing) = timing {
        let articulation_policy = match grace {
            Some(GraceView::Combined(qualified)) => qualified.articulations().map(|a| a.policy()),
            _ => None,
        };
        if articulation_policy.is_some() {
            warnings.add("articulation.explicit_joint_policy", "articulation.authored", RealizationEffect::Policy,
                &Origin { object_id: "document".into(), source: doc.source },
                "Explicit SimpleStaccatoAccentV1: half gate and +16 accent velocity policy, only away from selected holds and grace/principal windows; source notation retained.");
        }
        fermata::validate_sound_policy(score, &continuity, timing, articulation_policy, &mut warnings)?
    } else {
        BTreeSet::new()
    };
    for direction in &score.directions {
        match &direction.kind {
            DirectionKind::Words {..} => warnings.add("words_not_realized", "direction.words", RealizationEffect::NotationOnly, &direction.origin, "Text is represented in the shared model; no musical effect is guessed from arbitrary words."),
            DirectionKind::Metronome {..} => warnings.add("metronome_visual_only", "tempo.metronome", RealizationEffect::NotationOnly, &direction.origin, "Printed metronome notation does not replace explicit playback tempo state."),
            _ => (),
        }
    }
    let pizzicato = pizzicato::State::new(score)?;
    let mut notes = Vec::new();
    let mut routes = BTreeSet::new();
    for (index, event) in score.events.iter().enumerate() {
        if continuity.targets.contains(&index) {
            continue;
        }
        let window = grace_windows.get(&event.id).copied();
        if event.duration == Duration::Grace && window.is_none() {
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
        let release = if let Some(window) = window {
            window.quarter_end.clone()
        } else if held_tails.contains(&tail) {
            score.events[tail].end()
        } else {
            continuity.release(&score.events[tail], tail)
        };
        let attack = window.map(|w| &w.quarter_start).unwrap_or(&event.start);
        pizzicato.check(event, attack)?;
        let start = tempo.seconds(attack)?;
        let end = tempo.seconds(&release)?;
        let duration = end - start;
        if duration < 0.000_001 {
            return Err("A note is shorter than the playback clock can represent".into());
        }
        let mut velocity = if window.is_some() {
            expression.attack_at(event, attack)
        } else {
            expression.attack(event)
        }
        .round()
        .clamp(1., 127.) as u8;
        let segments: Vec<_> = chain.iter().map(|i| &score.events[*i]).collect();
        let mut envelope = if window.is_some() {
            expression.envelope_from(&segments, attack, &release, velocity, &tempo)?
        } else {
            expression.envelope(&segments, &release, velocity, &tempo)?
        };
        if let Some((override_velocity, override_envelope)) = attack_dynamics::apply(event, &envelope, duration)? {
            if override_velocity == 0 {
                warnings.add("note_dynamics.silent_attack", "dynamics.note_attack", RealizationEffect::Policy, &event.origin, "Explicit zero note dynamics is a silent attack; written event markers remain and no NoteOff command or later state is inferred.");
                continue;
            }
            velocity = override_velocity;
            envelope = override_envelope;
        }
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
