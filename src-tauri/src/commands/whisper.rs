//! Local speech recognition: model download and removal, and transcribing a
//! recording with Whisper on this PC.

use super::CmdResult;
use crate::state::AppState;
use crate::whisper;
use base64::Engine;
use std::sync::Mutex;
use std::time::Instant;
use tauri::{AppHandle, Emitter, State};
use tokio_util::sync::CancellationToken;
use wisp_core::settings::{DictationStyle, WhisperModel};
use wisp_core::storage::RequestLog;
use wisp_core::WispError;

/// The download in progress, if any (one at a time).
static DOWNLOAD: Mutex<Option<(WhisperModel, CancellationToken)>> = Mutex::new(None);

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelState {
    pub model: WhisperModel,
    pub downloaded: bool,
    pub bytes: u64,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WhisperStatus {
    pub models: Vec<ModelState>,
    pub downloading: Option<WhisperModel>,
    pub loaded: Option<WhisperModel>,
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct Progress {
    model: WhisperModel,
    done: u64,
    total: u64,
}

#[tauri::command]
pub fn whisper_status(app: AppHandle) -> CmdResult<WhisperStatus> {
    let dir = whisper::models_dir(&app)?;
    Ok(WhisperStatus {
        models: wisp_core::whisper::MODELS
            .iter()
            .map(|m| ModelState {
                model: m.model,
                downloaded: whisper::downloaded(&dir, m.model),
                bytes: m.bytes,
            })
            .collect(),
        downloading: DOWNLOAD.lock().expect("download").as_ref().map(|d| d.0),
        loaded: whisper::loaded(),
    })
}

/// Download a model from whisper.cpp's repository, with progress events
/// (`wisp://whisper-download`). Resolves when the model is ready.
#[tauri::command]
pub async fn whisper_download(app: AppHandle, model: WhisperModel) -> CmdResult<()> {
    let dir = whisper::models_dir(&app)?;
    if whisper::downloaded(&dir, model) {
        return Ok(());
    }
    let cancel = {
        let mut d = DOWNLOAD.lock().expect("download");
        if d.is_some() {
            return Err(WispError::invalid("A speech model is already downloading."));
        }
        let token = CancellationToken::new();
        *d = Some((model, token.clone()));
        token
    };
    let a = app.clone();
    let result = wisp_core::whisper::download(
        &wisp_core::whisper::http_client(),
        wisp_core::whisper::MODEL_BASE_URL,
        model,
        &dir,
        &cancel,
        move |done, total| {
            let _ = a.emit("wisp://whisper-download", Progress { model, done, total });
        },
    )
    .await;
    *DOWNLOAD.lock().expect("download") = None;
    result.map(|_| ())
}

#[tauri::command]
pub fn whisper_cancel_download() {
    if let Some((_, token)) = DOWNLOAD.lock().expect("download").as_ref() {
        token.cancel();
    }
}

#[tauri::command]
pub fn whisper_delete(app: AppHandle, model: WhisperModel) -> CmdResult<()> {
    if DOWNLOAD
        .lock()
        .expect("download")
        .as_ref()
        .is_some_and(|d| d.0 == model)
    {
        return Err(WispError::invalid("Wait for the download to finish first."));
    }
    if whisper::loaded() == Some(model) {
        whisper::unload();
    }
    let path = wisp_core::whisper::model_path(&whisper::models_dir(&app)?, model);
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(e.into()),
        _ => Ok(()),
    }
}

/// Load the chosen model in the background while the user is still talking.
#[tauri::command]
pub async fn whisper_prewarm(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    let model = state.settings().whisper_model;
    let dir = whisper::models_dir(&app)?;
    if whisper::downloaded(&dir, model) && whisper::loaded() != Some(model) {
        tauri::async_runtime::spawn_blocking(move || whisper::prewarm(&dir, model));
    }
    Ok(())
}

/// About 10 MB of WAV: a little over the 2-minute dictation limit.
const MAX_WAV_B64: usize = 6 * 1024 * 1024;

/// Transcribe a 16 kHz WAV recording on this PC. `purpose` is "dictation"
/// (the user's dictation style applies) or "voice" (a spoken question).
#[tauri::command]
pub async fn transcribe_local(
    app: AppHandle,
    state: State<'_, AppState>,
    audio_base64: String,
    purpose: String,
) -> CmdResult<String> {
    if audio_base64.is_empty() {
        return Err(WispError::invalid("No audio was recorded"));
    }
    if audio_base64.len() > MAX_WAV_B64 {
        return Err(WispError::invalid(
            "That recording is too long. Keep dictation under 2 minutes.",
        ));
    }
    let settings = state.settings();
    let model = settings.whisper_model;
    let dir = whisper::models_dir(&app)?;
    if !whisper::downloaded(&dir, model) {
        return Err(whisper::not_downloaded());
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(&audio_base64)
        .map_err(|_| WispError::invalid("That recording couldn't be read."))?;
    drop(audio_base64);
    if settings.retain_audio {
        super::dictation::retain_recording(&app, "wav", &bytes);
    }
    let samples = wisp_core::whisper::decode_wav(&bytes)?;
    drop(bytes);
    let language = wisp_core::whisper::language_code(&settings.voice_language);
    let prompt = wisp_core::whisper::initial_prompt(&settings.vocabulary);
    let style = if purpose == "dictation" {
        settings.dictation_style
    } else {
        DictationStyle::Clean
    };
    let started = Instant::now();
    let result = tauri::async_runtime::spawn_blocking(move || {
        whisper::transcribe(&dir, model, &samples, &language, &prompt)
    })
    .await
    .map_err(|e| WispError::internal(e.to_string()))?
    .map(|t| wisp_core::whisper::tidy(&t, style));
    if settings.history_retention_days > 0 {
        let name = wisp_core::whisper::spec(model).file;
        let mut log = RequestLog::new("dictation", name, "dictation");
        log.latency_ms = started.elapsed().as_millis() as u64;
        match &result {
            Err(_) => log.status = "error".into(),
            Ok(t) if settings.content_history => log.response = Some(t.clone()),
            Ok(_) => {}
        }
        let _ = state.db.log_request(&log);
    }
    result
}
