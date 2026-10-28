//! The bar's other searches: file previews, browser tabs and history,
//! running programs and exchange rates. Everything is looked up by id on
//! the host; nothing found here is sent to Gemini.

use super::CmdResult;
use crate::browsers::Tab;
use crate::preview::FilePreview;
use crate::state::AppState;
use crate::stats::ProcGroup;
use tauri::{AppHandle, Manager, State};
use tauri_plugin_opener::OpenerExt;
use wisp_core::browsers::Page;
use wisp_core::net::{self, Rates};
use wisp_core::storage::ActionAudit;
use wisp_core::{ErrorCode, WispError};

fn join<T>(r: Result<T, tauri::Error>) -> CmdResult<T> {
    r.map_err(|e| WispError::internal(e.to_string()))
}

fn browsers_off() -> WispError {
    WispError::invalid("Browser search is off. Turn it on in Settings → Search.")
}

#[tauri::command]
pub async fn file_preview(app: AppHandle, id: i64) -> CmdResult<FilePreview> {
    join(
        tauri::async_runtime::spawn_blocking(move || {
            let state = app.state::<AppState>();
            if !state.settings().index_files {
                return Err(WispError::invalid("File search is off."));
            }
            let hit = state
                .files
                .get(id)
                .ok_or_else(|| WispError::invalid("That file isn't in the index any more."))?;
            let path = wisp_core::files::check_openable(&hit, &state.files.roots())?;
            Ok(crate::preview::cached(&hit, &path))
        })
        .await,
    )?
}

/// Browsers with a profile on this PC (for Settings → Search).
#[tauri::command]
pub async fn browsers_installed() -> Vec<String> {
    tauri::async_runtime::spawn_blocking(crate::browsers::installed)
        .await
        .unwrap_or_default()
}

#[tauri::command]
pub async fn browser_tabs(app: AppHandle) -> CmdResult<Vec<Tab>> {
    if !app.state::<AppState>().settings().browser_search {
        return Ok(Vec::new());
    }
    let tabs = crate::com_worker::run(crate::browsers::list_tabs).await?;
    app.state::<AppState>().browsers.set_tabs(tabs.clone());
    Ok(tabs)
}

#[tauri::command]
pub async fn browser_tab_switch(app: AppHandle, id: i64) -> CmdResult<()> {
    let state = app.state::<AppState>();
    if !state.settings().browser_search {
        return Err(browsers_off());
    }
    let tab = state
        .browsers
        .tab(id)
        .ok_or_else(|| WispError::invalid("That tab is closed."))?;
    let ok = crate::com_worker::run(move || crate::browsers::switch_to(&tab)).await?;
    if ok {
        Ok(())
    } else {
        Err(WispError::invalid(
            "Couldn't switch to that tab. It may have closed.",
        ))
    }
}

#[tauri::command]
pub async fn browser_search(
    app: AppHandle,
    query: String,
    limit: Option<u32>,
) -> CmdResult<Vec<Page>> {
    let state = app.state::<AppState>();
    if !state.settings().browser_search || query.trim().chars().count() < 2 {
        return Ok(Vec::new());
    }
    let query: String = query.chars().take(200).collect();
    let limit = limit.unwrap_or(6).min(30) as usize;
    join(
        tauri::async_runtime::spawn_blocking(move || {
            app.state::<AppState>().browsers.search(&query, limit)
        })
        .await,
    )
}

/// Open a page from history or bookmarks: the address comes from the
/// host's own copy and is checked again.
#[tauri::command]
pub async fn browser_open(app: AppHandle, id: i64) -> CmdResult<()> {
    let state = app.state::<AppState>();
    if !state.settings().browser_search {
        return Err(browsers_off());
    }
    let page = state
        .browsers
        .page(id)
        .ok_or_else(|| WispError::invalid("That page isn't in your history any more."))?;
    let url = wisp_core::tools::validate_public_url(&page.url)?;
    app.opener()
        .open_url(&url, None::<&str>)
        .map_err(|e| WispError::internal(format!("Couldn't open the page: {e}")))?;
    let host = wisp_core::browsers::host(&url).to_string();
    let _ = state.db.log_action(&ActionAudit::new(
        "open_page",
        &format!("host={host}"),
        "done",
        false,
    ));
    Ok(())
}

#[tauri::command]
pub async fn processes_list(app: AppHandle) -> CmdResult<Vec<ProcGroup>> {
    join(
        tauri::async_runtime::spawn_blocking(move || {
            let windows: Vec<(String, i64)> = crate::desktop::open_windows()
                .into_iter()
                .map(|w| (crate::stats::process_key(&w.app), w.id))
                .collect();
            app.state::<AppState>().stats.processes(&windows)
        })
        .await,
    )
}

/// End a program by name (the UI asks first). Windows' own processes and
/// Wisp are refused.
#[tauri::command]
pub async fn process_end(state: State<'_, AppState>, name: String) -> CmdResult<String> {
    let n = name.clone();
    let result =
        join(tauri::async_runtime::spawn_blocking(move || crate::stats::end_processes(&n)).await)?;
    let key = crate::stats::process_key(&name);
    let summary = format!("process={}", wisp_core::redact::summarize_text(&key, 40));
    match result {
        Ok(count) => {
            let _ = state
                .db
                .log_action(&ActionAudit::new("end_process", &summary, "done", true));
            Ok(if count == 1 {
                format!("Ended {name}.")
            } else {
                format!("Ended {count} {name} processes.")
            })
        }
        Err(e) => {
            let mut audit = ActionAudit::new("end_process", &summary, "failed", true);
            audit.detail = Some(e.message.clone());
            let _ = state.db.log_action(&audit);
            Err(e)
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct SavedRates {
    fetched_at: i64,
    rates: Rates,
}

/// Exchange rates: fetched at most every 12 hours, kept in `rates.json`,
/// and the saved ones are used when offline.
#[tauri::command]
pub async fn currency_rates(app: AppHandle) -> CmdResult<Rates> {
    let file = app
        .path()
        .app_data_dir()
        .map_err(|e| WispError::internal(e.to_string()))?
        .join("rates.json");
    let saved: Option<SavedRates> = std::fs::read_to_string(&file)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .filter(|s: &SavedRates| net::valid_rates(&s.rates));
    let now = chrono::Utc::now().timestamp();
    if let Some(s) = &saved {
        if now - s.fetched_at < 12 * 3600 {
            return Ok(s.rates.clone());
        }
    }
    match net::fetch_rates(net::RATES_BASE).await {
        Ok(rates) => {
            let keep = SavedRates {
                fetched_at: now,
                rates: rates.clone(),
            };
            if let Ok(json) = serde_json::to_string(&keep) {
                let _ = std::fs::write(&file, json);
            }
            Ok(rates)
        }
        Err(e) => saved.map(|s| s.rates).ok_or_else(|| {
            WispError::new(
                ErrorCode::Offline,
                format!("Couldn't get exchange rates: {}", e.message),
            )
        }),
    }
}
