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
    pub shortcut_issues: Vec<ShortcutIssue>,
    pub recovered_from: Option<String>,
    pub version: String,
}

fn snapshot(app: &AppHandle, state: &AppState) -> SettingsSnapshot {
    SettingsSnapshot {
        settings: state.settings(),
        has_api_key: state.secrets.get().ok().flatten().is_some(),
        shortcut_issues: state.shortcut_issues.lock().expect("issues").clone(),
        recovered_from: state
            .settings_recovered_from
            .lock()
            .expect("recovered")
            .clone(),
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
    if old.global_shortcuts != new.global_shortcuts {
        shortcuts::register_all(app, &new.global_shortcuts);
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

fn commit(app: &AppHandle, state: &AppState, new: Settings) -> CmdResult<SettingsSnapshot> {
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
    state.gemini.list_models(&key).await
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
