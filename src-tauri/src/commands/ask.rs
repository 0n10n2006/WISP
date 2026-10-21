use super::CmdResult;
use crate::state::AppState;
use serde::Serialize;
use std::time::Instant;
use tauri::ipc::Channel;
use tauri::State;
use tokio_util::sync::CancellationToken;
use wisp_core::gemini::prompt::{build_request, PromptContext};
use wisp_core::gemini::types::{AskRequest, GenerateOutcome, UsageMetadata};
use wisp_core::settings::Settings;
use wisp_core::storage::RequestLog;
use wisp_core::{ErrorCode, WispError};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum AskEvent {
    Delta { text: String },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AskResult {
    pub text: String,
    pub model: String,
    pub latency_ms: u64,
    pub usage: Option<UsageMetadata>,
    pub finish_reason: Option<String>,
}

/// Drop the cancellation token when the request ends, however it ends.
struct RequestGuard<'a> {
    state: &'a AppState,
    id: String,
}

impl Drop for RequestGuard<'_> {
    fn drop(&mut self) {
        self.state
            .requests
            .lock()
            .expect("requests")
            .remove(&self.id);
    }
}

#[tauri::command]
pub async fn ask(
    state: State<'_, AppState>,
    request: AskRequest,
    on_event: Channel<AskEvent>,
) -> CmdResult<AskResult> {
    if request.prompt.trim().is_empty() && request.attachments.is_empty() {
        return Err(WispError::invalid("Type a question first."));
    }
    let key = state.secrets.require()?;
    let settings = state.settings();
    let cancel = CancellationToken::new();
    state
        .requests
        .lock()
        .expect("requests")
        .insert(request.request_id.clone(), cancel.clone());
    let _guard = RequestGuard {
        state: &state,
        id: request.request_id.clone(),
    };

    let now_local = chrono::Local::now().to_rfc3339();
    // Tools and memory arrive in later milestones; requests are text/image only for now.
    let request = AskRequest {
        enable_tools: false,
        ..request
    };
    let ctx = PromptContext {
        now_local: &now_local,
        memories: &[],
        allowed_apps: &[],
    };
    let body = build_request(&request, &ctx);

    let started = Instant::now();
    let result = state
        .gemini
        .stream(&key, &settings.selected_model, &body, &cancel, |delta| {
            let _ = on_event.send(AskEvent::Delta {
                text: delta.to_string(),
            });
        })
        .await;
    log_request(&state, &settings, &request, &result, started);
    let out = result?;
    Ok(AskResult {
        text: out.text,
        model: settings.selected_model,
        latency_ms: started.elapsed().as_millis() as u64,
        usage: out.usage,
        finish_reason: out.finish_reason,
    })
}

/// Record request metadata (and content only if the user opted in).
fn log_request(
    state: &AppState,
    settings: &Settings,
    request: &AskRequest,
    result: &Result<GenerateOutcome, WispError>,
    started: Instant,
) {
    if settings.history_retention_days == 0 {
        return;
    }
    let kind = serde_json::to_value(&request.context_kind)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_else(|| "text".into());
    let mut entry = RequestLog::new("ask", &settings.selected_model, &kind);
    entry.latency_ms = started.elapsed().as_millis() as u64;
    match result {
        Ok(out) => {
            entry.total_tokens = out.usage.as_ref().map(|u| u.total_token_count);
            if settings.content_history {
                entry.prompt = Some(request.prompt.clone());
                entry.response = Some(out.text.clone());
            }
        }
        Err(e) => {
            entry.status = if e.code == ErrorCode::Cancelled {
                "cancelled"
            } else {
                "error"
            }
            .into();
            entry.error_code = serde_json::to_value(e.code)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned));
            if settings.content_history {
                entry.prompt = Some(request.prompt.clone());
            }
        }
    }
    let _ = state.db.log_request(&entry);
}

#[tauri::command]
pub fn ask_cancel(state: State<'_, AppState>, request_id: String) -> bool {
    match state.requests.lock().expect("requests").get(&request_id) {
        Some(token) => {
            token.cancel();
            true
        }
        None => false,
    }
}
