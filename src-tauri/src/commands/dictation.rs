use super::CmdResult;
use crate::foreground::{self, TargetInfo};
use crate::state::AppState;
use crate::windows;
use base64::Engine;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tokio_util::sync::CancellationToken;
use wisp_core::gemini::prompt::{transcription_request, TranscriptionHints};
use wisp_core::storage::RequestLog;
use wisp_core::{ErrorCode, WispError};

const AUDIO_TYPES: [&str; 6] = [
    "audio/webm",
    "audio/ogg",
    "audio/wav",
    "audio/mp4",
    "audio/mpeg",
    "audio/aac",
];
/// About 10 MB of audio — far more than the 2-minute recording limit needs.
const MAX_AUDIO_B64: usize = 14 * 1024 * 1024;

#[tauri::command]
pub fn dictation_target(state: State<'_, AppState>) -> TargetInfo {
    foreground::info(*state.insert_target.lock().expect("target"))
}

/// Transcribe recorded audio with Gemini. Audio is discarded afterwards unless
/// the user enabled retention.
#[tauri::command]
pub async fn transcribe(
    app: AppHandle,
    state: State<'_, AppState>,
    audio_base64: String,
    mime_type: String,
) -> CmdResult<String> {
    let base_mime = mime_type
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_lowercase();
    if !AUDIO_TYPES.contains(&base_mime.as_str()) {
        return Err(WispError::invalid("Unsupported audio format"));
    }
    if audio_base64.is_empty() {
        return Err(WispError::invalid("No audio was recorded"));
    }
    if audio_base64.len() > MAX_AUDIO_B64 {
        return Err(WispError::invalid(
            "That recording is too long. Keep dictation under 2 minutes.",
        ));
    }
    let key = state.secrets.require()?;
    let settings = state.settings();
    if settings.retain_audio {
        if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(&audio_base64) {
            let dir = app.path().app_data_dir().map(|d| d.join("recordings"));
            if let Ok(dir) = dir {
                let ext = if base_mime.ends_with("ogg") {
                    "ogg"
                } else {
                    "webm"
                };
                let name = format!(
                    "dictation-{}.{ext}",
                    chrono::Local::now().format("%Y%m%d-%H%M%S")
                );
                let _ = std::fs::create_dir_all(&dir)
                    .and_then(|_| std::fs::write(dir.join(name), bytes));
            }
        }
    }
    let started = Instant::now();
    let apps: Vec<String> = if settings.app_awareness {
        super::dashboard::cached_catalogue(&app, false)
            .await
            .iter()
            .map(|i| i.name.clone())
            .collect()
    } else {
        Vec::new()
    };
    let req = transcription_request(
        &base_mime,
        &audio_base64,
        settings.dictation_style,
        &settings.selected_model,
        &TranscriptionHints {
            language: &settings.voice_language,
            vocabulary: &settings.vocabulary,
            apps: &apps,
        },
    );
    drop(audio_base64);
    let result = state
        .gemini
        .generate(
            &key,
            &settings.selected_model,
            &req,
            &CancellationToken::new(),
        )
        .await;
    if settings.history_retention_days > 0 {
        let mut log = RequestLog::new("dictation", &settings.selected_model, "dictation");
        log.latency_ms = started.elapsed().as_millis() as u64;
        if let Err(e) = &result {
            log.status = "error".into();
            log.error_code = serde_json::to_value(e.code)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned));
        } else if settings.content_history {
            log.response = result.as_ref().ok().map(|o| o.text.clone());
        }
        let _ = state.db.log_request(&log);
    }
    match result {
        Ok(out) => Ok(out.text.trim().to_string()),
        // An empty answer means no intelligible speech, not a failure.
        Err(e) if e.code == ErrorCode::MalformedResponse && e.message.contains("empty") => {
            Ok(String::new())
        }
        Err(e) => Err(e),
    }
}

/// Paste text into the app that was in front before Wisp was summoned.
#[tauri::command]
pub async fn insert_text(
    app: AppHandle,
    state: State<'_, AppState>,
    text: String,
) -> CmdResult<()> {
    if text.trim().is_empty() {
        return Err(WispError::invalid("Nothing to insert"));
    }
    if text.chars().count() > wisp_core::tools::MAX_COPY_CHARS {
        return Err(WispError::invalid("Text is too long to insert"));
    }
    let target = state
        .insert_target
        .lock()
        .expect("target")
        .filter(|t| foreground::is_valid(*t))
        .ok_or_else(|| {
            WispError::new(
                ErrorCode::Unsupported,
                "Wisp doesn't know where to insert. Use Copy, then paste where you want it.",
            )
        })?;
    let clipboard = app.clipboard();
    let previous = clipboard.read_text().ok();
    clipboard
        .write_text(text)
        .map_err(|e| WispError::internal(format!("Couldn't use the clipboard: {e}")))?;
    windows::close_bar(&app);
    if !foreground::focus(target) {
        return Err(WispError::new(
            ErrorCode::Unsupported,
            "Couldn't switch back to the other app. The text is on your clipboard — paste it with Ctrl+V.",
        ));
    }
    tokio::time::sleep(Duration::from_millis(150)).await;
    if !foreground::paste() {
        return Err(WispError::new(
            ErrorCode::Unsupported,
            "Couldn't paste automatically. The text is on your clipboard — paste it with Ctrl+V.",
        ));
    }
    // Give the target app time to read the clipboard before restoring it.
    tokio::time::sleep(Duration::from_millis(500)).await;
    if let Some(prev) = previous {
        let _ = clipboard.write_text(prev);
    }
    Ok(())
}
