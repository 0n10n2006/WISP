//! Island and settings window management.

use crate::state::{AppState, IslandLayout};
use std::sync::atomic::Ordering;
use std::time::Duration;
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, Position, Size, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder,
};
use wisp_core::settings::WindowPosition;

pub const ISLAND: &str = "island";
pub const SETTINGS: &str = "settings";

/// Logical sizes of the island window.
pub const COLLAPSED: (f64, f64) = (88.0, 88.0);
pub const EXPANDED: (f64, f64) = (400.0, 560.0);
const EDGE_MARGIN: i32 = 24;

pub fn island(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(ISLAND)
}

fn physical(win: &WebviewWindow, logical: (f64, f64)) -> (i32, i32) {
    let s = win.scale_factor().unwrap_or(1.0);
    (
        (logical.0 * s).round() as i32,
        (logical.1 * s).round() as i32,
    )
}

/// Work-area rectangles (x, y, w, h) of every monitor.
fn monitor_rects(win: &WebviewWindow) -> Vec<(i32, i32, i32, i32)> {
    win.available_monitors()
        .unwrap_or_default()
        .iter()
        .map(|m| {
            let a = m.work_area();
            (
                a.position.x,
                a.position.y,
                a.size.width as i32,
                a.size.height as i32,
            )
        })
        .collect()
}

fn rect_for_point(rects: &[(i32, i32, i32, i32)], x: i32, y: i32) -> Option<(i32, i32, i32, i32)> {
    rects
        .iter()
        .copied()
        .find(|(rx, ry, rw, rh)| x >= *rx && y >= *ry && x < rx + rw && y < ry + rh)
}

/// Place the island at its saved position, or bottom-right of the primary monitor.
pub fn place_island_initially(app: &AppHandle) {
    let Some(win) = island(app) else { return };
    let state = app.state::<AppState>();
    let saved = state.settings().window_position;
    let (cw, ch) = physical(&win, COLLAPSED);
    let rects = monitor_rects(&win);
    let pos = saved
        .filter(|p| rect_for_point(&rects, p.x + cw / 2, p.y + ch / 2).is_some())
        .map(|p| (p.x, p.y))
        .or_else(|| {
            win.primary_monitor().ok().flatten().map(|m| {
                let a = m.work_area();
                let s = m.scale_factor();
                let margin = (EDGE_MARGIN as f64 * s) as i32;
                (
                    a.position.x + a.size.width as i32 - cw - margin,
                    a.position.y + a.size.height as i32 - ch - margin,
                )
            })
        })
        .unwrap_or((100, 100));
    *state.anchor.lock().expect("anchor") = Some(pos);
    let _ = win.set_size(Size::Physical(PhysicalSize::new(cw as u32, ch as u32)));
    let _ = win.set_position(Position::Physical(PhysicalPosition::new(pos.0, pos.1)));
}

/// Resize the island between orb and panel, keeping the orb where it was and
/// the panel on-screen. Returns the layout the UI should render.
pub fn set_island_expanded(app: &AppHandle, expanded: bool) -> IslandLayout {
    let state = app.state::<AppState>();
    let Some(win) = island(app) else {
        return IslandLayout::default();
    };
    let (cw, ch) = physical(&win, COLLAPSED);
    let anchor = state
        .anchor
        .lock()
        .expect("anchor")
        .or_else(|| win.outer_position().ok().map(|p| (p.x, p.y)))
        .unwrap_or((100, 100));
    let mut layout = IslandLayout {
        expanded,
        ..Default::default()
    };
    if expanded {
        let (ew, eh) = physical(&win, EXPANDED);
        let rects = monitor_rects(&win);
        let (mx, my, mw, mh) = rect_for_point(&rects, anchor.0 + cw / 2, anchor.1 + ch / 2)
            .unwrap_or((0, 0, i32::MAX / 2, i32::MAX / 2));
        let mut x = anchor.0;
        let mut y = anchor.1;
        if x + ew > mx + mw {
            x = anchor.0 + cw - ew;
            layout.align_right = true;
        }
        if y + eh > my + mh {
            y = anchor.1 + ch - eh;
            layout.align_bottom = true;
        }
        x = x.clamp(mx, (mx + mw - ew).max(mx));
        y = y.clamp(my, (my + mh - eh).max(my));
        *state.island.lock().expect("island") = layout;
        let _ = win.set_size(Size::Physical(PhysicalSize::new(ew as u32, eh as u32)));
        let _ = win.set_position(Position::Physical(PhysicalPosition::new(x, y)));
    } else {
        *state.island.lock().expect("island") = layout;
        let _ = win.set_size(Size::Physical(PhysicalSize::new(cw as u32, ch as u32)));
        let _ = win.set_position(Position::Physical(PhysicalPosition::new(
            anchor.0, anchor.1,
        )));
    }
    let _ = app.emit_to(ISLAND, "wisp://layout", layout);
    layout
}

/// Called on every island move: track where the orb is and persist it (debounced).
pub fn on_island_moved(app: &AppHandle, pos: PhysicalPosition<i32>) {
    let state = app.state::<AppState>();
    let Some(win) = island(app) else { return };
    let layout = *state.island.lock().expect("island");
    let (cw, ch) = physical(&win, COLLAPSED);
    let size = win
        .outer_size()
        .unwrap_or(PhysicalSize::new(cw as u32, ch as u32));
    let ax = if layout.expanded && layout.align_right {
        pos.x + size.width as i32 - cw
    } else {
        pos.x
    };
    let ay = if layout.expanded && layout.align_bottom {
        pos.y + size.height as i32 - ch
    } else {
        pos.y
    };
    *state.anchor.lock().expect("anchor") = Some((ax, ay));

    let generation = state.move_generation.fetch_add(1, Ordering::SeqCst) + 1;
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(500)).await;
        let state = app.state::<AppState>();
        if state.move_generation.load(Ordering::SeqCst) != generation {
            return;
        }
        let mut s = state.settings.lock().expect("settings");
        s.window_position = Some(WindowPosition { x: ax, y: ay });
        let _ = state.settings_store.save(&s);
    });
}

pub fn set_island_visible(app: &AppHandle, visible: bool) {
    let Some(win) = island(app) else { return };
    if visible {
        let _ = win.show();
    } else {
        let _ = win.hide();
    }
    let state = app.state::<AppState>();
    let mut s = state.settings.lock().expect("settings");
    if s.island_visible != visible {
        s.island_visible = visible;
        let _ = state.settings_store.save(&s);
    }
    let _ = app.emit_to(ISLAND, "wisp://visibility", visible);
}

/// Global toggle: hidden → show expanded with the prompt focused; visible → hide.
pub fn toggle_island(app: &AppHandle) {
    let Some(win) = island(app) else { return };
    if win.is_visible().unwrap_or(false) {
        set_island_expanded(app, false);
        set_island_visible(app, false);
    } else {
        summon_island(app);
    }
}

/// Show the island expanded and focused, e.g. for a shortcut-triggered flow.
pub fn summon_island(app: &AppHandle) {
    let Some(win) = island(app) else { return };
    if let Some(t) = crate::foreground::current() {
        *app.state::<AppState>()
            .insert_target
            .lock()
            .expect("target") = Some(t);
    }
    set_island_visible(app, true);
    set_island_expanded(app, true);
    let _ = win.set_focus();
    let _ = app.emit_to(ISLAND, "wisp://focus-prompt", ());
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
        .inner_size(820.0, 640.0)
        .min_inner_size(620.0, 480.0)
        .center()
        .build();
}

#[cfg(test)]
mod tests {
    use super::rect_for_point;

    #[test]
    fn finds_containing_monitor() {
        let rects = [(0, 0, 1920, 1040), (1920, -200, 2560, 1400)];
        assert_eq!(rect_for_point(&rects, 100, 100), Some(rects[0]));
        assert_eq!(rect_for_point(&rects, 2000, -100), Some(rects[1]));
        assert_eq!(rect_for_point(&rects, -5, 5), None);
    }
}
