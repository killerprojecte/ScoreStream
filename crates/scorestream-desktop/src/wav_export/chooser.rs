//! Only WAV export uses this adapter. Ordinary SFS Open/Save stay unchanged.
use super::{PathBuf, Reservation, Result, WebviewWindow};

#[cfg(not(target_os = "linux"))]
pub(super) async fn destination(_window: &WebviewWindow, _reservation: &Reservation) -> Result<Option<PathBuf>> {
    // Preserve the existing native Windows/macOS route.
    Ok(rfd::AsyncFileDialog::new()
        .set_title("Export WAV (choose a new filename)")
        .set_file_name("score.wav")
        .add_filter("WAV audio", &["wav"])
        .save_file().await.map(|file| file.path().to_path_buf()))
}

#[cfg(target_os = "linux")]
pub(super) use linux::destination;

#[cfg(target_os = "linux")]
pub(crate) mod linux {
    use super::*;
    use super::super::WavError;
    use gtk::{FileChooserAction, FileChooserDialog, FileFilter, ResponseType, prelude::*};
    use std::{cell::RefCell, rc::Rc};
    use tauri::async_runtime::{Sender, channel};

    type Choice = Result<Option<PathBuf>>;

    // Pure classification: only the explicit Cancel/Escape/window-close responses
    // are quiet. A backend failure or Accept without a local filename is not cancel.
    pub(in crate::wav_export) fn classify(response: ResponseType, path: Option<PathBuf>) -> Choice {
        match response {
            ResponseType::Accept => path.filter(|path| path.is_absolute())
                .map(Some).ok_or_else(WavError::chooser),
            ResponseType::Cancel | ResponseType::DeleteEvent => Ok(None),
            _ => Err(WavError::chooser()),
        }
    }

    pub(in crate::wav_export) fn received(result: Option<Choice>) -> Choice {
        result.ok_or_else(WavError::chooser)?
    }

    // response and destroy may both fire. Consume the sole sender exactly once;
    // dropping/closing the receiver is harmless and never blocks the GTK thread.
    pub(in crate::wav_export) struct Reply(pub(in crate::wav_export) Option<Sender<Choice>>);
    impl Reply {
        pub(in crate::wav_export) fn complete(&mut self, result: Choice) -> bool {
            let Some(sender) = self.0.take() else { return false; };
            let _ = sender.try_send(result);
            true
        }
    }

    pub(in crate::wav_export) async fn destination(window: &WebviewWindow, reservation: &Reservation) -> Choice {
        let state = reservation.state.clone();
        let owner = reservation.owner.clone();
        let id = reservation.id.clone();
        choose_local(window, "Export WAV (choose a new filename)", true, "WAV audio", "*.[wW][aA][vV]",
            move || state.lock().map(|s| s.matches(&owner, &id)).map_err(|_| WavError::chooser())).await
    }

    /// Shared parented native chooser. No webview path or owner is accepted.
    pub(crate) async fn choose_local(window: &WebviewWindow, title: &'static str,
        save: bool, filter_name: &'static str, pattern: &'static str,
        active: impl Fn() -> Result<bool> + Send + 'static) -> Choice {
        let (sender, mut receiver) = channel(1);
        let parent = window.clone();
        window.run_on_main_thread(move || {
            // Never create a late dialog for an invalidated invocation/window.
            if sender.is_closed() { return; }
            match active() {
                Ok(true) => (),
                Ok(false) => { let _ = sender.try_send(Err(WavError::cancelled())); return; }
                Err(error) => { let _ = sender.try_send(Err(error)); return; }
            }
            // GTK objects, callbacks and destruction all stay on Tauri's GTK
            // thread. No nested dialog.run() loop or blocking channel receive.
            if !gtk::is_initialized_main_thread() {
                let _ = sender.try_send(Err(WavError::chooser()));
                return;
            }
            let Ok(parent) = parent.gtk_window() else {
                let _ = sender.try_send(Err(WavError::chooser()));
                return;
            };
            let dialog = FileChooserDialog::with_buttons(
                Some(title), Some(&parent), if save { FileChooserAction::Save } else { FileChooserAction::Open },
                &[("_Cancel", ResponseType::Cancel), (if save { "_Save" } else { "_Open" }, ResponseType::Accept)],
            );
            dialog.set_modal(true);
            dialog.set_destroy_with_parent(true);
            dialog.set_local_only(true);
            if save { dialog.set_current_name("score.wav"); }
            // The sink cannot overwrite. Do not offer a misleading Replace prompt.
            dialog.set_do_overwrite_confirmation(false);
            let filter = FileFilter::new();
            filter.set_name(Some(filter_name));
            filter.add_pattern(pattern);
            dialog.add_filter(filter);
            dialog.set_default_response(ResponseType::Accept);

            let reply = Rc::new(RefCell::new(Reply(Some(sender))));
            let response_reply = reply.clone();
            dialog.connect_response(move |dialog, response| {
                let path = if response == ResponseType::Accept { dialog.filename() } else { None };
                let first = response_reply.borrow_mut().complete(classify(response, path));
                if first {
                    // SAFETY: this is the GTK main thread, the path was copied,
                    // no callback retains the widget, and we never query it again.
                    unsafe { dialog.destroy(); }
                }
            });
            dialog.connect_destroy(move |_| {
                // Unexpected destruction is a failure, unless a response already
                // settled it. The caller separately checks window invalidation.
                reply.borrow_mut().complete(Err(WavError::chooser()));
            });
            // GTK owns the shown toplevel until destroy; signal closures do not
            // capture it, so there is no dialog/reference cycle.
            dialog.show_all();
        }).map_err(|_| WavError::chooser())?;
        received(receiver.recv().await)
    }

}
