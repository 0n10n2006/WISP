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

pub const MAX_PERSONA_CHARS: usize = 1000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Tone {
    #[default]
    Concise,
    Friendly,
    Professional,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Accent {
    #[default]
    Cyan,
    Lavender,
    Ember,
    Mint,
    /// Uses `accent_color`.
    Custom,
}

/// Where along the top edge the notch sits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum NotchPosition {
    Left,
    #[default]
    Center,
    Right,
}

/// When Wisp asks before doing something the model proposed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ConfirmPolicy {
    /// Only for links, long delays, and anything prompted by screen,
    /// clipboard or web content.
    #[default]
    Risky,
    /// Every action.
    Always,
}

/// Bumped when a stored setting's meaning changes; see `migrated`.
pub const SETTINGS_REV: u32 = 2;

/// What Wisp looks like when it's resting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum NotchStyle {
    /// A notch along the top edge.
    #[default]
    Notch,
    /// A small floating ball in a corner of the screen.
    Ball,
}

/// Which corner of the screen the floating ball sits in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum BallCorner {
    TopLeft,
    TopRight,
    BottomLeft,
    #[default]
    BottomRight,
}

/// How visible the idle notch is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum SliverStyle {
    /// A soft accent glow.
    #[default]
    Glow,
    /// A faint grey line.
    Subtle,
    /// Invisible until you hover the top edge.
    Hidden,
}

pub const DASHBOARD_TABS: [&str; 8] = [
    "today",
    "controls",
    "stats",
    "clipboard",
    "apps",
    "agents",
    "wallpaper",
    "power",
];
pub const MAX_NEVER_OVER: usize = 50;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Persona {
    pub tone: Tone,
    /// Free-form instructions appended to Wisp's system prompt.
    pub instructions: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WeatherPlace {
    pub name: String,
    pub latitude: f64,
    pub longitude: f64,
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
    /// Size multiplier for the island and its text (accessibility): 1.0, 1.15 or 1.3.
    pub island_scale: f32,
    pub window_position: Option<WindowPosition>,
    pub global_shortcuts: Shortcuts,
    pub selected_model: String,
    pub voice_enabled: bool,
    /// Soft sounds when Wisp starts and stops listening.
    pub voice_sounds: bool,
    /// Skip the model's "thinking" step for much faster answers.
    pub fast_mode: bool,
    /// Tap Ctrl+Win to dictate and Win+Alt to ask by voice.
    pub hold_to_talk: bool,
    /// Modifier pair tapped to dictate, e.g. "ctrl+win" (or "off").
    pub hold_dictate: String,
    /// Modifier pair tapped to ask by voice, e.g. "win+alt" (or "off").
    pub hold_ask: String,
    /// Hide the notch while a fullscreen app (game, video) is in front.
    pub auto_hide_fullscreen: bool,
    /// The notch is tucked away until summoned.
    pub tucked: bool,
    /// Local clipboard history (off by default; never records excluded content).
    pub clipboard_history: bool,
    /// Place used for the weather card; `None` turns weather off.
    pub weather_place: Option<WeatherPlace>,
    pub persona: Persona,
    pub accent: Accent,
    /// Launcher ids the user starred.
    pub starred_apps: Vec<String>,
    /// Default focus-timer length in minutes.
    pub focus_minutes: u32,
    /// Wallpapers set from the dashboard, most recent first.
    pub recent_wallpapers: Vec<String>,
    /// Folders the quick-switch wallpaper carousel shows (chosen in a
    /// native folder picker; host-only, like `recent_wallpapers`).
    pub wallpaper_folders: Vec<String>,
    /// Custom accent colour (`#rrggbb`) used when `accent` is `custom`.
    pub accent_color: String,
    /// Keep an index of file names in the user's folders for instant search.
    pub index_files: bool,
    /// Extra folders to index (chosen in a native folder picker; host-only).
    pub index_folders: Vec<String>,
    /// Search open browser tabs, history and bookmarks from the bar.
    pub browser_search: bool,
    /// "g cats" style shortcuts that search a site.
    pub web_shortcuts: Vec<WebShortcut>,
    pub notch_position: NotchPosition,
    pub sliver_style: SliverStyle,
    pub notch_style: NotchStyle,
    pub ball_corner: BallCorner,
    /// Dashboard tabs to show, in order.
    pub dashboard_tabs: Vec<String>,
    /// Fold the bar away when you click another app.
    pub close_on_blur: bool,
    /// Fold the bar away shortly after the mouse leaves it.
    pub fold_on_leave: bool,
    /// When a model's free quota runs out, answer with the next one
    /// (Gemini → Flash-Lite → Gemma → OpenRouter).
    pub auto_fallback: bool,
    /// Send simple jobs to OpenRouter first (when a key is saved).
    pub simple_via_openrouter: bool,
    /// OpenRouter model to use; empty picks the biggest free one.
    pub openrouter_model: String,
    /// When to ask before running an action.
    pub confirm_actions: ConfirmPolicy,
    /// Which `migrate` steps a stored file has had (missing = 0).
    #[serde(default)]
    pub settings_rev: u32,
    /// Hide the notch and pause hold-to-talk while a game is in front.
    pub hide_in_games: bool,
    /// Let Wisp's shortcuts work while a game is in front.
    pub shortcuts_in_games: bool,
    /// Programs Wisp must never appear over (process names, no ".exe").
    pub never_over_apps: Vec<String>,
    /// Tell Gemini which app you're in and which apps are open/installed.
    pub app_awareness: bool,
    /// Include window titles in that context (they can contain document names).
    pub share_window_titles: bool,
    /// Forget history (requests, actions, clipboard) whenever Wisp closes
    /// or Windows shuts down.
    pub clear_history_on_exit: bool,
    pub screen_context_enabled: bool,
    pub use_memory: bool,
    pub content_history: bool,
    pub history_retention_days: u32,
    pub notifications_enabled: bool,
    /// Always false: Wisp ships no telemetry. Kept for forward compatibility.
    pub telemetry_opt_in: bool,
    /// Allowlisted applications the model may ask to open.
    pub allowed_apps: Vec<AllowedApp>,
}

/// A site search the bar can run: `prefix` then the words, e.g. "g cats".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebShortcut {
    pub prefix: String,
    pub name: String,
    /// https URL with `%s` where the search words go.
    pub url: String,
}

pub const MAX_WEB_SHORTCUTS: usize = 30;

pub fn default_web_shortcuts() -> Vec<WebShortcut> {
    [
        ("g", "Google", "https://www.google.com/search?q=%s"),
        (
            "yt",
            "YouTube",
            "https://www.youtube.com/results?search_query=%s",
        ),
        (
            "wiki",
            "Wikipedia",
            "https://en.wikipedia.org/w/index.php?search=%s",
        ),
        (
            "maps",
            "Google Maps",
            "https://www.google.com/maps/search/%s",
        ),
        ("gh", "GitHub", "https://github.com/search?q=%s"),
        ("ddg", "DuckDuckGo", "https://duckduckgo.com/?q=%s"),
        ("amz", "Amazon", "https://www.amazon.com/s?k=%s"),
        ("r", "Reddit", "https://www.reddit.com/search/?q=%s"),
    ]
    .into_iter()
    .map(|(p, n, u)| WebShortcut {
        prefix: p.into(),
        name: n.into(),
        url: u.into(),
    })
    .collect()
}

/// Tidy and check web shortcuts: short lowercase prefixes, unique, and
/// public https addresses with a `%s`.
pub fn validate_web_shortcuts(list: &mut [WebShortcut]) -> Result<()> {
    if list.len() > MAX_WEB_SHORTCUTS {
        return Err(WispError::invalid("Up to 30 search shortcuts"));
    }
    let mut seen = std::collections::HashSet::new();
    for w in list.iter_mut() {
        w.prefix = w.prefix.trim().to_lowercase();
        w.name = w.name.trim().chars().take(40).collect();
        w.url = w.url.trim().to_string();
        if w.prefix.is_empty()
            || w.prefix.len() > 10
            || !w.prefix.chars().all(|c| c.is_ascii_alphanumeric())
        {
            return Err(WispError::invalid(
                "A shortcut is 1–10 letters or digits, like \"g\" or \"yt\"",
            ));
        }
        if !seen.insert(w.prefix.clone()) {
            return Err(WispError::invalid(format!(
                "\"{}\" is used by two shortcuts",
                w.prefix
            )));
        }
        if w.name.is_empty() {
            return Err(WispError::invalid("Give the shortcut a name"));
        }
        if w.url.len() > 500 || w.url.matches("%s").count() != 1 || !w.url.starts_with("https://") {
            return Err(WispError::invalid(
                "The address must start with https:// and contain %s once",
            ));
        }
        crate::tools::validate_public_url(&w.url.replace("%s", "test"))?;
    }
    Ok(())
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
            island_scale: 1.0,
            window_position: None,
            global_shortcuts: Shortcuts::default(),
            selected_model: DEFAULT_MODEL.into(),
            voice_enabled: true,
            voice_sounds: true,
            fast_mode: true,
            hold_to_talk: true,
            hold_dictate: "ctrl+win".into(),
            hold_ask: "win+alt".into(),
            auto_hide_fullscreen: true,
            tucked: false,
            clipboard_history: false,
            weather_place: None,
            persona: Persona::default(),
            accent: Accent::Cyan,
            starred_apps: Vec::new(),
            focus_minutes: 25,
            recent_wallpapers: Vec::new(),
            wallpaper_folders: Vec::new(),
            accent_color: "#78e4f2".into(),
            index_files: true,
            index_folders: Vec::new(),
            browser_search: true,
            web_shortcuts: default_web_shortcuts(),
            notch_position: NotchPosition::Center,
            sliver_style: SliverStyle::Glow,
            notch_style: NotchStyle::Notch,
            ball_corner: BallCorner::BottomRight,
            dashboard_tabs: DASHBOARD_TABS.iter().map(|t| t.to_string()).collect(),
            close_on_blur: true,
            fold_on_leave: true,
            auto_fallback: true,
            simple_via_openrouter: true,
            openrouter_model: String::new(),
            confirm_actions: ConfirmPolicy::Risky,
            settings_rev: SETTINGS_REV,
            hide_in_games: true,
            shortcuts_in_games: false,
            never_over_apps: Vec::new(),
            app_awareness: true,
            share_window_titles: false,
            clear_history_on_exit: false,
            screen_context_enabled: true,
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
        if !(1.0..=1.3).contains(&s.island_scale) || !s.island_scale.is_finite() {
            return Err(WispError::invalid(
                "Island size must be between 100% and 130%",
            ));
        }
        s.persona.instructions = s.persona.instructions.trim().to_string();
        if s.persona.instructions.chars().count() > MAX_PERSONA_CHARS {
            return Err(WispError::invalid(format!(
                "Custom instructions must be under {MAX_PERSONA_CHARS} characters"
            )));
        }
        if !(1..=180).contains(&s.focus_minutes) {
            return Err(WispError::invalid("Focus length must be 1–180 minutes"));
        }
        if s.starred_apps.len() > 50 || s.starred_apps.iter().any(|a| a.len() > 500) {
            return Err(WispError::invalid("Too many starred apps"));
        }
        let hex = s.accent_color.trim().to_ascii_lowercase();
        if hex.len() != 7
            || !hex.starts_with('#')
            || !hex[1..].chars().all(|c| c.is_ascii_hexdigit())
        {
            return Err(WispError::invalid("Accent colour must look like #78e4f2"));
        }
        s.accent_color = hex;
        let mut seen = std::collections::HashSet::new();
        s.dashboard_tabs
            .retain(|t| DASHBOARD_TABS.contains(&t.as_str()) && seen.insert(t.clone()));
        if s.dashboard_tabs.is_empty() {
            return Err(WispError::invalid("Keep at least one dashboard tab"));
        }
        s.hold_dictate = s.hold_dictate.trim().to_ascii_lowercase();
        s.hold_ask = s.hold_ask.trim().to_ascii_lowercase();
        if crate::hold::parse_chord(&s.hold_dictate).is_none()
            || crate::hold::parse_chord(&s.hold_ask).is_none()
        {
            return Err(WispError::invalid("Choose a hold key pair from the list"));
        }
        if s.hold_dictate != "off" && s.hold_dictate == s.hold_ask {
            return Err(WispError::invalid(
                "Dictation and asking need different hold keys",
            ));
        }
        s.openrouter_model = s.openrouter_model.trim().to_string();
        if !s.openrouter_model.is_empty() && !crate::openrouter::valid_model_id(&s.openrouter_model)
        {
            return Err(WispError::invalid("That OpenRouter model name isn't valid"));
        }
        s.never_over_apps = s
            .never_over_apps
            .iter()
            .map(|a| crate::screen::process_name(a.trim()))
            .filter(|a| !a.is_empty() && a.len() <= 80)
            .collect();
        s.never_over_apps.sort();
        s.never_over_apps.dedup();
        if s.never_over_apps.len() > MAX_NEVER_OVER {
            return Err(WispError::invalid("Too many apps in the list"));
        }
        if s.wallpaper_folders.len() > 10 || s.wallpaper_folders.iter().any(|p| p.len() > 1000) {
            return Err(WispError::invalid("Too many wallpaper folders"));
        }
        validate_web_shortcuts(&mut s.web_shortcuts)?;
        if s.index_folders.len() > crate::files::MAX_USER_FOLDERS
            || s.index_folders.iter().any(|p| p.len() > 1000)
        {
            return Err(WispError::invalid("Too many indexed folders"));
        }
        if s.recent_wallpapers.len() > 12 || s.recent_wallpapers.iter().any(|p| p.len() > 1000) {
            return Err(WispError::invalid("Too many recent wallpapers"));
        }
        if let Some(p) = &mut s.weather_place {
            p.name = p.name.trim().chars().take(120).collect();
            let ok = p.latitude.is_finite()
                && p.longitude.is_finite()
                && (-90.0..=90.0).contains(&p.latitude)
                && (-180.0..=180.0).contains(&p.longitude);
            if !ok || p.name.is_empty() {
                return Err(WispError::invalid("Invalid weather location"));
            }
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
    /// Bring settings saved by an older version up to date.
    pub fn migrated(mut self) -> Self {
        // 0.2.11: voice uses Windows voice typing; the old engine, model,
        // language and pause settings are simply dropped when loading.
        self.settings_rev = SETTINGS_REV;
        self
    }

    pub fn patched(&self, patch: &serde_json::Value) -> Result<Self> {
        let mut base =
            serde_json::to_value(self).map_err(|e| WispError::internal(e.to_string()))?;
        merge(&mut base, patch);
        let mut s: Settings = serde_json::from_value(base)
            .map_err(|e| WispError::invalid(format!("Invalid setting: {e}")))?;
        // Kept by the host only: these paths are trusted for reading images,
        // so the UI must not be able to point them anywhere.
        s.recent_wallpapers = self.recent_wallpapers.clone();
        s.wallpaper_folders = self.wallpaper_folders.clone();
        s.index_folders = self.index_folders.clone();
        s.settings_rev = self.settings_rev;
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
            .map(Settings::migrated)
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
    fn old_voice_settings_are_dropped() {
        let old: Settings = serde_json::from_value(json!({
            "speechEngine": "whisper", "whisperModel": "accurate", "silenceMs": 800,
            "vocabulary": ["Ada"], "dictationStyle": "formal", "retainAudio": true,
            "voiceEnabled": false
        }))
        .unwrap();
        assert_eq!(old.settings_rev, 0);
        let m = old.migrated();
        assert!(!m.voice_enabled);
        assert_eq!(m.settings_rev, SETTINGS_REV);
        let v = serde_json::to_value(&m).unwrap();
        assert!(v.get("speechEngine").is_none() && v.get("vocabulary").is_none());
        assert_eq!(Settings::default().confirm_actions, ConfirmPolicy::Risky);
    }

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
            .patched(&json!({"focusMinutes": 0}))
            .is_err());
        assert!(Settings::default()
            .patched(&json!({"persona": {"instructions": "x".repeat(2000)}}))
            .is_err());
        assert!(Settings::default()
            .patched(
                &json!({"weatherPlace": {"name": "Nowhere", "latitude": 200.0, "longitude": 0.0}})
            )
            .is_err());
        let ok = Settings::default()
            .patched(&json!({"weatherPlace": {"name": " Leeds ", "latitude": 53.8, "longitude": -1.55}, "accent": "ember"}))
            .unwrap();
        assert_eq!(ok.weather_place.unwrap().name, "Leeds");
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
    fn customisation_is_validated_and_normalised() {
        let s = Settings::default()
            .patched(&json!({
                "accentColor": "#FFAA00",
                "dashboardTabs": ["power", "bogus", "power", "today"],
                "notchPosition": "right",
                "sliverStyle": "hidden",
                "notchStyle": "ball",
                "ballCorner": "top-left",
                "openrouterModel": " meta-llama/llama-3.3-70b-instruct:free ",
                "neverOverApps": ["C:\\Tools\\OBS64.EXE", "obs64", "photoshop.exe"],
            }))
            .unwrap();
        assert_eq!(s.accent_color, "#ffaa00");
        assert_eq!(s.dashboard_tabs, vec!["power", "today"]);
        assert_eq!(s.notch_position, NotchPosition::Right);
        assert_eq!(s.sliver_style, SliverStyle::Hidden);
        assert_eq!(s.notch_style, NotchStyle::Ball);
        assert_eq!(s.ball_corner, BallCorner::TopLeft);
        assert_eq!(s.openrouter_model, "meta-llama/llama-3.3-70b-instruct:free");
        assert_eq!(s.never_over_apps, vec!["obs64", "photoshop"]);

        for bad in [
            json!({"holdDictate": "ctrl+x"}),
            json!({"holdDictate": "win+alt"}),
            json!({"accentColor": "red"}),
            json!({"dashboardTabs": ["bogus"]}),
            json!({"notchPosition": "bottom"}),
            json!({"notchStyle": "square"}),
            json!({"ballCorner": "middle"}),
            json!({"openrouterModel": "../../etc"}),
            json!({"openrouterModel": "Bad Model"}),
            json!({"webShortcuts": [{"prefix": "g", "name": "G", "url": "http://x.com/?q=%s"}]}),
            json!({"webShortcuts": [{"prefix": "g", "name": "G", "url": "https://x.com/"}]}),
            json!({"webShortcuts": [{"prefix": "g", "name": "G", "url": "https://localhost/?q=%s"}]}),
            json!({"webShortcuts": [{"prefix": "g g", "name": "G", "url": "https://x.com/?q=%s"}]}),
            json!({"webShortcuts": [{"prefix": "g", "name": "A", "url": "https://a.com/?q=%s"}, {"prefix": "G", "name": "B", "url": "https://b.com/?q=%s"}]}),
        ] {
            assert!(Settings::default().patched(&bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn web_shortcuts_are_tidied() {
        let s = Settings::default()
            .patched(&json!({"webShortcuts": [{"prefix": " NPM ", "name": " npm ", "url": "https://www.npmjs.com/search?q=%s"}]}))
            .unwrap();
        assert_eq!(s.web_shortcuts[0].prefix, "npm");
        assert_eq!(s.web_shortcuts[0].name, "npm");
        assert_eq!(Settings::default().web_shortcuts.len(), 8);
        let mut defaults = default_web_shortcuts();
        assert!(validate_web_shortcuts(&mut defaults).is_ok());
    }

    #[test]
    fn patches_cannot_set_host_only_paths() {
        let s = Settings::default()
            .patched(&json!({"recentWallpapers": ["\\\\attacker\\share\\x.png"], "wallpaperFolders": ["\\\\attacker\\share"], "indexFolders": ["C:\\Windows"]}))
            .unwrap();
        assert!(s.recent_wallpapers.is_empty());
        assert!(s.wallpaper_folders.is_empty());
        assert!(s.index_folders.is_empty());
    }

    #[test]
    fn missing_file_gives_defaults() {
        let dir = tempfile::tempdir().unwrap();
        let loaded = SettingsStore::new(dir.path().join("nope.json")).load();
        assert_eq!(loaded.settings, Settings::default());
        assert!(loaded.recovered_from.is_none());
    }
}
