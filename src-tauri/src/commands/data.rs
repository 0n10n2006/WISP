use super::CmdResult;
use crate::state::AppState;
use tauri::{AppHandle, Emitter, Manager, State};
use wisp_core::diagnostics::{self, DiagnosticsInput};
use wisp_core::settings::Settings;
use wisp_core::storage::{ActionAudit, DataScope, RequestLog, StorageStats};
use wisp_core::WispError;

#[tauri::command]
pub fn history_list(state: State<'_, AppState>, limit: Option<u32>) -> CmdResult<Vec<RequestLog>> {
    state.db.list_requests(limit.unwrap_or(200))
}

#[tauri::command]
pub fn actions_list(state: State<'_, AppState>, limit: Option<u32>) -> CmdResult<Vec<ActionAudit>> {
    state.db.list_actions(limit.unwrap_or(200))
}

#[tauri::command]
pub fn data_stats(state: State<'_, AppState>) -> CmdResult<StorageStats> {
    state.db.stats()
}

/// Windows' own confirmation, drawn by the host rather than the webview, so
/// the most destructive action can't be triggered by page content alone.
async fn native_confirm(app: &AppHandle, title: &str, body: &str, ok: &str) -> bool {
    use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
    let (a, title, body, ok) = (
        app.clone(),
        title.to_string(),
        body.to_string(),
        ok.to_string(),
    );
    tauri::async_runtime::spawn_blocking(move || {
        a.dialog()
            .message(body)
            .title(title)
            .kind(MessageDialogKind::Warning)
            .buttons(MessageDialogButtons::OkCancelCustom(ok, "Cancel".into()))
            .blocking_show()
    })
    .await
    .unwrap_or(false)
}

/// Delete local data. `"settings"` resets preferences; `"everything"` also
/// removes the API key, backups, kept audio and crash logs (after Windows
/// asks the user to confirm).
#[tauri::command]
pub async fn data_delete(
    app: AppHandle,
    state: State<'_, AppState>,
    scope: String,
) -> CmdResult<StorageStats> {
    if scope == "everything"
        && !native_confirm(
            &app,
            "Delete everything in Wisp?",
            "History, memory, reminders, clipboard history, saved audio, crash logs, settings and your API key will be removed from this PC. Files agents saved in Documents\\Wisp are kept.",
            "Delete everything",
        )
        .await
    {
        return Err(WispError::new(
            wisp_core::ErrorCode::Cancelled,
            "Nothing was deleted.",
        ));
    }
    let reset_settings = |state: &AppState, keep_onboarding: bool| -> CmdResult<()> {
        let old = state.settings();
        let new = Settings {
            onboarding_complete: keep_onboarding && old.onboarding_complete,
            ..Settings::default()
        };
        state.settings_store.save(&new)?;
        *state.settings.lock().expect("settings") = new.clone();
        super::settings::apply(&app, &old, &new);
        Ok(())
    };
    match scope.as_str() {
        "history" => state.db.delete_data(DataScope::History)?,
        "memory" => state.db.delete_data(DataScope::Memory)?,
        "tasks" => state.db.delete_data(DataScope::Tasks)?,
        "clipboard" => state.db.delete_data(DataScope::Clipboard)?,
        "settings" => reset_settings(&state, true)?,
        "everything" => {
            state.db.delete_data(DataScope::All)?;
            state.secrets.delete()?;
            reset_settings(&state, false)?;
            if let Ok(dir) = app.path().app_data_dir() {
                crate::cleanup::wipe_leftovers(&dir);
            }
        }
        _ => return Err(WispError::invalid("Unknown data category")),
    }
    if let Ok(dir) = app.path().app_data_dir() {
        crate::cleanup::remove_backups(&dir);
    }
    let snap = super::settings::get_settings(app.clone(), app.state::<AppState>());
    let _ = app.emit("wisp://settings", &snap);
    let _ = app.emit("wisp://data-changed", &scope);
    state.db.stats()
}

fn report(app: &AppHandle, state: &AppState) -> CmdResult<String> {
    let requests = state.db.list_requests(50)?;
    let actions = state.db.list_actions(50)?;
    let settings = state.settings();
    let r = diagnostics::build(DiagnosticsInput {
        app_version: &app.package_info().version.to_string(),
        has_api_key: state.secrets.get().ok().flatten().is_some(),
        settings: &settings,
        storage: state.db.stats()?,
        shortcut_issues: state
            .shortcut_issues
            .lock()
            .expect("issues")
            .iter()
            .map(|i| i.message.clone())
            .collect(),
        requests: &requests,
        actions: &actions,
    });
    serde_json::to_string_pretty(&r).map_err(|e| WispError::internal(e.to_string()))
}

/// Preview of exactly what an export would contain.
#[tauri::command]
pub fn diagnostics_report(app: AppHandle, state: State<'_, AppState>) -> CmdResult<String> {
    report(&app, &state)
}

/// Write the report to the Downloads folder. Only called after the user confirms.
#[tauri::command]
pub fn diagnostics_save(app: AppHandle, state: State<'_, AppState>) -> CmdResult<String> {
    let body = report(&app, &state)?;
    let dir = app
        .path()
        .download_dir()
        .or_else(|_| app.path().document_dir())
        .map_err(|e| WispError::storage(format!("Couldn't find your Downloads folder: {e}")))?;
    let name = format!(
        "wisp-diagnostics-{}.json",
        chrono::Local::now().format("%Y%m%d-%H%M%S")
    );
    let path = dir.join(name);
    std::fs::write(&path, body)?;
    Ok(path.display().to_string())
}
