use super::CmdResult;
use crate::capture;
use tauri::AppHandle;
use wisp_core::capture::SelectionRect;

#[tauri::command]
pub fn capture_start(app: AppHandle) {
    capture::start(&app);
}

#[tauri::command]
pub async fn capture_preview(app: AppHandle, monitor: usize) -> CmdResult<String> {
    capture::preview(&app, monitor)
}

#[tauri::command]
pub async fn capture_select(app: AppHandle, monitor: usize, rect: SelectionRect) -> CmdResult<()> {
    capture::select(&app, monitor, rect)
}

#[tauri::command]
pub fn capture_cancel(app: AppHandle) {
    capture::cancel(&app);
}
