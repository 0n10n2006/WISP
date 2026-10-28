//! File search: the UI sends a query and gets back ids, names and where
//! each file is; opening goes by id, so the webview never names a path.

use super::CmdResult;
use crate::fileindex::{self, FilesStatus};
use crate::state::AppState;
use tauri::{AppHandle, Manager, State};
use wisp_core::files::{FileHit, MAX_RESULTS, MAX_USER_FOLDERS};
use wisp_core::WispError;

use fileindex::status;

#[tauri::command]
pub async fn files_search(
    app: AppHandle,
    query: String,
    limit: Option<u32>,
) -> CmdResult<Vec<FileHit>> {
    if !app.state::<AppState>().settings().index_files || query.trim().chars().count() < 2 {
        return Ok(Vec::new());
    }
    let query: String = query.chars().take(200).collect();
    let limit = limit.unwrap_or(8).min(MAX_RESULTS as u32) as usize;
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<AppState>().files.search(&query, limit)
    })
    .await
    .map_err(|e| WispError::internal(e.to_string()))
}

#[tauri::command]
pub async fn file_open(app: AppHandle, id: i64) -> CmdResult<String> {
    tauri::async_runtime::spawn_blocking(move || fileindex::open(&app, id, false))
        .await
        .map_err(|e| WispError::internal(e.to_string()))?
}

#[tauri::command]
pub async fn file_reveal(app: AppHandle, id: i64) -> CmdResult<String> {
    tauri::async_runtime::spawn_blocking(move || fileindex::open(&app, id, true))
        .await
        .map_err(|e| WispError::internal(e.to_string()))?
}

#[tauri::command]
pub fn files_status(app: AppHandle) -> FilesStatus {
    status(&app)
}

#[tauri::command]
pub fn files_rebuild(app: AppHandle, state: State<'_, AppState>) -> FilesStatus {
    if state.settings().index_files {
        state.files.rebuild();
    }
    status(&app)
}

/// Choose a folder in a native picker and add it to the index.
#[tauri::command]
pub async fn files_folder_add(app: AppHandle) -> CmdResult<FilesStatus> {
    use tauri_plugin_dialog::DialogExt;
    let a = app.clone();
    let picked = tauri::async_runtime::spawn_blocking(move || {
        a.dialog()
            .file()
            .set_title("Choose a folder for Wisp to search")
            .blocking_pick_folder()
    })
    .await
    .map_err(|e| WispError::internal(e.to_string()))?;
    let Some(folder) = picked
        .and_then(|p| p.into_path().ok())
        .map(|p| p.to_string_lossy().into_owned())
    else {
        return Ok(status(&app));
    };
    if !crate::wallpaper::is_local_folder(&folder) {
        return Err(WispError::invalid("Choose a folder on this PC."));
    }
    let state = app.state::<AppState>();
    let mut s = state.settings();
    if !s
        .index_folders
        .iter()
        .any(|f| f.eq_ignore_ascii_case(&folder))
    {
        if s.index_folders.len() >= MAX_USER_FOLDERS {
            return Err(WispError::invalid("Up to 20 extra folders."));
        }
        s.index_folders.push(folder);
    }
    super::settings::commit(&app, &state, s)?;
    Ok(status(&app))
}

#[tauri::command]
pub fn files_folder_remove(app: AppHandle, folder: String) -> CmdResult<FilesStatus> {
    let state = app.state::<AppState>();
    let mut s = state.settings();
    s.index_folders.retain(|f| f != &folder);
    super::settings::commit(&app, &state, s)?;
    Ok(status(&app))
}
