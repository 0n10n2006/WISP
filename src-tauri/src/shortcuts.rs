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

/// Release Wisp's key combinations while a game is in front (so the game
/// gets them), and take them back afterwards.
pub fn set_game_mode(app: &AppHandle, game: bool) {
    let state = app.state::<AppState>();
    let s = state.settings();
    let release = game && !s.shortcuts_in_games;
    let app2 = app.clone();
    let _ = app.run_on_main_thread(move || {
        if release {
            let _ = app2.global_shortcut().unregister_all();
        } else {
            register_all(&app2, &s.global_shortcuts);
        }
    });
}

fn dispatch(app: &AppHandle, action: &str) {
    match action {
        "toggleIsland" => windows::toggle_island(app),
        "selectToAsk" => crate::capture::start(app),
        "tuck" => {
            let tucked = app.state::<crate::state::AppState>().settings().tucked;
            windows::set_tucked(app, !tucked);
        }
        other => {
            windows::summon_island(app);
            let _ = app.emit_to(windows::ISLAND, "wisp://shortcut", other);
        }
    }
}
