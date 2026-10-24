//! Whisper on this PC (whisper.cpp). The model is loaded the first time it
//! is needed and unloaded after a few idle minutes, so it uses no memory
//! while you aren't talking. Audio never leaves the machine.

use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};
use wisp_core::settings::WhisperModel;
use wisp_core::{ErrorCode, Result, WispError};

/// Unload the model after this long without use.
const IDLE_UNLOAD: Duration = Duration::from_secs(180);
/// Segments Whisper itself thinks are probably silence are dropped.
const NO_SPEECH: f32 = 0.6;

struct Loaded {
    model: WhisperModel,
    ctx: WhisperContext,
    used: Instant,
}

static ENGINE: Mutex<Option<Loaded>> = Mutex::new(None);

/// Where models are kept (app data\models).
pub fn models_dir(app: &tauri::AppHandle) -> Result<PathBuf> {
    use tauri::Manager;
    app.path()
        .app_data_dir()
        .map(|d| d.join("models"))
        .map_err(|e| WispError::internal(e.to_string()))
}

pub fn downloaded(dir: &Path, model: WhisperModel) -> bool {
    wisp_core::whisper::model_path(dir, model).is_file()
}

/// Which model is in memory right now.
pub fn loaded() -> Option<WhisperModel> {
    ENGINE.lock().ok()?.as_ref().map(|l| l.model)
}

pub fn unload() {
    if let Ok(mut e) = ENGINE.lock() {
        *e = None;
    }
}

fn threads() -> i32 {
    let cores = std::thread::available_parallelism().map_or(4, |n| n.get());
    cores.saturating_sub(1).clamp(1, 8) as i32
}

/// Drops the model once it has been idle for a while. One sleeping thread.
fn start_reaper() {
    static STARTED: OnceLock<()> = OnceLock::new();
    STARTED.get_or_init(|| {
        let _ = std::thread::Builder::new()
            .name("wisp-whisper-idle".into())
            .spawn(|| loop {
                std::thread::sleep(Duration::from_secs(30));
                if let Ok(mut e) = ENGINE.lock() {
                    if e.as_ref().is_some_and(|l| l.used.elapsed() >= IDLE_UNLOAD) {
                        *e = None;
                    }
                }
            });
    });
}

fn ensure_loaded<'a>(
    slot: &'a mut Option<Loaded>,
    dir: &Path,
    model: WhisperModel,
) -> Result<&'a mut Loaded> {
    if slot.as_ref().is_some_and(|l| l.model != model) {
        *slot = None;
    }
    if slot.is_none() {
        let path = wisp_core::whisper::model_path(dir, model);
        if !path.is_file() {
            return Err(not_downloaded());
        }
        whisper_rs::install_logging_hooks();
        let ctx = WhisperContext::new_with_params(&path, WhisperContextParameters::default())
            .map_err(|_| {
                WispError::new(
                    ErrorCode::Unsupported,
                    "The speech model couldn't be loaded. Delete it in Settings → Voice and download it again.",
                )
            })?;
        *slot = Some(Loaded {
            model,
            ctx,
            used: Instant::now(),
        });
        start_reaper();
    }
    Ok(slot.as_mut().expect("just loaded"))
}

pub fn not_downloaded() -> WispError {
    WispError::new(
        ErrorCode::Unsupported,
        "The speech model isn't downloaded yet.",
    )
}

/// Load the model ahead of time (when the user starts talking), so the
/// transcription right after they stop doesn't wait for it.
pub fn prewarm(dir: &Path, model: WhisperModel) {
    if let Ok(mut e) = ENGINE.lock() {
        if let Ok(l) = ensure_loaded(&mut e, dir, model) {
            l.used = Instant::now();
        }
    }
}

/// Turn 16 kHz mono samples into text (blocking; run off the async runtime).
pub fn transcribe(
    dir: &Path,
    model: WhisperModel,
    samples: &[f32],
    language: &str,
    prompt: &str,
) -> Result<String> {
    let mut guard = ENGINE
        .lock()
        .map_err(|_| WispError::internal("speech engine"))?;
    let loaded = ensure_loaded(&mut guard, dir, model)?;
    let mut state = loaded
        .ctx
        .create_state()
        .map_err(|_| WispError::internal("Couldn't start speech recognition"))?;
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_n_threads(threads());
    params.set_language(Some(language));
    params.set_translate(false);
    params.set_no_context(true);
    params.set_no_timestamps(true);
    params.set_suppress_blank(true);
    params.set_suppress_nst(true);
    params.set_temperature(0.0);
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    if !prompt.is_empty() {
        params.set_initial_prompt(prompt);
    }
    // whisper.cpp wants at least a second of audio; pad short clips.
    let min = wisp_core::whisper::SAMPLE_RATE as usize * 11 / 10;
    let mut padded;
    let audio = if samples.len() < min {
        padded = samples.to_vec();
        padded.resize(min, 0.0);
        &padded[..]
    } else {
        samples
    };
    state
        .full(params, audio)
        .map_err(|_| WispError::internal("Speech recognition failed"))?;
    let mut text = String::new();
    for segment in state.as_iter() {
        if segment.no_speech_probability() > NO_SPEECH {
            continue;
        }
        if let Ok(s) = segment.to_str_lossy() {
            text.push_str(&s);
        }
    }
    loaded.used = Instant::now();
    Ok(text.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_model_is_reported_not_crashed() {
        let dir = tempfile::tempdir().unwrap();
        let err =
            transcribe(dir.path(), WhisperModel::Standard, &[0.0; 1600], "en", "").unwrap_err();
        assert_eq!(err.code, ErrorCode::Unsupported);
        let bad = wisp_core::whisper::model_path(dir.path(), WhisperModel::Standard);
        std::fs::write(&bad, b"not a model").unwrap();
        let err =
            transcribe(dir.path(), WhisperModel::Standard, &[0.0; 1600], "en", "").unwrap_err();
        assert!(err.message.contains("couldn't be loaded"));
        assert_eq!(loaded(), None);
    }

    /// End-to-end with a real model: set WISP_WHISPER_MODEL to a ggml model
    /// file and WISP_WHISPER_WAV to a 16-bit WAV (whisper.cpp's samples/jfk.wav).
    #[test]
    #[ignore = "needs a downloaded model"]
    fn transcribes_a_real_recording() {
        let model = std::env::var("WISP_WHISPER_MODEL").expect("WISP_WHISPER_MODEL");
        let wav = std::env::var("WISP_WHISPER_WAV").expect("WISP_WHISPER_WAV");
        let dir = tempfile::tempdir().unwrap();
        std::fs::copy(
            &model,
            wisp_core::whisper::model_path(dir.path(), WhisperModel::Standard),
        )
        .unwrap();
        let samples = wisp_core::whisper::decode_wav(&std::fs::read(wav).unwrap()).unwrap();
        let started = Instant::now();
        let text = transcribe(dir.path(), WhisperModel::Standard, &samples, "en", "").unwrap();
        eprintln!("{:?} in {:?}", text, started.elapsed());
        assert!(!text.is_empty());
        assert_eq!(loaded(), Some(WhisperModel::Standard));
    }
}
