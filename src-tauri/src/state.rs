use crate::secrets::SecretStore;
use std::collections::HashMap;
use std::sync::Mutex;
use tokio_util::sync::CancellationToken;
use wisp_core::gemini::GeminiClient;
use wisp_core::settings::{Settings, SettingsStore};
use wisp_core::storage::Db;

pub use crate::windows::IslandLayout;

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutIssue {
    pub action: String,
    pub accelerator: String,
    pub message: String,
}

pub struct AppState {
    /// Pending action confirmations, by id.
    pub confirmations: Mutex<HashMap<String, tokio::sync::oneshot::Sender<bool>>>,
    /// The app that was in front before Wisp was summoned (for Insert).
    pub insert_target: Mutex<Option<crate::foreground::Target>>,
    pub capture: Mutex<Option<crate::capture::CaptureSession>>,
    pub settings: Mutex<Settings>,
    pub settings_store: SettingsStore,
    pub settings_recovered_from: Mutex<Option<String>>,
    /// Problems found at startup that the user should know about.
    pub startup_warnings: Mutex<Vec<String>>,
    pub gemini: GeminiClient,
    pub db: Db,
    pub secrets: SecretStore,
    /// Optional OpenRouter key (free text models).
    pub openrouter_key: SecretStore,
    /// Which model answers what, and which are used up for today.
    pub routing: crate::routing::Routing,
    pub requests: Mutex<HashMap<String, CancellationToken>>,
    pub shortcut_issues: Mutex<Vec<ShortcutIssue>>,
    pub island: Mutex<IslandLayout>,
    /// Something Wisp must not cover is in front (fullscreen, game,
    /// presentation or an excluded app): the notch stays hidden.
    pub fullscreen_active: Mutex<bool>,
    /// A game is in front: hold-to-talk and shortcuts stay quiet.
    pub game_active: std::sync::atomic::AtomicBool,
    /// Why the notch is currently hidden (for the UI and diagnostics).
    pub cover: Mutex<wisp_core::screen::Cover>,
    pub stats: crate::stats::Stats,
    /// Launcher catalogue, built once in the background.
    pub catalogue: Mutex<Option<std::sync::Arc<Vec<wisp_core::launcher::LaunchItem>>>>,
    /// Regions for the Show-me overlay.
    pub highlights: Mutex<Vec<wisp_core::capture::Highlight>>,
}

impl AppState {
    pub fn settings(&self) -> Settings {
        self.settings.lock().expect("settings").clone()
    }
}
