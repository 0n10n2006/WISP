use crate::secrets::SecretStore;
use std::collections::HashMap;
use std::sync::atomic::AtomicU64;
use std::sync::Mutex;
use tokio_util::sync::CancellationToken;
use wisp_core::gemini::GeminiClient;
use wisp_core::settings::{Settings, SettingsStore};

#[derive(Debug, Clone, Copy, Default, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IslandLayout {
    pub expanded: bool,
    /// The orb sits at the right edge of the expanded panel.
    pub align_right: bool,
    /// The orb sits at the bottom edge of the expanded panel.
    pub align_bottom: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutIssue {
    pub action: String,
    pub accelerator: String,
    pub message: String,
}

pub struct AppState {
    pub settings: Mutex<Settings>,
    pub settings_store: SettingsStore,
    pub settings_recovered_from: Mutex<Option<String>>,
    pub gemini: GeminiClient,
    pub secrets: SecretStore,
    pub requests: Mutex<HashMap<String, CancellationToken>>,
    pub shortcut_issues: Mutex<Vec<ShortcutIssue>>,
    pub island: Mutex<IslandLayout>,
    /// Orb (collapsed) top-left position in physical pixels.
    pub anchor: Mutex<Option<(i32, i32)>>,
    pub move_generation: AtomicU64,
}

impl AppState {
    pub fn settings(&self) -> Settings {
        self.settings.lock().expect("settings").clone()
    }
}
