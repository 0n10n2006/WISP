//! OpenRouter client (optional): free text models for simple jobs and as a
//! last resort when Gemini's free quota is used up.
//!
//! Text only: requests carrying images or audio are refused before anything
//! is sent. The key lives in the OS credential vault and only ever travels
//! in the `Authorization` header — never in URLs, logs or errors.

use crate::error::{ErrorCode, Result, WispError};
use crate::gemini::sse::SseParser;
use crate::gemini::types::{GenerateContentRequest, GenerateOutcome, UsageMetadata};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

pub const DEFAULT_BASE_URL: &str = "https://openrouter.ai/api/v1";
const MAX_ERROR_BODY: usize = 64 * 1024;
const MAX_RESPONSE: usize = 4 * 1024 * 1024;
const MAX_LIST_BODY: usize = 8 * 1024 * 1024;
const MAX_REQUEST_BYTES: usize = 2 * 1024 * 1024;

/// A free model OpenRouter offers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FreeModel {
    pub id: String,
    pub name: String,
    pub context_length: u64,
}

#[derive(Clone)]
pub struct OpenRouterClient {
    http: reqwest::Client,
    base_url: String,
    pub first_byte_timeout: Duration,
    pub idle_timeout: Duration,
}

impl Default for OpenRouterClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Model ids are short ASCII like `meta-llama/llama-3.3-70b-instruct:free`.
pub fn valid_model_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 100
        && !id.contains("..")
        && !id.starts_with(['/', '.'])
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "._/:-".contains(c))
}

/// OpenRouter keys look like `sk-or-v1-…`.
pub fn valid_key(key: &str) -> bool {
    let k = key.trim();
    k.starts_with("sk-or-")
        && k.len() >= 20
        && k.len() <= 200
        && k.chars().all(|c| c.is_ascii_graphic())
}

impl OpenRouterClient {
    pub fn new() -> Self {
        Self::with_base_url(DEFAULT_BASE_URL)
    }

    pub fn with_base_url(base: impl Into<String>) -> Self {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            // Never forward the key to another host.
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(concat!("Wisp/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("http client");
        Self {
            http,
            base_url: base.into().trim_end_matches('/').to_string(),
            first_byte_timeout: Duration::from_secs(45),
            idle_timeout: Duration::from_secs(60),
        }
    }

    fn check_key(key: &str) -> Result<()> {
        if key.trim().is_empty() {
            return Err(WispError::new(
                ErrorCode::MissingKey,
                "Add an OpenRouter key in Settings → Model to use OpenRouter.",
            ));
        }
        Ok(())
    }

    /// Free text models this key can use, biggest first.
    pub async fn list_free_models(&self, key: &str) -> Result<Vec<FreeModel>> {
        Self::check_key(key)?;
        // Check the key itself (the model list is public).
        let auth = self
            .send(
                self.http
                    .get(format!("{}/key", self.base_url))
                    .bearer_auth(key),
            )
            .await?;
        drop(auth);
        let resp = self
            .send(self.http.get(format!("{}/models", self.base_url)))
            .await?;
        let body: Value = serde_json::from_slice(&read_limited(resp, MAX_LIST_BODY).await?)
            .map_err(|_| malformed("Couldn't read OpenRouter's model list."))?;
        let mut out: Vec<FreeModel> = body["data"]
            .as_array()
            .into_iter()
            .flatten()
            .filter(|m| {
                let id = m["id"].as_str().unwrap_or_default();
                let free = id.ends_with(":free")
                    || (m["pricing"]["prompt"].as_str() == Some("0")
                        && m["pricing"]["completion"].as_str() == Some("0"));
                let text_out = m["architecture"]["output_modalities"]
                    .as_array()
                    .is_none_or(|o| o.iter().any(|x| x == "text"));
                free && text_out && valid_model_id(id) && id != "openrouter/auto"
            })
            .map(|m| FreeModel {
                id: m["id"].as_str().unwrap_or_default().to_string(),
                name: m["name"]
                    .as_str()
                    .unwrap_or_default()
                    .chars()
                    .take(80)
                    .collect(),
                context_length: m["context_length"].as_u64().unwrap_or(0),
            })
            .collect();
        out.sort_by_key(|m| std::cmp::Reverse((size_hint(&m.id), m.context_length)));
        Ok(out)
    }

    async fn send(&self, rb: reqwest::RequestBuilder) -> Result<reqwest::Response> {
        let resp = tokio::time::timeout(self.first_byte_timeout, rb.send())
            .await
            .map_err(|_| {
                WispError::new(ErrorCode::Timeout, "OpenRouter took too long to respond.")
            })?
            .map_err(|e| transport_error(&e))?;
        if resp.status().is_success() {
            return Ok(resp);
        }
        let status = resp.status().as_u16();
        let retry_after = resp
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse::<u64>().ok());
        let body = read_limited(resp, MAX_ERROR_BODY)
            .await
            .map(|b| String::from_utf8_lossy(&b).into_owned())
            .unwrap_or_default();
        Err(map_error(status, &body).with_retry_after(retry_after))
    }

    /// Stream an answer from `model`. `on_delta` gets visible text as it arrives.
    pub async fn stream(
        &self,
        key: &str,
        model: &str,
        req: &GenerateContentRequest,
        cancel: &CancellationToken,
        mut on_delta: impl FnMut(&str),
    ) -> Result<GenerateOutcome> {
        Self::check_key(key)?;
        if !valid_model_id(model) {
            return Err(WispError::invalid(
                "That OpenRouter model name isn't valid.",
            ));
        }
        let body = to_chat_request(model, req)?;
        let body = serde_json::to_vec(&body).map_err(|e| WispError::internal(e.to_string()))?;
        if body.len() > MAX_REQUEST_BYTES {
            return Err(WispError::invalid("This request is too large to send."));
        }
        let rb = self
            .http
            .post(format!("{}/chat/completions", self.base_url))
            .bearer_auth(key)
            .header("content-type", "application/json")
            .header("X-Title", "Wisp")
            .body(body);
        let resp = tokio::select! {
            _ = cancel.cancelled() => return Err(cancelled()),
            r = self.send(rb) => r?,
        };
        let mut stream = resp.bytes_stream();
        let mut parser = SseParser::new();
        let mut out = GenerateOutcome::default();
        let mut done = false;
        while !done {
            let next = tokio::select! {
                _ = cancel.cancelled() => return Err(cancelled()),
                n = tokio::time::timeout(self.idle_timeout, stream.next()) => n,
            };
            let bytes = match next {
                Err(_) => {
                    return Err(WispError::new(
                        ErrorCode::Timeout,
                        "OpenRouter stopped responding mid-answer. Try again.",
                    ))
                }
                Ok(None) => break,
                Ok(Some(Err(e))) => return Err(transport_error(&e)),
                Ok(Some(Ok(b))) => b,
            };
            for data in parser.push(&bytes) {
                if data.trim() == "[DONE]" {
                    done = true;
                    break;
                }
                let delta = absorb(&mut out, &data)?;
                if !delta.is_empty() {
                    on_delta(&delta);
                }
            }
            if parser.pending() > MAX_RESPONSE || out.text.len() > MAX_RESPONSE {
                return Err(malformed("OpenRouter's answer was too large."));
            }
        }
        if out.text.trim().is_empty() {
            return Err(WispError::new(
                ErrorCode::ServerError,
                "The OpenRouter model returned an empty answer.",
            ));
        }
        Ok(out)
    }
}

/// Rough parameter count from an id like `…-70b-…` (bigger is usually better).
fn size_hint(id: &str) -> u32 {
    id.split(['-', '/', ':', '_'])
        .filter_map(|p| p.strip_suffix('b'))
        .filter_map(|n| n.parse::<f32>().ok())
        .map(|n| n as u32)
        .max()
        .unwrap_or(0)
}

/// Pick a model: the user's choice if it's offered, else the biggest free one.
pub fn pick_model(chosen: &str, free: &[FreeModel]) -> Option<String> {
    if !chosen.is_empty() && (free.is_empty() || free.iter().any(|m| m.id == chosen)) {
        return Some(chosen.to_string());
    }
    free.first().map(|m| m.id.clone())
}

/// Convert Wisp's Gemini-shaped request into an OpenAI-style chat request.
/// Images and audio are refused: OpenRouter only ever gets text.
pub fn to_chat_request(model: &str, req: &GenerateContentRequest) -> Result<Value> {
    let mut messages = Vec::new();
    if let Some(sys) = &req.system_instruction {
        let text = parts_text(&sys.parts);
        if !text.is_empty() {
            messages.push(json!({ "role": "system", "content": text }));
        }
    }
    for c in &req.contents {
        if c.parts.iter().any(|p| p.inline_data.is_some()) {
            return Err(WispError::new(
                ErrorCode::Unsupported,
                "Wisp never sends images or audio to OpenRouter.",
            ));
        }
        let text = parts_text(&c.parts);
        if text.is_empty() {
            continue;
        }
        let role = if c.role == "model" {
            "assistant"
        } else {
            "user"
        };
        messages.push(json!({ "role": role, "content": text }));
    }
    if !messages.iter().any(|m| m["role"] == "user") {
        return Err(WispError::invalid("Type a question first."));
    }
    let cfg = req.generation_config.clone().unwrap_or(Value::Null);
    let mut body = json!({
        "model": model,
        "messages": messages,
        "stream": true,
        "max_tokens": cfg["maxOutputTokens"].as_u64().unwrap_or(4096).min(8192),
    });
    if let Some(t) = cfg["temperature"].as_f64() {
        body["temperature"] = json!(t);
    }
    Ok(body)
}

fn parts_text(parts: &[crate::gemini::types::Part]) -> String {
    parts
        .iter()
        .filter(|p| !p.thought)
        .filter_map(|p| p.text.as_deref())
        .collect::<Vec<_>>()
        .join("\n")
}

/// Merge one streamed chunk; returns the visible text.
fn absorb(out: &mut GenerateOutcome, data: &str) -> Result<String> {
    let v: Value = serde_json::from_str(data)
        .map_err(|_| malformed("OpenRouter sent a response Wisp couldn't read."))?;
    if v.get("error").is_some() {
        let code = v["error"]["code"].as_u64().unwrap_or(500) as u16;
        return Err(map_error(code, data));
    }
    if let Some(u) = v.get("usage").filter(|u| u.is_object()) {
        out.usage = Some(UsageMetadata {
            prompt_token_count: u["prompt_tokens"].as_u64().unwrap_or(0),
            candidates_token_count: u["completion_tokens"].as_u64().unwrap_or(0),
            total_token_count: u["total_tokens"].as_u64().unwrap_or(0),
        });
    }
    let choice = &v["choices"][0];
    if let Some(r) = choice["finish_reason"].as_str() {
        out.finish_reason = Some(r.to_uppercase());
    }
    let delta = choice["delta"]["content"]
        .as_str()
        .unwrap_or_default()
        .to_string();
    out.text.push_str(&delta);
    Ok(delta)
}

/// Map an OpenRouter error status (and body) to a user-facing error.
pub fn map_error(status: u16, body: &str) -> WispError {
    let v: Value = serde_json::from_str(body).unwrap_or(Value::Null);
    let msg = v["error"]["message"]
        .as_str()
        .unwrap_or_default()
        .to_ascii_lowercase();
    match status {
        401 | 403 => WispError::new(
            ErrorCode::InvalidKey,
            "Your OpenRouter key was rejected. Check or replace it in Settings → Model.",
        ),
        402 => WispError::new(
            ErrorCode::QuotaExhausted,
            "OpenRouter says this account is out of credit for that model.",
        ),
        404 => WispError::new(
            ErrorCode::ModelUnavailable,
            "That OpenRouter model isn't available any more. Pick another in Settings → Model.",
        ),
        429 if msg.contains("per-day") || msg.contains("per day") || msg.contains("daily") => {
            WispError::new(
                ErrorCode::QuotaExhausted,
                "You've used today's free OpenRouter requests (50 a day, or 1,000 after a one-time $10 top-up).",
            )
        }
        429 => WispError::new(
            ErrorCode::RateLimited,
            "OpenRouter's free model is busy right now. Wait a moment and try again.",
        ),
        408 | 504 => WispError::new(ErrorCode::Timeout, "OpenRouter took too long to respond."),
        400 => WispError::new(ErrorCode::BadRequest, "OpenRouter couldn't process that request."),
        500..=599 => WispError::new(
            ErrorCode::ServerError,
            "OpenRouter or its model provider is having trouble right now.",
        ),
        _ => WispError::new(
            ErrorCode::Internal,
            format!("Unexpected response from OpenRouter (HTTP {status})"),
        ),
    }
}

fn transport_error(e: &reqwest::Error) -> WispError {
    if e.is_timeout() {
        WispError::new(ErrorCode::Timeout, "OpenRouter took too long to respond.")
    } else if e.is_connect() || e.is_request() {
        WispError::new(
            ErrorCode::Offline,
            "Couldn't reach OpenRouter. Check your connection.",
        )
    } else {
        WispError::new(
            ErrorCode::MalformedResponse,
            "The connection to OpenRouter was interrupted.",
        )
    }
}

fn malformed(msg: &str) -> WispError {
    WispError::new(ErrorCode::MalformedResponse, msg)
}

fn cancelled() -> WispError {
    WispError::new(ErrorCode::Cancelled, "Cancelled")
}

async fn read_limited(mut resp: reqwest::Response, max: usize) -> Result<Vec<u8>> {
    let read = async {
        let mut buf = Vec::new();
        while let Some(chunk) = resp.chunk().await.map_err(|e| transport_error(&e))? {
            if buf.len() + chunk.len() > max {
                return Err(malformed("OpenRouter's response was too large."));
            }
            buf.extend_from_slice(&chunk);
        }
        Ok(buf)
    };
    tokio::time::timeout(Duration::from_secs(60), read)
        .await
        .map_err(|_| WispError::new(ErrorCode::Timeout, "OpenRouter took too long to answer."))?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gemini::types::{Content, Part};

    fn req(parts: Vec<Part>) -> GenerateContentRequest {
        GenerateContentRequest {
            contents: vec![
                Content {
                    role: "user".into(),
                    parts: vec![Part::text("earlier")],
                },
                Content {
                    role: "model".into(),
                    parts: vec![Part::text("reply")],
                },
                Content {
                    role: "user".into(),
                    parts,
                },
            ],
            system_instruction: Some(Content {
                role: String::new(),
                parts: vec![Part::text("Be brief.")],
            }),
            tools: Some(json!([{ "functionDeclarations": [] }])),
            generation_config: Some(json!({ "temperature": 0.7, "maxOutputTokens": 8192 })),
        }
    }

    #[test]
    fn converts_text_and_refuses_images() {
        let body = to_chat_request("x/y:free", &req(vec![Part::text("hi")])).unwrap();
        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(msgs[0]["role"], "system");
        assert_eq!(msgs[2]["role"], "assistant");
        assert_eq!(msgs[3]["content"], "hi");
        assert!(body.get("tools").is_none());
        assert_eq!(body["stream"], true);
        let err = to_chat_request(
            "x/y:free",
            &req(vec![Part::inline("image/png", "AA"), Part::text("hi")]),
        )
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::Unsupported);
    }

    #[test]
    fn errors_map_to_fallback_codes() {
        let day = r#"{"error":{"code":429,"message":"Rate limit exceeded: free-models-per-day"}}"#;
        assert_eq!(map_error(429, day).code, ErrorCode::QuotaExhausted);
        let min = r#"{"error":{"code":429,"message":"Rate limit exceeded: free-models-per-min"}}"#;
        assert_eq!(map_error(429, min).code, ErrorCode::RateLimited);
        assert_eq!(map_error(401, "").code, ErrorCode::InvalidKey);
        assert_eq!(map_error(404, "").code, ErrorCode::ModelUnavailable);
    }

    #[test]
    fn model_ids_keys_and_picks() {
        assert!(valid_model_id("meta-llama/llama-3.3-70b-instruct:free"));
        assert!(!valid_model_id("x y"));
        assert!(!valid_model_id("../etc"));
        assert!(valid_key("sk-or-v1-0123456789abcdef0123"));
        assert!(!valid_key("AIzaSyFAKE"));
        let free = vec![
            FreeModel {
                id: "a/big-70b:free".into(),
                name: "Big".into(),
                context_length: 1,
            },
            FreeModel {
                id: "a/small-8b:free".into(),
                name: "Small".into(),
                context_length: 1,
            },
        ];
        assert_eq!(pick_model("", &free).as_deref(), Some("a/big-70b:free"));
        assert_eq!(
            pick_model("a/small-8b:free", &free).as_deref(),
            Some("a/small-8b:free")
        );
        assert_eq!(
            pick_model("gone/model:free", &free).as_deref(),
            Some("a/big-70b:free")
        );
        assert_eq!(size_hint("qwen/qwen3-235b-a22b:free"), 235);
    }
}
