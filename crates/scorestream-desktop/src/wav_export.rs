//! WAV-only chooser and transactional file authority. SFS commands are separate.
use scorestream_app::{playback::{PlaybackPlan, plan_source}, wav::{WavLayout, WavStream, WavWriteError}};
use serde::Serialize;
use std::{collections::BTreeMap, io::ErrorKind, path::PathBuf, sync::{Arc, Mutex}};
use tauri::{State, WebviewWindow, ipc::{InvokeBody, Request}};

mod chooser;

const SESSION_HEADER: &str = "x-scorestream-wav-session";
const OFFSET_HEADER: &str = "x-scorestream-wav-offset";
const MAX_SOURCE_BYTES: usize = 4 * 1024 * 1024;
pub type WavExports = Arc<Mutex<Sessions>>;
type Result<T> = std::result::Result<T, WavError>;

/// Stable codes and path/source-free messages, suitable for frontend mapping.
#[derive(Debug, Serialize)]
pub struct WavError {
    code: &'static str,
    message: &'static str,
}
impl WavError {
    fn new(code: &'static str, message: &'static str) -> Self { Self { code, message } }
    fn io() -> Self { Self::new("WAV_IO", "The WAV temporary file could not be written or saved.") }
    fn bad_request() -> Self { Self::new("WAV_BAD_REQUEST", "The WAV stream does not match the export contract.") }
    fn missing() -> Self { Self::new("WAV_SESSION_NOT_FOUND", "This window has no matching WAV export.") }
    fn cancelled() -> Self { Self::new("WAV_CANCELLED", "The WAV export was cancelled before saving.") }
    #[cfg(any(target_os = "linux", test))]
    fn chooser() -> Self {
        Self::new("WAV_CHOOSER_FAILED", "The WAV save dialog could not select a local destination.")
    }
    fn exists() -> Self {
        Self::new("WAV_DESTINATION_EXISTS", "Choose a new filename. WAV export does not replace existing files.")
    }
    fn stream(error: WavWriteError) -> Self {
        match error { WavWriteError::Io => Self::io(), WavWriteError::Invalid(_) => Self::bad_request() }
    }
    fn cleanup() -> Self {
        Self::new("WAV_CLEANUP_FAILED", "A private WAV temporary file could not be removed.")
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StartedExport {
    session_id: String,
    path: String,
    #[serde(flatten)]
    layout: WavLayout,
    plan: PlaybackPlan,
}

struct FileSession {
    path: PathBuf,
    temporary: tempfile::NamedTempFile,
    stream: WavStream,
}
impl FileSession {
    fn create(path: PathBuf, layout: WavLayout) -> Result<Self> {
        // The renderer never supplies a path. Reject directories, files and even
        // dangling symlinks. persist_noclobber ALSO checks at the commit point.
        match std::fs::symlink_metadata(&path) {
            Ok(_) => return Err(WavError::exists()),
            Err(error) if error.kind() == ErrorKind::NotFound => (),
            Err(_) => return Err(WavError::io()),
        }
        if !path.extension().and_then(|value| value.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("wav"))
        {
            return Err(WavError::new("WAV_BAD_REQUEST", "Choose a destination ending in .wav."));
        }
        let parent = path.parent().ok_or_else(WavError::bad_request)?;
        let temporary = tempfile::Builder::new().prefix(".scorestream-wav-")
            .tempfile_in(parent).map_err(|_| WavError::io())?;
        Ok(Self { path, temporary, stream: WavStream::new(layout) })
    }

    fn discard(self) -> Result<()> {
        self.temporary.close().map_err(|_| WavError::cleanup())
    }

    fn finish(mut self) -> Result<String> {
        // Any failure before persist discards the private output. Never rewrite
        // the destination, patch header lengths, or pretend truncated PCM worked.
        let prepared = (|| {
            self.stream.finish(&mut self.temporary).map_err(WavError::stream)?;
            let length = self.temporary.as_file().metadata().map_err(|_| WavError::io())?.len();
            if length != u64::from(self.stream.layout().file_bytes()) {
                return Err(WavError::bad_request());
            }
            self.temporary.as_file().sync_all().map_err(|_| WavError::io())
        })();
        if let Err(error) = prepared {
            self.discard()?;
            return Err(error);
        }
        let Self { path, temporary, .. } = self;
        match temporary.persist_noclobber(&path) {
            Ok(_) => Ok(path.display().to_string()),
            Err(error) => {
                let reason = if error.error.kind() == ErrorKind::AlreadyExists {
                    WavError::exists()
                } else { WavError::io() };
                error.file.close().map_err(|_| WavError::cleanup())?;
                Err(reason)
            }
        }
    }
}

enum Phase { Choosing, Writing(FileSession) }
struct Slot { id: String, phase: Phase }
#[derive(Default)]
pub struct Sessions {
    next_id: u64,
    windows: BTreeMap<String, Slot>,
}
impl Sessions {
    fn reserve(&mut self, owner: &str) -> Result<String> {
        if self.windows.contains_key(owner) {
            return Err(WavError::new("WAV_BUSY", "This window already has a WAV export in progress."));
        }
        self.next_id = self.next_id.checked_add(1).ok_or_else(WavError::io)?;
        let id = format!("wav-{:016x}", self.next_id);
        self.windows.insert(owner.into(), Slot { id: id.clone(), phase: Phase::Choosing });
        Ok(id)
    }
    fn matches(&self, owner: &str, id: &str) -> bool {
        self.windows.get(owner).is_some_and(|slot| slot.id == id)
    }
    fn check(&self, owner: &str, id: &str) -> Result<()> {
        if self.matches(owner, id) { Ok(()) } else { Err(WavError::missing()) }
    }
    fn open(&mut self, owner: &str, id: &str, path: PathBuf, layout: WavLayout) -> Result<()> {
        self.check(owner, id)?;
        let slot = self.windows.get_mut(owner).ok_or_else(WavError::missing)?;
        if !matches!(slot.phase, Phase::Choosing) { return Err(WavError::bad_request()); }
        slot.phase = Phase::Writing(FileSession::create(path, layout)?);
        Ok(())
    }
    fn abort(&mut self, owner: &str, id: &str) -> Result<()> {
        // Stale aborts are no-ops, so a previous finally cannot kill a new export.
        if self.matches(owner, id) { self.remove_window(owner)?; }
        Ok(())
    }
    fn remove_window(&mut self, owner: &str) -> Result<()> {
        if let Some(Slot { phase: Phase::Writing(file), .. }) = self.windows.remove(owner) {
            file.discard()?;
        }
        Ok(())
    }
    fn write(&mut self, owner: &str, id: &str, offset: Result<u32>, bytes: Result<&[u8]>) -> Result<u32> {
        self.check(owner, id)?;
        let result = (|| {
            let offset = offset?;
            let bytes = bytes?;
            let slot = self.windows.get_mut(owner).ok_or_else(WavError::missing)?;
            let Phase::Writing(file) = &mut slot.phase else { return Err(WavError::bad_request()); };
            file.stream.write(&mut file.temporary, offset, bytes).map_err(WavError::stream)
        })();
        if result.is_err() { self.remove_window(owner)?; }
        result
    }
    fn finish(&mut self, owner: &str, id: &str) -> Result<String> {
        self.check(owner, id)?;
        let slot = self.windows.remove(owner).ok_or_else(WavError::missing)?;
        let Phase::Writing(file) = slot.phase else { return Err(WavError::bad_request()); };
        file.finish()
    }
}

/// This ID is a bounded generation discriminator, NOT a bearer capability.
/// Tauri injects the true calling WebviewWindow; a supplied window label is never
/// read. Every operation looks up that owner first, then compares its generation.
fn owner(window: &WebviewWindow) -> Result<String> {
    if window.label() != "main" {
        return Err(WavError::missing());
    }
    Ok(window.label().to_owned())
}
fn valid_id(id: &str) -> bool {
    id.len() == 20 && id.starts_with("wav-")
        && id.as_bytes()[4..].iter().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
}
fn header<'a>(request: &'a Request<'_>, name: &str, max: usize) -> Result<&'a str> {
    let values = request.headers().get_all(name);
    let mut values = values.iter();
    let value = values.next().ok_or_else(WavError::bad_request)?;
    if values.next().is_some() || value.as_bytes().is_empty() || value.as_bytes().len() > max {
        return Err(WavError::bad_request());
    }
    value.to_str().map_err(|_| WavError::bad_request())
}
fn offset(value: &str) -> Result<u32> {
    if value.is_empty() || value.len() > 8 || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        return Err(WavError::bad_request());
    }
    value.parse().map_err(|_| WavError::bad_request())
}

fn prepare(source: &str) -> Result<(PlaybackPlan, WavLayout)> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err(WavError::new("WAV_LIMIT_EXCEEDED", "The score exceeds the WAV export budget."));
    }
    let plan = plan_source(source).map_err(|_| WavError::new("WAV_SOURCE_INVALID", "The score cannot be rendered."))?;
    let layout = WavLayout::for_plan(&plan)
        .map_err(|_| WavError::new("WAV_LIMIT_EXCEEDED", "The score exceeds the WAV export budget."))?;
    Ok((plan, layout))
}

struct Reservation { state: WavExports, owner: String, id: String, armed: bool }
impl Drop for Reservation {
    fn drop(&mut self) {
        if self.armed {
            let mut sessions = self.state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            if sessions.abort(&self.owner, &self.id).is_err() {
                eprintln!("WAV temporary cleanup failed after an interrupted chooser.");
            }
        }
    }
}

#[tauri::command]
pub async fn wav_export_begin(source: String, window: WebviewWindow, state: State<'_, WavExports>) -> Result<Option<StartedExport>> {
    let owner = owner(&window)?;
    let state = state.inner().clone();
    let id = state.lock().map_err(|_| WavError::io())?.reserve(&owner)?;
    let mut reservation = Reservation { state: state.clone(), owner: owner.clone(), id: id.clone(), armed: true };
    // Full source planning and the PCM budget precede the native chooser. No
    // destination or file is authorized by merely supplying a plan or filename.
    let (plan, layout) = tauri::async_runtime::spawn_blocking(move || prepare(&source))
        .await.map_err(|_| WavError::io())??;
    if !state.lock().map_err(|_| WavError::io())?.matches(&owner, &id) {
        return Err(WavError::cancelled());
    }
    let chosen = chooser::destination(&window, &reservation).await;
    if !state.lock().map_err(|_| WavError::io())?.matches(&owner, &id) {
        return Err(WavError::cancelled());
    }
    let Some(path) = chosen? else { return Ok(None); };
    let display_path = path.display().to_string();
    // Window close/reload invalidates the reservation even while the OS chooser
    // is open. In that case no temporary file is created when it returns.
    {
        let mut sessions = state.lock().map_err(|_| WavError::io())?;
        if !sessions.matches(&owner, &id) { return Err(WavError::cancelled()); }
        sessions.open(&owner, &id, path, layout)?;
    }
    reservation.armed = false;
    Ok(Some(StartedExport { session_id: id, path: display_path, layout, plan }))
}

#[tauri::command]
pub fn wav_export_write(window: WebviewWindow, request: Request<'_>, state: State<'_, WavExports>) -> Result<u32> {
    let owner = owner(&window)?;
    let id = header(&request, SESSION_HEADER, 20)?;
    if !valid_id(id) { return Err(WavError::bad_request()); }
    let cursor = header(&request, OFFSET_HEADER, 8).and_then(offset);
    let bytes = match request.body() {
        InvokeBody::Raw(bytes) if !bytes.is_empty() && bytes.len() <= scorestream_app::wav::MAX_CHUNK_BYTES => Ok(bytes.as_slice()),
        _ => Err(WavError::bad_request()),
    };
    // Borrow the raw IPC payload. Never decode a JSON number array, clone a
    // whole file, spawn an unbounded chunk queue or accept renderer file paths.
    state.lock().map_err(|_| WavError::io())?.write(&owner, id, cursor, bytes)
}

#[tauri::command]
pub async fn wav_export_finish(session_id: String, window: WebviewWindow, state: State<'_, WavExports>) -> Result<String> {
    let owner = owner(&window)?;
    if !valid_id(&session_id) { return Err(WavError::bad_request()); }
    let state = state.inner().clone();
    // Keep the session lock until sync/persist settles. Abort and begin cannot
    // overtake commit; once finalization starts its success/failure wins.
    tauri::async_runtime::spawn_blocking(move || {
        state.lock().map_err(|_| WavError::io())?.finish(&owner, &session_id)
    }).await.map_err(|_| WavError::io())?
}

#[tauri::command]
pub fn wav_export_abort(session_id: String, window: WebviewWindow, state: State<'_, WavExports>) -> Result<()> {
    let owner = owner(&window)?;
    if !valid_id(&session_id) { return Err(WavError::bad_request()); }
    state.lock().map_err(|_| WavError::io())?.abort(&owner, &session_id)
}

/// Called on navigation/reload, actual destruction (not a cancellable close
/// request), and application exit. NamedTempFile also provides Drop cleanup.
pub fn discard_window(state: &WavExports, owner: &str) {
    let mut sessions = state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    if sessions.remove_window(owner).is_err() { eprintln!("WAV temporary cleanup failed on window teardown."); }
}
pub fn discard_all(state: &WavExports) {
    let mut sessions = state.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let windows: Vec<_> = sessions.windows.keys().cloned().collect();
    for owner in windows {
        if sessions.remove_window(&owner).is_err() { eprintln!("WAV temporary cleanup failed on app exit."); }
    }
}

#[cfg(test)]
mod tests;
