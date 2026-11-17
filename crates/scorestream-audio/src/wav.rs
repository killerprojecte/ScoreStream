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

/// Validated, synced private WAV awaiting one host-authorized no-clobber commit.
/// Dropping it removes the temporary file. There is no path override.
pub struct PendingWav<'a> {
    temporary: tempfile::NamedTempFile,
    output: &'a Path,
    identity: std::sync::Arc<()>,
}
/// Receipt obtainable only by committing the corresponding pending WAV.
pub struct PublishedWav { identity: std::sync::Arc<()> }
impl PendingWav<'_> {
    /// Call only inside the host's short owner/revision publication lock.
    pub fn commit(self) -> Result<PublishedWav> {
        self.temporary.persist_noclobber(self.output)
            .map_err(|failure| AudioError::Io(failure.error))?;
        Ok(PublishedWav { identity: self.identity })
    }
}

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
pub(crate) fn render_with<R>(
    frames: u64,
    options: &RenderOptions,
    cancel: &AtomicBool,
    output: &Path,
    render: impl FnOnce(&mut Sink<'_>) -> Result<R>,
) -> Result<R> {
    render_with_gate(frames, options, cancel, output, render, |pending| pending.commit())
}

pub(crate) fn render_with_gate<R>(
    frames: u64,
    options: &RenderOptions,
    cancel: &AtomicBool,
    output: &Path,
    render: impl FnOnce(&mut Sink<'_>) -> Result<R>,
    gate: impl FnOnce(PendingWav<'_>) -> Result<PublishedWav>,
) -> Result<R> {
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
    // Rendering, validation, flushing and sync above run OUTSIDE the host gate.
    // The host must synchronously return this commit's receipt; never retain the
    // pending file or return a later cancellation after a successful commit.
    let identity = std::sync::Arc::new(());
    let receipt = gate(PendingWav { temporary, output, identity: identity.clone() })?;
    if !std::sync::Arc::ptr_eq(&identity, &receipt.identity) {
        return Err(AudioError::Native("Publication receipt belongs to another WAV".into()));
    }
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

#[cfg(test)]
#[path = "../../scorestream-desktop/src/offline_export/lease.rs"]
mod desktop_lease;

#[cfg(test)]
mod gate_tests {
    use super::*;
    use super::desktop_lease::Leases;
    use std::sync::{Arc, Mutex, mpsc};

    #[test]
    fn invalidation_at_precommit_barrier_removes_real_temp_without_output() {
        for action in ["cancel", "edit", "reload"] {
            let directory = tempfile::tempdir().unwrap();
            let output = directory.path().join("out.wav");
            let mut leases = Leases::default();
            let work = leases.begin("main", "r1", "score").unwrap();
            leases.ready("main", &work, ()).unwrap();
            leases.start_render("main", &work).unwrap();
            let state = Arc::new(Mutex::new(leases));
            let (arrived, wait) = mpsc::channel();
            let (resume, go) = mpsc::channel();
            let worker_state = state.clone();
            let worker_work = work.clone();
            let result = std::thread::scope(|scope| {
                let output_ref = &output;
                let worker = scope.spawn(move || render_with_gate(1, &RenderOptions::default(),
                    worker_work.cancellation(), output_ref,
                    |sink| { assert!(worker_state.try_lock().is_ok()); sink.write(0, &[0.25, -0.25]) },
                    |pending| {
                        // Reached AFTER final writer cancel check and sync, before persist.
                        arrived.send(()).unwrap(); go.recv().unwrap();
                        worker_state.lock().unwrap().publish("main", &worker_work, || pending.commit())
                            .map_err(|_| AudioError::Cancelled)?
                    }));
                wait.recv().unwrap();
                assert!(!output.exists());
                assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
                let mut host = state.lock().unwrap();
                match action {
                    "cancel" => host.cancel("main", &work),
                    "edit" => host.observe_snapshot("main", "r2", "score"),
                    _ => host.close("main"),
                }
                drop(host); resume.send(()).unwrap();
                worker.join().unwrap()
            });
            assert!(matches!(result, Err(AudioError::Cancelled)));
            assert!(!output.exists());
            assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
        }
    }

    #[test]
    fn real_commit_wins_then_cancel_cannot_remove_artifact() {
        let directory = tempfile::tempdir().unwrap(); let output = directory.path().join("out.wav");
        let mut leases = Leases::default(); let work = leases.begin("main", "r1", "score").unwrap();
        leases.ready("main", &work, ()).unwrap(); leases.start_render("main", &work).unwrap();
        let state = Mutex::new(leases);
        render_with_gate(1, &RenderOptions::default(), work.cancellation(), &output,
            |sink| { assert!(state.try_lock().is_ok()); sink.write(0, &[0.25, -0.25]) },
            |pending| state.lock().unwrap().publish("main", &work, || pending.commit())
                .map_err(|_| AudioError::Cancelled)?).unwrap();
        let bytes = std::fs::read(&output).unwrap();
        assert_eq!(bytes.len(), 48); assert_eq!(&bytes[..4], b"RIFF");
        state.lock().unwrap().cancel("main", &work);
        state.lock().unwrap().observe_snapshot("main", "r2", "changed");
        assert_eq!(std::fs::read(&output).unwrap(), bytes);
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn destination_created_at_gate_is_never_overwritten() {
        let directory = tempfile::tempdir().unwrap(); let output = directory.path().join("out.wav");
        let result = render_with_gate(1, &RenderOptions::default(), &AtomicBool::new(false), &output,
            |sink| sink.write(0, &[0.0, 0.0]),
            |pending| { std::fs::write(&output, b"other owner").unwrap(); pending.commit() });
        assert!(matches!(result, Err(AudioError::Io(_))));
        assert_eq!(std::fs::read(&output).unwrap(), b"other owner");
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }
}
