//! Fixed-profile WAV validation and a sequential streaming writer.
//! No chooser, paths, audio rendering, platform APIs or new dependencies.
use crate::playback::PlaybackPlan;
use serde::Serialize;
use std::{fmt, io::Write};

pub const SAMPLE_RATE: u32 = 48_000;
pub const CHANNELS: u16 = 2;
pub const BITS_PER_SAMPLE: u16 = 16;
pub const BYTES_PER_FRAME: u32 = 4;
pub const HEADER_BYTES: u32 = 44;
pub const MAX_CHUNK_BYTES: usize = 64 * 1024;
pub const MAX_SCORE_SECONDS: f64 = 120.0;
pub const TAIL_FRAMES: u32 = 12_000;
pub const MAX_FRAMES: u32 = 120 * SAMPLE_RATE + TAIL_FRAMES;
pub const MAX_NOTES: usize = 4_096;
pub const MAX_EXPRESSION_POINTS: usize = 32_768;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WavLayout {
    frame_count: u32,
    data_bytes: u32,
    file_bytes: u32,
}

fn bounded(value: f64, min: f64, max: f64) -> bool {
    value.is_finite() && value >= min && value <= max
}

impl WavLayout {
    /// Same fixed budget as desktop/src/playback/pcm.ts, before any file/graph.
    pub fn for_plan(plan: &PlaybackPlan) -> Result<Self, String> {
        let duration = plan.duration_seconds;
        if !bounded(duration, 0.0, MAX_SCORE_SECONDS) || duration == 0.0 {
            return Err("WAV score duration must be positive and at most 120 seconds".into());
        }
        if plan.notes.len() > MAX_NOTES {
            return Err("WAV export accepts at most 4096 note voices".into());
        }
        let mut points = 0_usize;
        for note in &plan.notes {
            points = points.checked_add(note.gain_envelope.len())
                .filter(|count| *count <= MAX_EXPRESSION_POINTS)
                .ok_or("WAV export accepts at most 32768 expression points")?;
        }
        let tolerance = 4.0 * f64::EPSILON * duration.max(1.0);
        let mut previous_start = 0.0;
        for note in &plan.notes {
            if !bounded(note.start_seconds, previous_start, duration)
                || !bounded(note.duration_seconds, 0.0, duration)
                || note.duration_seconds == 0.0
                || note.start_seconds + note.duration_seconds > duration + tolerance
                || !bounded(note.midi_note, 0.0, 127.0)
                || !bounded(note.frequency_hz, 0.0, SAMPLE_RATE as f64 / 2.0)
                || note.frequency_hz == 0.0
                || note.frequency_hz == SAMPLE_RATE as f64 / 2.0
                || !(1..=127).contains(&note.velocity)
            {
                return Err("WAV note has invalid timing, pitch, frequency or velocity".into());
            }
            previous_start = note.start_seconds;
            let mut previous_offset = 0.0;
            for point in &note.gain_envelope {
                if !bounded(point.offset_seconds, previous_offset, note.duration_seconds + tolerance)
                    || !bounded(point.gain, 0.0, 127.0)
                {
                    return Err("WAV expression point is outside the audio budget".into());
                }
                previous_offset = point.offset_seconds;
            }
        }
        Self::for_frames((duration * SAMPLE_RATE as f64).ceil() as u32 + TAIL_FRAMES)
    }

    /// Zero frames is a valid primitive RIFF; source-based export requires time.
    pub fn for_frames(frame_count: u32) -> Result<Self, String> {
        if frame_count > MAX_FRAMES {
            return Err("WAV frame count exceeds the fixed export budget".into());
        }
        let data_bytes = frame_count.checked_mul(BYTES_PER_FRAME)
            .ok_or("WAV data size overflow")?;
        let file_bytes = data_bytes.checked_add(HEADER_BYTES)
            .ok_or("WAV file size overflow")?;
        Ok(Self { frame_count, data_bytes, file_bytes })
    }

    pub fn frame_count(self) -> u32 { self.frame_count }
    pub fn data_bytes(self) -> u32 { self.data_bytes }
    pub fn file_bytes(self) -> u32 { self.file_bytes }

    /// Canonical RIFF/WAVE format 1, fixed stereo PCM16 LE at 48 kHz.
    pub fn header(self) -> [u8; HEADER_BYTES as usize] {
        let mut bytes = [0; HEADER_BYTES as usize];
        bytes[0..4].copy_from_slice(b"RIFF");
        bytes[4..8].copy_from_slice(&(self.file_bytes - 8).to_le_bytes());
        bytes[8..12].copy_from_slice(b"WAVE");
        bytes[12..16].copy_from_slice(b"fmt ");
        bytes[16..20].copy_from_slice(&16_u32.to_le_bytes());
        bytes[20..22].copy_from_slice(&1_u16.to_le_bytes());
        bytes[22..24].copy_from_slice(&CHANNELS.to_le_bytes());
        bytes[24..28].copy_from_slice(&SAMPLE_RATE.to_le_bytes());
        bytes[28..32].copy_from_slice(&(SAMPLE_RATE * BYTES_PER_FRAME).to_le_bytes());
        bytes[32..34].copy_from_slice(&(BYTES_PER_FRAME as u16).to_le_bytes());
        bytes[34..36].copy_from_slice(&BITS_PER_SAMPLE.to_le_bytes());
        bytes[36..40].copy_from_slice(b"data");
        bytes[40..44].copy_from_slice(&self.data_bytes.to_le_bytes());
        bytes
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum WavWriteError {
    Invalid(&'static str),
    Io,
}
impl fmt::Display for WavWriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(message) => formatter.write_str(message),
            Self::Io => formatter.write_str("WAV temporary write or flush failed"),
        }
    }
}
impl std::error::Error for WavWriteError {}

/// Fail-closed write protocol. The owner must discard its temporary target after
/// ANY error. No seeking, payload buffering, retry/replay or header rewriting.
#[derive(Debug)]
pub struct WavStream {
    layout: WavLayout,
    cursor: u32,
    failed: bool,
    finished: bool,
}
impl WavStream {
    pub fn new(layout: WavLayout) -> Self {
        Self { layout, cursor: 0, failed: false, finished: false }
    }
    pub fn layout(&self) -> WavLayout { self.layout }

    pub fn write(&mut self, target: &mut impl Write, offset: u32, bytes: &[u8]) -> Result<u32, WavWriteError> {
        if self.failed || self.finished {
            return Err(WavWriteError::Invalid("WAV stream is no longer writable"));
        }
        // Leave the stream poisoned on any validation error or partial I/O.
        self.failed = true;
        if offset != self.cursor {
            return Err(WavWriteError::Invalid("WAV chunks must be sequential and cannot be replayed"));
        }
        if bytes.is_empty() || bytes.len() > MAX_CHUNK_BYTES {
            return Err(WavWriteError::Invalid("WAV chunks must contain 1..65536 raw bytes"));
        }
        if self.cursor == 0 {
            if bytes != self.layout.header().as_slice() {
                return Err(WavWriteError::Invalid("WAV header must exactly match the planned stereo PCM16 layout"));
            }
        } else if bytes.len() % BYTES_PER_FRAME as usize != 0 {
            return Err(WavWriteError::Invalid("WAV payload chunks must contain whole stereo frames"));
        }
        let next = self.cursor.checked_add(bytes.len() as u32)
            .filter(|end| *end <= self.layout.file_bytes)
            .ok_or(WavWriteError::Invalid("WAV chunk exceeds the planned byte count"))?;
        target.write_all(bytes).map_err(|_| WavWriteError::Io)?;
        self.cursor = next;
        self.failed = false;
        Ok(next)
    }

    /// Only validates/flushes. The platform adapter must sync and publish after
    /// this succeeds, or discard the private output if finalization fails.
    pub fn finish(&mut self, target: &mut impl Write) -> Result<(), WavWriteError> {
        if self.failed || self.finished || self.cursor != self.layout.file_bytes {
            self.failed = true;
            return Err(WavWriteError::Invalid("WAV cannot finish before its exact planned byte count"));
        }
        self.failed = true;
        target.flush().map_err(|_| WavWriteError::Io)?;
        self.finished = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
