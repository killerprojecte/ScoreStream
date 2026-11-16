use crate::{
    AudioError, FluidSynthBackend, PcmSink, RenderOptions, RenderReport, Result, RouteAssignment,
    plan::Prepared,
};
use scorestream_app::playback::PlaybackPlan;
use std::{
    fs,
    io::{Seek, SeekFrom, Write},
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};

fn header(frames: u64, rate: u32) -> Result<[u8; 44]> {
    let bytes = frames
        .checked_mul(4)
        .filter(|v| *v <= u64::from(u32::MAX - 36))
        .ok_or_else(|| {
            AudioError::Unsupported(
                "Output exceeds standard RIFF WAV size; RF64 is not implemented".into(),
            )
        })? as u32;
    let mut result = [0; 44];
    result[0..4].copy_from_slice(b"RIFF");
    result[4..8].copy_from_slice(&(bytes + 36).to_le_bytes());
    result[8..16].copy_from_slice(b"WAVEfmt ");
    result[16..20].copy_from_slice(&16_u32.to_le_bytes());
    result[20..22].copy_from_slice(&1_u16.to_le_bytes());
    result[22..24].copy_from_slice(&2_u16.to_le_bytes());
    result[24..28].copy_from_slice(&rate.to_le_bytes());
    result[28..32].copy_from_slice(&(rate * 4).to_le_bytes());
    result[32..34].copy_from_slice(&4_u16.to_le_bytes());
    result[34..36].copy_from_slice(&16_u16.to_le_bytes());
    result[36..40].copy_from_slice(b"data");
    result[40..44].copy_from_slice(&bytes.to_le_bytes());
    Ok(result)
}
pub(crate) struct Sink<'a> {
    file: &'a mut fs::File,
    frame: u64,
    bytes: Vec<u8>,
}
impl PcmSink for Sink<'_> {
    fn write(&mut self, start_frame: u64, samples: &[f32]) -> Result<()> {
        if start_frame != self.frame || samples.len() % 2 != 0 {
            return Err(AudioError::Native("Discontinuous stereo PCM stream".into()));
        }
        self.bytes.clear();
        for &sample in samples {
            // render_pcm rejects clipping and nonfinite data before this sink.
            let multiplier = if sample < 0.0 { 32768.0 } else { 32767.0 };
            self.bytes.extend_from_slice(
                &((f64::from(sample) * multiplier).round() as i16).to_le_bytes(),
            );
        }
        self.file.write_all(&self.bytes)?;
        self.frame += (samples.len() / 2) as u64;
        Ok(())
    }
}

pub(crate) fn render(
    backend: &FluidSynthBackend,
    plan: &PlaybackPlan,
    routes: &[RouteAssignment],
    bank: &Path,
    options: &RenderOptions,
    cancel: &AtomicBool,
    output: &Path,
) -> Result<RenderReport> {
    let prepared = Prepared::new(plan, routes, options)?;
    render_with(prepared.frames, options, cancel, output, |sink| {
        backend.render_pcm(plan, routes, bank, options, cancel, sink)
    })
}
pub(crate) fn render_with(
    frames: u64,
    options: &RenderOptions,
    cancel: &AtomicBool,
    output: &Path,
    render: impl FnOnce(&mut Sink<'_>) -> Result<RenderReport>,
) -> Result<RenderReport> {
    let expected_header = header(frames, options.sample_rate)?;
    if cancel.load(Ordering::Relaxed) {
        return Err(AudioError::Cancelled);
    }
    match fs::symlink_metadata(output) {
        Ok(_) => {
            return Err(AudioError::Io(std::io::Error::new(
                std::io::ErrorKind::AlreadyExists,
                "Refusing to overwrite a file or symlink",
            )));
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut temporary = tempfile::Builder::new()
        .prefix(".scorestream-audio-")
        .tempfile_in(parent)?;
    temporary.write_all(&expected_header)?;
    let mut sink = Sink {
        file: temporary.as_file_mut(),
        frame: 0,
        bytes: Vec::with_capacity(options.block_frames * 4),
    };
    let report = render(&mut sink)?;
    if sink.frame != frames {
        return Err(AudioError::Native("Incomplete PCM output".into()));
    }
    temporary.flush()?;
    temporary.as_file_mut().sync_all()?;
    let actual_size = temporary.seek(SeekFrom::End(0))?;
    if actual_size != 44 + frames * 4 {
        return Err(AudioError::Native("WAV byte count mismatch".into()));
    }
    if cancel.load(Ordering::Relaxed) {
        return Err(AudioError::Cancelled);
    }
    // tempfile uses no-clobber publication. On failure its retained TempPath is
    // dropped and removed; existing/concurrently-created destination is untouched.
    // Cancellation is no longer accepted once publication begins.
    temporary
        .persist_noclobber(output)
        .map_err(|failure| AudioError::Io(failure.error))?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn header_is_exact_and_format_limited_not_duration_limited() {
        let result = header(48_000 * 3600, 48_000).unwrap();
        assert_eq!(&result[..4], b"RIFF");
        assert_eq!(
            u32::from_le_bytes(result[40..44].try_into().unwrap()),
            48_000 * 3600 * 4
        );
        assert!(header(u64::from(u32::MAX), 48_000).is_err());
    }
}
