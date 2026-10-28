//! Execution of validated desktop actions. Only `ValidatedAction`s reach
//! here; there is no path from model text to a shell.

use crate::state::AppState;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_opener::OpenerExt;
use wisp_core::storage::{NewTask, Repeat};
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
                repeat: Repeat::None,
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
        ValidatedAction::Launch { item } => {
            let res = if item.id.starts_with("lnk:") {
                app.opener().open_path(&item.target, None::<&str>)
            } else {
                app.opener().open_url(&item.target, None::<&str>)
            };
            res.map_err(|e| failed(format!("Couldn't open {}: {e}", item.name)))?;
            let _ = state.db.record_launch(&item.id);
            Ok(format!("Opened {}.", item.name))
        }
        ValidatedAction::ChangeSetting { change } => crate::winsettings::apply(change),
        ValidatedAction::OpenFile { id, .. } => crate::fileindex::open(app, *id, false),
        ValidatedAction::SwitchTo { id, app: name } => {
            if crate::desktop::focus_window(*id) {
                Ok(format!("Switched to {name}."))
            } else {
                Err(failed(format!("{name} isn't open any more.")))
            }
        }
    }
}

/// Run an action from async code: Windows settings go to the COM worker
/// (audio devices, brightness); everything else runs here.
pub async fn run(app: &AppHandle, action: &ValidatedAction) -> Result<String> {
    if let ValidatedAction::ChangeSetting { change } = action {
        let change = change.clone();
        return crate::com_worker::run(move || crate::winsettings::apply(&change)).await?;
    }
    execute(app, action)
}

fn failed(msg: String) -> WispError {
    WispError::new(ErrorCode::Internal, msg)
}
