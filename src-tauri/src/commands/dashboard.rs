//! Dashboard tabs: controls, stats, clipboard history, launcher, power,
//! wallpaper, weather and speed test. Every input is typed and validated.

use super::CmdResult;
use crate::power::PowerAction;
use crate::state::AppState;
use crate::system_controls::{self, ControlsState, MediaCommand};
use serde::Serialize;
use tauri::{AppHandle, Emitter, State};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_opener::OpenerExt;
use wisp_core::launcher::{self, LaunchItem};
use wisp_core::net::{self, SpeedResult, Weather};
use wisp_core::settings::WeatherPlace;
use wisp_core::storage::{ActionAudit, AgentJob, AppUsage, ClipEntry};
use wisp_core::{ErrorCode, WispError};

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> CmdResult<T> + Send + 'static,
) -> CmdResult<T> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| WispError::internal(e.to_string()))?
}

fn audit(state: &AppState, kind: &str, summary: &str, outcome: &str) {
    let _ = state
        .db
        .log_action(&ActionAudit::new(kind, summary, outcome, true));
}

// ---- Controls (run on the shared COM worker) ------------------------------

use crate::com_worker;

#[tauri::command]
pub async fn controls_state() -> ControlsState {
    com_worker::run(system_controls::state)
        .await
        .unwrap_or_default()
}

/// What's playing, for the notch peek (cached briefly; cheap to call).
#[tauri::command]
pub async fn now_playing() -> Option<system_controls::NowPlaying> {
    com_worker::run(system_controls::now_playing)
        .await
        .ok()
        .flatten()
}

#[tauri::command]
pub async fn set_volume(percent: u8) -> CmdResult<()> {
    com_worker::run(move || system_controls::set_volume(percent.min(100))).await?
}

#[tauri::command]
pub async fn set_muted(muted: bool) -> CmdResult<()> {
    com_worker::run(move || system_controls::set_muted(muted)).await?
}

#[tauri::command]
pub async fn media_command(command: MediaCommand) -> CmdResult<()> {
    com_worker::run(move || system_controls::media(command)).await?
}

#[tauri::command]
pub async fn set_radio(kind: String, on: bool) -> CmdResult<()> {
    if kind != "wifi" && kind != "bluetooth" {
        return Err(WispError::invalid("Unknown radio"));
    }
    com_worker::run(move || system_controls::set_radio(&kind, on)).await?
}

#[tauri::command]
pub async fn set_dark_mode(app: AppHandle, dark: bool) -> CmdResult<()> {
    blocking(move || system_controls::set_dark_mode(dark)).await?;
    crate::tray::refresh(&app);
    Ok(())
}

// ---- Stats ---------------------------------------------------------------

#[tauri::command]
pub async fn stats_snapshot(app: AppHandle) -> CmdResult<crate::stats::StatsSnapshot> {
    blocking(move || {
        use tauri::Manager;
        Ok(app.state::<AppState>().stats.snapshot())
    })
    .await
}

#[tauri::command]
pub async fn speed_test() -> CmdResult<SpeedResult> {
    net::speed_test(net::SPEED_URL).await
}

// ---- Weather -------------------------------------------------------------

#[tauri::command]
pub async fn weather_search(query: String) -> CmdResult<Vec<WeatherPlace>> {
    net::search_places(net::GEOCODE_BASE, &query).await
}

#[tauri::command]
pub async fn weather_get(state: State<'_, AppState>) -> CmdResult<Option<Weather>> {
    let Some(place) = state.settings().weather_place else {
        return Ok(None);
    };
    net::forecast(net::FORECAST_BASE, &place).await.map(Some)
}

// ---- Clipboard history -----------------------------------------------------

#[tauri::command]
pub fn clips_list(state: State<'_, AppState>, kind: Option<String>) -> CmdResult<Vec<ClipEntry>> {
    let kind = kind.filter(|k| k == "text" || k == "link");
    state.db.list_clips(kind.as_deref())
}

#[tauri::command]
pub fn clip_pin(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    pinned: bool,
) -> CmdResult<bool> {
    let ok = state.db.pin_clip(&id, pinned)?;
    let _ = app.emit("wisp://data-changed", "clipboard");
    Ok(ok)
}

#[tauri::command]
pub fn clip_delete(app: AppHandle, state: State<'_, AppState>, id: String) -> CmdResult<bool> {
    let ok = state.db.delete_clip(&id)?;
    let _ = app.emit("wisp://data-changed", "clipboard");
    Ok(ok)
}

#[tauri::command]
pub fn clips_clear(
    app: AppHandle,
    state: State<'_, AppState>,
    include_pinned: bool,
) -> CmdResult<u64> {
    let n = state.db.clear_clips(include_pinned)?;
    let _ = app.emit("wisp://data-changed", "clipboard");
    Ok(n)
}

/// Copy a stored entry back to the clipboard.
#[tauri::command]
pub fn clip_copy(app: AppHandle, state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let clip = state
        .db
        .list_clips(None)?
        .into_iter()
        .find(|c| c.id == id)
        .ok_or_else(|| WispError::invalid("That clipboard item is gone"))?;
    app.clipboard()
        .write_text(clip.text)
        .map_err(|e| WispError::new(ErrorCode::Internal, format!("Couldn't copy: {e}")))
}

// ---- Launcher ------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Catalogue {
    pub items: Vec<LaunchItem>,
    pub recent: Vec<AppUsage>,
}

/// The launcher catalogue, scanned once and then served from memory.
pub async fn cached_catalogue(app: &AppHandle, refresh: bool) -> std::sync::Arc<Vec<LaunchItem>> {
    use tauri::Manager;
    if !refresh {
        if let Some(c) = app
            .state::<AppState>()
            .catalogue
            .lock()
            .expect("catalogue")
            .clone()
        {
            return c;
        }
    }
    let items = std::sync::Arc::new(
        tauri::async_runtime::spawn_blocking(crate::apps::catalogue)
            .await
            .unwrap_or_default(),
    );
    *app.state::<AppState>().catalogue.lock().expect("catalogue") = Some(items.clone());
    items
}

#[tauri::command]
pub async fn launcher_list(app: AppHandle, refresh: Option<bool>) -> CmdResult<Catalogue> {
    let items = cached_catalogue(&app, refresh.unwrap_or(false)).await;
    use tauri::Manager;
    let recent = app.state::<AppState>().db.recent_apps(20)?;
    Ok(Catalogue {
        items: items.as_ref().clone(),
        recent,
    })
}

/// Launch a catalogue item. Only the id is trusted from the UI: the target
/// always comes from the host's own catalogue (Start-menu shortcuts, Steam
/// games by id, built-in Windows URIs), and is validated again.
#[tauri::command]
pub async fn launch_item(
    app: AppHandle,
    state: State<'_, AppState>,
    item: LaunchItem,
) -> CmdResult<()> {
    let catalogue = cached_catalogue(&app, false).await;
    let item = match launcher::find_by_id(&item.id, &catalogue) {
        Some(found) => found.clone(),
        None => {
            // The Start menu may have changed since the catalogue was built.
            let fresh = cached_catalogue(&app, true).await;
            launcher::find_by_id(&item.id, &fresh)
                .cloned()
                .ok_or_else(|| WispError::invalid("That app isn't in the launcher any more"))?
        }
    };
    launcher::validate_target(&item.target)?;
    let result = if item.id.starts_with("lnk:") {
        app.opener().open_path(&item.target, None::<&str>)
    } else {
        app.opener().open_url(&item.target, None::<&str>)
    };
    let name: String = item.name.chars().take(80).collect();
    match result {
        Ok(()) => {
            let _ = state.db.record_launch(&item.id);
            audit(&state, "launch_app", &format!("Opened {name}"), "done");
            Ok(())
        }
        Err(e) => {
            audit(&state, "launch_app", &format!("Opened {name}"), "failed");
            Err(WispError::new(
                ErrorCode::Internal,
                format!("Couldn't open {name}: {e}"),
            ))
        }
    }
}

// ---- Power ---------------------------------------------------------------

/// The UI shows a confirmation before calling this.
#[tauri::command]
pub async fn power_action(app: AppHandle, action: PowerAction) -> CmdResult<()> {
    let result = blocking(move || crate::power::run(action)).await;
    use tauri::Manager;
    audit(
        &app.state::<AppState>(),
        "power",
        action.label(),
        if result.is_ok() { "done" } else { "failed" },
    );
    result
}

// ---- Wallpaper -----------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WallpaperInfo {
    pub current: Option<String>,
    pub recent: Vec<String>,
}

#[tauri::command]
pub async fn wallpaper_info(state: State<'_, AppState>) -> CmdResult<WallpaperInfo> {
    let current = tauri::async_runtime::spawn_blocking(crate::wallpaper::current)
        .await
        .unwrap_or(None);
    Ok(WallpaperInfo {
        current,
        recent: state.settings().recent_wallpapers,
    })
}

/// Preview of a recent wallpaper (only paths Wisp itself set are readable).
#[tauri::command]
pub async fn wallpaper_thumb(state: State<'_, AppState>, path: String) -> CmdResult<String> {
    if !super::personalize::known_wallpaper(&state, &path) {
        return Err(WispError::invalid("Unknown wallpaper"));
    }
    blocking(move || {
        crate::wallpaper::validate(&path)?;
        crate::wallpaper::cached_thumbnail(&path)
    })
    .await
}

fn remember_wallpaper(app: &AppHandle, path: &str) -> CmdResult<()> {
    use tauri::Manager;
    let state = app.state::<AppState>();
    let mut s = state.settings();
    s.recent_wallpapers = crate::wallpaper::push_recent(&s.recent_wallpapers, path);
    super::settings::commit(app, &state, s).map(|_| ())
}

/// Ask the user to choose an image, then set it as the wallpaper.
#[tauri::command]
pub async fn wallpaper_pick(app: AppHandle) -> CmdResult<Option<String>> {
    use tauri_plugin_dialog::DialogExt;
    let a = app.clone();
    let picked = tauri::async_runtime::spawn_blocking(move || {
        a.dialog()
            .file()
            .set_title("Choose a wallpaper")
            .add_filter("Images", &["jpg", "jpeg", "png", "bmp"])
            .blocking_pick_file()
    })
    .await
    .map_err(|e| WispError::internal(e.to_string()))?;
    let Some(path) = picked.and_then(|p| p.into_path().ok()) else {
        return Ok(None);
    };
    let path = path.to_string_lossy().into_owned();
    let p = path.clone();
    blocking(move || crate::wallpaper::set(&p)).await?;
    remember_wallpaper(&app, &path)?;
    Ok(Some(path))
}

/// Switch to one of the recent wallpapers.
#[tauri::command]
pub async fn wallpaper_set(app: AppHandle, path: String) -> CmdResult<()> {
    use tauri::Manager;
    if !super::personalize::known_wallpaper(&app.state::<AppState>(), &path) {
        return Err(WispError::invalid("Choose the image again with Browse"));
    }
    let p = path.clone();
    blocking(move || crate::wallpaper::set(&p)).await?;
    remember_wallpaper(&app, &path)
}

// ---- Agents --------------------------------------------------------------

#[tauri::command]
pub async fn agent_start(app: AppHandle, kind: crate::agents::AgentKind) -> CmdResult<AgentJob> {
    crate::agents::start(app, kind).await
}

#[tauri::command]
pub fn agent_cancel(app: AppHandle, id: String) {
    crate::agents::cancel(&app, &id);
}

#[tauri::command]
pub fn agents_list(state: State<'_, AppState>) -> CmdResult<Vec<AgentJob>> {
    state.db.list_jobs(30)
}

#[tauri::command]
pub fn agent_delete(app: AppHandle, state: State<'_, AppState>, id: String) -> CmdResult<bool> {
    // Remove the file it wrote, too (only if it's still Wisp's).
    if let Some(path) = state
        .db
        .list_jobs(200)?
        .into_iter()
        .find(|j| j.id == id)
        .and_then(|j| j.output_path)
        .and_then(|p| crate::agents::owned_output(&app, &p))
    {
        let _ = std::fs::remove_file(path);
    }
    let ok = state.db.delete_job(&id)?;
    let _ = app.emit("wisp://data-changed", "jobs");
    Ok(ok)
}

/// Open a finished agent's output file (only files Wisp wrote to Documents\Wisp).
#[tauri::command]
pub fn agent_open(app: AppHandle, state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let job = state
        .db
        .list_jobs(200)?
        .into_iter()
        .find(|j| j.id == id)
        .ok_or_else(|| WispError::invalid("Unknown job"))?;
    let path = job
        .output_path
        .ok_or_else(|| WispError::invalid("This job has no output yet"))?;
    let real = crate::agents::owned_output(&app, &path)
        .ok_or_else(|| WispError::invalid("That file has moved or isn't Wisp's"))?;
    // Replicas made by older versions get the same clean-up before opening.
    if real
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("html"))
    {
        let html =
            std::fs::read_to_string(&real).map_err(|e| WispError::internal(e.to_string()))?;
        let clean = wisp_core::sanitize::sanitize_html(&html);
        if clean != html {
            std::fs::write(&real, clean).map_err(|e| WispError::internal(e.to_string()))?;
        }
    }
    app.opener()
        .open_path(real.to_string_lossy(), None::<&str>)
        .map_err(|e| WispError::new(ErrorCode::Internal, format!("Couldn't open it: {e}")))
}

/// The foreground app's process name (local only) for app-aware suggestions.
#[tauri::command]
pub fn foreground_app(state: State<'_, AppState>) -> Option<String> {
    let t = (*state.insert_target.lock().expect("target"))?;
    crate::foreground::process_name(t)
}

// ---- Desktop awareness ------------------------------------------------------

/// The user's open windows, for "switch to …" in the command bar. Titles
/// stay on this PC.
#[tauri::command]
pub async fn open_windows() -> Vec<wisp_core::tools::OpenWindow> {
    tauri::async_runtime::spawn_blocking(crate::desktop::open_windows)
        .await
        .unwrap_or_default()
}

/// Bring one of those windows to the front.
#[tauri::command]
pub async fn switch_window(app: AppHandle, id: i64) -> CmdResult<()> {
    let ok = tauri::async_runtime::spawn_blocking(move || crate::desktop::focus_window(id))
        .await
        .unwrap_or(false);
    if !ok {
        return Err(WispError::invalid("That window isn't open any more"));
    }
    crate::windows::close_bar(&app);
    Ok(())
}

/// Why the notch is hidden right now ("show" when it isn't).
#[tauri::command]
pub fn screen_status(state: State<'_, AppState>) -> wisp_core::screen::Cover {
    *state.cover.lock().expect("cover")
}
