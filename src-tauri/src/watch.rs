//! Keeps Wisp out of the way. Re-checks what's in front the moment the
//! foreground window changes (and once a second as a fallback): the notch
//! hides over fullscreen apps, games, presentations and apps the user
//! excluded, and hold-to-talk/shortcuts pause while a game is in front.
//! Also keeps the tray icon matched to the taskbar theme.

use crate::state::AppState;
use crate::windows::{self, NotchMode};
use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Notify;
use wisp_core::screen::{self, Rules};

/// Native handle of the notch window, used to compare monitors.
fn notch_handle(app: &AppHandle) -> Option<isize> {
    #[cfg(windows)]
    {
        windows::island(app)
            .and_then(|w| w.hwnd().ok())
            .map(|h| h.0 as isize)
    }
    #[cfg(not(windows))]
    {
        let _ = app;
        None
    }
}

/// Re-evaluate what's in front and show or hide the notch accordingly.
pub fn evaluate(app: &AppHandle) {
    let state = app.state::<AppState>();
    let s = state.settings();
    let facts = crate::desktop::facts(notch_handle(app));
    let rules = Rules {
        hide_fullscreen: s.auto_hide_fullscreen,
        hide_in_games: s.hide_in_games,
        never_over: &s.never_over_apps,
    };
    let cover = screen::decide(&facts, &rules);
    let game = screen::game_active(&facts, &rules);
    if state.game_active.swap(game, Ordering::SeqCst) != game {
        crate::shortcuts::set_game_mode(app, game);
    }
    crate::hotkeys::set_paused(game);
    let hidden = cover.hidden();
    let changed = {
        let mut f = state.fullscreen_active.lock().expect("fullscreen");
        let changed = *f != hidden;
        *f = hidden;
        changed
    };
    let prev = std::mem::replace(&mut *state.cover.lock().expect("cover"), cover);
    let open = state.island.lock().expect("island").mode == NotchMode::Open;
    if changed {
        if hidden && open && game {
            // A game took the foreground while the bar was open: get out of the way.
            windows::close_bar(app);
        } else if !open {
            windows::set_island_visible(app, windows::sliver_wanted(app));
        }
    }
    if prev != cover {
        let _ = app.emit_to(windows::ISLAND, "wisp://cover", cover);
    }
}

pub fn spawn(app: AppHandle) {
    let wake = Arc::new(Notify::new());
    let w = wake.clone();
    crate::desktop::watch_foreground(move || w.notify_one());
    tauri::async_runtime::spawn(async move {
        let mut last_light = crate::system_theme::get().system_light;
        let mut tick: u64 = 0;
        loop {
            // Wake immediately on a foreground change, else poll every second.
            let _ = tokio::time::timeout(Duration::from_secs(1), wake.notified()).await;
            // Let the new window settle (fullscreen transitions take a frame).
            tokio::time::sleep(Duration::from_millis(60)).await;
            tick += 1;
            evaluate(&app);
            // The taskbar theme rarely changes; check it every few seconds.
            if !tick.is_multiple_of(5) {
                continue;
            }
            let light = crate::system_theme::get().system_light;
            if light != last_light {
                last_light = light;
                crate::tray::refresh(&app);
            }
        }
    });
}
