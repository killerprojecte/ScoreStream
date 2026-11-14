use super::*;
use std::fs;

fn count(directory: &std::path::Path) -> usize { fs::read_dir(directory).unwrap().count() }
fn start(sessions: &mut Sessions, owner: &str, path: PathBuf) -> String {
    let id = sessions.reserve(owner).unwrap();
    sessions.open(owner, &id, path, WavLayout::for_frames(2).unwrap()).unwrap();
    id
}
fn write_all(sessions: &mut Sessions, owner: &str, id: &str) {
    let layout = WavLayout::for_frames(2).unwrap();
    assert_eq!(sessions.write(owner, id, Ok(0), Ok(&layout.header())).unwrap(), 44);
    assert_eq!(sessions.write(owner, id, Ok(44), Ok(&[0,128,255,127,0,0,1,0])).unwrap(), 52);
}

#[test]
fn correct_complete_file_is_private_until_finish() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("output.wav");
    let mut sessions = Sessions::default();
    let id = start(&mut sessions, "main", path.clone());
    assert!(!path.exists()); assert_eq!(count(dir.path()), 1);
    write_all(&mut sessions, "main", &id);
    assert!(!path.exists());
    assert_eq!(sessions.finish("main", &id).unwrap(), path.display().to_string());
    assert_eq!(count(dir.path()), 1);
    assert!(sessions.windows.is_empty());
    let file = fs::read(&path).unwrap();
    assert_eq!(&file[..44], &WavLayout::for_frames(2).unwrap().header());
    assert_eq!(&file[44..], &[0,128,255,127,0,0,1,0]);
    assert_eq!(file.len(), 52);
    sessions.abort("main", &id).unwrap();
    assert!(path.exists());
}

#[test]
fn existing_destination_and_commit_time_race_preserve_original_bytes() {
    let dir = tempfile::tempdir().unwrap(); let path = dir.path().join("output.wav");
    fs::write(&path, b"original").unwrap();
    assert!(matches!(FileSession::create(path.clone(), WavLayout::for_frames(2).unwrap()), Err(WavError { code: "WAV_DESTINATION_EXISTS", .. })));
    assert_eq!(fs::read(&path).unwrap(), b"original"); assert_eq!(count(dir.path()), 1);
    fs::remove_file(&path).unwrap();
    let mut sessions = Sessions::default(); let id = start(&mut sessions, "main", path.clone());
    write_all(&mut sessions, "main", &id);
    fs::write(&path, b"race winner").unwrap();
    assert_eq!(sessions.finish("main", &id).unwrap_err().code, "WAV_DESTINATION_EXISTS");
    assert_eq!(fs::read(&path).unwrap(), b"race winner");
    assert_eq!(count(dir.path()), 1); assert!(sessions.windows.is_empty());
}

#[test]
fn invalid_paths_do_not_create_files() {
    let dir = tempfile::tempdir().unwrap();
    assert!(FileSession::create(dir.path().join("output.sfs"), WavLayout::for_frames(2).unwrap()).is_err());
    assert!(FileSession::create(dir.path().join("missing").join("output.wav"), WavLayout::for_frames(2).unwrap()).is_err());
    assert_eq!(count(dir.path()), 0);
}

#[cfg(unix)]
#[test]
fn dangling_destination_symlink_is_not_followed_or_replaced() {
    let dir = tempfile::tempdir().unwrap(); let path = dir.path().join("output.wav");
    std::os::unix::fs::symlink(dir.path().join("missing.wav"), &path).unwrap();
    assert!(matches!(FileSession::create(path.clone(), WavLayout::for_frames(2).unwrap()), Err(WavError { code: "WAV_DESTINATION_EXISTS", .. })));
    assert!(fs::symlink_metadata(&path).unwrap().file_type().is_symlink());
    assert_eq!(count(dir.path()), 1);
}

#[test]
fn window_ownership_stale_tokens_and_busy_picker_are_enforced() {
    let dir = tempfile::tempdir().unwrap(); let mut sessions = Sessions::default();
    let reserved = sessions.reserve("main").unwrap();
    assert!(valid_id(&reserved));
    assert_eq!(sessions.reserve("main").unwrap_err().code, "WAV_BUSY");
    sessions.abort("other", &reserved).unwrap();
    assert!(sessions.matches("main", &reserved));
    assert_eq!(sessions.finish("other", &reserved).unwrap_err().code, "WAV_SESSION_NOT_FOUND");
    assert_eq!(sessions.write("other", &reserved, Ok(0), Ok(&[0;4])).unwrap_err().code, "WAV_SESSION_NOT_FOUND");
    sessions.abort("main", &reserved).unwrap();
    let current = start(&mut sessions, "main", dir.path().join("output.wav"));
    assert_ne!(reserved, current);
    sessions.abort("main", &reserved).unwrap();
    assert!(sessions.matches("main", &current));
    assert_eq!(sessions.write("main", &reserved, Ok(0), Ok(&[0;4])).unwrap_err().code, "WAV_SESSION_NOT_FOUND");
    assert!(sessions.matches("main", &current));
    sessions.abort("main", &current).unwrap();
    assert_eq!(count(dir.path()), 0);
}

#[test]
fn bad_chunk_bad_cursor_incomplete_finish_and_abort_remove_temporary_output() {
    let dir = tempfile::tempdir().unwrap(); let path = dir.path().join("output.wav");
    let mut sessions = Sessions::default();
    for kind in 0..5 {
        let id = start(&mut sessions, "main", path.clone());
        match kind {
            0 => { assert!(sessions.write("main", &id, Ok(0), Ok(&[0;44])).is_err()); },
            1 => { assert!(sessions.write("main", &id, Err(WavError::bad_request()), Ok(&[0;4])).is_err()); },
            2 => { assert!(sessions.write("main", &id, Ok(0), Err(WavError::bad_request())).is_err()); },
            3 => { assert!(sessions.finish("main", &id).is_err()); },
            _ => { write_all(&mut sessions, "main", &id); sessions.abort("main", &id).unwrap(); },
        }
        assert!(!path.exists()); assert_eq!(count(dir.path()), 0);
        assert!(sessions.windows.is_empty());
    }
}

#[test]
fn reservation_close_drop_and_app_teardown_do_not_publish() {
    let dir = tempfile::tempdir().unwrap(); let path = dir.path().join("output.wav");
    let state = WavExports::default();
    let id = state.lock().unwrap().reserve("main").unwrap();
    let reservation = Reservation { state: state.clone(), owner: "main".into(), id: id.clone(), armed: true };
    discard_window(&state, "main");
    assert!(state.lock().unwrap().open("main", &id, path.clone(), WavLayout::for_frames(2).unwrap()).is_err());
    drop(reservation);
    assert_eq!(count(dir.path()), 0);
    start(&mut state.lock().unwrap(), "main", path.clone());
    discard_window(&state, "main"); assert_eq!(count(dir.path()), 0);
    start(&mut state.lock().unwrap(), "main", path.clone());
    discard_all(&state); assert_eq!(count(dir.path()), 0);
    start(&mut state.lock().unwrap(), "main", path.clone());
    drop(state); assert_eq!(count(dir.path()), 0); assert!(!path.exists());
}

#[test]
fn bounded_canonical_headers_and_path_free_errors() {
    for value in ["", "00", "-1", "+1", " 44", "44 ", "1e3", "1.0", "123456789", "１２"] {
        assert!(offset(value).is_err(), "{value}");
    }
    assert_eq!(offset("0").unwrap(), 0); assert_eq!(offset("23088044").unwrap(), 23_088_044);
    for value in ["", "wav-1", "wav-000000000000000G", "other-window", "wav-0000000000000001-extra"] {
        assert!(!valid_id(value));
    }
    assert!(valid_id("wav-0123456789abcdef"));
    assert_eq!(serde_json::to_value(WavError::exists()).unwrap(), serde_json::json!({
        "code": "WAV_DESTINATION_EXISTS",
        "message": "Choose a new filename. WAV export does not replace existing files.",
    }));
}

#[test]
fn source_preflight_returns_exact_plan_or_stable_error_before_file_setup() {
    let (plan, layout) = prepare(scorestream_app::DEMO_SOURCE).unwrap();
    assert_eq!(layout, WavLayout::for_plan(&plan).unwrap());
    assert_eq!(prepare("not a score").unwrap_err().code, "WAV_SOURCE_INVALID");
    assert_eq!(prepare(&" ".repeat(MAX_SOURCE_BYTES + 1)).unwrap_err().code, "WAV_LIMIT_EXCEEDED");
}
