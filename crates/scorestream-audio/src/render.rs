use crate::{
    AudioError, BackendCapabilities, PcmSink, RenderOptions, RenderReport, RenderedRoute, Result,
    RouteAssignment,
    native::{Api, Bank, Synth},
    plan::Prepared,
};
use scorestream_app::playback::PlaybackPlan;
use std::{
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

/// Linux installed-library adapter. Thread-bound by its native library owner.
/// It opens no audio device and never starts MIDI/audio driver threads.
pub struct FluidSynthBackend {
    api: Api,
}

fn cancelled(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::Relaxed) {
        Err(AudioError::Cancelled)
    } else {
        Ok(())
    }
}

impl FluidSynthBackend {
    pub fn open_installed() -> Result<Self> {
        Ok(Self { api: Api::open()? })
    }
    pub fn capabilities(&self) -> &BackendCapabilities {
        &self.api.capabilities
    }

    pub fn render_pcm(
        &self,
        plan: &PlaybackPlan,
        routes: &[RouteAssignment],
        bank_path: &Path,
        options: &RenderOptions,
        cancel: &AtomicBool,
        sink: &mut impl PcmSink,
    ) -> Result<RenderReport> {
        let prepared = Prepared::new(plan, routes, options)?;
        self.render_prepared(plan, routes, bank_path, options, cancel, sink, prepared)
    }
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_prepared(
        &self,
        plan: &PlaybackPlan,
        routes: &[RouteAssignment],
        bank_path: &Path,
        options: &RenderOptions,
        cancel: &AtomicBool,
        sink: &mut impl PcmSink,
        prepared: Prepared,
    ) -> Result<RenderReport> {
        cancelled(cancel)?;
        let mut bank = Bank::open(bank_path, options, cancel)?;
        if options
            .bank_verification
            .as_ref()
            .is_some_and(|expected| !bank.hash.eq_ignore_ascii_case(&expected.sha256))
        {
            return Err(AudioError::InvalidPlan(
                "Selected bank bytes do not match the explicitly assigned SHA-256".into(),
            ));
        }
        cancelled(cancel)?;
        let mut synth = Synth::new(&self.api, &mut bank, routes, options, cancel)?;
        if options
            .bank_verification
            .as_ref()
            .is_some_and(|expected| synth.names != expected.preset_names)
        {
            return Err(AudioError::InvalidPlan(
                "Selected native preset names do not match the explicit sample profile".into(),
            ));
        }
        cancelled(cancel)?;
        let mut report = RenderReport {
            backend: self.api.capabilities.clone(),
            soundfont_path: bank.path,
            soundfont_sha256: bank.hash,
            sample_rate: options.sample_rate,
            score_frames: prepared.score_frames,
            frames: prepared.frames,
            note_count: prepared.notes.len(),
            local_midi_mix: options.local_midi_mix.clone(),
            routes: routes
                .iter()
                .enumerate()
                .map(|(index, route)| RenderedRoute {
                    assignment: route.clone(),
                    native_channel: index as u16,
                    preset_name: synth.names[index].clone(),
                })
                .collect(),
            internal_quantum_frames: synth.quantum,
            block_frames: options.block_frames,
            gain: options.gain,
            native_polyphony_limit: options.polyphony,
            peak_native_voices: 0,
            noteoffs_without_live_voice: 0,
            peak: 0.0,
            rms: 0.0,
            final_50ms_peak: 0.0,
            tail_may_be_truncated: false,
            warnings: plan.warnings.clone(),
            diagnostics: plan.diagnostics.clone(),
        };
        report.warnings.extend([
            "Native event response is quantized by FluidSynth's reported internal block size; not sample-accurate.".into(),
            "A fixed release tail is used. Its final-level check does not prove every patch tail is complete.".into(),
            "Native voice-count saturation fails rendering; this is not an exhaustive proof against all layered-patch voice stealing.".into(),
            "Consumes already-realized score note gates; no additional pedal, slur, fermata or articulation interpretation.".into(),
        ]);
        let mut cursor = 0_u64;
        let mut index = 0_usize;
        let mut square_sum = 0_f64;
        let mut buffer = vec![0_f32; options.block_frames * 2];
        let tail_start = prepared
            .frames
            .saturating_sub(u64::from(options.sample_rate / 20));
        while cursor <= prepared.frames {
            cancelled(cancel)?;
            while index < prepared.events.len() && prepared.events[index].frame == cursor {
                let event = &prepared.events[index];
                report.noteoffs_without_live_voice +=
                    u64::from(synth.event(&prepared.notes[event.note], event.on)?);
                Self::check_voices(&synth, options, &mut report)?;
                index += 1;
            }
            if cursor == prepared.frames {
                break;
            }
            let mut end = prepared.frames.min(cursor + options.block_frames as u64);
            if index < prepared.events.len() {
                end = end.min(prepared.events[index].frame);
            }
            let frames = usize::try_from(end - cursor).expect("bounded PCM block");
            let samples = &mut buffer[..frames * 2];
            synth.render(samples)?;
            Self::check_voices(&synth, options, &mut report)?;
            for (offset, value) in samples.iter().enumerate() {
                if !value.is_finite() {
                    return Err(AudioError::Native(
                        "Nonfinite PCM from native engine".into(),
                    ));
                }
                let peak = value.abs();
                if peak >= 1.0 {
                    return Err(AudioError::Clipping {
                        frame: cursor + (offset / 2) as u64,
                        peak,
                    });
                }
                report.peak = report.peak.max(peak);
                square_sum += f64::from(*value).powi(2);
                if cursor + (offset / 2) as u64 >= tail_start {
                    report.final_50ms_peak = report.final_50ms_peak.max(peak);
                }
            }
            cancelled(cancel)?;
            sink.write(cursor, samples)?;
            cursor = end;
        }
        cancelled(cancel)?;
        report.rms = (square_sum / (prepared.frames.max(1) as f64 * 2.0)).sqrt();
        report.tail_may_be_truncated = report.final_50ms_peak > 0.0001;
        Ok(report)
    }

    fn check_voices(
        synth: &Synth<'_>,
        options: &RenderOptions,
        report: &mut RenderReport,
    ) -> Result<()> {
        let voices = synth.voices()?;
        report.peak_native_voices = report.peak_native_voices.max(voices);
        if voices >= options.polyphony {
            return Err(AudioError::PolyphonyLimit {
                voices,
                limit: options.polyphony,
            });
        }
        Ok(())
    }

    pub fn render_wav(
        &self,
        plan: &PlaybackPlan,
        routes: &[RouteAssignment],
        bank: &Path,
        options: &RenderOptions,
        cancel: &AtomicBool,
        output: &Path,
    ) -> Result<RenderReport> {
        crate::wav::render(self, plan, routes, bank, options, cancel, output)
    }
}
