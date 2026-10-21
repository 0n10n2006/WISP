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
use wisp_core::capture::{fit_within, to_pixels, SelectionRect, MAX_SEND_SIDE};
use wisp_core::{ErrorCode, Result, WispError};

pub struct CapturedMonitor {
    pub x: i32,
    pub y: i32,
    pub image: RgbaImage,
}

#[derive(Default)]
pub struct CaptureSession {
    pub monitors: Vec<CapturedMonitor>,
    pub island_was_visible: bool,
}

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
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        // Give the compositor a moment to remove Wisp from the screen.
        tokio::time::sleep(Duration::from_millis(180)).await;
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
        *app.state::<AppState>().capture.lock().expect("capture") = Some(CaptureSession {
            monitors,
            island_was_visible,
        });
        for (i, (x, y, w, h)) in geometry.into_iter().enumerate() {
            let url = format!("index.html?window=overlay&monitor={i}");
            let built = WebviewWindowBuilder::new(
                &app,
                format!("{OVERLAY_PREFIX}{i}"),
                WebviewUrl::App(url.into()),
            )
            .title("Wisp — select an area")
            .decorations(false)
            .resizable(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .shadow(false)
            .visible(false)
            .build();
            match built {
                Ok(win) => {
                    let _ = win.set_position(Position::Physical(PhysicalPosition::new(x, y)));
                    let _ = win.set_size(Size::Physical(PhysicalSize::new(w, h)));
                    let _ = win.show();
                    if i == 0 {
                        let _ = win.set_focus();
                    }
                }
                Err(e) => {
                    cancel(&app);
                    return emit_error(
                        &app,
                        WispError::internal(format!("Couldn't open the selection overlay: {e}")),
                    );
                }
            }
        }
    });
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

fn close_overlays(app: &AppHandle) {
    for (label, win) in app.webview_windows() {
        if label.starts_with(OVERLAY_PREFIX) {
            let _ = win.close();
        }
    }
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

/// JPEG of a whole captured monitor for the overlay background.
pub fn preview(app: &AppHandle, index: usize) -> Result<String> {
    let state = app.state::<AppState>();
    let guard = state.capture.lock().expect("capture");
    let session = guard
        .as_ref()
        .ok_or_else(|| WispError::invalid("No capture in progress"))?;
    let mon = session
        .monitors
        .get(index)
        .ok_or_else(|| WispError::invalid("Unknown screen"))?;
    let rgb = DynamicImage::ImageRgba8(mon.image.clone()).to_rgb8();
    let bytes = encode(&DynamicImage::ImageRgb8(rgb), ImageFormat::Jpeg)?;
    Ok(format!(
        "data:image/jpeg;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(bytes)
    ))
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
    // The full-screen captures are no longer needed.
    *state.capture.lock().expect("capture") = None;
    close_overlays(app);

    let (w, h) = fit_within(crop.width(), crop.height(), MAX_SEND_SIDE);
    let img = if (w, h) == (crop.width(), crop.height()) {
        DynamicImage::ImageRgba8(crop)
    } else {
        DynamicImage::ImageRgba8(crop).resize_exact(w, h, FilterType::Triangle)
    };
    let png = encode(&img, ImageFormat::Png)?;
    let b64 = base64::engine::general_purpose::STANDARD.encode(png);
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
