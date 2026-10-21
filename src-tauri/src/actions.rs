//! Execution of validated desktop actions. Only `ValidatedAction`s reach
//! here; there is no path from model text to a shell.

use crate::state::AppState;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_opener::OpenerExt;
use wisp_core::storage::NewTask;
use wisp_core::tools::ValidatedAction;
use wisp_core::{ErrorCode, Result, WispError};

/// Run an action and return a short result message for the model and UI.
pub fn execute(app: &AppHandle, action: &ValidatedAction) -> Result<String> {
    let state = app.state::<AppState>();
    match action {
        ValidatedAction::OpenApp { name, target } => {
            app.opener()
                .open_path(target, None::<&str>)
                .map_err(|e| failed(format!("Couldn't open {name}: {e}")))?;
            Ok(format!("Opened {name}."))
        }
        ValidatedAction::OpenUrl { url } => {
            app.opener()
                .open_url(url, None::<&str>)
                .map_err(|e| failed(format!("Couldn't open the link: {e}")))?;
            Ok("Opened the link in the default browser.".into())
        }
        ValidatedAction::CopyText { text } => {
            app.clipboard()
                .write_text(text.clone())
                .map_err(|e| failed(format!("Couldn't copy: {e}")))?;
            Ok("Copied to the clipboard.".into())
        }
        ValidatedAction::CreateReminder {
            title,
            notes,
            due_at,
        } => {
            let task = state.db.create_task(&NewTask {
                title: title.clone(),
                notes: notes.clone(),
                due_at: Some(*due_at),
                notification_enabled: true,
            })?;
            let _ = app.emit("wisp://data-changed", "tasks");
            Ok(format!(
                "Reminder \"{}\" created for {}.",
                task.title,
                due_at
                    .with_timezone(&chrono::Local)
                    .format("%a %-d %b at %H:%M")
            ))
        }
        ValidatedAction::Remember { content, category } => {
            state.db.add_memory(content, category, true)?;
            let _ = app.emit("wisp://data-changed", "memory");
            Ok("Saved to memory.".into())
        }
    }
}

fn failed(msg: String) -> WispError {
    WispError::new(ErrorCode::Internal, msg)
}
