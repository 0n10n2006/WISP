//! Voice: Windows voice typing (Win+H), started where the words should go.
//! Dictation types into the app you were using; asking by voice types into
//! Wisp's own bar. Wisp never records or uploads audio.

use super::CmdResult;
use crate::state::AppState;
use crate::{foreground, voicetyping, windows};
use std::time::Duration;
use tauri::{AppHandle, Emitter, State};
use wisp_core::{ErrorCode, WispError};

/// Where the words go.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VoiceTarget {
    /// The app the user was in before Wisp.
    App,
    /// Wisp's command bar (a spoken question).
    Bar,
}

#[tauri::command]
pub async fn voice_typing(
    app: AppHandle,
    state: State<'_, AppState>,
    target: VoiceTarget,
) -> CmdResult<()> {
    if !cfg!(windows) {
        return Err(WispError::new(
            ErrorCode::Unsupported,
            "Voice typing uses Windows 11's own dictation.",
        ));
    }
    if !state.settings().voice_enabled {
        return Err(WispError::new(
            ErrorCode::PermissionDenied,
            "Voice is turned off. Turn it on in Settings → Voice.",
        ));
    }
    // A shortcut's keys may still be down: Ctrl+Shift+D must not become
    // Ctrl+Shift+Win+H.
    let _ = tauri::async_runtime::spawn_blocking(|| {
        voicetyping::wait_for_release(
            voicetyping::modifiers_down,
            Duration::from_millis(1500),
            Duration::from_millis(15),
        )
    })
    .await;
    match target {
        VoiceTarget::App => {
            let back = *state.insert_target.lock().expect("target");
            windows::close_bar(&app);
            if let Some(t) = back.filter(|t| foreground::is_valid(*t)) {
                foreground::focus(t);
            }
        }
        VoiceTarget::Bar => {
            if let Some(w) = windows::island(&app) {
                let _ = w.set_focus();
            }
            let _ = app.emit_to(windows::ISLAND, "wisp://focus-prompt", ());
        }
    }
    // Let the focus change land before Windows looks for a text box.
    tokio::time::sleep(Duration::from_millis(90)).await;
    if voicetyping::press_win_h() {
        Ok(())
    } else {
        Err(WispError::new(
            ErrorCode::Unsupported,
            "Couldn't start Windows voice typing. Press Win+H to start it yourself.",
        ))
    }
}
