use super::CmdResult;
use crate::state::IslandLayout;
use crate::windows;
use tauri::AppHandle;
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_opener::OpenerExt;
use wisp_core::tools::{validate_url, MAX_COPY_CHARS};
use wisp_core::{ErrorCode, WispError};

#[tauri::command]
pub fn set_island_expanded(app: AppHandle, expanded: bool) -> IslandLayout {
    windows::set_island_expanded(&app, expanded)
}

#[tauri::command]
pub fn hide_island(app: AppHandle) {
    windows::set_island_expanded(&app, false);
    windows::set_island_visible(&app, false);
}

#[tauri::command]
pub fn open_settings(app: AppHandle, tab: Option<String>) {
    windows::open_settings(&app, tab);
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}

/// Open a validated http(s) URL in the default browser.
#[tauri::command]
pub fn open_external(app: AppHandle, url: String) -> CmdResult<()> {
    let url = validate_url(&url)?;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| WispError::new(ErrorCode::Internal, format!("Couldn't open the link: {e}")))
}

#[tauri::command]
pub fn copy_text(app: AppHandle, text: String) -> CmdResult<()> {
    if text.chars().count() > MAX_COPY_CHARS * 5 {
        return Err(WispError::invalid("Text is too long to copy"));
    }
    app.clipboard()
        .write_text(text)
        .map_err(|e| WispError::new(ErrorCode::Internal, format!("Couldn't copy: {e}")))
}
