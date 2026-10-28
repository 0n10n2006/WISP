//! Wisp desktop host (Tauri 2). OS integration lives here; logic lives in `wisp-core`.

mod actions;
mod agents;
mod apps;
mod browsers;
mod capture;
mod chrome;
mod cleanup;
mod clipboard_history;
mod com_worker;
mod commands;
mod crash;
mod desktop;
mod errlog;
mod fileindex;
mod foreground;
mod hotkeys;
mod power;
mod preview;
mod reminders;
mod routing;
mod secrets;
mod selftest;
mod shortcuts;
mod speech;
mod startmenu;
mod state;
mod stats;
mod system_controls;
mod system_theme;
mod tray;
mod voicetyping;
mod wallpaper;
mod watch;
mod windows;
mod winsettings;

use state::AppState;
use std::collections::HashMap;
use std::sync::Mutex;
use tauri::{Emitter, Manager, WindowEvent};
use wisp_core::gemini::GeminiClient;
use wisp_core::settings::SettingsStore;
use wisp_core::storage::Db;

pub fn run() {
    let process_start = selftest::start();
    let self_test = selftest::requested();
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            windows::summon_island(app);
        }))
        .plugin(navigation_guard())
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
        ))
        .setup(move |app| {
            selftest::mark("setup-start");
            // Before any window exists, so taskbar and search group them as Wisp.
            startmenu::set_app_id();
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            startmenu::register_notifications(&data_dir);
            selftest::mark("data-dir");
            let store = SettingsStore::new(data_dir.join("settings.json"));
            let loaded = store.load();
            let settings = loaded.settings;
            selftest::mark("settings");
            let log_dir = data_dir.join("logs");
            crash::install(log_dir.clone(), app.package_info().version.to_string());
            errlog::init(&log_dir);
            let mut warnings: Vec<String> = crash::unseen_crashes(&log_dir).into_iter().collect();
            let db = match Db::open(&data_dir.join("wisp.db")) {
                Ok(db) => db,
                Err(e) => {
                    warnings.push(format!(
                        "Wisp couldn't open its local database ({}). History, reminders and memory won't be saved this session.",
                        e.message
                    ));
                    Db::open_in_memory()?
                }
            };
            let _ = db.fail_interrupted_jobs();
            selftest::mark("database");
            app.manage(AppState {
                capture: Mutex::new(None),
                scheduled: Mutex::new(Vec::new()),
                confirmations: Mutex::new(HashMap::new()),
                insert_target: Mutex::new(None),
                settings: Mutex::new(settings.clone()),
                settings_store: store,
                startup_warnings: Mutex::new(warnings),
                settings_recovered_from: Mutex::new(
                    loaded.recovered_from.map(|p| p.display().to_string()),
                ),
                gemini: GeminiClient::new(),
                db,
                secrets: secrets::SecretStore::gemini(),
                openrouter_key: secrets::SecretStore::openrouter(),
                routing: routing::Routing::load(Some(data_dir.join("quota.json"))),
                requests: Mutex::new(HashMap::new()),
                shortcut_issues: Mutex::new(Vec::new()),
                island: Mutex::new(Default::default()),
                fullscreen_active: Mutex::new(false),
                game_active: Default::default(),
                cover: Mutex::new(wisp_core::screen::Cover::Show),
                stats: Default::default(),
                catalogue: Mutex::new(None),
                files: fileindex::Indexer::open(&data_dir.join("files.db")),
                browsers: Default::default(),
                highlights: Mutex::new(Vec::new()),
            });

            let handle = app.handle().clone();
            selftest::mark("state");
            tray::create(&handle)?;
            selftest::mark("tray");
            reminders::spawn(handle.clone());
            windows::place_island_initially(&handle);
            if let Some(win) = windows::island(&handle) {
                chrome::borderless(&win);
                let _ = win.set_always_on_top(settings.always_on_top);
                let autostarted = std::env::args().any(|a| a == "--autostart");
                let show = !settings.tucked && !(settings.start_hidden && autostarted);
                if show || !settings.onboarding_complete || self_test {
                    let _ = win.show();
                }
                selftest::mark("sliver-shown");
                let h = handle.clone();
                win.on_window_event(move |e| {
                    // Clicking elsewhere lets the UI decide whether to fold away.
                    match e {
                        WindowEvent::Focused(false) => {
                            let _ = h.emit_to(windows::ISLAND, "wisp://blur", ());
                        }
                        // Windows rescaled the notch (another monitor's DPI):
                        // the next layout must be applied in full.
                        WindowEvent::ScaleFactorChanged { .. } => windows::forget_rect(),
                        _ => {}
                    }
                });
            }
            // Registering global shortcuts can take a few hundred ms; do it
            // right after the notch is on screen instead of before.
            let sc = handle.clone();
            let shortcut_settings = settings.global_shortcuts.clone();
            let _ = handle.run_on_main_thread(move || {
                shortcuts::register_all(&sc, &shortcut_settings);
                selftest::mark("shortcuts");
            });
            watch::spawn(handle.clone());
            fileindex::spawn(handle.clone());
            clipboard_history::spawn(handle.clone());
            hotkeys::set_enabled(settings.hold_to_talk && settings.voice_enabled);
            hotkeys::install(handle.clone());
            hotkeys::configure(&settings.hold_dictate, &settings.hold_ask);
            // Get the speech recognizer ready so the first word isn't lost.
            if settings.voice_enabled {
                std::thread::spawn(|| {
                    let _ = speech::prepare();
                });
            }
            // Last session ended without a clean exit (crash, power cut):
            // still honour "clear history when Wisp closes".
            if settings.clear_history_on_exit {
                clear_history(&handle);
            }
            // Housekeeping that doesn't need to delay the notch appearing.
            let warm = handle.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
                housekeeping(&warm);
                // The saved app list is ready at once; scan again quietly.
                commands::dashboard::cached_catalogue(&warm, false).await;
                let apps = warm.clone();
                tauri::async_runtime::spawn(async move {
                    loop {
                        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                        commands::dashboard::cached_catalogue(&apps, true).await;
                        tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
                    }
                });
                // Installed builds only: a dev build shouldn't add itself to Start.
                if !cfg!(debug_assertions) {
                    let _ = com_worker::run(startmenu::ensure_shortcut).await;
                }
                // Retention applies while Wisp keeps running, too.
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(6 * 3600)).await;
                    housekeeping(&warm);
                }
            });
            if !settings.onboarding_complete && !self_test {
                windows::open_settings(&handle, Some("welcome".into()));
            }
            selftest::mark("setup-end");
            if self_test {
                selftest::spawn(handle.clone(), process_start, process_start.elapsed().as_millis());
            }
            Ok(())
        })
        .on_window_event(|win, e| {
            // Closing the island hides it; Wisp keeps running in the tray.
            if let WindowEvent::CloseRequested { api, .. } = e {
                if win.label() == windows::ISLAND {
                    api.prevent_close();
                    windows::close_bar(win.app_handle());
                }
            }
            // A capture overlay closed outside the normal flow (Alt+F4, a
            // crash) ends the capture, so the next one can start and the
            // notch comes back.
            if let WindowEvent::Destroyed = e {
                // Settings closed mid-recording: give the shortcuts back.
                if win.label() == windows::SETTINGS && hotkeys::RECORDING.load(std::sync::atomic::Ordering::SeqCst) {
                    shortcuts::pause_for_recording(win.app_handle(), false);
                }
                if let Some(i) = win
                    .label()
                    .strip_prefix("overlay-")
                    .and_then(|n| n.parse::<usize>().ok())
                {
                    let app = win.app_handle();
                    // Only an overlay of the running capture ends it.
                    let active = app
                        .state::<AppState>()
                        .capture
                        .lock()
                        .expect("capture")
                        .as_ref()
                        .is_some_and(|s| i < s.monitors.len());
                    if active {
                        capture::cancel(app);
                    }
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::settings::get_settings,
            commands::settings::update_settings,
            commands::settings::reset_shortcuts,
            commands::settings::reset_settings,
            commands::settings::set_api_key,
            commands::settings::remove_api_key,
            commands::settings::list_models,
            commands::settings::test_connection,
            commands::voice::voice_start,
            commands::voice::voice_stop,
            commands::voice::voice_cancel,
            commands::personalize::win_settings_get,
            commands::personalize::win_setting_set,
            commands::personalize::win_settings_page,
            commands::personalize::wallpaper_list,
            commands::personalize::wallpaper_folder_add,
            commands::personalize::wallpaper_folder_remove,
            commands::personalize::wallpaper_next,
            commands::files::files_search,
            commands::files::file_open,
            commands::files::file_reveal,
            commands::files::files_status,
            commands::files::files_rebuild,
            commands::files::files_folder_add,
            commands::files::files_folder_remove,
            commands::system::pc_info,
            commands::search::file_preview,
            commands::search::browsers_installed,
            commands::search::browser_tabs,
            commands::search::browser_tab_switch,
            commands::search::browser_search,
            commands::search::browser_open,
            commands::search::processes_list,
            commands::search::process_end,
            commands::search::currency_rates,
            commands::ask::scheduled_list,
            commands::ask::scheduled_cancel,
            commands::settings::test_openrouter_key,
            commands::settings::set_openrouter_key,
            commands::settings::remove_openrouter_key,
            commands::settings::model_usage,
            commands::settings::register_shortcuts,
            commands::settings::validate_shortcut,
            commands::ask::ask,
            commands::ask::ask_cancel,
            commands::ask::action_respond,
            commands::ask::prewarm,
            commands::data::history_list,
            commands::data::actions_list,
            commands::data::data_stats,
            commands::data::data_delete,
            commands::data::diagnostics_report,
            commands::data::diagnostics_save,
            commands::capture::capture_start,
            commands::capture::capture_preview,
            commands::capture::capture_select,
            commands::capture::capture_cancel,
            commands::capture::capture_overlay_ready,
            commands::capture::reverse_image_search,
            commands::capture::capture_select_lasso,
            commands::capture::show_me,
            commands::capture::highlights_get,
            commands::local::tasks_list,
            commands::local::task_create,
            commands::local::task_update,
            commands::local::task_complete,
            commands::local::task_snooze,
            commands::local::task_delete,
            commands::local::memory_list,
            commands::local::memory_add,
            commands::local::memory_update,
            commands::local::memory_delete,
            commands::local::clipboard_read,
            commands::system::installed_apps,
            commands::system::check_for_updates,
            commands::system::ui_ready,
            commands::settings::pause_shortcuts,
            commands::system::report_ui_error,
            selftest::selftest_probe,
            selftest::selftest_action,
            selftest::selftest_report,
            commands::system::set_island_expanded,
            commands::system::hide_island,
            commands::system::set_island_mode,
            commands::system::set_tucked,
            commands::system::open_settings,
            commands::system::quit_app,
            commands::system::open_external,
            commands::system::copy_text,
            commands::dashboard::controls_state,
            commands::dashboard::now_playing,
            commands::dashboard::set_volume,
            commands::dashboard::set_muted,
            commands::dashboard::media_command,
            commands::dashboard::set_radio,
            commands::dashboard::set_dark_mode,
            commands::dashboard::stats_snapshot,
            commands::dashboard::speed_test,
            commands::dashboard::weather_search,
            commands::dashboard::weather_get,
            commands::dashboard::clips_list,
            commands::dashboard::clip_pin,
            commands::dashboard::clip_delete,
            commands::dashboard::clips_clear,
            commands::dashboard::clip_copy,
            commands::dashboard::launcher_list,
            commands::dashboard::launch_item,
            commands::dashboard::power_action,
            commands::dashboard::wallpaper_info,
            commands::dashboard::wallpaper_thumb,
            commands::dashboard::wallpaper_pick,
            commands::dashboard::wallpaper_set,
            commands::dashboard::agent_start,
            commands::dashboard::agent_cancel,
            commands::dashboard::agents_list,
            commands::dashboard::agent_delete,
            commands::dashboard::agent_open,
            commands::dashboard::foreground_app,
            commands::dashboard::open_windows,
            commands::dashboard::switch_window,
            commands::dashboard::screen_status,
        ])
        .build(tauri::generate_context!())
        .expect("error while building Wisp")
        // Quitting from the tray and Windows shutting down both end here.
        .run(|app, event| {
            if let tauri::RunEvent::Exit = event {
                if app
                    .try_state::<AppState>()
                    .is_some_and(|s| s.settings().clear_history_on_exit)
                {
                    clear_history(app);
                }
            }
        });
}

/// "Clear history when Wisp closes": request and action history and
/// unpinned clipboard history. Reminders, memory and settings are kept.
fn clear_history(app: &tauri::AppHandle) {
    use tauri::Manager;
    let state = app.state::<AppState>();
    let _ = state.db.clear_requests();
    let _ = state.db.clear_actions();
    let _ = state.db.clear_clips(false);
}

/// Apply the history retention period, and tidy old voice files.
fn housekeeping(app: &tauri::AppHandle) {
    use tauri::Manager;
    let state = app.state::<AppState>();
    let s = state.settings();
    let _ = state
        .db
        .prune_history(s.history_retention_days.max(1), s.content_history);
    if let Ok(dir) = app.path().app_data_dir() {
        // Voice no longer records anything: clear what older versions kept
        // (recordings, the offline speech model).
        cleanup::remove_voice_leftovers(&dir);
    }
}

/// Wisp's webviews only ever show Wisp's own pages. Any attempt to navigate
/// elsewhere (a link that slipped through, injected content) is blocked;
/// links are opened in the browser through the validated `open_external`.
fn navigation_guard<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("navigation-guard")
        .on_navigation(|_webview, url| is_app_url(url))
        .build()
}

fn is_app_url(url: &tauri::Url) -> bool {
    match url.scheme() {
        "tauri" => url.host_str() == Some("localhost"),
        "http" | "https" => {
            url.host_str() == Some("tauri.localhost")
                || (cfg!(debug_assertions)
                    && matches!(url.host_str(), Some("localhost") | Some("127.0.0.1"))
                    && url.port() == Some(1420))
        }
        "about" => url.as_str() == "about:blank",
        _ => false,
    }
}

#[cfg(test)]
mod acl_tests {
    /// Tauri's `freezePrototype` freezes `Object.prototype`, which breaks
    /// every Preact re-render after the first frame (Preact copies vnodes
    /// with `obj[key] = value`, and `constructor` is then read-only): the
    /// notch shows but never reacts. It must stay off.
    #[test]
    fn prototype_is_not_frozen() {
        let conf: serde_json::Value =
            serde_json::from_str(include_str!("../tauri.conf.json")).unwrap();
        assert!(conf["app"]["security"]
            .get("freezePrototype")
            .is_none_or(|v| v == false));
    }

    #[test]
    fn webviews_stay_on_wisp_pages() {
        let ok = |u: &str| super::is_app_url(&tauri::Url::parse(u).unwrap());
        assert!(ok("tauri://localhost/index.html?window=settings"));
        assert!(ok("http://tauri.localhost/index.html"));
        assert!(!ok("https://evil.example/"));
        assert!(!ok("http://tauri.localhost.evil.example/"));
        assert!(!ok("file:///C:/Users/me/x.html"));
        assert!(!ok("javascript:alert(1)"));
    }

    /// Commands from a `generate_handler![...]` block or a quoted list.
    fn names(text: &str, start: &str, end: &str, quoted: bool) -> Vec<String> {
        let from = text.find(start).expect("start marker") + start.len();
        let body = &text[from..];
        let body = &body[..body.find(end).expect("end marker")];
        body.split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| {
                if quoted {
                    s.trim_matches('"').to_string()
                } else {
                    s.rsplit("::").next().unwrap().to_string()
                }
            })
            .collect()
    }

    #[test]
    fn every_command_is_declared_and_granted() {
        let lib = include_str!("lib.rs");
        let build = include_str!("../build.rs");
        let mut handled = names(lib, "generate_handler![", "]", false);
        let mut declared = names(build, "const COMMANDS: &[&str] = &[", "];", true);
        handled.sort();
        declared.sort();
        assert_eq!(handled, declared, "build.rs COMMANDS out of sync");
        let main: serde_json::Value =
            serde_json::from_str(include_str!("../capabilities/main.json")).unwrap();
        let granted: Vec<&str> = main["permissions"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|p| p.as_str()?.strip_prefix("allow-"))
            .collect();
        for c in &declared {
            assert!(
                granted.contains(&c.replace('_', "-").as_str()),
                "{c} not granted to the main windows"
            );
        }
        for small in ["overlay", "highlight"] {
            let path = format!("{}/capabilities/{small}.json", env!("CARGO_MANIFEST_DIR"));
            let cap: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
            let perms = cap["permissions"].as_array().unwrap();
            for risky in [
                "allow-update-settings",
                "allow-launch-item",
                "allow-power-action",
            ] {
                assert!(!perms.iter().any(|p| p == risky), "{small} has {risky}");
            }
        }
    }
}
