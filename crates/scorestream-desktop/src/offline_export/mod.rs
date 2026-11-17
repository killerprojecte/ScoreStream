//! Linux-only host state. IPC never accepts owner labels or filesystem paths.
pub mod lease;

use std::sync::{Arc, Mutex};
use tauri::{Manager, WebviewWindow};

mod state;
mod errors;
use state::Host;
pub type Exports = Arc<Mutex<Host>>;
const OWNER: &str = "main";
fn identity(current: &scorestream_app::SourceRevision<'_>) -> String {
    // Also catches same-byte apply/preview through the shared MCP authority.
    format!("{}:{}", current.revision_id, current.draft_seq)
}
fn owner(window: &WebviewWindow) -> Result<(), String> {
    if window.label() == OWNER { Ok(()) } else { Err("OFFLINE_OWNER".into()) }
}
/// Call with the workspace mutex already held, before any source mutation.
/// Publication uses the same workspace -> exports lock order.
pub fn invalidate(app: &tauri::AppHandle) {
    if let Some(state) = app.try_state::<Exports>() {
        if let Ok(mut host) = state.lock() { host.close(); }
    }
}
pub fn discard_window(app: &tauri::AppHandle, label: &str) {
    if label == OWNER { invalidate(app); }
}

#[tauri::command]
pub fn offline_export_reserve(
    expected_revision_id: String, window: WebviewWindow,
    authority: tauri::State<'_, crate::Authority>, state: tauri::State<'_, Exports>,
) -> Result<String, String> {
    owner(&window)?;
    let workspace = authority.lock().map_err(|_| "OFFLINE_STATE")?;
    let current = workspace.source_revision();
    if current.dirty || current.revision_id != expected_revision_id { return Err("OFFLINE_STALE".into()); }
    state.lock().map_err(|_| "OFFLINE_STATE")?.begin(&identity(&current), current.source)
}
#[tauri::command]
pub fn offline_export_cancel(id: String, window: WebviewWindow, state: tauri::State<'_, Exports>) -> Result<(), String> {
    owner(&window)?;
    state.lock().map_err(|_| "OFFLINE_STATE")?.cancel(&id);
    Ok(())
}

struct Reservation { state: Exports, id: String }
impl Drop for Reservation {
    fn drop(&mut self) {
        if let Ok(mut host) = self.state.lock() { host.cancel(&self.id); }
    }
}
async fn choose(window: &WebviewWindow, state: &Exports, id: &str,
    title: &'static str, save: bool, name: &'static str, pattern: &'static str,
) -> Result<Option<std::path::PathBuf>, String> {
    let guard = state.clone(); let token = id.to_owned();
    let result = crate::wav_export::chooser::linux::choose_local(window, title, save, name, pattern,
        move || Ok(guard.lock().is_ok_and(|h| h.work(&token).is_ok_and(|w|
            h.leases.phase(OWNER, &w).is_ok())))).await;
    let host = state.lock().map_err(|_| "OFFLINE_STATE")?;
    let work = host.work(id)?;
    host.leases.phase(OWNER, &work).map_err(|_| "OFFLINE_STALE")?;
    result.map_err(|_| "OFFLINE_CHOOSER".into())
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportMode { DisclosedRehearsalV1 }

/// Requires an explicit named rehearsal choice, not an inferred permissive mode.
/// All paths come from three parented native choosers, never command arguments.
#[tauri::command]
pub async fn offline_export_run(id: String, mode: ExportMode, window: WebviewWindow,
    authority: tauri::State<'_, crate::Authority>, state: tauri::State<'_, Exports>,
) -> Result<Option<serde_json::Value>, String> {
    owner(&window)?;
    let state = state.inner().clone(); let authority = authority.inner().clone();
    let (source, work) = {
        let workspace = authority.lock().map_err(|_| "OFFLINE_STATE")?;
        let current = workspace.source_revision();
        if current.dirty { return Err("OFFLINE_DIRTY".into()); }
        let mut host = state.lock().map_err(|_| "OFFLINE_STATE")?;
        let work = host.work(&id)?;
        host.leases.observe_snapshot(OWNER, &identity(&current), current.source);
        host.leases.phase(OWNER, &work).map_err(|_| "OFFLINE_STALE")?;
        if host.running { return Err("OFFLINE_BUSY".into()); }
        host.running = true;
        (current.source.to_owned(), work)
    };
    let _reservation = Reservation { state: state.clone(), id: id.clone() };
    let Some(policy) = choose(&window, &state, &id, "Offline export: select source-bound policy JSON", false, "Policy JSON", "*.json").await? else { return Ok(None); };
    let Some(bank) = choose(&window, &state, &id, "Offline export: select your local TimGM6mb bank", false, "SoundFont", "*.sf2").await? else { return Ok(None); };
    let Some(output) = choose(&window, &state, &id, "Offline export: choose a NEW WAV filename", true, "WAV audio", "*.[wW][aA][vV]").await? else { return Ok(None); };
    if !output.extension().and_then(|s| s.to_str()).is_some_and(|s| s.eq_ignore_ascii_case("wav")) {
        return Err("OFFLINE_WAV_EXTENSION".into());
    }
    tauri::async_runtime::spawn_blocking(move || -> Result<Option<serde_json::Value>, String> {
        use scorestream_audio::{audition_session::{Audition, Config}, AudioError};
        if work.cancellation().load(std::sync::atomic::Ordering::Acquire) { return Err("OFFLINE_CANCELLED".into()); }
        let file = std::fs::File::open(policy).map_err(|_| "OFFLINE_POLICY_READ")?;
        if !file.metadata().map_err(|_| "OFFLINE_POLICY_READ")?.is_file() { return Err("OFFLINE_POLICY_READ".into()); }
        let config: Config = serde_json::from_reader(file).map_err(|_| "OFFLINE_POLICY_INVALID")?;
        let ExportMode::DisclosedRehearsalV1 = mode;
        let session = Audition::compile_practice(&source,
            scorestream_app::playback::PracticeTieNoteOnPolicy::InertWithoutNewNoteOnV1,
            scorestream_app::playback::PracticeHoldEndpointPolicy::ReleaseUnmarkedAtBoundaryBeforeHoldV1,
            scorestream_app::playback::PracticeNoteExpressionPolicy::AuthoredNoteOnWithRelativeTrajectoryV1,
            config, scorestream_audio::audition_session::PracticeReviewPolicy::WrittenNotesWithoutWedgeDashesOrnamentsSlurV1,
            scorestream_audio::midi_mixer::LocalMidiPolicy::InitialGmCc7CenteredV1,
        ).map_err(|_| "OFFLINE_POLICY_UNSUPPORTED")?;
        let prepared = session.prepare_dry_snapshot(&source, &bank, work.cancellation())
            .map_err(|error| errors::audio_code(error.as_ref(), "OFFLINE_PREFLIGHT_FAILED"))?;
        {
            let workspace = authority.lock().map_err(|_| "OFFLINE_STATE")?;
            let current = workspace.source_revision();
            if current.dirty { return Err("OFFLINE_DIRTY".into()); }
            let mut host = state.lock().map_err(|_| "OFFLINE_STATE")?;
            host.leases.observe_snapshot(OWNER, &identity(&current), current.source);
            host.leases.ready(OWNER, &work, ()).map_err(|_| "OFFLINE_STALE")?;
            host.leases.start_render(OWNER, &work).map_err(|_| "OFFLINE_STALE")?;
        }
        let report = session.render_prepared_snapshot_with_gate(&source, &prepared, &output,
            work.cancellation(), |pending| {
                // Native authority and lease share the same lock order as edits.
                let workspace = authority.lock().map_err(|_| AudioError::Cancelled)?;
                let current = workspace.source_revision();
                if current.dirty { return Err(AudioError::Cancelled); }
                let mut host = state.lock().map_err(|_| AudioError::Cancelled)?;
                host.leases.observe_snapshot(OWNER, &identity(&current), current.source);
                host.leases.publish(OWNER, &work, || pending.commit()).map_err(|_| AudioError::Cancelled)?
            }).map_err(|error| errors::audio_code(error.as_ref(), "OFFLINE_RENDER_FAILED"))?;
        Ok(Some(report))
    }).await.map_err(|_| "OFFLINE_WORKER_FAILED".to_owned())?
}
