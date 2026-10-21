//! Wisp desktop host (Tauri 2). OS integration lives here; logic lives in `wisp-core`.

mod actions;
mod agents;
mod apps;
mod capture;
mod chrome;
mod clipboard_history;
mod com_worker;
mod commands;
mod crash;
mod foreground;
mod hotkeys;
mod power;
mod reminders;
mod secrets;
mod selftest;
mod shortcuts;
mod state;
mod stats;
mod system_controls;
mod system_theme;
mod tray;
mod wallpaper;
mod watch;
mod windows;

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
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            selftest::mark("data-dir");
            let store = SettingsStore::new(data_dir.join("settings.json"));
            let loaded = store.load();
            let settings = loaded.settings;
            selftest::mark("settings");
            let log_dir = data_dir.join("logs");
            crash::install(log_dir.clone(), app.package_info().version.to_string());
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
                secrets: secrets::SecretStore::default(),
                requests: Mutex::new(HashMap::new()),
                shortcut_issues: Mutex::new(Vec::new()),
                island: Mutex::new(Default::default()),
                fullscreen_active: Mutex::new(false),
                stats: Default::default(),
                catalogue: Mutex::new(None),
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
                    if let WindowEvent::Focused(false) = e {
                        let _ = h.emit_to(windows::ISLAND, "wisp://blur", ());
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
            clipboard_history::spawn(handle.clone());
            hotkeys::set_enabled(settings.hold_to_talk && settings.voice_enabled);
            hotkeys::install(handle.clone());
            // Housekeeping that doesn't need to delay the notch appearing.
            let warm = handle.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
                let s = warm.state::<AppState>().settings();
                let _ = warm
                    .state::<AppState>()
                    .db
                    .prune_history(s.history_retention_days.max(1), s.content_history);
                commands::dashboard::cached_catalogue(&warm, false).await;
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
            commands::capture::capture_select_lasso,
            commands::capture::show_me,
            commands::capture::highlights_get,
            commands::dictation::dictation_target,
            commands::dictation::transcribe,
            commands::dictation::insert_text,
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
        ])
        .run(tauri::generate_context!())
        .expect("error while running Wisp");
}
