//! Diagnostics report: enough to debug a problem, nothing private.
//! Contains no API key, prompts, responses, memory or reminder text.

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
}

pub struct DiagnosticsInput<'a> {
    pub app_version: &'a str,
    pub has_api_key: bool,
    pub settings: &'a Settings,
    pub storage: StorageStats,
    pub shortcut_issues: Vec<String>,
    pub requests: &'a [RequestLog],
    pub actions: &'a [ActionAudit],
}

pub fn build(input: DiagnosticsInput) -> DiagnosticsReport {
    let mut settings = serde_json::to_value(input.settings).unwrap_or_default();
    if let Some(obj) = settings.as_object_mut() {
        // App paths can contain the Windows user name.
        obj.insert(
            "allowedApps".into(),
            serde_json::json!(input.settings.allowed_apps.len()),
        );
        obj.remove("windowPosition");
    }
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
        });
        let json = serde_json::to_string(&report).unwrap();
        assert!(!json.contains("secret"));
        assert!(!json.contains("detail text"));
        assert!(!json.contains("host=example.com"));
        assert!(!json.contains("notepad.exe"));
        assert!(json.contains("key=[redacted]"));
    }
}
