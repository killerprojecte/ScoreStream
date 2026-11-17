//! Explicit offline admission experiment: computes/discards dry PCM, never writes
//! audio or changes default rendering. Complete voice tails, no timeout eviction.
use crate::dry_lanes::{DryEnvelopeSchedule, DryLane, DryTailPolicy};
use crate::native::{Api, Bank, dry::DrySynth};
use crate::pizzicato::{PizzicatoSamplePolicy, TechniqueChannel};
use crate::*;
use scorestream_app::playback::TechniquePlaybackPlan;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug, Serialize)]
pub struct DrySelectedRoute {
    pub bank: i32,
    pub program: i32,
    pub cc7: i32,
    pub cc10: i32,
}
#[derive(Debug, Serialize)]
pub struct DryAdmissionNote {
    pub event: String,
    pub ideal_on: u64,
    pub rendered_on: u64,
    pub ideal_off: u64,
    pub rendered_off: u64,
    pub authorized_route: usize,
    pub lane: Option<usize>,
    pub selected_route: Option<DrySelectedRoute>,
}
#[derive(Debug, Serialize)]
pub struct DryAdmissionReport {
    pub source_sha256: String,
    pub plan_sha256: String,
    pub bank_sha256: String,
    pub backend: BackendCapabilities,
    pub physical_group_limit: usize,
    pub quantum_frames: u32,
    pub completed_frames: u64,
    pub observation_policy: String,
    pub selected_window_frames: u64,
    pub last_ideal_noteoff: u64,
    pub last_rendered_noteoff: u64,
    pub first_idle_frame: Option<u64>,
    pub admitted_notes: usize,
    pub noteoffs: usize,
    pub noteoffs_without_live_voice: usize,
    pub peak_owned_lanes: usize,
    pub peak_playing_lanes: usize,
    pub peak_native_voices: usize,
    pub playing_lanes_at_end: usize,
    pub sample_peak: f64,
    pub sample_rms: f64,
    pub notes: Vec<DryAdmissionNote>,
    pub technique_channels: Vec<TechniqueChannel>,
    pub original_envelopes: DryEnvelopeSchedule,
    pub local_midi_mix: Option<midi_mixer::LocalMidiMix>,
    pub warnings: Vec<String>,
    pub performance_diagnostics: Vec<scorestream_app::playback::PerformanceDiagnostic>,
    pub practice_nonrealizations: Vec<String>,
    pub note_dynamics_receipts: Vec<scorestream_app::playback::PracticeNoteDynamicsReceipt>,
}
fn invalid(s: &str) -> AudioError {
    AudioError::InvalidPlan(s.into())
}
fn quantize(frame: u64, q: u32) -> Result<u64> {
    frame
        .checked_add(u64::from(q) - 1)
        .map(|v| v / u64::from(q) * u64::from(q))
        .ok_or_else(|| invalid("Native event frame overflow"))
}
/// Run the authenticated plan through the installed dry engine, discarding PCM.
/// Physical groups are discovered from native settings. Failure to admit at a
/// fixed quantized onset is an error, never a queued/delayed note or killed tail.
/// Resource budgets remain explicit caller options; no score-size cap is added.
#[allow(clippy::too_many_arguments)]
pub fn probe(
    source: &str,
    plan: &TechniquePlaybackPlan,
    routes: &[RouteAssignment],
    bank_path: &Path,
    options: &RenderOptions,
    tail_policy: DryTailPolicy,
    pizzicato: PizzicatoSamplePolicy,
    cancel: &AtomicBool,
) -> Result<DryAdmissionReport> {
    probe_selected(
        source,
        plan,
        routes,
        bank_path,
        options,
        tail_policy,
        pizzicato,
        cancel,
        false,
        None,
    )
}
/// Explicit observation through authoritative native voice exhaustion. No
/// amplitude threshold or fixed-duration eviction; cancellation remains available.
#[allow(clippy::too_many_arguments)]
pub fn probe_until_idle(
    source: &str,
    plan: &TechniquePlaybackPlan,
    routes: &[RouteAssignment],
    bank_path: &Path,
    options: &RenderOptions,
    tail_policy: DryTailPolicy,
    pizzicato: PizzicatoSamplePolicy,
    cancel: &AtomicBool,
) -> Result<DryAdmissionReport> {
    probe_selected(
        source,
        plan,
        routes,
        bank_path,
        options,
        tail_policy,
        pizzicato,
        cancel,
        true,
        None,
    )
}
#[allow(clippy::too_many_arguments)]
fn probe_selected(
    source: &str,
    plan: &TechniquePlaybackPlan,
    routes: &[RouteAssignment],
    bank_path: &Path,
    options: &RenderOptions,
    tail_policy: DryTailPolicy,
    pizzicato: PizzicatoSamplePolicy,
    cancel: &AtomicBool,
    until_idle: bool,
    mut sink: Option<&mut dyn PcmSink>,
) -> Result<DryAdmissionReport> {
    let mix = options
        .local_midi_mix
        .as_ref()
        .ok_or_else(|| invalid("Dry admission requires source mixer"))?;
    mix.verify_source(source)?;
    if mix.source_sha256() != plan.source_sha256() {
        return Err(invalid("Dry admission compiler/source mismatch"));
    }
    let schedule = DryEnvelopeSchedule::prepare(plan, options.sample_rate, tail_policy)?;
    schedule.verify_plan(plan)?;
    let palette = crate::pizzicato::lanes(plan, routes, options, pizzicato)?;
    let prepared = crate::plan::Prepared::new_dry(plan, routes, options, &palette.channels)?;
    let api = Api::open()?;
    let groups = DrySynth::physical_group_limit(&api)?;
    let mut bank = Bank::open(bank_path, &palette.options, cancel)?;
    let mut synth = DrySynth::new_pool(
        &api,
        &mut bank,
        source,
        &palette.routes,
        &palette.options,
        cancel,
        groups,
    )?;
    let q = synth.quantum();
    let terminal = quantize(prepared.frames, q)?;
    let mut notes = Vec::new();
    let mut events = Vec::new();
    for (i, n) in prepared.notes.iter().enumerate() {
        let on = quantize(n.start, q)?;
        let off = quantize(n.end, q)?;
        if on >= off {
            return Err(AudioError::Unsupported(format!(
                "{} collapses under declared native quantum",
                n.event_id
            )));
        }
        notes.push(DryAdmissionNote {
            event: n.event_id.clone(),
            ideal_on: n.start,
            rendered_on: on,
            ideal_off: n.end,
            rendered_off: off,
            authorized_route: usize::from(n.channel),
            lane: None,
            selected_route: None,
        });
        events.push((on, true, i));
        events.push((off, false, i));
    }
    events.sort_unstable(); // NoteOff before NoteOn at equal rendered boundary
    let mut owners: Vec<_> = (0..groups).map(|_| DryLane::default()).collect();
    let mut owner_note = vec![None; groups];
    let mut gates = vec![false; groups];
    let mut cursor = 0;
    let mut index = 0;
    let mut squares = 0f64;
    let mut report = DryAdmissionReport {
        source_sha256: plan.source_sha256().into(),
        plan_sha256: schedule.plan_sha256().into(),
        bank_sha256: bank.hash.clone(),
        backend: api.capabilities.clone(),
        physical_group_limit: groups,
        quantum_frames: q,
        completed_frames: 0,
        observation_policy: if until_idle { "until_native_idle" } else { "selected_window" }.into(),
        selected_window_frames: terminal,
        last_ideal_noteoff: prepared.notes.iter().map(|n|n.end).max().unwrap_or(0),
        last_rendered_noteoff: events.iter().filter(|(_,on,_)|!*on).map(|(f,_,_)|*f).max().unwrap_or(0),
        first_idle_frame: None,
        admitted_notes: 0,
        noteoffs: 0,
        noteoffs_without_live_voice: 0,
        peak_owned_lanes: 0,
        peak_playing_lanes: 0,
        peak_native_voices: 0,
        playing_lanes_at_end: 0,
        sample_peak: 0.,
        sample_rms: 0.,
        notes,
        technique_channels: palette.evidence,
        original_envelopes: schedule,
        local_midi_mix: palette.options.local_midi_mix.clone(),
        performance_diagnostics: plan.plan().diagnostics.clone(),
        practice_nonrealizations: plan.practice_nonrealizations().to_vec(),
        note_dynamics_receipts: plan.note_dynamics_receipts().to_vec(),
        warnings: vec![
            "Admission experiment only: dry PCM computed then discarded; no audio publication".into(),
            "Native ceil-to-quantum event frames retained alongside ideal frames; no further admission delay".into(),
            "Envelope gains follow the ideal sample clock; native attacks remain quantum-limited".into(),
            "Released tails explicitly hold endpoint gain; no timeout or amplitude-based release".into(),
            "Voice saturation fails; counts are not an exhaustive SoundFont exclusive-class detector".into(),
        ],
    };
    report.warnings.extend(plan.plan().warnings.clone());
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(AudioError::Cancelled);
        }
        while index < events.len() && events[index].0 == cursor {
            let (_, on, i) = events[index];
            let source_note = &prepared.notes[i];
            let lane = if on {
                let live = synth.snapshot()?;
                let lane=(0..groups).find(|l|!gates[*l] && live.playing_per_channel()[*l]==0)
                    .ok_or_else(||AudioError::Unsupported(format!("Dry capacity exhausted at rendered frame {cursor}, ideal frame {}, event {}; all {groups} physical lanes owned or playing release tails",source_note.start,source_note.event_id)))?;
                let binding = synth.reselect_idle_route(lane, usize::from(source_note.channel))?;
                report.notes[i].selected_route = Some(DrySelectedRoute {
                    bank: binding.bank,
                    program: binding.program,
                    cc7: binding.cc7,
                    cc10: binding.cc10,
                });
                owners[lane].admit(lane, i, cursor, &synth)?;
                owner_note[lane] = Some(i);
                gates[lane] = true;
                report.notes[i].lane = Some(lane);
                report.admitted_notes += 1;
                lane
            } else {
                let lane = report.notes[i]
                    .lane
                    .ok_or_else(|| invalid("NoteOff without admitted lane"))?;
                owners[lane].noteoff(i, cursor)?;
                gates[lane] = false;
                report.noteoffs += 1;
                lane
            };
            let native_note = crate::plan::Note {
                event_id: source_note.event_id.clone(),
                channel: u16::try_from(lane)
                    .map_err(|_| invalid("Native channel representability"))?,
                key: source_note.key,
                velocity: source_note.velocity,
                start: source_note.start,
                end: source_note.end,
            };
            report.noteoffs_without_live_voice += usize::from(synth.event(&native_note, on)?);
            index += 1;
        }
        let live = synth.snapshot()?;
        let playing = live
            .playing_per_channel()
            .iter()
            .filter(|n| **n > 0)
            .count();
        report.peak_playing_lanes = report.peak_playing_lanes.max(playing);
        report.peak_owned_lanes = report.peak_owned_lanes.max(
            (0..groups)
                .filter(|i| gates[*i] || live.playing_per_channel()[*i] > 0)
                .count(),
        );
        report.peak_native_voices = report
            .peak_native_voices
            .max(live.playing_per_channel().iter().sum());
        if index == events.len()
            && playing == 0
            && !gates.iter().any(|g| *g)
            && report.first_idle_frame.is_none()
        {
            report.first_idle_frame = Some(cursor);
        }
        let observation_finished = if until_idle {
            report.first_idle_frame.is_some() && cursor >= quantize(prepared.score_frames, q)?
        } else {
            cursor == terminal
        };
        if observation_finished {
            report.playing_lanes_at_end = playing;
            break;
        }
        let raw = synth.render(q)?;
        let mut sum_l = vec![0f64; q as usize];
        let mut sum_r = vec![0f64; q as usize];
        for lane in 0..groups {
            if owner_note[lane].is_none() {
                if raw[2 * lane]
                    .iter()
                    .chain(&raw[2 * lane + 1])
                    .any(|x| *x != 0.)
                {
                    return Err(AudioError::Native(
                        "Dry audio without a source owner".into(),
                    ));
                }
                continue;
            }
            let mut left: Vec<f64> = raw[2 * lane].iter().copied().map(f64::from).collect();
            let mut right: Vec<f64> = raw[2 * lane + 1].iter().copied().map(f64::from).collect();
            if let Some(note) = owner_note[lane] {
                report
                    .original_envelopes
                    .apply(note, cursor, &mut left, &mut right)?;
            } else if left.iter().chain(&right).any(|x| *x != 0.) {
                return Err(AudioError::Native(
                    "Dry audio without a source owner".into(),
                ));
            }
            for j in 0..q as usize {
                sum_l[j] += left[j];
                sum_r[j] += right[j];
            }
        }
        for (j, value) in sum_l.iter().zip(&sum_r).enumerate() {
            for sample in [value.0, value.1] {
                if !sample.is_finite() {
                    return Err(AudioError::Native("Nonfinite dry sum".into()));
                }
                if sample.abs() >= 1. {
                    return Err(AudioError::Clipping {
                        frame: cursor + j as u64,
                        peak: sample.abs() as f32,
                    });
                }
                report.sample_peak = report.sample_peak.max(sample.abs());
                squares += sample * sample;
            }
        }
        if let Some(output) = sink.as_deref_mut() {
            let mut interleaved = Vec::with_capacity(q as usize * 2);
            for (left, right) in sum_l.iter().zip(&sum_r) {
                for sample in [left, right] {
                    let value = *sample as f32;
                    if !value.is_finite() || value.abs() >= 1.0 {
                        return Err(AudioError::Clipping {
                            frame: cursor + interleaved.len() as u64 / 2,
                            peak: value.abs(),
                        });
                    }
                    interleaved.push(value);
                }
            }
            output.write(cursor, &interleaved)?;
        }
        cursor = cursor
            .checked_add(u64::from(q))
            .ok_or_else(|| invalid("Native tail observation frame overflow"))?;
    }
    if index != events.len() {
        return Err(invalid("Unconsumed dry plan events"));
    }
    report.completed_frames = cursor;
    report.sample_rms = (squares / (cursor.max(1) as f64 * 2.)).sqrt();
    Ok(report)
}

fn observation_value(value: &impl Serialize) -> Result<serde_json::Value> {
    let bytes =
        serde_json::to_vec(value).map_err(|_| invalid("Observation serialization failed"))?;
    serde_json::from_slice(&bytes).map_err(|_| invalid("Observation parsing failed"))
}

/// Explicit artifact path. The JSON is an expected observation, NOT an imported
/// authority token: original opaque plan/mixer/bank/native admissions are freshly
/// verified while replaying. No file is published unless the complete report,
/// including voice exhaustion and every event/preset/CC receipt, matches exactly.
#[allow(clippy::too_many_arguments)]
pub fn render_verified_wav(
    source: &str,
    plan: &TechniquePlaybackPlan,
    routes: &[RouteAssignment],
    bank_path: &Path,
    options: &RenderOptions,
    tail_policy: DryTailPolicy,
    pizzicato: PizzicatoSamplePolicy,
    cancel: &AtomicBool,
    expected: &serde_json::Value,
    output: &Path,
) -> Result<DryAdmissionReport> {
    render_verified_wav_with_gate(source, plan, routes, bank_path, options, tail_policy,
        pizzicato, cancel, expected, output, |pending| pending.commit())
}

/// Same verified replay, with a host-controlled final no-clobber publication gate.
#[allow(clippy::too_many_arguments)]
pub fn render_verified_wav_with_gate(
    source: &str,
    plan: &TechniquePlaybackPlan,
    routes: &[RouteAssignment],
    bank_path: &Path,
    options: &RenderOptions,
    tail_policy: DryTailPolicy,
    pizzicato: PizzicatoSamplePolicy,
    cancel: &AtomicBool,
    expected: &serde_json::Value,
    output: &Path,
    gate: impl FnOnce(crate::PendingWav<'_>) -> Result<crate::PublishedWav>,
) -> Result<DryAdmissionReport> {
    let frames = expected["completed_frames"]
        .as_u64()
        .ok_or_else(|| invalid("Missing observed frame extent"))?;
    let idle = expected["first_idle_frame"]
        .as_u64()
        .ok_or_else(|| invalid("No authoritative idle observation"))?;
    let schedule = DryEnvelopeSchedule::prepare(plan, options.sample_rate, tail_policy)?;
    if expected["observation_policy"] != "until_native_idle"
        || expected["playing_lanes_at_end"] != 0
        || frames < idle
        || expected["source_sha256"] != plan.source_sha256()
        || expected["plan_sha256"] != schedule.plan_sha256()
        || expected["admitted_notes"].as_u64() != Some(plan.plan().notes.len() as u64)
        || expected["noteoffs"].as_u64() != Some(plan.plan().notes.len() as u64)
        || expected["original_envelopes"] != observation_value(&schedule)?
    {
        return Err(invalid(
            "Readiness does not cover this exact complete dry plan",
        ));
    }
    crate::wav::render_with_gate(frames, options, cancel, output, |sink| {
        let report = probe_selected(
            source,
            plan,
            routes,
            bank_path,
            options,
            tail_policy,
            pizzicato,
            cancel,
            true,
            Some(sink),
        )?;
        let actual = observation_value(&report)?;
        if actual != *expected {
            return Err(AudioError::Native(
                "Dry replay observation differs; refusing artifact publication".into(),
            ));
        }
        Ok(report)
    }, gate)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn declared_quantization_never_adds_admission_delay() {
        assert_eq!(quantize(0, 64).unwrap(), 0);
        assert_eq!(quantize(1, 64).unwrap(), 64);
        assert_eq!(quantize(64, 64).unwrap(), 64);
        assert_eq!(quantize(65, 64).unwrap(), 128);
        assert!(quantize(u64::MAX, 64).is_err());
    }
}
