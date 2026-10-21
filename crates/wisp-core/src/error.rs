//! User-facing error model shared by every Wisp subsystem.
//!
//! Errors cross the Rust → UI boundary as [`WispError`], a small serialisable
//! struct with a stable machine code. The UI maps the code to microcopy; the
//! `message` is a concise English fallback and never contains secrets.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// No API key configured.
    MissingKey,
    /// Key rejected by the provider (401/403 or API_KEY_INVALID).
    InvalidKey,
    /// Short-term rate limit (requests/tokens per minute).
    RateLimited,
    /// Daily or project quota exhausted; never retried automatically.
    QuotaExhausted,
    /// Requested model does not exist or is not available to this key.
    ModelUnavailable,
    /// Request was malformed or rejected by safety/validation on the provider side.
    BadRequest,
    /// Prompt or response was blocked by provider safety filters.
    Blocked,
    /// Device appears to be offline / DNS / connect failure.
    Offline,
    /// Request timed out.
    Timeout,
    /// Provider returned 5xx.
    ServerError,
    /// Response could not be parsed.
    MalformedResponse,
    /// User cancelled.
    Cancelled,
    /// Local validation failed (settings, tool arguments, …).
    Invalid,
    /// OS permission missing (microphone, screen capture, …).
    PermissionDenied,
    /// Local storage failure.
    Storage,
    /// A feature is not supported on this platform/build.
    Unsupported,
    /// Anything else. Carries an identifier the user can quote.
    Internal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "camelCase")]
#[error("{code:?}: {message}")]
pub struct WispError {
    pub code: ErrorCode,
    pub message: String,
    /// Seconds after which a retry may succeed, when the provider said so.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after_secs: Option<u64>,
    /// Whether automatic or manual retry is sensible.
    pub retryable: bool,
}

impl WispError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        let retryable = matches!(
            code,
            ErrorCode::RateLimited
                | ErrorCode::Offline
                | ErrorCode::Timeout
                | ErrorCode::ServerError
                | ErrorCode::MalformedResponse
        );
        Self {
            code,
            message: crate::redact::redact(&message.into()),
            retry_after_secs: None,
            retryable,
        }
    }

    pub fn with_retry_after(mut self, secs: Option<u64>) -> Self {
        self.retry_after_secs = secs;
        self
    }

    pub fn invalid(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Invalid, message)
    }

    pub fn storage(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Storage, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        let id = uuid::Uuid::new_v4().simple().to_string();
        Self::new(
            ErrorCode::Internal,
            format!("{} (ref {})", message.into(), &id[..8]),
        )
    }
}

impl From<rusqlite::Error> for WispError {
    fn from(e: rusqlite::Error) -> Self {
        WispError::storage(format!("Database error: {e}"))
    }
}

impl From<std::io::Error> for WispError {
    fn from(e: std::io::Error) -> Self {
        WispError::storage(format!("File error: {e}"))
    }
}

pub type Result<T> = std::result::Result<T, WispError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialises_with_snake_case_code() {
        let e = WispError::new(ErrorCode::QuotaExhausted, "Daily quota used")
            .with_retry_after(Some(30));
        let v = serde_json::to_value(&e).unwrap();
        assert_eq!(v["code"], "quota_exhausted");
        assert_eq!(v["retryAfterSecs"], 30);
        assert_eq!(v["retryable"], false);
    }

    #[test]
    fn messages_are_redacted() {
        let e = WispError::new(
            ErrorCode::InvalidKey,
            "key AIzaSyA1234567890abcdefghijklmnopqrstuv is bad",
        );
        assert!(!e.message.contains("AIza"));
    }

    #[test]
    fn internal_errors_carry_reference() {
        let e = WispError::internal("boom");
        assert!(e.message.contains("(ref "));
    }
}
