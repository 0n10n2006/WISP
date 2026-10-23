//! Diagnostics report: enough to debug a problem, nothing private.
//! Contains no API key, prompts, responses, memory or reminder text, and no
//! paths, places, names or free-text settings (only counts of them).

use crate::redact::redact;
use crate::settings::Settings;
use crate::storage::{ActionAudit, RequestLog, StorageStats};
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsReport {
    pub generated_at: String,
    pub app_version: String,
    pub os: String,
    pub arch: String,
    pub has_api_key: bool,
    pub settings: serde_json::Value,
    pub storage: StorageStats,
    pub shortcut_issues: Vec<String>,
    pub recent_requests: Vec<serde_json::Value>,
    pub recent_actions: Vec<serde_json::Value>,
    /// Newest entries of the local error log (already redacted).
    pub recent_errors: Vec<String>,
}

pub struct DiagnosticsInput<'a> {
    pub app_version: &'a str,
    pub has_api_key: bool,
    pub settings: &'a Settings,
    pub storage: StorageStats,
    pub shortcut_issues: Vec<String>,
    pub requests: &'a [RequestLog],
    pub actions: &'a [ActionAudit],
    pub recent_errors: Vec<String>,
}

/// Settings copied into the report as they are: switches and choices only.
const SAFE_SETTINGS: &[&str] = &[
    "onboardingComplete",
    "launchAtLogin",
    "alwaysOnTop",
    "startHidden",
    "islandVisible",
    "theme",
    "motion",
    "islandScale",
    "globalShortcuts",
    "selectedModel",
    "voiceEnabled",
    "fastMode",
    "dictationStyle",
    "holdToTalk",
    "holdDictate",
    "holdAsk",
    "clearHistoryOnExit",
    "autoHideFullscreen",
    "tucked",
    "clipboardHistory",
    "accent",
    "focusMinutes",
    "accentColor",
    "notchPosition",
    "sliverStyle",
    "dashboardTabs",
    "closeOnBlur",
    "foldOnLeave",
    "autoFallback",
    "simpleViaOpenrouter",
    "openrouterModel",
    "notchStyle",
    "ballCorner",
    "autoStopVoice",
    "silenceMs",
    "speechEngine",
    "voiceLanguage",
    "hideInGames",
    "shortcutsInGames",
    "appAwareness",
    "shareWindowTitles",
    "screenContextEnabled",
    "dictationAutoInsert",
    "retainAudio",
    "useMemory",
    "contentHistory",
    "historyRetentionDays",
    "notificationsEnabled",
    "telemetryOptIn",
];

/// Settings that can hold paths, places, names or free text: reported only
/// as counts or yes/no.
fn summarised_settings(s: &Settings) -> serde_json::Map<String, serde_json::Value> {
    let mut m = serde_json::Map::new();
    let mut put = |k: &str, v: serde_json::Value| {
        m.insert(k.into(), v);
    };
    put("allowedApps", serde_json::json!(s.allowed_apps.len()));
    put("starredApps", serde_json::json!(s.starred_apps.len()));
    put(
        "recentWallpapers",
        serde_json::json!(s.recent_wallpapers.len()),
    );
    put(
        "weatherPlaceSet",
        serde_json::json!(s.weather_place.is_some()),
    );
    put(
        "persona",
        serde_json::json!({
            "tone": s.persona.tone,
            "instructionsChars": s.persona.instructions.chars().count(),
        }),
    );
    put(
        "wallpaperFolders",
        serde_json::json!(s.wallpaper_folders.len()),
    );
    put("vocabulary", serde_json::json!(s.vocabulary.len()));
    put("neverOverApps", serde_json::json!(s.never_over_apps.len()));
    put(
        "customMicrophone",
        serde_json::json!(!s.mic_device.is_empty()),
    );
    m
}

/// Keys of `Settings` that are summarised rather than copied.
#[cfg(test)]
const SUMMARISED_KEYS: &[&str] = &[
    "allowedApps",
    "starredApps",
    "recentWallpapers",
    "wallpaperFolders",
    "weatherPlace",
    "persona",
    "vocabulary",
    "neverOverApps",
    "micDevice",
    "windowPosition",
];

pub fn build(input: DiagnosticsInput) -> DiagnosticsReport {
    let full = serde_json::to_value(input.settings).unwrap_or_default();
    let mut settings = summarised_settings(input.settings);
    if let Some(obj) = full.as_object() {
        for k in SAFE_SETTINGS {
            if let Some(v) = obj.get(*k) {
                settings.insert((*k).into(), v.clone());
            }
        }
    }
    let settings = serde_json::Value::Object(settings);
    DiagnosticsReport {
        generated_at: chrono::Utc::now().to_rfc3339(),
        app_version: input.app_version.to_string(),
        os: std::env::consts::OS.to_string(),
        arch: std::env::consts::ARCH.to_string(),
        has_api_key: input.has_api_key,
        settings,
        storage: input.storage,
        shortcut_issues: input.shortcut_issues.iter().map(|s| redact(s)).collect(),
        recent_requests: input
            .requests
            .iter()
            .take(50)
            .map(|r| {
                serde_json::json!({
                    "timestamp": r.timestamp,
                    "type": r.request_type,
                    "model": r.model,
                    "status": r.status,
                    "latencyMs": r.latency_ms,
                    "errorCode": r.error_code,
                    "contextKind": r.context_kind,
                })
            })
            .collect(),
        recent_actions: input
            .actions
            .iter()
            .take(50)
            .map(|a| {
                serde_json::json!({
                    "timestamp": a.timestamp,
                    "type": a.action_type,
                    "outcome": a.outcome,
                    "userConfirmed": a.user_confirmed,
                })
            })
            .collect(),
        recent_errors: input.recent_errors.iter().map(|e| redact(e)).collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_excludes_content() {
        let mut r = RequestLog::new("ask", "m", "text");
        r.prompt = Some("secret prompt".into());
        r.response = Some("secret answer".into());
        let mut a = ActionAudit::new("open_url", "host=example.com", "done", true);
        a.detail = Some("detail text".into());
        let settings = Settings::default();
        let report = build(DiagnosticsInput {
            app_version: "1.0",
            has_api_key: true,
            settings: &settings,
            storage: StorageStats::default(),
            shortcut_issues: vec!["key=abc failed".into()],
            requests: &[r],
            actions: &[a],
            recent_errors: vec![],
        });
        let json = serde_json::to_string(&report).unwrap();
        assert!(!json.contains("secret"));
        assert!(!json.contains("detail text"));
        assert!(!json.contains("host=example.com"));
        assert!(!json.contains("notepad.exe"));
        assert!(json.contains("key=[redacted]"));
    }

    #[test]
    fn personal_settings_are_only_counted() {
        let mut settings = Settings::default();
        settings.persona.instructions = "I'm Alexandra, a nurse in Leeds".into();
        settings.starred_apps = vec!["lnk:C:\\Users\\alexandra\\x.lnk".into()];
        settings.vocabulary = vec!["Alexandra".into()];
        settings.mic_device = "abc123".into();
        let report = build(DiagnosticsInput {
            app_version: "1.0",
            has_api_key: false,
            settings: &settings,
            storage: StorageStats::default(),
            shortcut_issues: vec![],
            requests: &[],
            actions: &[],
            recent_errors: vec![],
        });
        let json = serde_json::to_string(&report).unwrap();
        assert!(!json.to_lowercase().contains("alexandra"));
        assert!(!json.contains("abc123"));
        assert!(json.contains("\"instructionsChars\":31"));
        // Every setting is either known to be safe or summarised, so a new
        // personal setting can't slip into reports unnoticed.
        let all = serde_json::to_value(Settings::default()).unwrap();
        for k in all.as_object().unwrap().keys() {
            assert!(
                SAFE_SETTINGS.contains(&k.as_str()) || SUMMARISED_KEYS.contains(&k.as_str()),
                "decide whether {k} is safe for diagnostics"
            );
        }
    }
}
