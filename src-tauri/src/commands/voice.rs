//! Voice: Wisp listens with Windows' speech recognizer (no pop-up). When
//! dictating, each phrase is typed into the app you're in as soon as it's
//! recognised; when asking by voice, the words fill Wisp's bar. Wisp never
//! records or uploads audio.

use super::CmdResult;
use crate::state::AppState;
use crate::{foreground, speech, voicetyping, windows};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, State};
use wisp_core::{ErrorCode, WispError};

/// Where the words go.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum VoiceTarget {
    /// Typed into the app the user was in.
    App,
    /// Into Wisp's command bar (a spoken question).
    Bar,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct Heard {
    target: VoiceTarget,
    text: String,
    /// Why listening ended early (no network, no microphone…).
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// A phrase as it should be typed into `process`: safe for terminals, with
/// the space that separates it from the previous phrase kept.
fn for_typing(piece: &str, process: Option<&str>) -> String {
    let console = process.is_some_and(wisp_core::tools::is_console_process);
    let lead = if piece.starts_with(' ') { " " } else { "" };
    let body = wisp_core::tools::text_for_insert(piece.trim_start(), console);
    if body.trim().is_empty() {
        String::new()
    } else {
        format!("{lead}{body}")
    }
}

#[tauri::command]
pub async fn voice_start(
    app: AppHandle,
    state: State<'_, AppState>,
    target: VoiceTarget,
) -> CmdResult<()> {
    if !state.settings().voice_enabled {
        return Err(WispError::new(
            ErrorCode::PermissionDenied,
            "Voice is turned off. Turn it on in Settings → Voice.",
        ));
    }
    if target == VoiceTarget::App {
        // Type into the app in front (Wisp's own windows don't count).
        if let Some(t) = foreground::current() {
            *state.insert_target.lock().expect("target") = Some(t);
        }
    }
    let (a, b, c) = (app.clone(), app.clone(), app.clone());
    let events = speech::Events {
        live: Box::new(move |text| {
            let _ = a.emit_to(
                windows::ISLAND,
                "wisp://speech",
                Heard {
                    target,
                    text,
                    error: None,
                },
            );
        }),
        phrase: Box::new(move |piece| {
            if target != VoiceTarget::App {
                return;
            }
            let into = *b.state::<AppState>().insert_target.lock().expect("target");
            let Some(into) = into.filter(|t| foreground::is_valid(*t)) else {
                return;
            };
            // Only type while that app is still in front.
            if foreground::current() != Some(into) {
                return;
            }
            let text = for_typing(&piece, foreground::process_name(into).as_deref());
            voicetyping::wait_for_release(
                voicetyping::modifiers_down,
                Duration::from_millis(1500),
                Duration::from_millis(15),
            );
            voicetyping::type_text(&text);
        }),
        done: Box::new(move |text, error| {
            let _ = c.emit_to(
                windows::ISLAND,
                "wisp://speech-done",
                Heard {
                    target,
                    text,
                    error,
                },
            );
        }),
    };
    tauri::async_runtime::spawn_blocking(move || speech::start(events))
        .await
        .map_err(|e| WispError::internal(e.to_string()))?
}

/// Stop listening now, keeping what was heard.
#[tauri::command]
pub async fn voice_stop() {
    let _ = tauri::async_runtime::spawn_blocking(speech::stop).await;
}

/// Stop listening and forget what was heard.
#[tauri::command]
pub async fn voice_cancel() {
    let _ = tauri::async_runtime::spawn_blocking(speech::cancel).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phrases_keep_their_spacing_and_are_terminal_safe() {
        assert_eq!(for_typing("Hello there", Some("notepad")), "Hello there");
        assert_eq!(for_typing(" and more", Some("notepad")), " and more");
        assert_eq!(for_typing(" two\nlines", Some("pwsh")), " two lines");
        assert_eq!(for_typing(" ", None), "");
    }
}
