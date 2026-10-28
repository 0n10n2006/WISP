use super::CmdResult;
use crate::state::{AppState, ShortcutIssue};
use crate::{shortcuts, windows};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_autostart::ManagerExt;
use wisp_core::settings::Settings;
use wisp_core::shortcuts::Shortcuts;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsSnapshot {
    pub settings: Settings,
    pub has_api_key: bool,
    /// An OpenRouter key is saved (the key itself never leaves the host).
    pub has_openrouter_key: bool,
    pub shortcut_issues: Vec<ShortcutIssue>,
    pub recovered_from: Option<String>,
    pub warnings: Vec<String>,
    pub version: String,
}

fn snapshot(app: &AppHandle, state: &AppState) -> SettingsSnapshot {
    SettingsSnapshot {
        settings: state.settings(),
        has_api_key: state.secrets.get().ok().flatten().is_some(),
        has_openrouter_key: state.openrouter_key.get().ok().flatten().is_some(),
        shortcut_issues: state.shortcut_issues.lock().expect("issues").clone(),
        recovered_from: state
            .settings_recovered_from
            .lock()
            .expect("recovered")
            .clone(),
        warnings: state.startup_warnings.lock().expect("warnings").clone(),
        version: app.package_info().version.to_string(),
    }
}

#[tauri::command]
pub fn get_settings(app: AppHandle, state: State<'_, AppState>) -> SettingsSnapshot {
    snapshot(&app, &state)
}

/// Apply side effects of a settings change (window flags, shortcuts, autostart).
pub fn apply(app: &AppHandle, old: &Settings, new: &Settings) {
    if let Some(win) = windows::island(app) {
        if old.always_on_top != new.always_on_top {
            let _ = win.set_always_on_top(new.always_on_top);
        }
    }
    if old.island_scale != new.island_scale
        || old.notch_position != new.notch_position
        || old.notch_style != new.notch_style
        || old.ball_corner != new.ball_corner
    {
        let expanded = app
            .state::<AppState>()
            .island
            .lock()
            .expect("island")
            .expanded;
        windows::set_island_expanded(app, expanded);
    }
    if old.content_history && !new.content_history {
        // Drop stored conversation text now, not at the next start.
        let _ = app
            .state::<AppState>()
            .db
            .prune_history(new.history_retention_days.max(1), false);
    }
    if old.index_files != new.index_files || old.index_folders != new.index_folders {
        app.state::<AppState>().files.reconfigure(new.index_files);
    }
    if old.clipboard_history && !new.clipboard_history {
        // Turning history off forgets what it kept (pinned clips stay).
        let _ = app.state::<AppState>().db.clear_clips(false);
        let _ = app.emit("wisp://data-changed", "clipboard");
    }
    crate::hotkeys::set_enabled(new.hold_to_talk && new.voice_enabled);
    if old.hold_dictate != new.hold_dictate || old.hold_ask != new.hold_ask {
        crate::hotkeys::configure(&new.hold_dictate, &new.hold_ask);
    }
    if old.theme != new.theme {
        if let Some(win) = app.get_webview_window(windows::SETTINGS) {
            crate::chrome::apply_theme(&win, new.theme);
        }
    }
    if old.tucked != new.tucked {
        let open =
            app.state::<AppState>().island.lock().expect("island").mode == windows::NotchMode::Open;
        if !open {
            windows::set_island_visible(app, windows::sliver_wanted(app));
        }
        let _ = app.emit("wisp://tucked", new.tucked);
        crate::tray::sync_notch(!new.tucked);
    }
    let game = app
        .state::<AppState>()
        .game_active
        .load(std::sync::atomic::Ordering::SeqCst);
    if old.global_shortcuts != new.global_shortcuts && !game {
        shortcuts::register_all(app, &new.global_shortcuts);
    }
    // In a game, shortcuts follow the "keep shortcuts in games" choice.
    if game
        && (old.global_shortcuts != new.global_shortcuts
            || old.shortcuts_in_games != new.shortcuts_in_games)
    {
        shortcuts::set_game_mode(app, true);
    }
    if old.launch_at_login != new.launch_at_login {
        let al = app.autolaunch();
        let _ = if new.launch_at_login {
            al.enable()
        } else {
            al.disable()
        };
    }
}

pub(crate) fn commit(
    app: &AppHandle,
    state: &AppState,
    new: Settings,
) -> CmdResult<SettingsSnapshot> {
    let old = state.settings();
    state.settings_store.save(&new)?;
    *state.settings.lock().expect("settings") = new.clone();
    apply(app, &old, &new);
    let snap = snapshot(app, state);
    let _ = app.emit("wisp://settings", &snap);
    Ok(snap)
}

#[tauri::command]
pub fn update_settings(
    app: AppHandle,
    state: State<'_, AppState>,
    patch: serde_json::Value,
) -> CmdResult<SettingsSnapshot> {
    let new = state.settings().patched(&patch)?;
    commit(&app, &state, new)
}

#[tauri::command]
pub fn reset_shortcuts(app: AppHandle, state: State<'_, AppState>) -> CmdResult<SettingsSnapshot> {
    let new = Settings {
        global_shortcuts: Shortcuts::default(),
        ..state.settings()
    };
    commit(&app, &state, new)
}

#[tauri::command]
pub fn reset_settings(app: AppHandle, state: State<'_, AppState>) -> CmdResult<SettingsSnapshot> {
    let new = Settings {
        onboarding_complete: state.settings().onboarding_complete,
        ..Settings::default()
    };
    commit(&app, &state, new)
}

#[tauri::command]
pub fn set_api_key(
    app: AppHandle,
    state: State<'_, AppState>,
    key: String,
) -> CmdResult<SettingsSnapshot> {
    state.secrets.set(&key)?;
    let snap = snapshot(&app, &state);
    let _ = app.emit("wisp://settings", &snap);
    Ok(snap)
}

#[tauri::command]
pub fn remove_api_key(app: AppHandle, state: State<'_, AppState>) -> CmdResult<SettingsSnapshot> {
    state.secrets.delete()?;
    let snap = snapshot(&app, &state);
    let _ = app.emit("wisp://settings", &snap);
    Ok(snap)
}

#[tauri::command]
pub async fn list_models(
    state: State<'_, AppState>,
) -> CmdResult<Vec<wisp_core::gemini::types::ModelInfo>> {
    let key = state.secrets.require()?;
    let models = state.gemini.list_models(&key).await?;
    state
        .routing
        .set_gemini_models(models.iter().map(|m| m.id.clone()).collect());
    Ok(models)
}

/// Test a candidate key (before saving) or the stored one.
#[tauri::command]
pub async fn test_connection(
    state: State<'_, AppState>,
    key: Option<String>,
    model: Option<String>,
) -> CmdResult<wisp_core::gemini::ConnectionReport> {
    let key = match key {
        Some(k) => crate::secrets::validate_key(&k)?,
        None => state.secrets.require()?,
    };
    let model = model.unwrap_or_else(|| state.settings().selected_model);
    state.gemini.test_connection(&key, &model).await
}

/// Check an OpenRouter key (before saving) or the stored one; returns the
/// free models it can use.
#[tauri::command]
pub async fn test_openrouter_key(
    state: State<'_, AppState>,
    key: Option<String>,
) -> CmdResult<Vec<wisp_core::openrouter::FreeModel>> {
    let key = match key {
        Some(k) if wisp_core::openrouter::valid_key(&k) => k.trim().to_string(),
        Some(_) => {
            return Err(wisp_core::WispError::invalid(
                "That doesn't look like an OpenRouter key (it starts with sk-or-).",
            ))
        }
        None => state.openrouter_key.get()?.ok_or_else(|| {
            wisp_core::WispError::new(
                wisp_core::ErrorCode::MissingKey,
                "No OpenRouter key is saved.",
            )
        })?,
    };
    let models = state.routing.openrouter.list_free_models(&key).await?;
    state.routing.set_openrouter_models(models.clone());
    Ok(models)
}

#[tauri::command]
pub fn set_openrouter_key(
    app: AppHandle,
    state: State<'_, AppState>,
    key: String,
) -> CmdResult<SettingsSnapshot> {
    state.openrouter_key.set(&key)?;
    let snap = snapshot(&app, &state);
    let _ = app.emit("wisp://settings", &snap);
    Ok(snap)
}

#[tauri::command]
pub fn remove_openrouter_key(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CmdResult<SettingsSnapshot> {
    state.openrouter_key.delete()?;
    let snap = snapshot(&app, &state);
    let _ = app.emit("wisp://settings", &snap);
    Ok(snap)
}

/// Today's requests per model and which are used up until when.
#[tauri::command]
pub fn model_usage(state: State<'_, AppState>) -> Vec<crate::routing::ModelUsage> {
    crate::routing::usage(&state)
}

#[tauri::command]
pub fn register_shortcuts(app: AppHandle) -> Vec<ShortcutIssue> {
    let state = app.state::<AppState>();
    let s = state.settings().global_shortcuts;
    shortcuts::register_all(&app, &s)
}

#[tauri::command]
pub fn validate_shortcut(accelerator: String) -> CmdResult<String> {
    wisp_core::shortcuts::normalize(&accelerator)
}

/// Settings is recording a new shortcut (true) or finished (false).
#[tauri::command]
pub fn pause_shortcuts(app: AppHandle, paused: bool) {
    shortcuts::pause_for_recording(&app, paused);
}
