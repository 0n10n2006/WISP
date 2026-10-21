//! Wisp desktop host (Tauri 2). OS integration lives here; logic lives in `wisp-core`.

mod commands;
mod secrets;
mod shortcuts;
mod state;
mod tray;
mod windows;

use state::AppState;
use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use std::sync::Mutex;
use tauri::{Manager, WindowEvent};
use wisp_core::gemini::GeminiClient;
use wisp_core::settings::SettingsStore;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            windows::summon_island(app);
        }))
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec!["--autostart"]),
        ))
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&data_dir)?;
            let store = SettingsStore::new(data_dir.join("settings.json"));
            let loaded = store.load();
            let settings = loaded.settings;
            app.manage(AppState {
                settings: Mutex::new(settings.clone()),
                settings_store: store,
                settings_recovered_from: Mutex::new(
                    loaded.recovered_from.map(|p| p.display().to_string()),
                ),
                gemini: GeminiClient::new(),
                secrets: secrets::SecretStore::default(),
                requests: Mutex::new(HashMap::new()),
                shortcut_issues: Mutex::new(Vec::new()),
                island: Mutex::new(Default::default()),
                anchor: Mutex::new(None),
                move_generation: AtomicU64::new(0),
            });

            let handle = app.handle().clone();
            tray::create(&handle)?;
            shortcuts::register_all(&handle, &settings.global_shortcuts);
            windows::place_island_initially(&handle);
            if let Some(win) = windows::island(&handle) {
                let _ = win.set_always_on_top(settings.always_on_top);
                let autostarted = std::env::args().any(|a| a == "--autostart");
                let show = settings.island_visible && !(settings.start_hidden && autostarted);
                if show || !settings.onboarding_complete {
                    let _ = win.show();
                }
                let h = handle.clone();
                win.on_window_event(move |e| {
                    if let WindowEvent::Moved(pos) = e {
                        windows::on_island_moved(&h, *pos);
                    }
                });
            }
            if !settings.onboarding_complete {
                windows::open_settings(&handle, Some("welcome".into()));
            }
            Ok(())
        })
        .on_window_event(|win, e| {
            // Closing the island hides it; Wisp keeps running in the tray.
            if let WindowEvent::CloseRequested { api, .. } = e {
                if win.label() == windows::ISLAND {
                    api.prevent_close();
                    windows::set_island_visible(win.app_handle(), false);
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
            commands::system::set_island_expanded,
            commands::system::hide_island,
            commands::system::open_settings,
            commands::system::quit_app,
            commands::system::open_external,
            commands::system::copy_text,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Wisp");
}
