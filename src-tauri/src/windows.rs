//! The notch (island) and settings windows.
//!
//! The island lives at the top-centre of a monitor and has three modes:
//! a tiny glowing **sliver** (idle), a small **peek** notch on hover, and the
//! **open** command bar. It only ever covers what its current mode needs.

use crate::state::AppState;
use serde::{Deserialize, Serialize};
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, Position, Size, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder,
};

pub const ISLAND: &str = "island";
pub const SETTINGS: &str = "settings";

/// Logical sizes per mode (open height is chosen by the UI).
pub const SLIVER: (f64, f64) = (180.0, 10.0);
pub const PEEK: (f64, f64) = (380.0, 46.0);
pub const OPEN_WIDTH: f64 = 760.0;
pub const OPEN_MIN_HEIGHT: f64 = 96.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum NotchMode {
    #[default]
    Sliver,
    Peek,
    Open,
}

#[derive(Debug, Clone, Copy, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IslandLayout {
    pub mode: NotchMode,
    /// Kept for older UI code paths: `mode == Open`.
    pub expanded: bool,
    /// Logical height of the open bar.
    pub height: f64,
}

pub fn island(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(ISLAND)
}

/// (x, y, w, h, scale) of the work area of the monitor under the cursor,
/// falling back to the primary monitor.
fn target_monitor(win: &WebviewWindow) -> Option<(i32, i32, i32, i32, f64)> {
    let cursor = win.cursor_position().ok();
    let monitors = win.available_monitors().unwrap_or_default();
    let pick = cursor
        .and_then(|c| {
            monitors.iter().find(|m| {
                let (p, s) = (m.position(), m.size());
                c.x >= p.x as f64
                    && c.y >= p.y as f64
                    && c.x < (p.x + s.width as i32) as f64
                    && c.y < (p.y + s.height as i32) as f64
            })
        })
        .cloned()
        .or_else(|| win.primary_monitor().ok().flatten())?;
    let a = pick.work_area();
    Some((
        a.position.x,
        a.position.y,
        a.size.width as i32,
        a.size.height as i32,
        pick.scale_factor(),
    ))
}

/// Top-centre placement for a logical size on a monitor (x, y, w, h, scale).
pub fn top_centre(
    monitor: (i32, i32, i32, i32, f64),
    logical: (f64, f64),
    user_scale: f64,
) -> (i32, i32, u32, u32) {
    let (mx, my, mw, mh, scale) = monitor;
    let s = scale * user_scale;
    let w = ((logical.0 * s).round() as i32).min(mw);
    let h = ((logical.1 * s).round() as i32).min(mh);
    (mx + (mw - w) / 2, my, w as u32, h as u32)
}

/// Switch the island's mode. `height` (logical) is used for the open bar.
pub fn set_mode(app: &AppHandle, mode: NotchMode, height: Option<f64>) -> IslandLayout {
    let state = app.state::<AppState>();
    let Some(win) = island(app) else {
        return IslandLayout::default();
    };
    let settings = state.settings();
    let mut layout = *state.island.lock().expect("island");
    layout.mode = mode;
    layout.expanded = mode == NotchMode::Open;
    let logical = match mode {
        NotchMode::Sliver => SLIVER,
        NotchMode::Peek => PEEK,
        NotchMode::Open => {
            if let Some(h) = height.filter(|h| h.is_finite()) {
                layout.height = h.max(OPEN_MIN_HEIGHT);
            }
            if layout.height < OPEN_MIN_HEIGHT {
                layout.height = OPEN_MIN_HEIGHT;
            }
            (OPEN_WIDTH, layout.height)
        }
    };
    if let Some(m) = target_monitor(&win) {
        // Never taller than 85% of the screen.
        let max_h = m.3 as f64 * 0.85 / (m.4 * settings.island_scale as f64);
        let logical = (logical.0, logical.1.min(max_h));
        let (x, y, w, h) = top_centre(m, logical, settings.island_scale as f64);
        let _ = win.set_size(Size::Physical(PhysicalSize::new(w, h)));
        let _ = win.set_position(Position::Physical(PhysicalPosition::new(x, y)));
    }
    *state.island.lock().expect("island") = layout;
    let _ = app.emit_to(ISLAND, "wisp://layout", layout);
    layout
}

/// Backwards-compatible helper: open the bar or return to the sliver.
pub fn set_island_expanded(app: &AppHandle, expanded: bool) -> IslandLayout {
    set_mode(
        app,
        if expanded {
            NotchMode::Open
        } else {
            NotchMode::Sliver
        },
        None,
    )
}

/// Initial placement at startup.
pub fn place_island_initially(app: &AppHandle) {
    set_mode(app, NotchMode::Sliver, None);
}

pub fn set_island_visible(app: &AppHandle, visible: bool) {
    let Some(win) = island(app) else { return };
    if visible {
        let _ = win.show();
    } else {
        let _ = win.hide();
    }
    let _ = app.emit_to(ISLAND, "wisp://visibility", visible);
}

/// Whether the idle sliver should be on screen right now.
pub fn sliver_wanted(app: &AppHandle) -> bool {
    let state = app.state::<AppState>();
    let s = state.settings();
    !s.tucked && !*state.fullscreen_active.lock().expect("fullscreen")
}

/// Close the bar: back to the sliver, or hidden when tucked/fullscreen.
pub fn close_bar(app: &AppHandle) {
    set_mode(app, NotchMode::Sliver, None);
    set_island_visible(app, sliver_wanted(app));
}

/// Shortcut toggle: open the bar, or close it if it's already open.
pub fn toggle_island(app: &AppHandle) {
    let open = app.state::<AppState>().island.lock().expect("island").mode == NotchMode::Open;
    let visible = island(app)
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false);
    if open && visible {
        close_bar(app);
    } else {
        summon_island(app);
    }
}

/// Open the command bar focused, remembering the app the user came from.
pub fn summon_island(app: &AppHandle) {
    let Some(win) = island(app) else { return };
    if let Some(t) = crate::foreground::current() {
        *app.state::<AppState>()
            .insert_target
            .lock()
            .expect("target") = Some(t);
    }
    set_mode(app, NotchMode::Open, None);
    set_island_visible(app, true);
    let _ = win.set_focus();
    let _ = app.emit_to(ISLAND, "wisp://focus-prompt", ());
}

/// Tuck the notch away completely (or bring it back). Saved like any other
/// setting, so the Settings window and the UI stay in sync.
pub fn set_tucked(app: &AppHandle, tucked: bool) {
    let state = app.state::<AppState>();
    let mut s = state.settings();
    if s.tucked == tucked {
        return;
    }
    s.tucked = tucked;
    let _ = crate::commands::settings::commit(app, &state, s);
}

pub fn open_settings(app: &AppHandle, tab: Option<String>) {
    if let Some(win) = app.get_webview_window(SETTINGS) {
        let _ = win.unminimize();
        let _ = win.show();
        let _ = win.set_focus();
        if let Some(t) = tab {
            let _ = app.emit_to(SETTINGS, "wisp://settings-tab", t);
        }
        return;
    }
    let url = match &tab {
        Some(t) => format!(
            "index.html?tab={}",
            t.chars()
                .filter(|c| c.is_ascii_alphanumeric())
                .collect::<String>()
        ),
        None => "index.html".to_string(),
    };
    let _ = WebviewWindowBuilder::new(app, SETTINGS, WebviewUrl::App(url.into()))
        .title("Wisp Settings")
        .inner_size(880.0, 660.0)
        .min_inner_size(640.0, 480.0)
        .center()
        .build();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn centres_on_the_monitor_top() {
        // 1920×1040 work area at 150% scaling.
        let m = (0, 0, 1920, 1040, 1.5);
        assert_eq!(top_centre(m, SLIVER, 1.0), (825, 0, 270, 15));
        // Second monitor to the right, larger UI scale; width is capped to the monitor.
        let m2 = (1920, 40, 800, 600, 1.0);
        let (x, y, w, _) = top_centre(m2, (OPEN_WIDTH, 300.0), 1.3);
        assert_eq!((x, y, w), (1920, 40, 800));
    }
}
