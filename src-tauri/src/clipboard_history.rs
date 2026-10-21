//! Opt-in clipboard history. Off by default; when the user turns it on, a
//! light poll notices clipboard changes and stores plain text and links.
//!
//! Content marked private by the copying app is never read: Windows password
//! managers set `ExcludeClipboardContentFromMonitorProcessing` or
//! `CanIncludeInClipboardHistory = 0`, and those copies are skipped. Text that
//! looks like a secret is also skipped (see `storage::looks_sensitive`).

use crate::state::AppState;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;

#[cfg(windows)]
mod imp {
    use windows::core::w;
    use windows::Win32::Foundation::HGLOBAL;
    use windows::Win32::System::DataExchange::{
        CloseClipboard, GetClipboardData, GetClipboardSequenceNumber, IsClipboardFormatAvailable,
        OpenClipboard, RegisterClipboardFormatW,
    };
    use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};

    pub fn sequence() -> u32 {
        unsafe { GetClipboardSequenceNumber() }
    }

    /// Whether the current clipboard content asked not to be recorded.
    pub fn excluded() -> bool {
        unsafe {
            let exclude =
                RegisterClipboardFormatW(w!("ExcludeClipboardContentFromMonitorProcessing"));
            let ignore = RegisterClipboardFormatW(w!("Clipboard Viewer Ignore"));
            let history = RegisterClipboardFormatW(w!("CanIncludeInClipboardHistory"));
            if IsClipboardFormatAvailable(exclude).is_ok()
                || IsClipboardFormatAvailable(ignore).is_ok()
            {
                return true;
            }
            if IsClipboardFormatAvailable(history).is_err() {
                return false;
            }
            // CanIncludeInClipboardHistory holds a DWORD; 0 means "don't keep".
            if OpenClipboard(None).is_err() {
                return true; // Be conservative if we can't check.
            }
            let mut skip = false;
            if let Ok(h) = GetClipboardData(history) {
                let g = HGLOBAL(h.0);
                let p = GlobalLock(g) as *const u32;
                if !p.is_null() && GlobalSize(g) >= 4 {
                    skip = *p == 0;
                }
                let _ = GlobalUnlock(g);
            }
            let _ = CloseClipboard();
            skip
        }
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn sequence() -> u32 {
        0
    }
    pub fn excluded() -> bool {
        false
    }
}

pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut last_seq = imp::sequence();
        let mut last_text: Option<String> = None;
        let mut was_enabled = false;
        loop {
            tokio::time::sleep(Duration::from_millis(800)).await;
            let state = app.state::<AppState>();
            let enabled = state.settings.lock().expect("settings").clipboard_history;
            if !enabled {
                was_enabled = false;
                continue;
            }
            if !was_enabled {
                // Don't record whatever was already on the clipboard when the
                // user turned history on.
                was_enabled = true;
                last_seq = imp::sequence();
                last_text = app.clipboard().read_text().ok();
                continue;
            }
            if cfg!(windows) {
                let seq = imp::sequence();
                if seq == last_seq {
                    continue;
                }
                last_seq = seq;
                if imp::excluded() {
                    continue;
                }
            }
            let Ok(text) = app.clipboard().read_text() else {
                continue;
            };
            if last_text.as_deref() == Some(text.as_str()) {
                continue;
            }
            last_text = Some(text.clone());
            if let Ok(Some(_)) = state.db.add_clip(&text) {
                let _ = app.emit("wisp://data-changed", "clipboard");
            }
        }
    });
}
