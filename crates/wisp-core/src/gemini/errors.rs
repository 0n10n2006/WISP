//! Mapping of Gemini HTTP/transport failures to [`WispError`].

use crate::error::{ErrorCode, WispError};
use serde_json::Value;

/// Parse a Google duration string such as `"37s"` or `"1.5s"`.
fn parse_duration_secs(s: &str) -> Option<u64> {
    let n: f64 = s.trim().strip_suffix('s')?.parse().ok()?;
    if n.is_finite() && n >= 0.0 {
        Some(n.ceil() as u64)
    } else {
        None
    }
}

/// Map a non-success HTTP response to a user-facing error.
pub fn map_http_error(status: u16, body: &str, retry_after_header: Option<&str>) -> WispError {
    let json: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let err = &json["error"];
    let api_status = err["status"].as_str().unwrap_or_default();
    let api_message = err["message"].as_str().unwrap_or_default();
    let details = err["details"].as_array().cloned().unwrap_or_default();

    let mut retry_after = retry_after_header.and_then(|h| h.trim().parse::<u64>().ok());
    let mut reasons: Vec<String> = Vec::new();
    let mut quota_ids: Vec<String> = Vec::new();
    for d in &details {
        let ty = d["@type"].as_str().unwrap_or_default();
        if ty.ends_with("RetryInfo") {
            if let Some(s) = d["retryDelay"].as_str().and_then(parse_duration_secs) {
                retry_after = Some(s);
            }
        } else if ty.ends_with("ErrorInfo") {
            if let Some(r) = d["reason"].as_str() {
                reasons.push(r.to_string());
            }
        } else if ty.ends_with("QuotaFailure") {
            for v in d["violations"].as_array().into_iter().flatten() {
                if let Some(id) = v["quotaId"].as_str() {
                    quota_ids.push(id.to_string());
                }
            }
        }
    }
    let has_reason = |r: &str| reasons.iter().any(|x| x == r);

    let lower_msg = api_message.to_ascii_lowercase();
    if has_reason("API_KEY_INVALID") || lower_msg.contains("api key not valid") {
        return WispError::new(
            ErrorCode::InvalidKey,
            "Your Gemini API key was rejected. Check or replace it in Settings → Model.",
        );
    }

    match status {
        401 | 403 => WispError::new(
            ErrorCode::InvalidKey,
            "Your Gemini API key isn't allowed to use this model or API. Check the key and that the Generative Language API is enabled for its project.",
        ),
        404 => WispError::new(
            ErrorCode::ModelUnavailable,
            "That model isn't available for your API key. Pick another model in Settings → Model.",
        ),
        429 => {
            let daily = quota_ids
                .iter()
                .any(|q| q.contains("PerDay") || q.contains("Daily"))
                || lower_msg.contains("per day")
                || (lower_msg.contains("quota") && retry_after.is_none_or(|s| s > 3600));
            if daily {
                WispError::new(
                    ErrorCode::QuotaExhausted,
                    "You've used today's Gemini quota for this model. Try again later, switch to another model, or set up billing in Google AI Studio.",
                )
                .with_retry_after(retry_after)
            } else {
                WispError::new(
                    ErrorCode::RateLimited,
                    "Gemini is rate-limiting requests right now. Wait a moment and try again.",
                )
                .with_retry_after(retry_after)
            }
        }
        400 if api_status == "FAILED_PRECONDITION" => WispError::new(
            ErrorCode::Unsupported,
            "Gemini isn't available for this account or region without billing enabled.",
        ),
        400 => WispError::new(
            ErrorCode::BadRequest,
            if api_message.is_empty() {
                "Gemini couldn't process that request.".to_string()
            } else {
                format!("Gemini couldn't process that request: {}", truncate(api_message, 200))
            },
        ),
        408 | 504 => WispError::new(ErrorCode::Timeout, "Gemini took too long to respond. Try again."),
        500..=599 => WispError::new(
            ErrorCode::ServerError,
            "Gemini is having trouble right now (it may be overloaded). Try again shortly.",
        )
        .with_retry_after(retry_after),
        _ => WispError::new(ErrorCode::Internal, format!("Unexpected response from Gemini (HTTP {status})")),
    }
}

/// Map a transport-level error.
pub fn map_transport_error(e: &reqwest::Error) -> WispError {
    if e.is_timeout() {
        WispError::new(
            ErrorCode::Timeout,
            "Gemini took too long to respond. Try again.",
        )
    } else if e.is_connect() || e.is_request() {
        WispError::new(
            ErrorCode::Offline,
            "Couldn't reach Gemini. Check your internet connection; local features still work.",
        )
    } else if e.is_decode() || e.is_body() {
        WispError::new(
            ErrorCode::MalformedResponse,
            "The connection to Gemini was interrupted.",
        )
    } else {
        WispError::new(ErrorCode::Offline, "Network error while contacting Gemini.")
    }
}

/// Map finish reasons that indicate the answer was withheld.
pub fn map_finish_reason(reason: &str) -> Option<WispError> {
    match reason {
        "SAFETY" | "PROHIBITED_CONTENT" | "BLOCKLIST" | "SPII" | "IMAGE_SAFETY" => {
            Some(WispError::new(
                ErrorCode::Blocked,
                "Gemini declined to answer this because of its safety filters.",
            ))
        }
        "RECITATION" => Some(WispError::new(
            ErrorCode::Blocked,
            "Gemini stopped because the answer would repeat copyrighted material.",
        )),
        _ => None,
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(n).collect::<String>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_key() {
        let body = r#"{"error":{"code":400,"message":"API key not valid. Please pass a valid API key.","status":"INVALID_ARGUMENT","details":[{"@type":"type.googleapis.com/google.rpc.ErrorInfo","reason":"API_KEY_INVALID"}]}}"#;
        assert_eq!(map_http_error(400, body, None).code, ErrorCode::InvalidKey);
        assert_eq!(map_http_error(403, "{}", None).code, ErrorCode::InvalidKey);
    }

    #[test]
    fn rate_limit_vs_daily_quota() {
        let rpm = r#"{"error":{"code":429,"status":"RESOURCE_EXHAUSTED","message":"Resource has been exhausted","details":[
            {"@type":"type.googleapis.com/google.rpc.QuotaFailure","violations":[{"quotaId":"GenerateRequestsPerMinutePerProjectPerModel-FreeTier"}]},
            {"@type":"type.googleapis.com/google.rpc.RetryInfo","retryDelay":"7s"}]}}"#;
        let e = map_http_error(429, rpm, None);
        assert_eq!(e.code, ErrorCode::RateLimited);
        assert_eq!(e.retry_after_secs, Some(7));
        assert!(e.retryable);

        let rpd = r#"{"error":{"code":429,"status":"RESOURCE_EXHAUSTED","message":"You exceeded your current quota","details":[
            {"@type":"type.googleapis.com/google.rpc.QuotaFailure","violations":[{"quotaId":"GenerateRequestsPerDayPerProjectPerModel-FreeTier"}]},
            {"@type":"type.googleapis.com/google.rpc.RetryInfo","retryDelay":"12.4s"}]}}"#;
        let e = map_http_error(429, rpd, None);
        assert_eq!(e.code, ErrorCode::QuotaExhausted);
        assert_eq!(e.retry_after_secs, Some(13));
        assert!(!e.retryable);
    }

    #[test]
    fn retry_after_header_used_as_fallback() {
        let e = map_http_error(429, "", Some("5"));
        assert_eq!(e.code, ErrorCode::RateLimited);
        assert_eq!(e.retry_after_secs, Some(5));
    }

    #[test]
    fn other_statuses() {
        assert_eq!(
            map_http_error(404, "{}", None).code,
            ErrorCode::ModelUnavailable
        );
        assert_eq!(map_http_error(503, "{}", None).code, ErrorCode::ServerError);
        assert_eq!(map_http_error(504, "{}", None).code, ErrorCode::Timeout);
        let e = map_http_error(
            400,
            r#"{"error":{"status":"FAILED_PRECONDITION","message":"User location is not supported"}}"#,
            None,
        );
        assert_eq!(e.code, ErrorCode::Unsupported);
        let e = map_http_error(
            400,
            r#"{"error":{"status":"INVALID_ARGUMENT","message":"bad thing"}}"#,
            None,
        );
        assert_eq!(e.code, ErrorCode::BadRequest);
        assert!(e.message.contains("bad thing"));
        assert_eq!(
            map_http_error(418, "teapot", None).code,
            ErrorCode::Internal
        );
    }

    #[test]
    fn finish_reasons() {
        assert!(map_finish_reason("STOP").is_none());
        assert!(map_finish_reason("MAX_TOKENS").is_none());
        assert_eq!(
            map_finish_reason("SAFETY").unwrap().code,
            ErrorCode::Blocked
        );
    }
}
