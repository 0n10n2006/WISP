//! Global shortcut registration. Failures are reported to the UI rather than
//! crashing, so the user can pick another combination in Settings.

use crate::state::{AppState, ShortcutIssue};
use crate::windows;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, ShortcutState};
use wisp_core::shortcuts::Shortcuts;

pub fn register_all(app: &AppHandle, shortcuts: &Shortcuts) -> Vec<ShortcutIssue> {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    let mut issues = Vec::new();
    for (action, accel) in shortcuts.entries() {
        if accel.is_empty() {
            continue;
        }
        let action_name = action.to_string();
        let res = gs.on_shortcut(accel, move |app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                dispatch(app, &action_name);
            }
        });
        if let Err(e) = res {
            issues.push(ShortcutIssue {
                action: action.to_string(),
                accelerator: accel.to_string(),
                message: format!("Couldn't register {accel}; another app may be using it ({e})."),
            });
        }
    }
    let state = app.state::<AppState>();
    *state.shortcut_issues.lock().expect("issues") = issues.clone();
    let _ = app.emit("wisp://shortcut-issues", &issues);
    issues
}

fn dispatch(app: &AppHandle, action: &str) {
    match action {
        "toggleIsland" => windows::toggle_island(app),
        "selectToAsk" => crate::capture::start(app),
        other => {
            windows::summon_island(app);
            let _ = app.emit_to(windows::ISLAND, "wisp://shortcut", other);
        }
    }
}
