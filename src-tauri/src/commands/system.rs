use super::CmdResult;
use crate::state::AppState;
use crate::state::IslandLayout;
use crate::windows::{self, NotchMode};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_opener::OpenerExt;
use wisp_core::tools::{validate_public_url, MAX_COPY_CHARS};
use wisp_core::{ErrorCode, WispError};

#[tauri::command]
pub fn set_island_expanded(app: AppHandle, expanded: bool) -> IslandLayout {
    if expanded {
        windows::set_island_expanded(&app, true)
    } else {
        windows::close_bar(&app);
        *app.state::<crate::state::AppState>()
            .island
            .lock()
            .expect("island")
    }
}

/// Switch notch mode; `height` sizes the open bar to its content.
#[tauri::command]
pub fn set_island_mode(app: AppHandle, mode: NotchMode, height: Option<f64>) -> IslandLayout {
    let current = *app.state::<AppState>().island.lock().expect("island");
    if windows::resolve_request(current.mode, mode) != mode {
        // The page asked for a peek while the bar is open (a hover racing the
        // shortcut): keep the bar, and tell the page what's really on screen.
        let _ = app.emit_to(windows::ISLAND, "wisp://layout", current);
        return current;
    }
    if mode == NotchMode::Open {
        let layout = windows::set_mode(&app, mode, height);
        // An open bar is always on screen, whatever hid the sliver before.
        if windows::island(&app).is_some_and(|w| !w.is_visible().unwrap_or(true)) {
            windows::set_island_visible(&app, true);
        }
        layout
    } else {
        let layout = windows::set_mode(&app, mode, None);
        if mode == NotchMode::Sliver {
            windows::set_island_visible(&app, windows::sliver_wanted(&app));
        }
        layout
    }
}

#[tauri::command]
pub fn hide_island(app: AppHandle) {
    windows::close_bar(&app);
}

#[tauri::command]
pub fn set_tucked(app: AppHandle, tucked: bool) {
    windows::set_tucked(&app, tucked);
}

#[tauri::command]
pub fn open_settings(app: AppHandle, tab: Option<String>) {
    windows::open_settings(&app, tab);
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}

/// Open a validated http(s) URL in the default browser.
#[tauri::command]
pub fn open_external(app: AppHandle, url: String) -> CmdResult<()> {
    let url = validate_public_url(&url)?;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| WispError::new(ErrorCode::Internal, format!("Couldn't open the link: {e}")))
}

#[tauri::command]
pub fn copy_text(app: AppHandle, text: String) -> CmdResult<()> {
    if text.chars().count() > MAX_COPY_CHARS * 5 {
        return Err(WispError::invalid("Text is too long to copy"));
    }
    app.clipboard()
        .write_text(text)
        .map_err(|e| WispError::new(ErrorCode::Internal, format!("Couldn't copy: {e}")))
}

/// Start-menu apps the user can add to the allowlist.
#[tauri::command]
pub async fn installed_apps() -> Vec<crate::apps::InstalledApp> {
    tauri::async_runtime::spawn_blocking(crate::apps::list)
        .await
        .unwrap_or_default()
}

const RELEASES_API: &str = "https://api.github.com";
const RELEASES_REPO: &str = "0n10n2006/WISP";

/// Manual update check against GitHub releases. Never runs in the background.
#[tauri::command]
pub async fn check_for_updates(app: AppHandle) -> CmdResult<wisp_core::updates::UpdateInfo> {
    let version = app.package_info().version.to_string();
    wisp_core::updates::check(RELEASES_API, RELEASES_REPO, &version).await
}

/// A script error or failed call in one of Wisp's windows, kept in the local
/// error log (redacted, rate-limited, never sent anywhere).
#[tauri::command]
pub fn report_ui_error(window: tauri::WebviewWindow, message: String) {
    let message: String = message.chars().take(2000).collect();
    crate::errlog::log(window.label(), &message);
}

/// The webview finished booting (used by `--self-test`).
#[tauri::command]
pub fn ui_ready() {
    crate::selftest::UI_READY.store(true, std::sync::atomic::Ordering::SeqCst);
}

/// Battery, disks, memory, CPU, uptime and the local address, for questions
/// answered on this PC ("battery", "disk space") without asking Gemini.
#[tauri::command]
pub async fn pc_info(app: AppHandle) -> CmdResult<crate::stats::PcInfo> {
    tauri::async_runtime::spawn_blocking(move || {
        let stats = &app.state::<crate::state::AppState>().stats;
        let mut snap = stats.snapshot();
        if snap.cpu == 0.0 {
            // CPU use needs two samples.
            std::thread::sleep(std::time::Duration::from_millis(250));
            snap = stats.snapshot();
        }
        crate::stats::PcInfo {
            battery: crate::stats::battery(),
            local_ip: crate::stats::local_ip(),
            stats: snap,
        }
    })
    .await
    .map_err(|e| WispError::internal(e.to_string()))
}
