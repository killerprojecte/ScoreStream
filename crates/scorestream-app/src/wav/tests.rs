use super::*;
use crate::playback::{EnvelopePoint, PlaybackNote};
use std::io;

fn plan() -> PlaybackPlan {
    PlaybackPlan {
        duration_seconds: 1.0,
        notes: vec![PlaybackNote {
            id: "note".into(), written_id: None, part: "part".into(), voice: "voice".into(), instrument: "piano".into(),
            start_seconds: 0.25, duration_seconds: 0.75, midi_note: 69.0,
            frequency_hz: 440.0, velocity: 64,
            gain_envelope: vec![EnvelopePoint { offset_seconds: 0.0, gain: 1.0 }],
        }],
        event_positions: vec![], measures: vec![], routes: vec![], warnings: vec![], diagnostics: vec![],
    }
}

#[test]
fn plan_budget_and_camel_case_metadata_match_frontend() {
    let layout = WavLayout::for_plan(&plan()).unwrap();
    assert_eq!(layout.frame_count(), 60_000);
    assert_eq!(layout.data_bytes(), 240_000);
    assert_eq!(layout.file_bytes(), 240_044);
    assert_eq!(serde_json::to_value(layout).unwrap(), serde_json::json!({
        "frameCount": 60_000, "dataBytes": 240_000, "fileBytes": 240_044,
    }));
    let mut score = plan();
    score.duration_seconds = MAX_SCORE_SECONDS;
    assert_eq!(WavLayout::for_plan(&score).unwrap().frame_count(), MAX_FRAMES);
    score.duration_seconds += 0.001;
    assert!(WavLayout::for_plan(&score).is_err());
    score.duration_seconds = 1.0 / SAMPLE_RATE as f64;
    score.notes.clear();
    assert_eq!(WavLayout::for_plan(&score).unwrap().frame_count(), TAIL_FRAMES + 1);
}

#[test]
fn work_limits_are_total_not_polyphony() {
    let mut score = plan();
    score.notes = vec![score.notes[0].clone(); MAX_NOTES];
    assert!(WavLayout::for_plan(&score).is_ok());
    score.notes.push(score.notes[0].clone());
    assert!(WavLayout::for_plan(&score).is_err());
    let mut score = plan();
    score.notes[0].gain_envelope = vec![EnvelopePoint { offset_seconds: 0.0, gain: 1.0 }; MAX_EXPRESSION_POINTS];
    assert!(WavLayout::for_plan(&score).is_ok());
    score.notes[0].gain_envelope.push(EnvelopePoint { offset_seconds: 0.0, gain: 1.0 });
    assert!(WavLayout::for_plan(&score).is_err());
}

#[test]
fn invalid_audio_fields_and_order_are_rejected() {
    for duration in [0.0, -1.0, f64::INFINITY, f64::NEG_INFINITY, f64::NAN] {
        let mut score = plan();
        score.duration_seconds = duration;
        assert!(WavLayout::for_plan(&score).is_err());
    }
    for pitch in [-1.0, 128.0, f64::NAN] {
        let mut score = plan(); score.notes[0].midi_note = pitch;
        assert!(WavLayout::for_plan(&score).is_err());
    }
    for frequency in [0.0, 24_000.0, f64::INFINITY] {
        let mut score = plan(); score.notes[0].frequency_hz = frequency;
        assert!(WavLayout::for_plan(&score).is_err());
    }
    for velocity in [0, 128, 255] {
        let mut score = plan(); score.notes[0].velocity = velocity;
        assert!(WavLayout::for_plan(&score).is_err());
    }
    for duration in [0.0, -0.01, 0.7501, f64::NAN] {
        let mut score = plan(); score.notes[0].duration_seconds = duration;
        assert!(WavLayout::for_plan(&score).is_err());
    }
    for start in [-0.1, 0.5, f64::NAN] {
        let mut score = plan(); score.notes[0].start_seconds = start;
        assert!(WavLayout::for_plan(&score).is_err());
    }
    let mut score = plan();
    let mut second = score.notes[0].clone(); second.start_seconds = 0.0;
    score.notes.push(second);
    assert!(WavLayout::for_plan(&score).is_err());
    let mut score = plan();
    score.notes[0].gain_envelope = vec![
        EnvelopePoint { offset_seconds: 0.5, gain: 2.0 },
        EnvelopePoint { offset_seconds: 0.25, gain: 2.0 },
    ];
    assert!(WavLayout::for_plan(&score).is_err());
    for point in [
        EnvelopePoint { offset_seconds: 0.751, gain: 1.0 },
        EnvelopePoint { offset_seconds: 0.0, gain: 128.0 },
        EnvelopePoint { offset_seconds: f64::NAN, gain: 1.0 },
    ] {
        let mut score = plan(); score.notes[0].gain_envelope = vec![point];
        assert!(WavLayout::for_plan(&score).is_err());
    }
}

#[test]
fn fractional_pitch_relative_gains_and_roundoff_are_allowed() {
    let mut score = plan();
    score.duration_seconds = 1.1;
    score.notes[0].midi_note = 69.5;
    score.notes[0].start_seconds = 0.3;
    score.notes[0].duration_seconds = 0.8;
    score.notes[0].gain_envelope = vec![
        EnvelopePoint { offset_seconds: 0.8, gain: 127.0 },
        EnvelopePoint { offset_seconds: 0.8, gain: 2.0 },
    ];
    assert!(WavLayout::for_plan(&score).is_ok());
    score.notes[0].duration_seconds += 1.0 / SAMPLE_RATE as f64;
    assert!(WavLayout::for_plan(&score).is_err());
}

#[test]
fn header_is_exact_and_frames_cannot_overflow() {
    let layout = WavLayout::for_frames(2).unwrap();
    assert_eq!(layout.header(), [
        82,73,70,70,44,0,0,0,87,65,86,69,102,109,116,32,
        16,0,0,0,1,0,2,0,128,187,0,0,0,238,2,0,4,0,16,0,
        100,97,116,97,8,0,0,0,
    ]);
    assert!(WavLayout::for_frames(MAX_FRAMES).is_ok());
    assert!(WavLayout::for_frames(MAX_FRAMES + 1).is_err());
    assert!(WavLayout::for_frames(u32::MAX).is_err());
}

#[test]
fn sequential_full_and_partial_chunks_finish_exactly() {
    let layout = WavLayout::for_frames((MAX_CHUNK_BYTES / 4 + 2) as u32).unwrap();
    let mut stream = WavStream::new(layout);
    let mut target = Vec::new();
    assert_eq!(stream.write(&mut target, 0, &layout.header()).unwrap(), 44);
    let chunk = vec![7; MAX_CHUNK_BYTES];
    let offset = stream.write(&mut target, 44, &chunk).unwrap();
    assert_eq!(stream.write(&mut target, offset, &[1,2,3,4,5,6,7,8]).unwrap(), layout.file_bytes());
    stream.finish(&mut target).unwrap();
    assert_eq!(target.len(), layout.file_bytes() as usize);
    assert_eq!(&target[..44], &layout.header());
    assert_eq!(&target[44..44 + MAX_CHUNK_BYTES], &chunk);
    assert!(stream.write(&mut target, layout.file_bytes(), &[0;4]).is_err());
    assert!(stream.finish(&mut target).is_err());
}

#[test]
fn malformed_headers_mono_wrong_format_and_wrong_sizes_never_write() {
    let layout = WavLayout::for_frames(2).unwrap();
    // Every header byte is authoritative, not just its magic.
    for index in 0..HEADER_BYTES as usize {
        let mut bytes = layout.header(); bytes[index] ^= 1;
        let mut stream = WavStream::new(layout); let mut target = Vec::new();
        assert!(stream.write(&mut target, 0, &bytes).is_err());
        assert!(target.is_empty());
        assert!(stream.write(&mut target, 0, &layout.header()).is_err());
    }
    for (start, replacement) in [
        (20, 3_u16.to_le_bytes().to_vec()), // IEEE float, not PCM.
        (22, 1_u16.to_le_bytes().to_vec()), // Mono is not this stereo profile.
        (24, 44_100_u32.to_le_bytes().to_vec()),
        (34, 32_u16.to_le_bytes().to_vec()),
    ] {
        let mut bytes = layout.header();
        bytes[start..start + replacement.len()].copy_from_slice(&replacement);
        let mut stream = WavStream::new(layout); let mut target = Vec::new();
        assert!(stream.write(&mut target, 0, &bytes).is_err());
        assert!(target.is_empty());
    }
}

#[test]
fn replay_gaps_unaligned_oversized_overrun_and_truncation_poison_the_stream() {
    let layout = WavLayout::for_frames(2).unwrap();
    for (offset, bytes) in [
        (0, vec![0;4]), (48, vec![0;4]), (44, vec![0;3]),
        (44, vec![]), (44, vec![0;MAX_CHUNK_BYTES + 4]), (44, vec![0;12]),
    ] {
        let mut stream = WavStream::new(layout); let mut target = Vec::new();
        stream.write(&mut target, 0, &layout.header()).unwrap();
        assert!(stream.write(&mut target, offset, &bytes).is_err());
        assert_eq!(target.len(), 44);
        assert!(stream.write(&mut target, 44, &[0;8]).is_err());
        assert!(stream.finish(&mut target).is_err());
    }
    let mut stream = WavStream::new(layout); let mut target = Vec::new();
    assert!(stream.finish(&mut target).is_err());
    let mut stream = WavStream::new(layout);
    stream.write(&mut target, 0, &layout.header()).unwrap();
    stream.write(&mut target, 44, &[0;4]).unwrap();
    assert!(stream.finish(&mut target).is_err());
}

struct BrokenWriter { accepted: usize, fail_after: usize, fail_flush: bool }
impl Write for BrokenWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.accepted == self.fail_after { return Err(io::Error::other("injected write failure")); }
        let count = bytes.len().min(self.fail_after - self.accepted);
        self.accepted += count;
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        if self.fail_flush { Err(io::Error::other("injected flush failure")) } else { Ok(()) }
    }
}
#[test]
fn partial_io_and_flush_errors_cannot_finish_or_resume() {
    let layout = WavLayout::for_frames(2).unwrap();
    let mut stream = WavStream::new(layout);
    let mut writer = BrokenWriter { accepted: 0, fail_after: 46, fail_flush: false };
    stream.write(&mut writer, 0, &layout.header()).unwrap();
    assert!(stream.write(&mut writer, 44, &[0;8]).is_err());
    assert_eq!(writer.accepted, 46);
    writer.fail_after = usize::MAX;
    assert!(stream.write(&mut writer, 44, &[0;8]).is_err());
    assert!(stream.finish(&mut writer).is_err());
    let mut stream = WavStream::new(layout);
    let mut writer = BrokenWriter { accepted: 0, fail_after: usize::MAX, fail_flush: true };
    stream.write(&mut writer, 0, &layout.header()).unwrap();
    stream.write(&mut writer, 44, &[0;8]).unwrap();
    assert!(stream.finish(&mut writer).is_err());
    writer.fail_flush = false;
    assert!(stream.finish(&mut writer).is_err());
}
