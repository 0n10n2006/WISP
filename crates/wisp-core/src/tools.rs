//! Allowlisted desktop actions.
//!
//! The model can *propose* an action via Gemini function calling. Everything
//! it sends is treated as untrusted input: the tool name must be known, the
//! arguments must match a strict schema, and values are bounded and checked.
//! Only a [`ValidatedAction`] can be executed by the host. There is no shell
//! or PowerShell tool, by design.

use crate::error::{Result, WispError};
use crate::redact::summarize_text;
use crate::settings::AllowedApp;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const MAX_COPY_CHARS: usize = 20_000;
pub const MAX_URL_LEN: usize = 2_048;
pub const MAX_TITLE_CHARS: usize = 200;
pub const MAX_NOTES_CHARS: usize = 2_000;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ValidatedAction {
    OpenApp {
        name: String,
        target: String,
    },
    OpenUrl {
        url: String,
    },
    CopyText {
        text: String,
    },
    CreateReminder {
        title: String,
        notes: Option<String>,
        due_at: DateTime<Utc>,
    },
    /// Save a memory entry. Always asks the user first.
    Remember {
        content: String,
        category: String,
    },
}

impl ValidatedAction {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::OpenApp { .. } => "open_app",
            Self::OpenUrl { .. } => "open_url",
            Self::CopyText { .. } => "copy_text",
            Self::CreateReminder { .. } => "create_reminder",
            Self::Remember { .. } => "remember",
        }
    }

    /// Actions that leave Wisp's own sandbox need the user's confirmation.
    pub fn requires_confirmation(&self) -> bool {
        matches!(
            self,
            Self::OpenApp { .. } | Self::OpenUrl { .. } | Self::Remember { .. }
        )
    }

    /// Short description safe for the audit log (no long free text).
    pub fn audit_summary(&self) -> String {
        match self {
            Self::OpenApp { name, .. } => format!("app={name}"),
            Self::OpenUrl { url } => {
                let host = url::Url::parse(url)
                    .ok()
                    .and_then(|u| u.host_str().map(str::to_owned))
                    .unwrap_or_default();
                format!("host={host}")
            }
            Self::CopyText { text } => format!("chars={}", text.chars().count()),
            Self::CreateReminder { title, due_at, .. } => {
                format!(
                    "title=\"{}\" due={}",
                    summarize_text(title, 40),
                    due_at.to_rfc3339()
                )
            }
            Self::Remember { content, .. } => format!("chars={}", content.chars().count()),
        }
    }

    /// Human readable description for the confirmation dialog.
    pub fn describe(&self) -> String {
        match self {
            Self::OpenApp { name, .. } => format!("Open {name}"),
            Self::OpenUrl { url } => format!("Open {url} in your browser"),
            Self::CopyText { text } => {
                format!("Copy {} characters to the clipboard", text.chars().count())
            }
            Self::CreateReminder { title, due_at, .. } => {
                let local = due_at.with_timezone(&chrono::Local);
                format!(
                    "Remind you \"{title}\" on {}",
                    local.format("%a %-d %b at %H:%M")
                )
            }
            Self::Remember { content, .. } => format!("Remember: \"{content}\""),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RememberArgs {
    content: String,
    #[serde(default)]
    category: Option<String>,
}

pub const MAX_MEMORY_CHARS: usize = 500;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OpenAppArgs {
    name: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OpenUrlArgs {
    url: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CopyTextArgs {
    text: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateReminderArgs {
    title: String,
    #[serde(default)]
    notes: Option<String>,
    due_at: String,
}

fn parse_args<T: for<'de> Deserialize<'de>>(tool: &str, args: &Value) -> Result<T> {
    serde_json::from_value(args.clone())
        .map_err(|e| WispError::invalid(format!("Rejected {tool}: invalid arguments ({e})")))
}

/// Validate a model-proposed tool call.
pub fn validate_call(
    name: &str,
    args: &Value,
    allowed_apps: &[AllowedApp],
    now: DateTime<Utc>,
) -> Result<ValidatedAction> {
    match name {
        "open_app" => {
            let a: OpenAppArgs = parse_args(name, args)?;
            let wanted = a.name.trim().to_lowercase();
            let app = allowed_apps
                .iter()
                .find(|app| app.name == wanted)
                .ok_or_else(|| {
                    WispError::invalid(format!(
                        "\"{}\" is not in your allowed apps. Add it in Settings → Actions.",
                        summarize_text(&a.name, 40)
                    ))
                })?;
            validate_app_target(&app.target)?;
            Ok(ValidatedAction::OpenApp {
                name: app.name.clone(),
                target: app.target.clone(),
            })
        }
        "open_url" => {
            let a: OpenUrlArgs = parse_args(name, args)?;
            Ok(ValidatedAction::OpenUrl {
                url: validate_url(&a.url)?,
            })
        }
        "copy_text" => {
            let a: CopyTextArgs = parse_args(name, args)?;
            if a.text.is_empty() {
                return Err(WispError::invalid("Nothing to copy"));
            }
            if a.text.chars().count() > MAX_COPY_CHARS {
                return Err(WispError::invalid("Text is too long to copy"));
            }
            Ok(ValidatedAction::CopyText { text: a.text })
        }
        "create_reminder" => {
            let a: CreateReminderArgs = parse_args(name, args)?;
            let title = validate_title(&a.title)?;
            let notes = validate_notes(a.notes)?;
            let due_at = parse_due(&a.due_at)?;
            validate_due(due_at, now)?;
            Ok(ValidatedAction::CreateReminder {
                title,
                notes,
                due_at,
            })
        }
        "remember" => {
            let a: RememberArgs = parse_args(name, args)?;
            let content = a.content.trim().to_string();
            if content.is_empty() || content.chars().count() > MAX_MEMORY_CHARS {
                return Err(WispError::invalid("Memory must be 1–500 characters"));
            }
            Ok(ValidatedAction::Remember {
                content,
                category: a.category.unwrap_or_else(|| "other".into()),
            })
        }
        other => Err(WispError::invalid(format!(
            "Rejected unknown action \"{}\"",
            summarize_text(other, 40)
        ))),
    }
}

pub fn validate_title(title: &str) -> Result<String> {
    let t = title.trim();
    if t.is_empty() {
        return Err(WispError::invalid("Reminder needs a title"));
    }
    if t.chars().count() > MAX_TITLE_CHARS {
        return Err(WispError::invalid("Reminder title is too long"));
    }
    Ok(t.to_string())
}

pub fn validate_notes(notes: Option<String>) -> Result<Option<String>> {
    match notes
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
    {
        Some(n) if n.chars().count() > MAX_NOTES_CHARS => {
            Err(WispError::invalid("Notes are too long"))
        }
        other => Ok(other),
    }
}

pub fn parse_due(raw: &str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(raw.trim())
        .map(|d| d.with_timezone(&Utc))
        .map_err(|_| WispError::invalid("Reminder time must be a full date and time with timezone"))
}

pub fn validate_due(due: DateTime<Utc>, now: DateTime<Utc>) -> Result<()> {
    if due < now - Duration::minutes(1) {
        return Err(WispError::invalid("Reminder time is in the past"));
    }
    if due > now + Duration::days(365 * 5) {
        return Err(WispError::invalid("Reminder time is too far in the future"));
    }
    Ok(())
}

pub fn validate_url(raw: &str) -> Result<String> {
    let raw = raw.trim();
    if raw.len() > MAX_URL_LEN {
        return Err(WispError::invalid("URL is too long"));
    }
    let u =
        url::Url::parse(raw).map_err(|_| WispError::invalid("That is not a valid web address"))?;
    if !matches!(u.scheme(), "http" | "https") {
        return Err(WispError::invalid(
            "Only http and https links can be opened",
        ));
    }
    if u.host_str().is_none_or(str::is_empty) {
        return Err(WispError::invalid("URL has no host"));
    }
    if !u.username().is_empty() || u.password().is_some() {
        return Err(WispError::invalid(
            "URLs with embedded credentials are not opened",
        ));
    }
    Ok(u.to_string())
}

/// App targets come from the user's own allowlist, but are still checked so
/// that nothing resembling a command line slips through.
pub fn validate_app_target(target: &str) -> Result<()> {
    let t = target.trim();
    let bad_chars = [
        '"', '\'', '&', '|', '<', '>', '^', '%', ';', '`', '$', '\n', '\r',
    ];
    if t.is_empty() || t.chars().any(|c| bad_chars.contains(&c)) {
        return Err(WispError::invalid(
            "App target must be a plain path to a program",
        ));
    }
    let lower = t.to_ascii_lowercase();
    let allowed_ext = [".exe", ".lnk", ".appref-ms"];
    if !allowed_ext.iter().any(|e| lower.ends_with(e)) {
        return Err(WispError::invalid(
            "App target must be an .exe or shortcut (.lnk)",
        ));
    }
    let blocked = [
        "cmd.exe",
        "powershell.exe",
        "pwsh.exe",
        "wscript.exe",
        "cscript.exe",
        "mshta.exe",
        "rundll32.exe",
        "regsvr32.exe",
        "reg.exe",
        "bash.exe",
        "wsl.exe",
        "msiexec.exe",
    ];
    let file = lower.rsplit(['\\', '/']).next().unwrap_or(&lower);
    if blocked.contains(&file) {
        return Err(WispError::invalid(
            "Shells and script hosts cannot be allowlisted",
        ));
    }
    let is_bare = !t.contains(['\\', '/', ':']);
    let is_abs = t.len() > 3 && t.as_bytes()[1] == b':' && matches!(t.as_bytes()[2], b'\\' | b'/');
    if !(is_bare || is_abs) || t.contains("..") {
        return Err(WispError::invalid(
            "App target must be a program name or an absolute path",
        ));
    }
    Ok(())
}

/// Gemini `functionDeclarations` for the tools above.
pub fn function_declarations() -> Value {
    json!([
        {
            "name": "open_app",
            "description": "Open an application from the user's allowlist by its short name.",
            "parameters": {
                "type": "object",
                "properties": { "name": { "type": "string", "description": "Allowlisted app name, e.g. notepad" } },
                "required": ["name"]
            }
        },
        {
            "name": "open_url",
            "description": "Open an http or https URL in the default browser.",
            "parameters": {
                "type": "object",
                "properties": { "url": { "type": "string" } },
                "required": ["url"]
            }
        },
        {
            "name": "copy_text",
            "description": "Copy text to the clipboard.",
            "parameters": {
                "type": "object",
                "properties": { "text": { "type": "string" } },
                "required": ["text"]
            }
        },
        {
            "name": "create_reminder",
            "description": "Create a local reminder that notifies the user at a time. due_at must be RFC 3339 with timezone offset.",
            "parameters": {
                "type": "object",
                "properties": {
                    "title": { "type": "string" },
                    "notes": { "type": "string" },
                    "due_at": { "type": "string", "description": "RFC 3339 timestamp, e.g. 2026-10-21T15:00:00+01:00" }
                },
                "required": ["title", "due_at"]
            }
        },
        {
            "name": "remember",
            "description": "Ask the user whether to save a short, lasting fact or preference to Wisp's local memory. Only use when the user asks you to remember something or clearly states a lasting preference. Never save sensitive personal data.",
            "parameters": {
                "type": "object",
                "properties": {
                    "content": { "type": "string", "description": "One concise sentence" },
                    "category": { "type": "string", "enum": ["preference", "fact", "project", "people", "other"] }
                },
                "required": ["content"]
            }
        }
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::Settings;

    fn now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-10-21T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
    }

    fn apps() -> Vec<AllowedApp> {
        Settings::default().allowed_apps
    }

    #[test]
    fn rejects_unknown_tools() {
        let e = validate_call("run_shell", &json!({"cmd": "del"}), &apps(), now()).unwrap_err();
        assert!(e.message.contains("unknown action"));
    }

    #[test]
    fn rejects_extra_or_missing_arguments() {
        assert!(validate_call(
            "open_url",
            &json!({"url": "https://a.com", "x": 1}),
            &apps(),
            now()
        )
        .is_err());
        assert!(validate_call("open_url", &json!({}), &apps(), now()).is_err());
        assert!(validate_call("open_url", &json!({"url": 5}), &apps(), now()).is_err());
        assert!(validate_call("open_url", &json!("https://a.com"), &apps(), now()).is_err());
    }

    #[test]
    fn open_app_requires_allowlist() {
        let ok = validate_call("open_app", &json!({"name": "Notepad"}), &apps(), now()).unwrap();
        assert_eq!(
            ok,
            ValidatedAction::OpenApp {
                name: "notepad".into(),
                target: "notepad.exe".into()
            }
        );
        assert!(ok.requires_confirmation());
        assert!(validate_call("open_app", &json!({"name": "regedit"}), &apps(), now()).is_err());
    }

    #[test]
    fn url_validation() {
        assert_eq!(
            validate_url("https://example.com/a?b=c").unwrap(),
            "https://example.com/a?b=c"
        );
        assert!(validate_url("file:///C:/Windows/system32/cmd.exe").is_err());
        assert!(validate_url("javascript:alert(1)").is_err());
        assert!(validate_url("ms-settings:privacy").is_err());
        assert!(validate_url("https://user:pw@example.com").is_err());
        assert!(validate_url("not a url").is_err());
        assert!(validate_url(&format!("https://a.com/{}", "x".repeat(3000))).is_err());
    }

    #[test]
    fn app_target_validation() {
        assert!(validate_app_target("notepad.exe").is_ok());
        assert!(validate_app_target(r"C:\Program Files\App\app.exe").is_ok());
        assert!(validate_app_target(r"C:\Users\me\Desktop\Thing.lnk").is_ok());
        assert!(validate_app_target("cmd.exe").is_err());
        assert!(
            validate_app_target(r"C:\Windows\System32\WindowsPowerShell\v1.0\powershell.exe")
                .is_err()
        );
        assert!(validate_app_target("notepad.exe & calc.exe").is_err());
        assert!(validate_app_target("app.bat").is_err());
        assert!(validate_app_target(r"..\..\x.exe").is_err());
        assert!(validate_app_target(r"relative\x.exe").is_err());
    }

    #[test]
    fn reminder_validation() {
        let ok = validate_call(
            "create_reminder",
            &json!({"title": " Call mum ", "due_at": "2026-10-21T15:00:00+01:00"}),
            &apps(),
            now(),
        )
        .unwrap();
        match ok {
            ValidatedAction::CreateReminder {
                title,
                due_at,
                notes,
            } => {
                assert_eq!(title, "Call mum");
                assert_eq!(due_at.to_rfc3339(), "2026-10-21T14:00:00+00:00");
                assert!(notes.is_none());
            }
            _ => panic!(),
        }
        let bad = |args: Value| validate_call("create_reminder", &args, &apps(), now()).is_err();
        assert!(bad(json!({"title": "x", "due_at": "tomorrow"})));
        assert!(bad(json!({"title": "x", "due_at": "2020-01-01T00:00:00Z"})));
        assert!(bad(json!({"title": "x", "due_at": "2099-01-01T00:00:00Z"})));
        assert!(bad(json!({"title": "", "due_at": "2026-10-22T00:00:00Z"})));
        assert!(bad(
            json!({"title": "x".repeat(300), "due_at": "2026-10-22T00:00:00Z"})
        ));
    }

    #[test]
    fn copy_text_limits() {
        assert!(validate_call("copy_text", &json!({"text": ""}), &apps(), now()).is_err());
        assert!(validate_call(
            "copy_text",
            &json!({"text": "x".repeat(MAX_COPY_CHARS + 1)}),
            &apps(),
            now()
        )
        .is_err());
        let a = validate_call("copy_text", &json!({"text": "hi"}), &apps(), now()).unwrap();
        assert!(!a.requires_confirmation());
        assert_eq!(a.audit_summary(), "chars=2");
    }

    #[test]
    fn audit_summary_hides_url_path() {
        let a = validate_call(
            "open_url",
            &json!({"url": "https://example.com/secret?token=abc"}),
            &apps(),
            now(),
        )
        .unwrap();
        assert_eq!(a.audit_summary(), "host=example.com");
    }

    #[test]
    fn remember_always_asks() {
        let a = validate_call(
            "remember",
            &json!({"content": " Prefers metric "}),
            &apps(),
            now(),
        )
        .unwrap();
        assert!(a.requires_confirmation());
        assert_eq!(
            a,
            ValidatedAction::Remember {
                content: "Prefers metric".into(),
                category: "other".into()
            }
        );
        assert!(validate_call("remember", &json!({"content": ""}), &apps(), now()).is_err());
    }

    #[test]
    fn declarations_cover_all_tools() {
        let names: Vec<String> = function_declarations()
            .as_array()
            .unwrap()
            .iter()
            .map(|d| d["name"].as_str().unwrap().to_string())
            .collect();
        assert_eq!(
            names,
            [
                "open_app",
                "open_url",
                "copy_text",
                "create_reminder",
                "remember"
            ]
        );
    }
}
