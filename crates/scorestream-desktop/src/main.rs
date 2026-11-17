#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use scorestream_app::{ViewResponse, Workspace};
use std::sync::{Arc, Mutex};
use tauri::{Manager, State};
mod mcp_service;
mod source_files;
use source_files::OpenedFile;
type PendingImport = Mutex<source_files::Pending>;
static IMPORT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
type ImportOrigin = Mutex<Option<std::path::PathBuf>>;
mod wav_export;
#[cfg(target_os = "linux")]
mod offline_export;
mod window_chrome;
use wav_export::WavExports;
#[cfg(feature = "smoke-test")]
mod smoke_profile;
use mcp_service::{McpService, McpSettings, McpStatus};
type Authority = Arc<Mutex<Workspace>>;
type McpHost = Mutex<McpService>;

#[tauri::command]
fn mcp_status(state: State<'_, McpHost>) -> Result<McpStatus, String> {
    Ok(state.lock().map_err(|e| e.to_string())?.status())
}
#[tauri::command]
async fn mcp_save_settings(
    settings: McpSettings,
    state: State<'_, McpHost>,
) -> Result<McpStatus, String> {
    state.lock().map_err(|e| e.to_string())?.save(settings)
}
#[tauri::command]
async fn mcp_start(
    settings: McpSettings,
    token: Option<String>,
    state: State<'_, McpHost>,
    authority: State<'_, Authority>,
) -> Result<McpStatus, String> {
    state
        .lock()
        .map_err(|e| e.to_string())?
        .start(settings, token, authority.inner().clone())
}
#[tauri::command]
async fn mcp_stop(state: State<'_, McpHost>) -> Result<McpStatus, String> {
    state.lock().map_err(|e| e.to_string())?.stop()
}
#[tauri::command]
fn offline_export_supported() -> bool { cfg!(target_os = "linux") }
#[tauri::command]
fn current(state: State<'_, Authority>) -> Result<ViewResponse, String> {
    Ok(state.lock().map_err(|e| e.to_string())?.current())
}
// Lock order is always workspace -> offline lease, including final publication.
fn change_workspace(app: &tauri::AppHandle, state: &Authority,
    change: impl FnOnce(&mut Workspace) -> Result<ViewResponse, String>) -> Result<ViewResponse, String> {
    let mut workspace = state.lock().map_err(|e| e.to_string())?;
    #[cfg(target_os = "linux")]
    offline_export::invalidate(app);
    #[cfg(not(target_os = "linux"))]
    let _ = app;
    change(&mut workspace)
}
#[tauri::command]
fn open_demo(app: tauri::AppHandle, state: State<'_, Authority>) -> Result<ViewResponse, String> {
    change_workspace(&app, state.inner(), |service| { *service = Workspace::demo()?; Ok(service.current()) })
}
#[tauri::command]
fn preview_source(source: String, app: tauri::AppHandle, state: State<'_, Authority>) -> Result<ViewResponse, String> {
    // Even an uncommitted editor draft invalidates its prior audition reservation.
    change_workspace(&app, state.inner(), |service| Ok(service.preview_source(&source)))
}
#[tauri::command]
async fn playback_plan(source: String) -> Result<scorestream_app::playback::PlaybackPlan, String> {
    // Parsing and planning are CPU work; do not block the UI or async executor.
    tauri::async_runtime::spawn_blocking(move || scorestream_app::playback::plan_source(&source))
        .await
        .map_err(|error| format!("Playback planning failed: {error}"))?
}
#[tauri::command]
fn apply_source(source: String, expected_revision_id: String, app: tauri::AppHandle,
    state: State<'_, Authority>) -> Result<ViewResponse, String> {
    change_workspace(&app, state.inner(), |w| w.apply_source(&source, &expected_revision_id))
}
#[tauri::command]
fn undo(app: tauri::AppHandle, state: State<'_, Authority>) -> Result<ViewResponse, String> {
    change_workspace(&app, state.inner(), Workspace::undo)
}
#[tauri::command]
fn redo(app: tauri::AppHandle, state: State<'_, Authority>) -> Result<ViewResponse, String> {
    change_workspace(&app, state.inner(), Workspace::redo)
}
#[tauri::command]
fn set_event_pitch(event_id: String, pitch: String, expected_revision_id: String,
    app: tauri::AppHandle, state: State<'_, Authority>) -> Result<ViewResponse, String> {
    change_workspace(&app, state.inner(), |w| w.set_event_pitch(&event_id, &pitch, &expected_revision_id))
}
#[tauri::command]
fn set_event_duration(event_id: String, duration: String, expected_revision_id: String,
    app: tauri::AppHandle, state: State<'_, Authority>) -> Result<ViewResponse, String> {
    change_workspace(&app, state.inner(), |w| w.set_event_duration(&event_id, &duration, &expected_revision_id))
}
#[tauri::command]
async fn open_source_file(pending: State<'_, PendingImport>) -> Result<Option<OpenedFile>, String> {
    let Some(file) = rfd::AsyncFileDialog::new()
        .add_filter("Scores", &["sfs", "musicxml", "xml", "mxl"])
        .pick_file()
        .await
    else {
        return Ok(None);
    };
    let path = file.path().to_path_buf();
    let mut prepared = tauri::async_runtime::spawn_blocking(move || source_files::read(&path))
        .await
        .map_err(|e| format!("Import task failed: {e}"))??;
    if let (Some(replacement), Some(origin)) = (prepared.workspace, prepared.origin) {
        let token = IMPORT_ID
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            .to_string();
        prepared.opened.import_token = Some(token.clone());
        *pending.lock().map_err(|e| e.to_string())? = Some((token, replacement, origin));
    }
    Ok(Some(prepared.opened))
}
#[tauri::command]
fn activate_import(
    app: tauri::AppHandle,
    token: String,
    expected_revision_id: String,
    authority: State<'_, Authority>,
    origin: State<'_, ImportOrigin>,
    pending: State<'_, PendingImport>,
) -> Result<ViewResponse, String> {
    let mut workspace = authority.lock().map_err(|e| e.to_string())?;
    #[cfg(target_os = "linux")]
    offline_export::invalidate(&app);
    let mut imported = origin.lock().map_err(|e| e.to_string())?;
    let mut prepared = pending.lock().map_err(|e| e.to_string())?;
    source_files::activate(
        &mut prepared,
        &mut workspace,
        &mut imported,
        &token,
        &expected_revision_id,
    )
}
#[tauri::command]
async fn save_source_file(
    source: String,
    origin: State<'_, ImportOrigin>,
) -> Result<Option<String>, String> {
    // Validate with the same application authority before writing. No renderer data is saved.
    let validated =
        tauri::async_runtime::spawn_blocking(move || source_files::ValidatedSave::prepare(source))
            .await
            .map_err(|e| format!("Save validation failed: {e}"))??;
    let Some(file) = rfd::AsyncFileDialog::new()
        .set_file_name("score.sfs")
        .add_filter("ScoreStream source", &["sfs"])
        .save_file()
        .await
    else {
        return Ok(None);
    };
    let path = file.path().to_path_buf();
    let imported = origin.lock().map_err(|e| e.to_string())?.clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<Option<String>, String> {
        validated.persist(&path, imported.as_deref())?;
        Ok(Some(path.display().to_string()))
    })
    .await
    .map_err(|e| format!("Save task failed: {e}"))?
}
#[cfg(feature = "smoke-test")]
#[tauri::command]
fn report_smoke_test(report: String) -> Result<(), String> {
    let path = std::env::var("SCORESTREAM_SMOKE_REPORT")
        .map_err(|_| "Smoke report path not configured")?;
    println!("SCORESTREAM_SMOKE_RESULT {report}");
    std::fs::write(path, report).map_err(|e| e.to_string())
}
fn main() {
    let authority = Workspace::demo().expect("embedded demo must validate");
    let builder = tauri::Builder::default()
        .manage(Arc::new(Mutex::new(authority)))
        .manage(WavExports::default())
        .manage(Mutex::new(None::<std::path::PathBuf>))
        .manage(Mutex::new(None::<(String, Workspace, std::path::PathBuf)>))
        .on_page_load(|webview, payload| {
            if matches!(payload.event(), tauri::webview::PageLoadEvent::Started) {
                #[cfg(target_os = "linux")]
                offline_export::discard_window(webview.app_handle(), webview.label());
                if let Some(pending) = webview.try_state::<PendingImport>() {
                    if let Ok(mut value) = pending.lock() {
                        *value = None;
                    }
                }
                if let Some(state) = webview.try_state::<WavExports>() {
                    wav_export::discard_window(state.inner(), webview.label());
                }
            }
        })
        .on_window_event(|window, event| {
            if matches!(event, tauri::WindowEvent::Destroyed) {
                #[cfg(target_os = "linux")]
                offline_export::discard_window(window.app_handle(), window.label());
                if let Some(state) = window.try_state::<WavExports>() {
                    wav_export::discard_window(state.inner(), window.label());
                }
            }
        })
        .setup(|app| {
            // Check before loading MCP settings or creating a WebView. An opt-in
            // smoke run must never read or write the production application's profile.
            #[cfg(feature = "smoke-test")]
            if std::env::var_os("SCORESTREAM_SMOKE_REPORT").is_some() {
                smoke_profile::ensure_isolated_identifier(&app.config().identifier)
                    .map_err(std::io::Error::other)?;
            }
            app.manage(Mutex::new(McpService::load(
                app.path().app_config_dir()?.join("mcp-service.json"),
            )));
            #[allow(unused_mut)]
            let mut window = tauri::WebviewWindowBuilder::new(
                app,
                "main",
                tauri::WebviewUrl::App("index.html".into()),
            )
            .title("ScoreStream")
            .inner_size(1280.0, 860.0)
            .min_inner_size(1120.0, 700.0)
            .shadow(true)
            .initialization_script(window_chrome::initialization_script());
            // macOS keeps AppKit traffic lights and native fullscreen/tiling. Other
            // platforms use current-window controls inside the React application header.
            #[cfg(target_os = "macos")]
            {
                window = window
                    .decorations(true)
                    .title_bar_style(tauri::TitleBarStyle::Overlay)
                    .hidden_title(true)
                    .traffic_light_position(tauri::LogicalPosition::new(12.0, 18.0));
            }
            #[cfg(not(target_os = "macos"))]
            {
                window = window.decorations(false);
            }
            #[cfg(feature = "smoke-test")]
            if std::env::var_os("SCORESTREAM_SMOKE_REPORT").is_some() {
                window = window.initialization_script(
                    if std::env::var_os("SCORESTREAM_V0_CONTROL_QA").is_some() {
                        "window.__SCORESTREAM_V0_CONTROL_QA__ = true;"
                    } else {
                        "window.__SCORESTREAM_SMOKE__ = true;"
                    },
                );
            }
            window.build()?;
            Ok(())
        });
    #[cfg(target_os = "linux")]
    let builder = builder.manage(offline_export::Exports::default());
    #[cfg(all(feature = "smoke-test", target_os = "linux"))]
    let builder = builder.invoke_handler(tauri::generate_handler![
        offline_export::offline_policy_draft,
        offline_export::offline_policy_validate,
        offline_export::offline_export_reserve,
        offline_export::offline_export_run,
        offline_export::offline_export_cancel,
        offline_export_supported,
        current,
        open_demo,
        preview_source,
        playback_plan,
        apply_source,
        undo,
        redo,
        set_event_pitch,
        set_event_duration,
        open_source_file,
        activate_import,
        save_source_file,
        wav_export::wav_export_begin,
        wav_export::wav_export_write,
        wav_export::wav_export_finish,
        wav_export::wav_export_abort,
        mcp_status,
        mcp_save_settings,
        mcp_start,
        mcp_stop,
        report_smoke_test
    ]);
    #[cfg(all(not(feature = "smoke-test"), target_os = "linux"))]
    let builder = builder.invoke_handler(tauri::generate_handler![
        offline_export::offline_policy_draft,
        offline_export::offline_policy_validate,
        offline_export::offline_export_reserve,
        offline_export::offline_export_run,
        offline_export::offline_export_cancel,
        offline_export_supported,
        current,
        open_demo,
        preview_source,
        playback_plan,
        apply_source,
        undo,
        redo,
        set_event_pitch,
        set_event_duration,
        open_source_file,
        activate_import,
        save_source_file,
        wav_export::wav_export_begin,
        wav_export::wav_export_write,
        wav_export::wav_export_finish,
        wav_export::wav_export_abort,
        mcp_status,
        mcp_save_settings,
        mcp_start,
        mcp_stop
    ]);
    #[cfg(all(feature = "smoke-test", not(target_os = "linux")))]
    let builder = builder.invoke_handler(tauri::generate_handler![
        offline_export_supported,
        current,
        open_demo,
        preview_source,
        playback_plan,
        apply_source,
        undo,
        redo,
        set_event_pitch,
        set_event_duration,
        open_source_file,
        activate_import,
        save_source_file,
        wav_export::wav_export_begin,
        wav_export::wav_export_write,
        wav_export::wav_export_finish,
        wav_export::wav_export_abort,
        mcp_status,
        mcp_save_settings,
        mcp_start,
        mcp_stop,
        report_smoke_test
    ]);
    #[cfg(all(not(feature = "smoke-test"), not(target_os = "linux")))]
    let builder = builder.invoke_handler(tauri::generate_handler![
        offline_export_supported,
        current,
        open_demo,
        preview_source,
        playback_plan,
        apply_source,
        undo,
        redo,
        set_event_pitch,
        set_event_duration,
        open_source_file,
        activate_import,
        save_source_file,
        wav_export::wav_export_begin,
        wav_export::wav_export_write,
        wav_export::wav_export_finish,
        wav_export::wav_export_abort,
        mcp_status,
        mcp_save_settings,
        mcp_start,
        mcp_stop
    ]);
    builder
        .build(tauri::generate_context!())
        .expect("failed to build ScoreStream")
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                #[cfg(target_os = "linux")]
                offline_export::invalidate(app);
                if let Some(state) = app.try_state::<WavExports>() {
                    wav_export::discard_all(state.inner());
                }
                if let Some(state) = app.try_state::<McpHost>() {
                    if let Ok(mut service) = state.lock() {
                        let _ = service.stop();
                    }
                }
            }
        });
}
