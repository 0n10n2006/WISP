//! Select to Ask: freeze the screen, let the user select a region, and hand
//! the crop to the island for confirmation. Captures live only in memory and
//! are dropped on cancel or as soon as the crop is produced.

use crate::state::AppState;
use crate::windows;
use base64::Engine;
use image::{imageops::FilterType, DynamicImage, ImageFormat, RgbaImage};
use serde::Serialize;
use std::io::Cursor;
use std::time::Duration;
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, Position, Size, WebviewUrl,
    WebviewWindowBuilder,
};
use wisp_core::capture::{
    fit_within, lasso_bounds, parse_highlights, point_in_polygon, polygon_in_crop, to_pixels,
    Highlight, Point, SelectionRect, MAX_SEND_SIDE,
};
use wisp_core::{ErrorCode, Result, WispError};

pub struct CapturedMonitor {
    pub x: i32,
    pub y: i32,
    pub image: RgbaImage,
}

#[derive(Default)]
pub struct CaptureSession {
    pub monitors: Vec<CapturedMonitor>,
    /// Ready-made JPEG of each monitor for the overlay background.
    pub previews: Vec<Vec<u8>>,
    pub island_was_visible: bool,
}

/// Bumped on every capture; an overlay that never reports ready is shown
/// anyway after a moment, and idle overlays are closed after a while.
static SESSION_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
/// Overlays stay loaded (hidden) this long after a capture, so the next one
/// opens instantly; then they're closed to free memory.
const OVERLAY_KEEP: Duration = Duration::from_secs(120);

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureReady {
    pub data_url: String,
    pub mime_type: String,
    pub data_base64: String,
    pub width: u32,
    pub height: u32,
}

const OVERLAY_PREFIX: &str = "overlay-";

fn emit_error(app: &AppHandle, err: WispError) {
    windows::summon_island(app);
    let _ = app.emit_to(windows::ISLAND, "wisp://capture-error", err);
}

fn encode(img: &DynamicImage, format: ImageFormat) -> Result<Vec<u8>> {
    let mut buf = Cursor::new(Vec::new());
    img.write_to(&mut buf, format)
        .map_err(|e| WispError::internal(format!("Couldn't encode the capture: {e}")))?;
    Ok(buf.into_inner())
}

/// Begin a capture: hide Wisp, grab every monitor, open selection overlays.
pub fn start(app: &AppHandle) {
    let state = app.state::<AppState>();
    if !state.settings().screen_context_enabled {
        emit_error(
            app,
            WispError::new(
                ErrorCode::PermissionDenied,
                "Select to ask is turned off. Turn it on in Settings → Privacy.",
            ),
        );
        return;
    }
    if state.capture.lock().expect("capture").is_some() {
        return;
    }
    let island_was_visible = windows::island(app)
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false);
    if let Some(w) = windows::island(app) {
        let _ = w.hide();
    }
    let generation = SESSION_GEN.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        // Give the compositor a moment to remove Wisp from the screen.
        tokio::time::sleep(Duration::from_millis(120)).await;
        let grabbed = tauri::async_runtime::spawn_blocking(grab_all).await;
        let monitors = match grabbed {
            Ok(Ok(m)) if !m.is_empty() => m,
            Ok(Err(e)) => return fail_start(&app, island_was_visible, e),
            _ => {
                return fail_start(
                    &app,
                    island_was_visible,
                    WispError::new(ErrorCode::Unsupported, "No screens could be captured."),
                )
            }
        };
        let geometry: Vec<(i32, i32, u32, u32)> = monitors
            .iter()
            .map(|m| (m.x, m.y, m.image.width(), m.image.height()))
            .collect();
        // Encode every screen's background in parallel.
        let (monitors, previews) = tauri::async_runtime::spawn_blocking(move || {
            let previews = encode_previews(&monitors);
            (monitors, previews)
        })
        .await
        .unwrap_or_default();
        // Overlays for screens that no longer exist go away (before the
        // session starts, so closing them can't cancel it).
        for (label, win) in app.webview_windows() {
            if let Some(i) = label
                .strip_prefix(OVERLAY_PREFIX)
                .and_then(|n| n.parse::<usize>().ok())
            {
                if i >= geometry.len() {
                    let _ = win.close();
                }
            }
        }
        *app.state::<AppState>().capture.lock().expect("capture") = Some(CaptureSession {
            monitors,
            previews,
            island_was_visible,
        });
        for (i, (x, y, w, h)) in geometry.into_iter().enumerate() {
            let label = format!("{OVERLAY_PREFIX}{i}");
            let win = match app.get_webview_window(&label) {
                // Reuse a loaded overlay: it reloads its background.
                Some(win) => {
                    let _ = app.emit_to(label.as_str(), "wisp://capture-session", generation);
                    win
                }
                None => {
                    let url = format!("index.html?window=overlay&monitor={i}");
                    let built =
                        WebviewWindowBuilder::new(&app, &label, WebviewUrl::App(url.into()))
                            .title("Wisp — select an area")
                            .decorations(false)
                            .resizable(false)
                            .always_on_top(true)
                            .skip_taskbar(true)
                            .shadow(false)
                            .transparent(true)
                            .focused(false)
                            .visible(false)
                            .build();
                    match built {
                        Ok(win) => {
                            crate::chrome::borderless(&win);
                            win
                        }
                        Err(e) => {
                            cancel(&app);
                            return emit_error(
                                &app,
                                WispError::internal(format!(
                                    "Couldn't open the selection overlay: {e}"
                                )),
                            );
                        }
                    }
                }
            };
            let _ = win.set_position(Position::Physical(PhysicalPosition::new(x, y)));
            let _ = win.set_size(Size::Physical(PhysicalSize::new(w, h)));
        }
        // The overlays show themselves once their frozen screen is painted
        // (`overlay_ready`), so the switch is seamless. Safety net: show them
        // anyway if one doesn't report in time.
        tokio::time::sleep(Duration::from_millis(1500)).await;
        if SESSION_GEN.load(std::sync::atomic::Ordering::SeqCst) == generation
            && app
                .state::<AppState>()
                .capture
                .lock()
                .expect("capture")
                .is_some()
        {
            for (label, win) in app.webview_windows() {
                if label.starts_with(OVERLAY_PREFIX) && !win.is_visible().unwrap_or(true) {
                    show_overlay(&win, &label);
                }
            }
        }
    });
}

fn show_overlay(win: &tauri::WebviewWindow, label: &str) {
    let _ = win.show();
    if label == format!("{OVERLAY_PREFIX}0") {
        let _ = win.set_focus();
    }
}

/// An overlay painted its frozen screen: show it now.
pub fn overlay_ready(app: &AppHandle, label: &str) {
    if !label.starts_with(OVERLAY_PREFIX)
        || app
            .state::<AppState>()
            .capture
            .lock()
            .expect("capture")
            .is_none()
    {
        return;
    }
    if let Some(win) = app.get_webview_window(label) {
        show_overlay(&win, label);
    }
}

/// JPEG backgrounds for the overlays, one per monitor, encoded in parallel.
fn encode_previews(monitors: &[CapturedMonitor]) -> Vec<Vec<u8>> {
    std::thread::scope(|scope| {
        let jobs: Vec<_> = monitors
            .iter()
            .map(|m| {
                scope.spawn(move || {
                    let rgb = DynamicImage::ImageRgba8(m.image.clone()).to_rgb8();
                    let mut out = Vec::new();
                    // High quality: the frozen screen should look just like
                    // the live one underneath.
                    let enc = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, 92);
                    rgb.write_with_encoder(enc).map(|_| out).unwrap_or_default()
                })
            })
            .collect();
        jobs.into_iter()
            .map(|j| j.join().unwrap_or_default())
            .collect()
    })
}

fn fail_start(app: &AppHandle, island_was_visible: bool, err: WispError) {
    if island_was_visible {
        windows::set_island_visible(app, true);
    }
    emit_error(app, err);
}

fn grab_all() -> Result<Vec<CapturedMonitor>> {
    let unsupported = |e: xcap::XCapError| {
        WispError::new(
            ErrorCode::PermissionDenied,
            format!(
                "Wisp couldn't capture the screen ({e}). Check that screen capture is allowed."
            ),
        )
    };
    let mut out = Vec::new();
    for m in xcap::Monitor::all().map_err(unsupported)? {
        let image = m.capture_image().map_err(unsupported)?;
        out.push(CapturedMonitor {
            x: m.x().unwrap_or(0),
            y: m.y().unwrap_or(0),
            image,
        });
    }
    Ok(out)
}

/// Put the overlays away. They stay loaded (hidden) for a while so the next
/// capture opens instantly, then close to free memory.
fn close_overlays(app: &AppHandle) {
    for (label, win) in app.webview_windows() {
        if label.starts_with(OVERLAY_PREFIX) {
            let _ = win.hide();
        }
    }
    let generation = SESSION_GEN.load(std::sync::atomic::Ordering::SeqCst);
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(OVERLAY_KEEP).await;
        let idle = SESSION_GEN.load(std::sync::atomic::Ordering::SeqCst) == generation
            && app
                .state::<AppState>()
                .capture
                .lock()
                .expect("capture")
                .is_none();
        if idle {
            for (label, win) in app.webview_windows() {
                if label.starts_with(OVERLAY_PREFIX) {
                    let _ = win.close();
                }
            }
        }
    });
}

/// Drop the capture and restore Wisp as it was.
pub fn cancel(app: &AppHandle) {
    let session = app
        .state::<AppState>()
        .capture
        .lock()
        .expect("capture")
        .take();
    close_overlays(app);
    if session.is_some_and(|s| s.island_was_visible) {
        windows::set_island_visible(app, true);
    }
}

/// JPEG of a whole captured monitor for the overlay background (raw bytes;
/// no base64 round trip).
pub fn preview(app: &AppHandle, index: usize) -> Result<Vec<u8>> {
    let state = app.state::<AppState>();
    let guard = state.capture.lock().expect("capture");
    let session = guard
        .as_ref()
        .ok_or_else(|| WispError::invalid("No capture in progress"))?;
    session
        .previews
        .get(index)
        .filter(|b| !b.is_empty())
        .cloned()
        .ok_or_else(|| WispError::invalid("Unknown screen"))
}

/// Crop the selection, close overlays, and give the island the result to confirm.
pub fn select(app: &AppHandle, index: usize, sel: SelectionRect) -> Result<()> {
    let state = app.state::<AppState>();
    let crop = {
        let guard = state.capture.lock().expect("capture");
        let session = guard
            .as_ref()
            .ok_or_else(|| WispError::invalid("No capture in progress"))?;
        let mon = session
            .monitors
            .get(index)
            .ok_or_else(|| WispError::invalid("Unknown screen"))?;
        let r = to_pixels(&sel, mon.image.width(), mon.image.height())?;
        image::imageops::crop_imm(&mon.image, r.x, r.y, r.width, r.height).to_image()
    };
    finish(app, crop)
}

/// Lasso: crop to the drawn shape's bounds and dim everything outside it,
/// so the model focuses on what the user circled.
pub fn select_lasso(
    app: &AppHandle,
    index: usize,
    points: &[Point],
    view_width: f64,
    view_height: f64,
) -> Result<()> {
    let state = app.state::<AppState>();
    let crop = {
        let guard = state.capture.lock().expect("capture");
        let session = guard
            .as_ref()
            .ok_or_else(|| WispError::invalid("No capture in progress"))?;
        let mon = session
            .monitors
            .get(index)
            .ok_or_else(|| WispError::invalid("Unknown screen"))?;
        let sel = lasso_bounds(points, view_width, view_height)?;
        let (iw, ih) = (mon.image.width(), mon.image.height());
        let r = to_pixels(&sel, iw, ih)?;
        let poly = polygon_in_crop(points, &sel, iw, ih, r);
        let mut crop =
            image::imageops::crop_imm(&mon.image, r.x, r.y, r.width, r.height).to_image();
        mask_outside(&mut crop, &poly);
        crop
    };
    finish(app, crop)
}

/// Darken pixels outside `poly` (crop pixel space).
pub fn mask_outside(img: &mut RgbaImage, poly: &[(f64, f64)]) {
    if poly.len() < 3 {
        return;
    }
    for (x, y, p) in img.enumerate_pixels_mut() {
        if !point_in_polygon(x as f64 + 0.5, y as f64 + 0.5, poly) {
            for c in 0..3 {
                p.0[c] = (p.0[c] as f32 * 0.22) as u8;
            }
        }
    }
}

fn to_png_b64(img: DynamicImage) -> Result<(String, u32, u32)> {
    let (w, h) = fit_within(img.width(), img.height(), MAX_SEND_SIDE);
    let img = if (w, h) == (img.width(), img.height()) {
        img
    } else {
        img.resize_exact(w, h, FilterType::Triangle)
    };
    let png = encode(&img, ImageFormat::Png)?;
    Ok((base64::engine::general_purpose::STANDARD.encode(png), w, h))
}

fn finish(app: &AppHandle, crop: RgbaImage) -> Result<()> {
    let state = app.state::<AppState>();
    // The full-screen captures are no longer needed.
    *state.capture.lock().expect("capture") = None;
    close_overlays(app);
    let (b64, w, h) = to_png_b64(DynamicImage::ImageRgba8(crop))?;
    let ready = CaptureReady {
        data_url: format!("data:image/png;base64,{b64}"),
        mime_type: "image/png".into(),
        data_base64: b64,
        width: w,
        height: h,
    };
    windows::summon_island(app);
    let _ = app.emit_to(windows::ISLAND, "wisp://capture-ready", ready);
    Ok(())
}

// ---------------------------------------------------------------------------
// One-shot screenshot of the screen the notch is on (Show me, replica agent).

pub struct Shot {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub png_b64: String,
}

/// Capture the monitor under the cursor with Wisp hidden for the moment.
/// Only called from an explicit user request.
pub async fn screenshot_here(app: &AppHandle) -> Result<Shot> {
    if !app.state::<AppState>().settings().screen_context_enabled {
        return Err(WispError::new(
            ErrorCode::PermissionDenied,
            "Screen features are turned off. Turn them on in Settings → Privacy.",
        ));
    }
    let cursor = windows::island(app).and_then(|w| w.cursor_position().ok());
    let was_visible = windows::island(app)
        .and_then(|w| w.is_visible().ok())
        .unwrap_or(false);
    windows::set_island_visible(app, false);
    tokio::time::sleep(Duration::from_millis(180)).await;
    let grabbed = tauri::async_runtime::spawn_blocking(grab_all).await;
    if was_visible {
        windows::set_island_visible(app, true);
    }
    let monitors = grabbed.map_err(|e| WispError::internal(e.to_string()))??;
    let pick = cursor
        .and_then(|c| {
            monitors.iter().position(|m| {
                c.x >= m.x as f64
                    && c.y >= m.y as f64
                    && c.x < (m.x + m.image.width() as i32) as f64
                    && c.y < (m.y + m.image.height() as i32) as f64
            })
        })
        .unwrap_or(0);
    let mon = monitors
        .into_iter()
        .nth(pick)
        .ok_or_else(|| WispError::new(ErrorCode::Unsupported, "No screens could be captured."))?;
    let (width, height) = (mon.image.width(), mon.image.height());
    let (png_b64, _, _) = tauri::async_runtime::spawn_blocking(move || {
        to_png_b64(DynamicImage::ImageRgba8(mon.image))
    })
    .await
    .map_err(|e| WispError::internal(e.to_string()))??;
    Ok(Shot {
        x: mon.x,
        y: mon.y,
        width,
        height,
        png_b64,
    })
}

pub const HIGHLIGHT: &str = "highlight";
static HIGHLIGHT_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

/// Show me: point at the on-screen element that answers `question`.
pub async fn show_me(app: &AppHandle, question: &str) -> Result<Vec<Highlight>> {
    let state = app.state::<AppState>();
    state.secrets.require()?;
    let keys = crate::routing::Keys::load(&state);
    let shot = screenshot_here(app).await?;
    let cancel = tokio_util::sync::CancellationToken::new();
    let out = crate::routing::run(app, wisp_core::router::Task::Vision, &keys, |route| {
        let req = wisp_core::gemini::prompt::show_me_request(&shot.png_b64, question, &route.model);
        let (state, keys, cancel) = (&state, &keys, &cancel);
        async move {
            (
                crate::routing::complete(state, keys, &route, &req, cancel).await,
                false,
            )
        }
    })
    .await?
    .value;
    let found = parse_highlights(&out.text);
    if found.is_empty() {
        return Ok(found);
    }
    *state.highlights.lock().expect("highlights") = found.clone();
    // Replace an earlier highlight: wait until it's really gone, since a new
    // window can't reuse the label while the old one still exists.
    if let Some(w) = app.get_webview_window(HIGHLIGHT) {
        let _ = w.close();
        for _ in 0..40 {
            if app.get_webview_window(HIGHLIGHT).is_none() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    }
    let generation = HIGHLIGHT_GEN.fetch_add(1, std::sync::atomic::Ordering::SeqCst) + 1;
    let win = WebviewWindowBuilder::new(
        app,
        HIGHLIGHT,
        WebviewUrl::App("index.html?window=highlight".into()),
    )
    .title("Wisp — show me")
    .decorations(false)
    .transparent(true)
    .resizable(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .shadow(false)
    .focused(false)
    .visible(false)
    .build()
    .map_err(|e| WispError::internal(format!("Couldn't show the highlight: {e}")))?;
    let _ = win.set_position(Position::Physical(PhysicalPosition::new(shot.x, shot.y)));
    let _ = win.set_size(Size::Physical(PhysicalSize::new(shot.width, shot.height)));
    crate::chrome::borderless(&win);
    let _ = win.set_ignore_cursor_events(true);
    let _ = win.show();
    let a = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_secs(6)).await;
        // Only close the highlight this call made, not a newer one.
        if HIGHLIGHT_GEN.load(std::sync::atomic::Ordering::SeqCst) == generation {
            if let Some(w) = a.get_webview_window(HIGHLIGHT) {
                let _ = w.close();
            }
        }
    });
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lasso_mask_dims_only_outside() {
        let mut img = RgbaImage::from_pixel(10, 10, image::Rgba([200, 200, 200, 255]));
        let tri = [(0.0, 0.0), (10.0, 0.0), (0.0, 10.0)];
        mask_outside(&mut img, &tri);
        assert_eq!(img.get_pixel(1, 1).0[0], 200);
        assert!(img.get_pixel(9, 9).0[0] < 50);
        assert_eq!(img.get_pixel(9, 9).0[3], 255);
    }
}
