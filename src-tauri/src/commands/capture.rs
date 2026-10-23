use super::CmdResult;
use crate::capture;
use crate::state::AppState;
use tauri::{AppHandle, State};
use wisp_core::capture::{Highlight, Point, SelectionRect};

#[tauri::command]
pub fn capture_start(app: AppHandle) {
    capture::start(&app);
}

#[tauri::command]
pub async fn capture_preview(app: AppHandle, monitor: usize) -> CmdResult<tauri::ipc::Response> {
    capture::preview(&app, monitor).map(tauri::ipc::Response::new)
}

/// The overlay painted its frozen screen and can be shown.
#[tauri::command]
pub fn capture_overlay_ready(app: AppHandle, window: tauri::WebviewWindow) {
    capture::overlay_ready(&app, window.label());
}

#[tauri::command]
pub async fn capture_select(app: AppHandle, monitor: usize, rect: SelectionRect) -> CmdResult<()> {
    capture::select(&app, monitor, rect)
}

#[tauri::command]
pub fn capture_cancel(app: AppHandle) {
    capture::cancel(&app);
}

#[tauri::command]
pub async fn capture_select_lasso(
    app: AppHandle,
    monitor: usize,
    points: Vec<Point>,
    view_width: f64,
    view_height: f64,
) -> CmdResult<()> {
    capture::select_lasso(&app, monitor, &points, view_width, view_height)
}

/// "Show me": highlight the on-screen element that answers the question.
#[tauri::command]
pub async fn show_me(app: AppHandle, question: String) -> CmdResult<Vec<Highlight>> {
    let q = question.trim();
    if q.is_empty() || q.chars().count() > 500 {
        return Err(wisp_core::WispError::invalid("Ask what to find on screen"));
    }
    capture::show_me(&app, q).await
}

#[tauri::command]
pub fn highlights_get(state: State<'_, AppState>) -> Vec<Highlight> {
    state.highlights.lock().expect("highlights").clone()
}

/// What happened when searching Google Lens for a circled area.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LensOutcome {
    /// The results page opened in the browser.
    pub opened: bool,
    /// Lens couldn't be reached directly: the image is on the clipboard and
    /// Lens is open, ready for Ctrl+V.
    pub pasted: bool,
}

/// Reverse image search: send the circled area (and only that) to Google
/// Lens and open the results. Only on an explicit click.
#[tauri::command]
pub async fn reverse_image_search(
    app: AppHandle,
    state: State<'_, AppState>,
    data_base64: String,
) -> CmdResult<LensOutcome> {
    use base64::Engine;
    use tauri_plugin_clipboard_manager::ClipboardExt;
    use tauri_plugin_opener::OpenerExt;
    use wisp_core::WispError;
    if !state.settings().screen_context_enabled {
        return Err(WispError::new(
            wisp_core::ErrorCode::PermissionDenied,
            "Screen features are turned off in Settings → Privacy.",
        ));
    }
    if data_base64.len() > wisp_core::net::MAX_LENS_BYTES * 4 / 3 + 8 {
        return Err(WispError::invalid("That image is too large to search"));
    }
    let png = base64::engine::general_purpose::STANDARD
        .decode(data_base64.as_bytes())
        .map_err(|_| WispError::invalid("That isn't an image Wisp can search for"))?;
    let audit = |outcome: &str| {
        let _ = state.db.log_action(&wisp_core::storage::ActionAudit::new(
            "reverse_image_search",
            "host=lens.google.com",
            outcome,
            true,
        ));
    };
    match wisp_core::net::lens_upload(wisp_core::net::LENS_BASE, &png).await {
        Ok(url) => {
            app.opener()
                .open_url(url, None::<&str>)
                .map_err(|e| WispError::internal(format!("Couldn't open the browser: {e}")))?;
            audit("done");
            Ok(LensOutcome {
                opened: true,
                pasted: false,
            })
        }
        Err(_) => {
            // Lens changed or is unreachable: hand the image over by paste.
            let img = image::load_from_memory(&png)
                .map_err(|_| WispError::invalid("That isn't an image Wisp can search for"))?
                .to_rgba8();
            let (w, h) = img.dimensions();
            app.clipboard()
                .write_image(&tauri::image::Image::new_owned(img.into_raw(), w, h))
                .map_err(|e| WispError::internal(format!("Couldn't copy the image: {e}")))?;
            app.opener()
                .open_url(wisp_core::net::LENS_BASE, None::<&str>)
                .map_err(|e| WispError::internal(format!("Couldn't open the browser: {e}")))?;
            audit("pasted");
            Ok(LensOutcome {
                opened: false,
                pasted: true,
            })
        }
    }
}
