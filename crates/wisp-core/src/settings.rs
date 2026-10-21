//! Settings: schema, validation, and crash-safe persistence.
//!
//! Settings live in a small JSON file (not the database) so they can be read
//! before anything else starts. Writes are atomic (temp file + rename). A file
//! that cannot be parsed is moved aside and defaults are used, so a corrupted
//! preference can never stop Wisp from starting.

use crate::error::{Result, WispError};
use crate::shortcuts::Shortcuts;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const DEFAULT_MODEL: &str = "gemini-2.5-flash";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    System,
    Dark,
    Light,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum MotionPreference {
    #[default]
    System,
    Reduced,
    Full,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WindowPosition {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub onboarding_complete: bool,
    pub launch_at_login: bool,
    pub always_on_top: bool,
    pub start_hidden: bool,
    pub island_visible: bool,
    pub theme: Theme,
    pub motion: MotionPreference,
    pub window_position: Option<WindowPosition>,
    pub global_shortcuts: Shortcuts,
    pub selected_model: String,
    pub voice_enabled: bool,
    pub speak_responses: bool,
    pub screen_context_enabled: bool,
    pub dictation_auto_insert: bool,
    pub retain_audio: bool,
    pub use_memory: bool,
    pub content_history: bool,
    pub history_retention_days: u32,
    pub notifications_enabled: bool,
    /// Always false: Wisp ships no telemetry. Kept for forward compatibility.
    pub telemetry_opt_in: bool,
    /// Allowlisted applications the model may ask to open.
    pub allowed_apps: Vec<AllowedApp>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AllowedApp {
    /// Short name the model refers to, e.g. "notepad".
    pub name: String,
    /// Absolute path to an executable or .lnk, or a well-known command.
    pub target: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            onboarding_complete: false,
            launch_at_login: false,
            always_on_top: true,
            start_hidden: false,
            island_visible: true,
            theme: Theme::System,
            motion: MotionPreference::System,
            window_position: None,
            global_shortcuts: Shortcuts::default(),
            selected_model: DEFAULT_MODEL.into(),
            voice_enabled: true,
            speak_responses: false,
            screen_context_enabled: true,
            dictation_auto_insert: false,
            retain_audio: false,
            use_memory: true,
            content_history: false,
            history_retention_days: 30,
            notifications_enabled: true,
            telemetry_opt_in: false,
            allowed_apps: vec![
                AllowedApp {
                    name: "notepad".into(),
                    target: "notepad.exe".into(),
                },
                AllowedApp {
                    name: "calculator".into(),
                    target: "calc.exe".into(),
                },
                AllowedApp {
                    name: "paint".into(),
                    target: "mspaint.exe".into(),
                },
                AllowedApp {
                    name: "file explorer".into(),
                    target: "explorer.exe".into(),
                },
            ],
        }
    }
}

impl Settings {
    /// Validate and normalise. Returns a corrected copy or a user-facing error.
    pub fn validated(&self) -> Result<Self> {
        let mut s = self.clone();
        s.global_shortcuts = s.global_shortcuts.validated()?;
        s.selected_model = s.selected_model.trim().to_string();
        if s.selected_model.is_empty() {
            return Err(WispError::invalid("Choose a model"));
        }
        if !s
            .selected_model
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_'))
            || s.selected_model.len() > 80
        {
            return Err(WispError::invalid(
                "Model name contains unsupported characters",
            ));
        }
        if s.history_retention_days > 365 {
            return Err(WispError::invalid("History retention must be 0–365 days"));
        }
        s.telemetry_opt_in = false;
        let mut names = std::collections::HashSet::new();
        for app in &mut s.allowed_apps {
            app.name = app.name.trim().to_lowercase();
            app.target = app.target.trim().to_string();
            if app.name.is_empty() || app.target.is_empty() {
                return Err(WispError::invalid("Allowed apps need a name and a target"));
            }
            if app.name.len() > 60 || app.target.len() > 400 {
                return Err(WispError::invalid("Allowed app entry is too long"));
            }
            if !names.insert(app.name.clone()) {
                return Err(WispError::invalid(format!(
                    "App \"{}\" is listed twice",
                    app.name
                )));
            }
            crate::tools::validate_app_target(&app.target)?;
        }
        Ok(s)
    }

    /// Apply a JSON merge-patch (shallow per top-level key, deep for objects).
    pub fn patched(&self, patch: &serde_json::Value) -> Result<Self> {
        let mut base =
            serde_json::to_value(self).map_err(|e| WispError::internal(e.to_string()))?;
        merge(&mut base, patch);
        let s: Settings = serde_json::from_value(base)
            .map_err(|e| WispError::invalid(format!("Invalid setting: {e}")))?;
        s.validated()
    }
}

fn merge(base: &mut serde_json::Value, patch: &serde_json::Value) {
    match (base, patch) {
        (serde_json::Value::Object(b), serde_json::Value::Object(p)) => {
            for (k, v) in p {
                if v.is_object() && b.get(k).is_some_and(|x| x.is_object()) {
                    merge(b.get_mut(k).expect("checked"), v);
                } else {
                    b.insert(k.clone(), v.clone());
                }
            }
        }
        (b, p) => *b = p.clone(),
    }
}

/// Outcome of loading settings, so the UI can tell the user about recovery.
#[derive(Debug, Clone)]
pub struct Loaded {
    pub settings: Settings,
    pub recovered_from: Option<PathBuf>,
}

pub struct SettingsStore {
    path: PathBuf,
}

impl SettingsStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn load(&self) -> Loaded {
        let raw = match fs::read_to_string(&self.path) {
            Ok(r) => r,
            Err(_) => {
                return Loaded {
                    settings: Settings::default(),
                    recovered_from: None,
                };
            }
        };
        let parsed = serde_json::from_str::<Settings>(&raw)
            .map_err(|e| e.to_string())
            .and_then(|s| s.validated().map_err(|e| e.message));
        match parsed {
            Ok(settings) => Loaded {
                settings,
                recovered_from: None,
            },
            Err(_) => {
                let stamp = chrono::Utc::now().format("%Y%m%d%H%M%S");
                let aside = self.path.with_extension(format!("corrupt-{stamp}.json"));
                let _ = fs::rename(&self.path, &aside);
                Loaded {
                    settings: Settings::default(),
                    recovered_from: Some(aside),
                }
            }
        }
    }

    pub fn save(&self, settings: &Settings) -> Result<()> {
        let s = settings.validated()?;
        if let Some(dir) = self.path.parent() {
            fs::create_dir_all(dir)?;
        }
        let tmp = self.path.with_extension("json.tmp");
        let body =
            serde_json::to_string_pretty(&s).map_err(|e| WispError::internal(e.to_string()))?;
        fs::write(&tmp, body)?;
        fs::rename(&tmp, &self.path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn defaults_validate() {
        let d = Settings::default();
        assert_eq!(d.validated().unwrap(), d);
    }

    #[test]
    fn patch_merges_nested_and_validates() {
        let s = Settings::default()
            .patched(&json!({"theme": "light", "globalShortcuts": {"voice": "alt+shift+v"}}))
            .unwrap();
        assert_eq!(s.theme, Theme::Light);
        assert_eq!(s.global_shortcuts.voice, "Alt+Shift+V");
        assert_eq!(s.global_shortcuts.dictation, "Ctrl+Shift+D");
    }

    #[test]
    fn patch_rejects_invalid_values() {
        assert!(Settings::default()
            .patched(&json!({"theme": "neon"}))
            .is_err());
        assert!(Settings::default()
            .patched(&json!({"historyRetentionDays": 9999}))
            .is_err());
        assert!(Settings::default()
            .patched(&json!({"selectedModel": "../etc"}))
            .is_err());
        assert!(Settings::default()
            .patched(&json!({"globalShortcuts": {"voice": "Ctrl+Shift+D"}}))
            .is_err());
    }

    #[test]
    fn telemetry_cannot_be_enabled() {
        let s = Settings::default()
            .patched(&json!({"telemetryOptIn": true}))
            .unwrap();
        assert!(!s.telemetry_opt_in);
    }

    #[test]
    fn unknown_and_missing_fields_fall_back_to_defaults() {
        let s: Settings = serde_json::from_str(r#"{"theme":"dark","futureThing":1}"#).unwrap();
        assert_eq!(s.theme, Theme::Dark);
        assert_eq!(s.selected_model, DEFAULT_MODEL);
    }

    #[test]
    fn save_then_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let store = SettingsStore::new(dir.path().join("settings.json"));
        let s = Settings {
            theme: Theme::Light,
            window_position: Some(WindowPosition { x: 10, y: -5 }),
            ..Default::default()
        };
        store.save(&s).unwrap();
        let loaded = store.load();
        assert_eq!(loaded.settings, s);
        assert!(loaded.recovered_from.is_none());
    }

    #[test]
    fn corrupt_file_is_moved_aside() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, "{not json").unwrap();
        let loaded = SettingsStore::new(&path).load();
        assert_eq!(loaded.settings, Settings::default());
        let aside = loaded.recovered_from.unwrap();
        assert!(aside.exists());
        assert!(!path.exists());
    }

    #[test]
    fn missing_file_gives_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let loaded = SettingsStore::new(dir.path().join("nope.json")).load();
        assert_eq!(loaded.settings, Settings::default());
        assert!(loaded.recovered_from.is_none());
    }
}
