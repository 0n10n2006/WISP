//! The notch (island) and settings windows.
//!
//! The island lives at the top of a monitor (or, as a floating ball, in one
//! of its corners) and has three modes: a tiny glowing **sliver** or ball
//! (idle), a small **peek** on hover, and the **open** command bar. It only
//! ever covers what its current mode needs.

use crate::state::AppState;
use serde::{Deserialize, Serialize};
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, Position, Size, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder,
};
use wisp_core::settings::{BallCorner, NotchPosition, NotchStyle, Settings};

pub const ISLAND: &str = "island";
pub const SETTINGS: &str = "settings";

/// Logical sizes per mode (open height is chosen by the UI).
pub const SLIVER: (f64, f64) = (180.0, 10.0);
/// The resting floating ball: the ball plus room for its glow.
pub const BALL: (f64, f64) = (56.0, 56.0);
/// Gap between the ball (and what grows from it) and the screen's edges.
pub const BALL_MARGIN: f64 = 12.0;
/// Peek window: the notch shape plus room for its shoulders and shadow.
pub const PEEK: (f64, f64) = (400.0, 58.0);
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

/// Where the island is anchored on its monitor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    /// Flush with the top edge, as a notch.
    Top(NotchPosition),
    /// A floating ball in a corner; whatever opens grows out of that corner.
    Corner(BallCorner),
}

impl Anchor {
    pub fn from_settings(s: &Settings) -> Self {
        match s.notch_style {
            NotchStyle::Notch => Anchor::Top(s.notch_position),
            NotchStyle::Ball => Anchor::Corner(s.ball_corner),
        }
    }
}

/// Physical rect (x, y, w, h) for `logical` at `anchor` on `monitor`.
pub fn place(
    monitor: (i32, i32, i32, i32, f64),
    logical: (f64, f64),
    user_scale: f64,
    anchor: Anchor,
) -> (i32, i32, u32, u32) {
    let corner = match anchor {
        Anchor::Top(position) => return place_top(monitor, logical, user_scale, position),
        Anchor::Corner(c) => c,
    };
    let (mx, my, mw, mh, scale) = monitor;
    let s = scale * user_scale;
    let w = ((logical.0 * s).round() as i32).min(mw);
    let h = ((logical.1 * s).round() as i32).min(mh);
    let margin = (BALL_MARGIN * s).round() as i32;
    let left = matches!(corner, BallCorner::TopLeft | BallCorner::BottomLeft);
    let top = matches!(corner, BallCorner::TopLeft | BallCorner::TopRight);
    // Keep the anchored edges fixed, so a growing bar extends away from them.
    let x = if left {
        mx + margin.min(mw - w)
    } else {
        mx + (mw - w - margin).max(0)
    };
    let y = if top {
        my + margin.min(mh - h)
    } else {
        my + (mh - h - margin).max(0)
    };
    (x, y, w as u32, h as u32)
}

/// Placement along the top edge (left, centre or right).
pub fn place_top(
    monitor: (i32, i32, i32, i32, f64),
    logical: (f64, f64),
    user_scale: f64,
    position: NotchPosition,
) -> (i32, i32, u32, u32) {
    let (mx, my, mw, mh, scale) = monitor;
    let s = scale * user_scale;
    let w = ((logical.0 * s).round() as i32).min(mw);
    let h = ((logical.1 * s).round() as i32).min(mh);
    let margin = (48.0 * s).round() as i32;
    let x = match position {
        NotchPosition::Left => mx + margin.min(mw - w),
        NotchPosition::Center => mx + (mw - w) / 2,
        NotchPosition::Right => mx + (mw - w - margin).max(0),
    };
    (x, my, w as u32, h as u32)
}

/// Physical rect of the island, cached so unchanged layouts cost nothing.
static LAST_RECT: std::sync::Mutex<Option<(i32, i32, u32, u32)>> = std::sync::Mutex::new(None);
/// Monitor the open bar lives on; it stays put while the bar resizes.
type Monitor = (i32, i32, i32, i32, f64);
static OPEN_MONITOR: std::sync::Mutex<Option<Monitor>> = std::sync::Mutex::new(None);

/// What has to change to go from `old` to `new` (move, resize).
pub fn rect_changes(old: Option<(i32, i32, u32, u32)>, new: (i32, i32, u32, u32)) -> (bool, bool) {
    match old {
        None => (true, true),
        Some(o) => ((o.0, o.1) != (new.0, new.1), (o.2, o.3) != (new.2, new.3)),
    }
}

/// The mode a request from the page really leads to. A hover that arrives
/// just after the shortcut opened the bar must not shrink it into the peek.
pub fn resolve_request(current: NotchMode, requested: NotchMode) -> NotchMode {
    if requested == NotchMode::Peek && current == NotchMode::Open {
        NotchMode::Open
    } else {
        requested
    }
}

/// Switch the island's mode. `height` (logical) is used for the open bar.
pub fn set_mode(app: &AppHandle, mode: NotchMode, height: Option<f64>) -> IslandLayout {
    let state = app.state::<AppState>();
    let Some(win) = island(app) else {
        return IslandLayout::default();
    };
    let settings = state.settings();
    let before = *state.island.lock().expect("island");
    let mut layout = before;
    layout.mode = mode;
    layout.expanded = mode == NotchMode::Open;
    let anchor = Anchor::from_settings(&settings);
    let logical = match mode {
        NotchMode::Sliver if matches!(anchor, Anchor::Corner(_)) => BALL,
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
    // Never hold a lock across a window call: off the main thread those calls
    // wait for the main thread, which may itself be waiting for the lock.
    let staying_open = mode == NotchMode::Open && before.mode == NotchMode::Open;
    let cached = *OPEN_MONITOR.lock().expect("monitor");
    let monitor = match cached {
        Some(m) if staying_open => Some(m),
        _ => {
            let m = target_monitor(&win);
            *OPEN_MONITOR.lock().expect("monitor") = m;
            m
        }
    };
    if let Some(m) = monitor {
        // Never taller than 85% of the screen.
        let max_h = m.3 as f64 * 0.85 / (m.4 * settings.island_scale as f64);
        let logical = (logical.0, logical.1.min(max_h));
        let rect = place(m, logical, settings.island_scale as f64, anchor);
        let (moved, resized) = {
            let mut last = LAST_RECT.lock().expect("rect");
            let changes = rect_changes(*last, rect);
            *last = Some(rect);
            changes
        };
        // Move first, then size: moving onto a monitor with another scale
        // makes Windows rescale the window, so the size must come last.
        if moved {
            let _ = win.set_position(Position::Physical(PhysicalPosition::new(rect.0, rect.1)));
        }
        if resized || moved {
            let _ = win.set_size(Size::Physical(PhysicalSize::new(rect.2, rect.3)));
        }
    }
    *state.island.lock().expect("island") = layout;
    if layout.mode != before.mode || (layout.height - before.height).abs() > 0.5 {
        let _ = app.emit_to(ISLAND, "wisp://layout", layout);
    }
    layout
}

/// Forget the cached rect (e.g. after Windows rescaled the window), so the
/// next layout is applied in full.
pub fn forget_rect() {
    *LAST_RECT.lock().expect("rect") = None;
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
    let theme = app.state::<AppState>().settings().theme;
    if let Some(win) = app.get_webview_window(SETTINGS) {
        crate::chrome::apply_theme(&win, theme);
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
    // Build off the main thread: on Windows, creating a webview from a sync
    // command or an event handler deadlocks WebView2 (and with it all IPC).
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if app.get_webview_window(SETTINGS).is_some() {
            return;
        }
        if let Ok(win) = WebviewWindowBuilder::new(&app, SETTINGS, WebviewUrl::App(url.into()))
            .title("Wisp Settings")
            .inner_size(880.0, 660.0)
            .min_inner_size(640.0, 480.0)
            .center()
            .build()
        {
            crate::chrome::apply_theme(&win, theme);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_late_hover_never_shrinks_the_open_bar() {
        use NotchMode::*;
        assert_eq!(resolve_request(Sliver, Peek), Peek);
        assert_eq!(resolve_request(Open, Peek), Open);
        assert_eq!(resolve_request(Open, Sliver), Sliver);
        assert_eq!(resolve_request(Peek, Open), Open);
    }

    #[test]
    fn centres_on_the_monitor_top() {
        // 1920×1040 work area at 150% scaling.
        let m = (0, 0, 1920, 1040, 1.5);
        assert_eq!(
            place_top(m, SLIVER, 1.0, NotchPosition::Center),
            (825, 0, 270, 15)
        );
        // Second monitor to the right, larger UI scale; width is capped to the monitor.
        let m2 = (1920, 40, 800, 600, 1.0);
        let (x, y, w, _) = place_top(m2, (OPEN_WIDTH, 300.0), 1.3, NotchPosition::Center);
        assert_eq!((x, y, w), (1920, 40, 800));
    }

    #[test]
    fn left_and_right_placements_keep_a_margin() {
        let m = (0, 0, 1920, 1040, 1.0);
        assert_eq!(place_top(m, (400.0, 58.0), 1.0, NotchPosition::Left).0, 48);
        assert_eq!(
            place_top(m, (400.0, 58.0), 1.0, NotchPosition::Right).0,
            1920 - 400 - 48
        );
    }

    #[test]
    fn the_ball_sits_in_its_corner_and_grows_away_from_it() {
        // 1920×1040 work area (taskbar excluded) at 100%.
        let m = (0, 0, 1920, 1040, 1.0);
        let at = |c, size| place(m, size, 1.0, Anchor::Corner(c));
        assert_eq!(at(BallCorner::TopLeft, BALL), (12, 12, 56, 56));
        assert_eq!(
            at(BallCorner::BottomRight, BALL),
            (1920 - 56 - 12, 1040 - 56 - 12, 56, 56)
        );
        // The bar opens from the same corner: bottom corners grow upward and
        // keep the bottom edge where the ball was.
        let ball = at(BallCorner::BottomLeft, BALL);
        let bar = at(BallCorner::BottomLeft, (OPEN_WIDTH, 400.0));
        assert_eq!(bar.0, ball.0);
        assert_eq!(bar.1 + bar.3 as i32, ball.1 + ball.3 as i32);
        let peek = at(BallCorner::TopRight, PEEK);
        assert_eq!(peek.0 + peek.2 as i32, 1920 - 12);
        assert_eq!(peek.1, 12);
        // A monitor elsewhere, at 150%: still inside it.
        let m2 = (-1280, 200, 1280, 984, 1.5);
        let r = place(m2, BALL, 1.0, Anchor::Corner(BallCorner::BottomRight));
        assert_eq!(r, (-84 - 18, 200 + 984 - 84 - 18, 84, 84));
        // Too big for the screen: clamped, never off it.
        let r = place(
            (0, 0, 700, 500, 1.0),
            (OPEN_WIDTH, 620.0),
            1.0,
            Anchor::Corner(BallCorner::BottomRight),
        );
        assert_eq!(r, (0, 0, 700, 500));
    }

    #[test]
    fn notch_anchor_matches_top_placement() {
        let m = (0, 0, 1920, 1040, 1.0);
        let s = Settings::default();
        assert_eq!(
            Anchor::from_settings(&s),
            Anchor::Top(NotchPosition::Center)
        );
        assert_eq!(
            place(m, PEEK, 1.0, Anchor::Top(NotchPosition::Left)),
            place_top(m, PEEK, 1.0, NotchPosition::Left)
        );
        let ball = Settings {
            notch_style: NotchStyle::Ball,
            ..Settings::default()
        };
        assert_eq!(
            Anchor::from_settings(&ball),
            Anchor::Corner(BallCorner::BottomRight)
        );
    }

    #[test]
    fn only_changed_parts_are_applied() {
        let a = (10, 0, 760, 300);
        assert_eq!(rect_changes(None, a), (true, true));
        assert_eq!(rect_changes(Some(a), a), (false, false));
        assert_eq!(rect_changes(Some(a), (10, 0, 760, 340)), (false, true));
        assert_eq!(rect_changes(Some(a), (20, 0, 760, 300)), (true, false));
    }
}
