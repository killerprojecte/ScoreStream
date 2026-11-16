use crate::{AudioError, RenderOptions, Result, RouteAssignment};
use scorestream_app::playback::PlaybackPlan;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
pub(crate) struct Note {
    pub channel: u16,
    pub key: u8,
    pub velocity: u8,
    pub start: u64,
    pub end: u64,
}
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) struct Event {
    pub frame: u64,
    pub on: bool,
    pub note: usize,
}
pub(crate) struct Prepared {
    pub score_frames: u64,
    pub frames: u64,
    pub notes: Vec<Note>,
    pub events: Vec<Event>,
}

fn invalid(message: impl Into<String>) -> AudioError {
    AudioError::InvalidPlan(message.into())
}
fn bounded(value: f64, low: f64, high: f64, name: &str) -> Result<f64> {
    if !value.is_finite() || !(low..=high).contains(&value) {
        return Err(invalid(format!("{name} outside [{low}, {high}]")));
    }
    Ok(value)
}
fn identity(value: &str) -> Result<()> {
    if value.is_empty() || value.len() > 1024 {
        return Err(invalid("Empty or excessive route/note identity"));
    }
    Ok(())
}
pub(crate) fn frame(seconds: f64, sample_rate: u32) -> Result<u64> {
    let value = (seconds * f64::from(sample_rate) + 0.5).floor();
    // Exact f64 integer domain, plus checked arithmetic below. No short product
    // duration cap; consumers such as RIFF WAV impose their own format limits.
    if !value.is_finite() || !(0.0..=9_007_199_254_740_991.0).contains(&value) {
        return Err(invalid(
            "Time cannot be represented as an exact frame index",
        ));
    }
    Ok(value as u64)
}

impl Prepared {
    pub fn new(
        plan: &PlaybackPlan,
        routes: &[RouteAssignment],
        options: &RenderOptions,
    ) -> Result<Self> {
        if plan
            .diagnostics
            .iter()
            .any(|d| d.code == "pizzicato.native_approximation_required")
        {
            return Err(AudioError::Unsupported(
                "Technique-bearing plan requires the opt-in pizzicato renderer and its receipt"
                    .into(),
            ));
        }
        Self::new_with_channels(plan, routes, options, &BTreeMap::new())
    }
    pub(crate) fn new_with_channels(
        plan: &PlaybackPlan,
        routes: &[RouteAssignment],
        options: &RenderOptions,
        channels: &BTreeMap<String, u16>,
    ) -> Result<Self> {
        if let Some(mix)=&options.local_midi_mix {
            mix.verify_routes(routes)?;
            if let Some(bank)=&options.bank_verification {mix.verify_bank(&bank.sha256)?;}
            if options.bank_verification.is_none(){return Err(invalid("Source-bound mixer requires explicit bank hash and exact preset-name verification"));}
        }
        if !(8000..=192_000).contains(&options.sample_rate)
            || !(1..=4096).contains(&options.block_frames)
        {
            return Err(invalid(
                "sample_rate must be 8000..=192000 and block_frames 1..=4096",
            ));
        }
        bounded(options.gain, 0.0001, 1.0, "gain")?;
        bounded(options.tail_seconds, 0.0, 600.0, "tail_seconds")?;
        if !(1..=65_535).contains(&options.polyphony)
            || options.max_notes == 0
            || options.max_soundfont_bytes < 12
        {
            return Err(invalid("Invalid resource budget"));
        }
        if plan.notes.len() > options.max_notes || routes.len() > 256 {
            return Err(invalid("Note or route budget exceeded"));
        }
        if let Some(expected) = &options.bank_verification {
            if expected.sha256.len() != 64
                || !expected.sha256.bytes().all(|b| b.is_ascii_hexdigit())
                || expected.preset_names.len() != routes.len()
                || expected
                    .preset_names
                    .iter()
                    .any(|name| name.is_empty() || name.len() > 4096 || name.contains('\0'))
            {
                return Err(invalid(
                    "Invalid bank digest or exact preset-name assertions",
                ));
            }
        }
        if !plan.duration_seconds.is_finite() || plan.duration_seconds < 0.0 {
            return Err(invalid("Invalid score duration"));
        }
        let score_frames = frame(plan.duration_seconds, options.sample_rate)?;
        let frames = score_frames
            .checked_add(frame(options.tail_seconds, options.sample_rate)?)
            .ok_or_else(|| invalid("Frame count overflow"))?;
        let mut route_map = BTreeMap::new();
        for (channel, route) in routes.iter().enumerate() {
            identity(&route.part)?;
            identity(&route.instrument)?;
            if route.program > 127 || route.bank > 16383 {
                return Err(invalid("Invalid explicit bank/program"));
            }
            if route_map
                .insert(
                    (route.part.as_str(), route.instrument.as_str()),
                    channel as u16,
                )
                .is_some()
            {
                return Err(invalid("Duplicate route assignment"));
            }
        }
        let declared: BTreeSet<_> = plan
            .routes
            .iter()
            .map(|r| (r.part.as_str(), r.instrument.as_str()))
            .collect();
        if declared.len() != plan.routes.len() {
            return Err(invalid("Duplicate declared plan route"));
        }
        let mut used = BTreeSet::new();
        let mut ids = BTreeSet::new();
        let mut notes = Vec::with_capacity(plan.notes.len());
        for note in &plan.notes {
            identity(&note.id)?;
            if !ids.insert(&note.id) {
                return Err(invalid("Duplicate note ID"));
            }
            let route = (note.part.as_str(), note.instrument.as_str());
            let channel = *route_map
                .get(&route)
                .ok_or_else(|| invalid(format!("No patch for {route:?}")))?;
            used.insert(route);
            let key = bounded(note.midi_note, 0.0, 127.0, "midi_note")?;
            if key.fract() != 0.0 {
                return Err(AudioError::Unsupported(
                    "Microtonal pitch; no rounding fallback".into(),
                ));
            }
            let expected = 440.0 * 2f64.powf((key - 69.0) / 12.0);
            if !note.frequency_hz.is_finite()
                || (note.frequency_hz - expected).abs() > expected * 1e-9
            {
                return Err(invalid("Frequency disagrees with MIDI key"));
            }
            if note.velocity == 0 || note.velocity > 127 {
                return Err(invalid("Velocity must be 1..=127"));
            }
            bounded(note.start_seconds, 0.0, plan.duration_seconds, "note start")?;
            bounded(
                note.duration_seconds,
                0.0,
                plan.duration_seconds,
                "note duration",
            )?;
            let end = note.start_seconds + note.duration_seconds;
            if note.duration_seconds <= 0.0 || end > plan.duration_seconds + 1e-9 {
                return Err(invalid("Note outside score duration"));
            }
            let start = frame(note.start_seconds, options.sample_rate)?;
            let end = frame(end.min(plan.duration_seconds), options.sample_rate)?;
            if end <= start {
                return Err(invalid("Note disappears at selected sample rate"));
            }
            let mut previous = 0.0;
            for point in &note.gain_envelope {
                bounded(
                    point.offset_seconds,
                    previous,
                    note.duration_seconds,
                    "envelope offset",
                )?;
                previous = point.offset_seconds;
                if point.gain != 1.0 {
                    return Err(AudioError::Unsupported("Per-note gain envelope; channel expression cannot preserve independent voices".into()));
                }
            }
            notes.push(Note {
                channel: channels.get(&note.id).copied().unwrap_or(channel),
                key: key as u8,
                velocity: note.velocity,
                start,
                end,
            });
        }
        let assigned: BTreeSet<_> = route_map.keys().copied().collect();
        if used != assigned || used != declared {
            return Err(invalid(
                "Declared, assigned and audible routes must match exactly",
            ));
        }
        let mut intervals: Vec<_> = notes.iter().collect();
        intervals.sort_by_key(|note| (note.start, note.end));
        let mut releases = BTreeMap::new();
        for note in intervals {
            let previous = releases.entry((note.channel, note.key)).or_insert(0);
            if note.start < *previous {
                return Err(AudioError::Unsupported(
                    "Overlapping same-key notes on one route".into(),
                ));
            }
            *previous = note.end;
        }
        let mut events = Vec::with_capacity(notes.len().saturating_mul(2));
        for (index, note) in notes.iter().enumerate() {
            events.push(Event {
                frame: note.start,
                on: true,
                note: index,
            });
            events.push(Event {
                frame: note.end,
                on: false,
                note: index,
            });
        }
        events.sort(); // false/off precedes true/on at a shared boundary.
        Ok(Self {
            score_frames,
            frames,
            notes,
            events,
        })
    }
}
