use super::CmdResult;
use crate::capture;
use crate::state::AppState;
use tauri::{AppHandle, State};
use wisp_core::capture::{Highlight, Point, SelectionRect};

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

#[tauri::command]
pub async fn capture_select_lasso(
    app: AppHandle,
    monitor: usize,
    points: Vec<Point>,
    view_width: f64,
    view_height: f64,
) -> CmdResult<()> {
    capture::select_lasso(&app, monitor, &points, view_width, view_height)
}

/// "Show me": highlight the on-screen element that answers the question.
#[tauri::command]
pub async fn show_me(app: AppHandle, question: String) -> CmdResult<Vec<Highlight>> {
    let q = question.trim();
    if q.is_empty() || q.chars().count() > 500 {
        return Err(wisp_core::WispError::invalid("Ask what to find on screen"));
    }
    capture::show_me(&app, q).await
}

#[tauri::command]
pub fn highlights_get(state: State<'_, AppState>) -> Vec<Highlight> {
    state.highlights.lock().expect("highlights").clone()
}
