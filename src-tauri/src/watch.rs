//! Light background watcher (once per second): hides the notch while a
//! fullscreen app is in front and keeps the tray icon matched to the
//! taskbar theme.

use crate::state::AppState;
use crate::windows::{self, NotchMode};
use std::time::Duration;
use tauri::{AppHandle, Manager};

pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut last_light = crate::system_theme::get().system_light;
        let mut tick: u64 = 0;
        loop {
            tick += 1;
            tokio::time::sleep(Duration::from_secs(1)).await;
            let state = app.state::<AppState>();
            let fullscreen =
                state.settings().auto_hide_fullscreen && crate::foreground::is_fullscreen();
            let changed = {
                let mut f = state.fullscreen_active.lock().expect("fullscreen");
                let changed = *f != fullscreen;
                *f = fullscreen;
                changed
            };
            let open = state.island.lock().expect("island").mode == NotchMode::Open;
            if changed && !open {
                windows::set_island_visible(&app, windows::sliver_wanted(&app));
            }
            // The taskbar theme rarely changes; check it every 5 s.
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
