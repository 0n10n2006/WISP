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
    state.secrets.require()?;
    let keys = crate::routing::Keys::load(&state);
    let settings = state.settings();
    if settings.retain_audio {
        if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(&audio_base64) {
            let ext = base_mime.rsplit('/').next().unwrap_or("webm");
            retain_recording(&app, ext, &bytes);
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
    let cancel = CancellationToken::new();
    let result = crate::routing::run(&app, wisp_core::router::Task::Audio, &keys, |route| {
        let req = transcription_request(
            &base_mime,
            &audio_base64,
            settings.dictation_style,
            &route.model,
            &TranscriptionHints {
                language: &settings.voice_language,
                vocabulary: &settings.vocabulary,
                apps: &apps,
            },
        );
        let (state, keys, cancel) = (&state, &keys, &cancel);
        async move {
            (
                crate::routing::complete(state, keys, &route, &req, cancel).await,
                false,
            )
        }
    })
    .await;
    drop(audio_base64);
    let model = result
        .as_ref()
        .map(|o| o.route.model.clone())
        .unwrap_or_else(|_| settings.selected_model.clone());
    let result = result.map(|o| o.value);
    if settings.history_retention_days > 0 {
        let mut log = RequestLog::new("dictation", &model, "dictation");
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

/// Keep a recording when the user turned on "keep recordings".
pub(crate) fn retain_recording(app: &AppHandle, ext: &str, bytes: &[u8]) {
    let Ok(dir) = app.path().app_data_dir().map(|d| d.join("recordings")) else {
        return;
    };
    let ext = match ext {
        "wav" | "ogg" | "mp4" | "mpeg" | "aac" => ext,
        _ => "webm",
    };
    let name = format!(
        "dictation-{}.{ext}",
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    );
    let _ = std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(dir.join(name), bytes));
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
    let console =
        foreground::process_name(target).is_some_and(|p| wisp_core::tools::is_console_process(&p));
    let text = wisp_core::tools::text_for_insert(&text, console);
    if text.trim().is_empty() {
        return Err(WispError::invalid("Nothing to insert"));
    }
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

// ---- Instant voice (Windows speech recognition) ---------------------------

/// Whether Windows can recognise speech right now (and why not).
#[tauri::command]
pub async fn speech_status(language: String) -> crate::speech::SpeechStatus {
    let language = clean_language(&language);
    tauri::async_runtime::spawn_blocking(move || crate::speech::status(&language))
        .await
        .unwrap_or(crate::speech::SpeechStatus {
            available: false,
            reason: Some("Speech recognition stopped unexpectedly.".into()),
            fix_uri: None,
        })
}

/// Start recognising; the words so far arrive as `wisp://speech` events.
#[tauri::command]
pub async fn speech_start(app: AppHandle, language: String) -> CmdResult<()> {
    use tauri::Emitter;
    let language = clean_language(&language);
    tauri::async_runtime::spawn_blocking(move || {
        crate::speech::start(&language, move |text| {
            let _ = app.emit("wisp://speech", text);
        })
    })
    .await
    .map_err(|e| WispError::internal(e.to_string()))?
}

#[tauri::command]
pub async fn speech_stop() -> CmdResult<crate::speech::Heard> {
    tauri::async_runtime::spawn_blocking(crate::speech::stop)
        .await
        .map_err(|e| WispError::internal(e.to_string()))?
}

#[tauri::command]
pub async fn speech_cancel() {
    let _ = tauri::async_runtime::spawn_blocking(crate::speech::cancel).await;
}

/// "en-GB"-style tags only.
fn clean_language(language: &str) -> String {
    let l = language.trim();
    if l.len() <= 20 && l.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        l.to_string()
    } else {
        String::new()
    }
}
