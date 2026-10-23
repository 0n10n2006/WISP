//! Gemini API client: model listing, connection test, streaming generation.
//!
//! The API key is passed per call and only ever placed in the
//! `x-goog-api-key` header — never in URLs, logs or errors.

pub mod errors;
pub mod prompt;
pub mod sse;
pub mod types;

use crate::error::{ErrorCode, Result, WispError};
use crate::retry::RetryPolicy;
use errors::{map_finish_reason, map_http_error, map_transport_error};
use futures_util::StreamExt;
use std::future::Future;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use types::*;

pub const DEFAULT_BASE_URL: &str = "https://generativelanguage.googleapis.com";
/// Hard cap on request bodies (images included) so nothing huge leaves the device by accident.
pub const MAX_REQUEST_BYTES: usize = 15 * 1024 * 1024;

#[derive(Clone)]
pub struct GeminiClient {
    http: reqwest::Client,
    base_url: String,
    pub retry: RetryPolicy,
    /// Max wait for the response headers / first chunk.
    pub first_byte_timeout: Duration,
    /// Max silence between streamed chunks.
    pub idle_timeout: Duration,
}

impl GeminiClient {
    pub fn new() -> Self {
        Self::with_base_url(DEFAULT_BASE_URL)
    }

    pub fn with_base_url(base: impl Into<String>) -> Self {
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            // The API key travels in a custom header that reqwest would
            // forward on a cross-host redirect; Gemini never redirects.
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(concat!("Wisp/", env!("CARGO_PKG_VERSION")))
            .build()
            .expect("http client");
        Self {
            http,
            base_url: base.into().trim_end_matches('/').to_string(),
            retry: RetryPolicy::default(),
            first_byte_timeout: Duration::from_secs(45),
            idle_timeout: Duration::from_secs(60),
        }
    }

    fn check_key(key: &str) -> Result<()> {
        if key.trim().is_empty() {
            return Err(WispError::new(
                ErrorCode::MissingKey,
                "Add your Gemini API key in Settings to ask Wisp.",
            ));
        }
        Ok(())
    }

    async fn with_retry<T, F, Fut>(&self, cancel: &CancellationToken, mut f: F) -> Result<T>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = Result<T>>,
    {
        let mut attempt = 1;
        loop {
            let res = tokio::select! {
                _ = cancel.cancelled() => return Err(cancelled()),
                r = f() => r,
            };
            match res {
                Ok(v) => return Ok(v),
                Err(e) => match self.retry.next_delay(attempt, &e) {
                    Some(d) => {
                        tokio::select! {
                            _ = cancel.cancelled() => return Err(cancelled()),
                            _ = tokio::time::sleep(d) => {}
                        }
                        attempt += 1;
                    }
                    None => return Err(e),
                },
            }
        }
    }

    async fn send_checked(&self, rb: reqwest::RequestBuilder) -> Result<reqwest::Response> {
        let resp = tokio::time::timeout(self.first_byte_timeout, rb.send())
            .await
            .map_err(|_| {
                WispError::new(
                    ErrorCode::Timeout,
                    "Gemini took too long to respond. Try again.",
                )
            })?
            .map_err(|e| map_transport_error(&e))?;
        let status = resp.status();
        if status.is_success() {
            return Ok(resp);
        }
        let retry_after = resp
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let body = read_limited(resp, MAX_ERROR_BODY)
            .await
            .map(|b| String::from_utf8_lossy(&b).into_owned())
            .unwrap_or_default();
        Err(map_http_error(
            status.as_u16(),
            &body,
            retry_after.as_deref(),
        ))
    }

    /// List models usable with `generateContent` for this key.
    pub async fn list_models(&self, key: &str) -> Result<Vec<ModelInfo>> {
        Self::check_key(key)?;
        let cancel = CancellationToken::new();
        let mut out = Vec::new();
        let mut page: Option<String> = None;
        for _ in 0..10 {
            let url = format!("{}/v1beta/models", self.base_url);
            let page_token = page.clone();
            let resp: ListModelsResponse = self
                .with_retry(&cancel, || {
                    let mut rb = self
                        .http
                        .get(&url)
                        .header("x-goog-api-key", key)
                        .query(&[("pageSize", "200")]);
                    if let Some(t) = &page_token {
                        rb = rb.query(&[("pageToken", t)]);
                    }
                    async move {
                        let r = self.send_checked(rb).await?;
                        read_json::<ListModelsResponse>(r).await.map_err(|_| {
                            WispError::new(
                                ErrorCode::MalformedResponse,
                                "Couldn't read the model list.",
                            )
                        })
                    }
                })
                .await?;
            out.extend(resp.models.into_iter().filter_map(|m| {
                let id = m
                    .name
                    .strip_prefix("models/")
                    .unwrap_or(&m.name)
                    .to_string();
                let usable = m
                    .supported_generation_methods
                    .iter()
                    .any(|g| g == "generateContent");
                (usable
                    && id.starts_with("gemini")
                    && !id.contains("embedding")
                    && !id.contains("tts")
                    && !id.contains("image"))
                .then(|| ModelInfo {
                    display_name: if m.display_name.is_empty() {
                        id.clone()
                    } else {
                        m.display_name
                    },
                    id,
                    description: m.description,
                    input_token_limit: m.input_token_limit,
                })
            }));
            match resp.next_page_token.filter(|t| !t.is_empty()) {
                Some(t) => page = Some(t),
                None => break,
            }
        }
        out.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(out)
    }

    /// Warm up DNS/TLS/HTTP2 to the API host (a tiny model-list call that
    /// doesn't count toward generation quota). Errors are ignored.
    pub async fn prewarm(&self, key: &str) {
        let url = format!("{}/v1beta/models", self.base_url);
        let _ = tokio::time::timeout(
            Duration::from_secs(5),
            self.http
                .get(url)
                .header("x-goog-api-key", key)
                .query(&[("pageSize", "1")])
                .send(),
        )
        .await;
    }

    /// Validate a key and report whether `model` is available to it.
    pub async fn test_connection(&self, key: &str, model: &str) -> Result<ConnectionReport> {
        let started = std::time::Instant::now();
        let models = self.list_models(key).await?;
        Ok(ConnectionReport {
            ok: true,
            model_available: models.iter().any(|m| m.id == model),
            models,
            latency_ms: started.elapsed().as_millis() as u64,
        })
    }

    fn encode(req: &GenerateContentRequest) -> Result<Vec<u8>> {
        let body = serde_json::to_vec(req).map_err(|e| WispError::internal(e.to_string()))?;
        if body.len() > MAX_REQUEST_BYTES {
            return Err(WispError::invalid(
                "This request is too large to send. Try a smaller selection.",
            ));
        }
        Ok(body)
    }

    /// Non-streaming generation (used for transcription and short utilities).
    pub async fn generate(
        &self,
        key: &str,
        model: &str,
        req: &GenerateContentRequest,
        cancel: &CancellationToken,
    ) -> Result<GenerateOutcome> {
        Self::check_key(key)?;
        let body = Self::encode(req)?;
        let url = format!("{}/v1beta/models/{}:generateContent", self.base_url, model);
        let resp: GenerateContentResponse = self
            .with_retry(cancel, || {
                let rb = self
                    .http
                    .post(&url)
                    .header("x-goog-api-key", key)
                    .header("content-type", "application/json")
                    .body(body.clone());
                async move {
                    let r = self.send_checked(rb).await?;
                    read_json::<GenerateContentResponse>(r).await.map_err(|_| {
                        WispError::new(
                            ErrorCode::MalformedResponse,
                            "Gemini sent a response Wisp couldn't read.",
                        )
                    })
                }
            })
            .await?;
        let mut out = GenerateOutcome::default();
        check_prompt_feedback(&resp)?;
        out.absorb(resp);
        finish(out)
    }

    /// Streaming generation. `on_delta` receives visible text as it arrives.
    /// Retries happen only before the first chunk, so text is never duplicated.
    pub async fn stream(
        &self,
        key: &str,
        model: &str,
        req: &GenerateContentRequest,
        cancel: &CancellationToken,
        mut on_delta: impl FnMut(&str),
    ) -> Result<GenerateOutcome> {
        Self::check_key(key)?;
        let body = Self::encode(req)?;
        let url = format!(
            "{}/v1beta/models/{}:streamGenerateContent?alt=sse",
            self.base_url, model
        );
        let resp = self
            .with_retry(cancel, || {
                let rb = self
                    .http
                    .post(&url)
                    .header("x-goog-api-key", key)
                    .header("content-type", "application/json")
                    .body(body.clone());
                self.send_checked(rb)
            })
            .await?;

        let mut stream = resp.bytes_stream();
        let mut parser = sse::SseParser::new();
        let mut out = GenerateOutcome::default();
        let mut handle = |data: String, out: &mut GenerateOutcome| -> Result<()> {
            let chunk: GenerateContentResponse = serde_json::from_str(&data).map_err(|_| {
                WispError::new(
                    ErrorCode::MalformedResponse,
                    "Gemini sent a response Wisp couldn't read.",
                )
            })?;
            check_prompt_feedback(&chunk)?;
            let delta = out.absorb(chunk);
            if !delta.is_empty() {
                on_delta(&delta);
            }
            Ok(())
        };
        loop {
            let next = tokio::select! {
                _ = cancel.cancelled() => return Err(cancelled()),
                n = tokio::time::timeout(self.idle_timeout, stream.next()) => n,
            };
            match next {
                Err(_) => {
                    return Err(WispError::new(
                        ErrorCode::Timeout,
                        "Gemini stopped responding mid-answer. Try again.",
                    ))
                }
                Ok(None) => break,
                Ok(Some(Err(e))) => return Err(map_transport_error(&e)),
                Ok(Some(Ok(bytes))) => {
                    for data in parser.push(&bytes) {
                        handle(data, &mut out)?;
                    }
                    if parser.pending() > MAX_RESPONSE_BODY || out.text.len() > MAX_RESPONSE_BODY {
                        return Err(WispError::new(
                            ErrorCode::MalformedResponse,
                            "Gemini's answer was too large.",
                        ));
                    }
                }
            }
        }
        if let Some(data) = parser.finish() {
            handle(data, &mut out)?;
        }
        finish(out)
    }
}

/// Largest error body read (they are short JSON messages).
const MAX_ERROR_BODY: usize = 64 * 1024;
/// Largest response or streamed answer accepted.
const MAX_RESPONSE_BODY: usize = 8 * 1024 * 1024;
/// A whole non-streamed body must arrive within this time.
const BODY_TIMEOUT: Duration = Duration::from_secs(180);

/// Read at most `max` bytes of a body, within `BODY_TIMEOUT`.
async fn read_limited(mut resp: reqwest::Response, max: usize) -> Result<Vec<u8>> {
    let read = async {
        let mut buf = Vec::new();
        while let Some(chunk) = resp.chunk().await.map_err(|e| map_transport_error(&e))? {
            if buf.len() + chunk.len() > max {
                return Err(WispError::new(
                    ErrorCode::MalformedResponse,
                    "Gemini's answer was too large.",
                ));
            }
            buf.extend_from_slice(&chunk);
        }
        Ok(buf)
    };
    tokio::time::timeout(BODY_TIMEOUT, read)
        .await
        .map_err(|_| {
            WispError::new(
                ErrorCode::Timeout,
                "Gemini took too long to answer. Try again.",
            )
        })?
}

async fn read_json<T: serde::de::DeserializeOwned>(resp: reqwest::Response) -> Result<T> {
    let bytes = read_limited(resp, MAX_RESPONSE_BODY).await?;
    serde_json::from_slice(&bytes).map_err(|_| {
        WispError::new(
            ErrorCode::MalformedResponse,
            "Gemini sent a response Wisp couldn't read.",
        )
    })
}

impl Default for GeminiClient {
    fn default() -> Self {
        Self::new()
    }
}

fn cancelled() -> WispError {
    WispError::new(ErrorCode::Cancelled, "Cancelled")
}

fn check_prompt_feedback(r: &GenerateContentResponse) -> Result<()> {
    if let Some(reason) = r
        .prompt_feedback
        .as_ref()
        .and_then(|f| f.block_reason.as_deref())
    {
        return Err(WispError::new(
            ErrorCode::Blocked,
            format!(
                "Gemini declined this request ({}).",
                reason.to_lowercase().replace('_', " ")
            ),
        ));
    }
    Ok(())
}

fn finish(out: GenerateOutcome) -> Result<GenerateOutcome> {
    if let Some(e) = out.finish_reason.as_deref().and_then(map_finish_reason) {
        if out.text.is_empty() {
            return Err(e);
        }
    }
    if out.text.is_empty() && out.function_calls.is_empty() {
        return Err(WispError::new(
            ErrorCode::MalformedResponse,
            "Gemini returned an empty answer. Try again.",
        ));
    }
    Ok(out)
}

#[derive(Debug, Clone, serde::Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionReport {
    pub ok: bool,
    pub model_available: bool,
    pub models: Vec<ModelInfo>,
    pub latency_ms: u64,
}
