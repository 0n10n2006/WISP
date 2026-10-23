//! Windows customisation for the control centre (look & feel, taskbar,
//! display & comfort, sound devices) and the folder wallpaper carousel.
//! Every change is typed and validated; each is written to the action log.

use super::CmdResult;
use crate::com_worker;
use crate::state::AppState;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager, State};
use wisp_core::storage::ActionAudit;
use wisp_core::winsettings::{WinChange, WinSettingsState};
use wisp_core::WispError;

#[tauri::command]
pub async fn win_settings_get() -> WinSettingsState {
    com_worker::run(crate::winsettings::state)
        .await
        .unwrap_or_default()
}

/// What a change did, plus where to finish it by hand if Windows refused.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangeOutcome {
    pub message: String,
}

#[tauri::command]
pub async fn win_setting_set(
    state: State<'_, AppState>,
    change: WinChange,
) -> CmdResult<ChangeOutcome> {
    let change = change.validated()?;
    let key = change.key();
    let c = change.clone();
    let result = com_worker::run(move || crate::winsettings::apply(&c)).await?;
    let outcome = if result.is_ok() { "done" } else { "failed" };
    let mut audit = ActionAudit::new("change_setting", &format!("setting={key}"), outcome, true);
    if let Err(e) = &result {
        audit.detail = Some(e.message.clone());
    }
    let _ = state.db.log_action(&audit);
    result.map(|message| ChangeOutcome { message })
}

/// The Windows Settings page for a change (for "open settings" fallbacks).
#[tauri::command]
pub fn win_settings_page(change: WinChange) -> String {
    crate::winsettings::settings_page(&change).to_string()
}

// ---- Wallpaper folders ---------------------------------------------------

/// (folders, images found, when).
type Listing = (Vec<String>, Vec<String>, Instant);
static LISTED: Mutex<Option<Listing>> = Mutex::new(None);

/// Images in the user's wallpaper folders (cached briefly).
pub fn folder_images(folders: &[String]) -> Vec<String> {
    let mut cache = LISTED.lock().expect("walls");
    if let Some((f, list, at)) = cache.as_ref() {
        if f == folders && at.elapsed() < Duration::from_secs(30) {
            return list.clone();
        }
    }
    let list = crate::wallpaper::scan(folders);
    *cache = Some((folders.to_vec(), list.clone(), Instant::now()));
    list
}

fn forget_listing() {
    *LISTED.lock().expect("walls") = None;
}

#[tauri::command]
pub async fn wallpaper_list(state: State<'_, AppState>) -> CmdResult<Vec<String>> {
    let folders = state.settings().wallpaper_folders;
    tauri::async_runtime::spawn_blocking(move || folder_images(&folders))
        .await
        .map_err(|e| WispError::internal(e.to_string()))
}

/// Choose a folder in a native picker and add it to the carousel.
#[tauri::command]
pub async fn wallpaper_folder_add(app: AppHandle) -> CmdResult<Vec<String>> {
    use tauri_plugin_dialog::DialogExt;
    let a = app.clone();
    let picked = tauri::async_runtime::spawn_blocking(move || {
        a.dialog()
            .file()
            .set_title("Choose a wallpaper folder")
            .blocking_pick_folder()
    })
    .await
    .map_err(|e| WispError::internal(e.to_string()))?;
    let state = app.state::<AppState>();
    let mut s = state.settings();
    let Some(folder) = picked
        .and_then(|p| p.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned())
    else {
        return Ok(s.wallpaper_folders);
    };
    if !crate::wallpaper::is_local_folder(&folder) {
        return Err(WispError::invalid("Choose a folder on this PC."));
    }
    if !s
        .wallpaper_folders
        .iter()
        .any(|f| f.eq_ignore_ascii_case(&folder))
    {
        if s.wallpaper_folders.len() >= crate::wallpaper::MAX_FOLDERS {
            return Err(WispError::invalid("Up to 10 wallpaper folders."));
        }
        s.wallpaper_folders.push(folder);
    }
    forget_listing();
    let snap = super::settings::commit(&app, &state, s)?;
    Ok(snap.settings.wallpaper_folders)
}

#[tauri::command]
pub fn wallpaper_folder_remove(app: AppHandle, folder: String) -> CmdResult<Vec<String>> {
    let state = app.state::<AppState>();
    let mut s = state.settings();
    s.wallpaper_folders.retain(|f| f != &folder);
    forget_listing();
    let snap = super::settings::commit(&app, &state, s)?;
    Ok(snap.settings.wallpaper_folders)
}

/// Whether `path` is one Wisp may read or set: a recent wallpaper, the
/// current one, or an image in the user's chosen folders.
pub fn known_wallpaper(state: &AppState, path: &str) -> bool {
    let s = state.settings();
    s.recent_wallpapers.iter().any(|p| p == path)
        || crate::wallpaper::current().as_deref() == Some(path)
        || folder_images(&s.wallpaper_folders)
            .iter()
            .any(|p| p == path)
}

/// Next (or a random) wallpaper from the folders, else from recent ones.
#[tauri::command]
pub async fn wallpaper_next(app: AppHandle, shuffle: bool) -> CmdResult<Option<String>> {
    let state = app.state::<AppState>();
    let s = state.settings();
    let folders = s.wallpaper_folders.clone();
    let recent = s.recent_wallpapers.clone();
    let picked = tauri::async_runtime::spawn_blocking(move || {
        let mut list = folder_images(&folders);
        if list.is_empty() {
            list = recent;
        }
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(7);
        let next =
            crate::wallpaper::next_in(&list, crate::wallpaper::current().as_deref(), shuffle, seed);
        if let Some(p) = &next {
            crate::wallpaper::set(p)?;
        }
        Ok::<_, WispError>(next)
    })
    .await
    .map_err(|e| WispError::internal(e.to_string()))??;
    if picked.is_none() {
        return Err(WispError::invalid(
            "Add a wallpaper folder first (Wallpaper → Add folder).",
        ));
    }
    let _ = state.db.log_action(&ActionAudit::new(
        "wallpaper",
        if shuffle { "shuffle" } else { "next" },
        "done",
        true,
    ));
    Ok(picked)
}
