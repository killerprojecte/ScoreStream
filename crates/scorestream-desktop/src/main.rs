#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use scorestream_app::{ViewResponse, Workspace};
use std::io::Write;
use std::sync::Mutex;
use tauri::State;
type Authority = Mutex<Workspace>;
#[tauri::command]
fn current(state: State<'_, Authority>) -> Result<ViewResponse, String> {
    Ok(state.lock().map_err(|e| e.to_string())?.current())
}
#[tauri::command]
fn open_demo(state: State<'_, Authority>) -> Result<ViewResponse, String> {
    let mut service = state.lock().map_err(|e| e.to_string())?;
    *service = Workspace::demo()?;
    Ok(service.current())
}
#[tauri::command]
fn preview_source(source: String, state: State<'_, Authority>) -> Result<ViewResponse, String> {
    Ok(state
        .lock()
        .map_err(|e| e.to_string())?
        .preview_source(&source))
}
#[tauri::command]
fn apply_source(
    source: String,
    expected_revision_id: String,
    state: State<'_, Authority>,
) -> Result<ViewResponse, String> {
    state
        .lock()
        .map_err(|e| e.to_string())?
        .apply_source(&source, &expected_revision_id)
}
#[tauri::command]
fn undo(state: State<'_, Authority>) -> Result<ViewResponse, String> {
    state.lock().map_err(|e| e.to_string())?.undo()
}
#[tauri::command]
fn redo(state: State<'_, Authority>) -> Result<ViewResponse, String> {
    state.lock().map_err(|e| e.to_string())?.redo()
}
#[tauri::command]
fn set_event_pitch(
    event_id: String,
    pitch: String,
    expected_revision_id: String,
    state: State<'_, Authority>,
) -> Result<ViewResponse, String> {
    state.lock().map_err(|e| e.to_string())?.set_event_pitch(
        &event_id,
        &pitch,
        &expected_revision_id,
    )
}
#[tauri::command]
fn set_event_duration(
    event_id: String,
    duration: String,
    expected_revision_id: String,
    state: State<'_, Authority>,
) -> Result<ViewResponse, String> {
    state.lock().map_err(|e| e.to_string())?.set_event_duration(
        &event_id,
        &duration,
        &expected_revision_id,
    )
}
#[derive(serde::Serialize)]
struct OpenedFile {
    path: String,
    source: String,
}
#[tauri::command]
async fn open_source_file() -> Result<Option<OpenedFile>, String> {
    let Some(file) = rfd::AsyncFileDialog::new()
        .add_filter("ScoreStream source", &["sfs"])
        .pick_file()
        .await
    else {
        return Ok(None);
    };
    let path = file.path().to_path_buf();
    let meta = std::fs::metadata(&path).map_err(|e| e.to_string())?;
    if meta.len() > 4 * 1024 * 1024 {
        return Err("This editor slice accepts SFS files up to 4 MiB.".into());
    }
    let source = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
    Ok(Some(OpenedFile {
        path: path.display().to_string(),
        source,
    }))
}
#[tauri::command]
async fn save_source_file(source: String) -> Result<Option<String>, String> {
    // Validate with the same application authority before writing. No renderer data is saved.
    Workspace::from_source(&source)?;
    let Some(file) = rfd::AsyncFileDialog::new()
        .set_file_name("score.sfs")
        .add_filter("ScoreStream source", &["sfs"])
        .save_file()
        .await
    else {
        return Ok(None);
    };
    let path = file.path().to_path_buf();
    let parent = path.parent().ok_or("Invalid save destination")?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    temporary
        .write_all(source.as_bytes())
        .map_err(|e| e.to_string())?;
    temporary.as_file().sync_all().map_err(|e| e.to_string())?;
    temporary.persist(&path).map_err(|e| {
        format!(
            "Atomic save failed; the previous file has been preserved: {}",
            e.error
        )
    })?;
    Ok(Some(path.display().to_string()))
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
        .manage(Mutex::new(authority))
        .setup(|app| {
            #[allow(unused_mut)]
            let mut window = tauri::WebviewWindowBuilder::new(
                app,
                "main",
                tauri::WebviewUrl::App("index.html".into()),
            )
            .title("ScoreStream")
            .inner_size(1480.0, 960.0)
            .min_inner_size(1120.0, 700.0);
            #[cfg(feature = "smoke-test")]
            if std::env::var_os("SCORESTREAM_SMOKE_REPORT").is_some() {
                window = window.initialization_script("window.__SCORESTREAM_SMOKE__ = true;");
            }
            window.build()?;
            Ok(())
        });
    #[cfg(feature = "smoke-test")]
    let builder = builder.invoke_handler(tauri::generate_handler![
        current,
        open_demo,
        preview_source,
        apply_source,
        undo,
        redo,
        set_event_pitch,
        set_event_duration,
        open_source_file,
        save_source_file,
        report_smoke_test
    ]);
    #[cfg(not(feature = "smoke-test"))]
    let builder = builder.invoke_handler(tauri::generate_handler![
        current,
        open_demo,
        preview_source,
        apply_source,
        undo,
        redo,
        set_event_pitch,
        set_event_duration,
        open_source_file,
        save_source_file
    ]);
    builder
        .run(tauri::generate_context!())
        .expect("failed to run ScoreStream");
}
