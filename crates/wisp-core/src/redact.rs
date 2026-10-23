//! Redaction of secrets before anything is logged, displayed or exported.

use regex::Regex;
use std::sync::LazyLock;

static GOOGLE_KEY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"AIza[0-9A-Za-z_\-]{30,}").expect("valid regex"));
static KEY_PARAM: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)(key=|x-goog-api-key:\s*)[^\s&]+").expect("valid regex"));
static BEARER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)bearer\s+[A-Za-z0-9._\-]+").expect("valid regex"));

static OPENAI_STYLE_KEY: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"sk-(?:or-)?[A-Za-z0-9_\-]{16,}").expect("valid regex"));

pub const REDACTED: &str = "[redacted]";

/// Remove anything that looks like an API key or credential from `input`.
pub fn redact(input: &str) -> String {
    let s = GOOGLE_KEY.replace_all(input, REDACTED);
    let s = KEY_PARAM.replace_all(&s, format!("${{1}}{REDACTED}"));
    let s = BEARER.replace_all(&s, format!("Bearer {REDACTED}"));
    let s = OPENAI_STYLE_KEY.replace_all(&s, REDACTED);
    s.into_owned()
}

/// Redact a specific known secret as well as generic patterns.
pub fn redact_secret(input: &str, secret: &str) -> String {
    let base = if secret.len() >= 8 {
        input.replace(secret, REDACTED)
    } else {
        input.to_owned()
    };
    redact(&base)
}

/// Summarise free text for audit logs: length and a short, redacted prefix.
pub fn summarize_text(input: &str, max_chars: usize) -> String {
    let cleaned = redact(input);
    let count = cleaned.chars().count();
    if count <= max_chars {
        cleaned
    } else {
        let prefix: String = cleaned.chars().take(max_chars).collect();
        format!("{prefix}… ({count} chars)")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_openrouter_keys() {
        assert_eq!(
            redact("key sk-or-v1-0123456789abcdef0123456789 failed"),
            "key [redacted] failed"
        );
    }

    #[test]
    fn redacts_google_keys() {
        let s = redact("using AIzaSyFAKE00000000000000000000000000000 now");
        assert_eq!(s, "using [redacted] now");
    }

    #[test]
    fn redacts_query_and_header_keys() {
        assert_eq!(
            redact("GET /models?key=abc123&x=1"),
            "GET /models?key=[redacted]&x=1"
        );
        assert_eq!(
            redact("x-goog-api-key: secretvalue"),
            "x-goog-api-key: [redacted]"
        );
        assert_eq!(
            redact("Authorization: Bearer abc.def"),
            "Authorization: Bearer [redacted]"
        );
    }

    #[test]
    fn redacts_specific_secret() {
        assert_eq!(
            redact_secret("my token is hunter2hunter2", "hunter2hunter2"),
            "my token is [redacted]"
        );
        // very short "secrets" are not blindly replaced
        assert_eq!(redact_secret("a b c", "a"), "a b c");
    }

    #[test]
    fn summarizes_long_text() {
        let s = summarize_text(&"x".repeat(100), 10);
        assert_eq!(s, "xxxxxxxxxx… (100 chars)");
        assert_eq!(summarize_text("short", 10), "short");
    }
}
