//! Reminders, memory and clipboard: local features that work offline.

use super::CmdResult;
use crate::state::AppState;
use chrono::{DateTime, Utc};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_clipboard_manager::ClipboardExt;
use wisp_core::storage::{MemoryEntry, NewTask, Task};

fn changed(app: &AppHandle, what: &str) {
    let _ = app.emit("wisp://data-changed", what);
}

#[tauri::command]
pub fn tasks_list(state: State<'_, AppState>, include_completed: bool) -> CmdResult<Vec<Task>> {
    state.db.list_tasks(include_completed)
}

#[tauri::command]
pub fn task_create(app: AppHandle, state: State<'_, AppState>, task: NewTask) -> CmdResult<Task> {
    let t = state.db.create_task(&task)?;
    changed(&app, "tasks");
    Ok(t)
}

#[tauri::command]
pub fn task_update(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    task: NewTask,
) -> CmdResult<Task> {
    let t = state.db.update_task(&id, &task)?;
    changed(&app, "tasks");
    Ok(t)
}

#[tauri::command]
pub fn task_complete(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    completed: bool,
) -> CmdResult<Task> {
    let t = state.db.set_task_completed(&id, completed)?;
    changed(&app, "tasks");
    Ok(t)
}

#[tauri::command]
pub fn task_snooze(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    until: DateTime<Utc>,
) -> CmdResult<Task> {
    let t = state.db.snooze_task(&id, until)?;
    changed(&app, "tasks");
    Ok(t)
}

#[tauri::command]
pub fn task_delete(app: AppHandle, state: State<'_, AppState>, id: String) -> CmdResult<bool> {
    let ok = state.db.delete_task(&id)?;
    changed(&app, "tasks");
    Ok(ok)
}

#[tauri::command]
pub fn memory_list(state: State<'_, AppState>) -> CmdResult<Vec<MemoryEntry>> {
    state.db.list_memory()
}

/// Called only from the Memory screen or after the user approves a suggestion.
#[tauri::command]
pub fn memory_add(
    app: AppHandle,
    state: State<'_, AppState>,
    content: String,
    category: String,
) -> CmdResult<MemoryEntry> {
    let m = state.db.add_memory(&content, &category, true)?;
    changed(&app, "memory");
    Ok(m)
}

#[tauri::command]
pub fn memory_update(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    content: String,
    category: String,
    enabled: bool,
) -> CmdResult<MemoryEntry> {
    let m = state.db.update_memory(&id, &content, &category, enabled)?;
    changed(&app, "memory");
    Ok(m)
}

#[tauri::command]
pub fn memory_delete(app: AppHandle, state: State<'_, AppState>, id: String) -> CmdResult<bool> {
    let ok = state.db.delete_memory(&id)?;
    changed(&app, "memory");
    Ok(ok)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClipboardText {
    pub text: Option<String>,
}

/// Read clipboard text once, on an explicit user action. Never polled.
#[tauri::command]
pub fn clipboard_read(app: AppHandle) -> CmdResult<ClipboardText> {
    match app.clipboard().read_text() {
        Ok(t) if !t.trim().is_empty() => Ok(ClipboardText { text: Some(t) }),
        Ok(_) => Ok(ClipboardText { text: None }),
        // Images, files and other formats: report "no text" instead of failing.
        Err(_) => Ok(ClipboardText { text: None }),
    }
}
