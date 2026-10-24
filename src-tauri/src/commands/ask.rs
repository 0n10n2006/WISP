use super::CmdResult;
use crate::actions;
use crate::routing;
use crate::state::AppState;
use chrono::Utc;
use serde::Serialize;
use serde_json::{json, Value};
use std::time::{Duration, Instant};
use tauri::ipc::Channel;
use tauri::{AppHandle, State};
use tokio::sync::oneshot;
use tokio_util::sync::CancellationToken;
use wisp_core::gemini::prompt::{
    build_request, with_function_results, DesktopContext, PromptContext,
};
use wisp_core::gemini::types::{
    AskRequest, FunctionCall, FunctionResponse, GenerateOutcome, Source, UsageMetadata,
};
use wisp_core::redact::summarize_text;
use wisp_core::router::{self, Route, Task};
use wisp_core::settings::Settings;
use wisp_core::storage::{ActionAudit, RequestLog};
use wisp_core::tools::{validate_call_in, ToolEnv};
use wisp_core::{ErrorCode, WispError};

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum AskEvent {
    Delta {
        text: String,
    },
    /// The model proposed an action that needs the user's go-ahead.
    #[serde(rename_all = "camelCase")]
    ActionProposed {
        id: String,
        kind: String,
        description: String,
    },
    #[serde(rename_all = "camelCase")]
    ActionResult {
        kind: String,
        description: String,
        /// "done" | "declined" | "rejected" | "failed"
        outcome: String,
        message: String,
    },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AskResult {
    pub text: String,
    pub model: String,
    pub latency_ms: u64,
    pub usage: Option<UsageMetadata>,
    pub finish_reason: Option<String>,
    /// Web sources for grounded answers.
    pub sources: Vec<Source>,
    /// Readable name of the model that answered.
    pub model_label: String,
    /// Another model answered because the first choice was used up or busy.
    pub fell_back: bool,
    /// Models skipped on the way (readable names).
    pub skipped: Vec<String>,
    /// Something the user should know about this answer.
    pub note: Option<String>,
}

/// Model ↔ tool round trips allowed per request.
const MAX_TOOL_ROUNDS: usize = 3;
/// How long a confirmation prompt waits before counting as "no".
const CONFIRM_TIMEOUT: Duration = Duration::from_secs(120);

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
    app: AppHandle,
    state: State<'_, AppState>,
    request: AskRequest,
    on_event: Channel<AskEvent>,
) -> CmdResult<AskResult> {
    if request.prompt.trim().is_empty() && request.attachments.is_empty() {
        return Err(WispError::invalid("Type a question first."));
    }
    let keys = routing::Keys::load(&state);
    if keys.gemini.is_none() && keys.openrouter.is_none() {
        state.secrets.require()?;
    }
    let settings = state.settings();
    // Shared window titles are text from other apps (a web page's title, say).
    let origin = if settings.app_awareness && settings.share_window_titles {
        wisp_core::tools::Origin::Untrusted
    } else {
        request.origin()
    };
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
    let memories = if request.use_memory && settings.use_memory {
        state.db.enabled_memories().unwrap_or_default()
    } else {
        Vec::new()
    };
    let apps: Vec<String> = settings
        .allowed_apps
        .iter()
        .map(|a| a.name.clone())
        .collect();
    let desktop = if settings.app_awareness {
        Some(desktop_context(&app, &state, settings.share_window_titles).await)
    } else {
        None
    };
    // The request as one model sees it: no tools or search it can't use.
    let body_for = |route: &Route| {
        let mut r = request.clone();
        r.enable_tools &= route.caps.tools;
        r.web &= route.caps.search;
        let ctx = PromptContext {
            now_local: &now_local,
            memories: &memories,
            allowed_apps: &apps,
            model: &route.model,
            fast: settings.fast_mode,
            persona: &settings.persona,
            desktop: desktop.as_ref(),
        };
        build_request(&r, &ctx)
    };

    let started = Instant::now();
    let task = router::classify(&request);
    let first = routing::run(&app, task, &keys, |route| {
        let body = body_for(&route);
        let (state, keys, cancel, on_event) = (&state, &keys, &cancel, &on_event);
        async move {
            let mut began = false;
            let res = routing::stream(state, keys, &route, &body, cancel, |delta| {
                began = true;
                let _ = on_event.send(AskEvent::Delta {
                    text: delta.to_string(),
                });
            })
            .await;
            (res.map(|out| (out, body)), began)
        }
    })
    .await;
    let model_for_log = first
        .as_ref()
        .map(|o| o.route.model.clone())
        .unwrap_or_else(|_| settings.selected_model.clone());
    log_request(
        &state,
        &settings,
        &request,
        first.as_ref().map(|o| &o.value.0),
        &model_for_log,
        started,
    );
    let routing::Outcome {
        value: (first_out, mut body),
        route,
        skipped,
    } = first?;

    let mut text = String::new();
    let mut usage = None;
    let mut finish_reason = None;
    let mut sources: Vec<Source> = Vec::new();
    let mut next = Some(first_out);
    for _ in 0..MAX_TOOL_ROUNDS {
        let out = match next.take() {
            Some(out) => out,
            None => {
                // Later tool rounds stay with the model that started.
                let mut first_delta = true;
                let separate = !text.is_empty();
                routing::stream(&state, &keys, &route, &body, &cancel, |delta| {
                    let mut d = delta.to_string();
                    if first_delta && separate {
                        d.insert_str(0, "\n\n");
                    }
                    first_delta = false;
                    let _ = on_event.send(AskEvent::Delta { text: d });
                })
                .await?
            }
        };
        if !out.text.is_empty() {
            if !text.is_empty() {
                text.push_str("\n\n");
            }
            text.push_str(&out.text);
        }
        usage = out.usage.clone().or(usage);
        for src in &out.sources {
            if !sources.contains(src) {
                sources.push(src.clone());
            }
        }
        finish_reason = out.finish_reason.clone();
        if out.function_calls.is_empty() || !request.enable_tools {
            break;
        }
        let mut responses = Vec::new();
        for (i, call) in out.function_calls.iter().enumerate() {
            // A reply asking for many actions at once is a sign of injected
            // instructions; only the first few are considered.
            let response = if i < wisp_core::tools::MAX_CALLS_PER_TURN {
                handle_call(&app, &state, call, origin, &on_event, &cancel).await
            } else {
                json!({ "ok": false, "error": "Too many actions at once; this one was not run." })
            };
            responses.push(FunctionResponse {
                name: call.name.clone(),
                response,
            });
        }
        if cancel.is_cancelled() {
            return Err(WispError::new(ErrorCode::Cancelled, "Cancelled"));
        }
        body = with_function_results(body, out.model_parts, responses);
    }
    let note = if request.web && !route.caps.search {
        Some(
            "Answered without web search: the models that can search are used up for now."
                .to_string(),
        )
    } else if request.enable_tools && !route.caps.tools && task == Task::Actions {
        Some(format!(
            "Desktop actions aren't available while Wisp is using {}.",
            route.label()
        ))
    } else {
        None
    };
    Ok(AskResult {
        text,
        model_label: route.label(),
        fell_back: !skipped.is_empty(),
        model: route.model,
        skipped,
        note,
        latency_ms: started.elapsed().as_millis() as u64,
        usage,
        finish_reason,
        sources,
    })
}

/// Open the TLS connection to Gemini ahead of the first question, so the
/// answer starts sooner. Costs no generation quota.
#[tauri::command]
pub async fn prewarm(state: State<'_, AppState>) -> CmdResult<()> {
    if let Ok(Some(key)) = state.secrets.get() {
        state.gemini.prewarm(&key).await;
    }
    Ok(())
}

/// Validate, confirm, execute and audit one model-proposed action.
/// The app the user came from, their open windows and installed apps, as
/// names (titles only when the user allows it).
async fn desktop_context(app: &AppHandle, state: &AppState, titles: bool) -> DesktopContext {
    let target = *state.insert_target.lock().expect("target");
    let foreground = target.and_then(crate::foreground::process_name);
    let foreground_title = if titles {
        target.and_then(crate::foreground::title)
    } else {
        None
    };
    let windows = tauri::async_runtime::spawn_blocking(crate::desktop::open_windows)
        .await
        .unwrap_or_default();
    let mut open: Vec<String> = Vec::new();
    for w in windows {
        let entry = if titles {
            format!(
                "{} — {}",
                w.app,
                w.title.chars().take(80).collect::<String>()
            )
        } else {
            w.app
        };
        if !open.contains(&entry) {
            open.push(entry);
        }
    }
    let installed = super::dashboard::cached_catalogue(app, false)
        .await
        .iter()
        .filter(|i| !wisp_core::launcher::is_shell(i))
        .map(|i| i.name.clone())
        .collect();
    DesktopContext {
        foreground,
        foreground_title,
        open,
        installed,
    }
}

async fn handle_call(
    app: &AppHandle,
    state: &AppState,
    call: &FunctionCall,
    origin: wisp_core::tools::Origin,
    on_event: &Channel<AskEvent>,
    cancel: &CancellationToken,
) -> Value {
    let settings = state.settings();
    let report = |kind: &str,
                  summary: &str,
                  description: &str,
                  outcome: &str,
                  message: &str,
                  confirmed: bool| {
        let mut audit = ActionAudit::new(kind, summary, outcome, confirmed);
        if outcome != "done" {
            audit.detail = Some(message.to_string());
        }
        let _ = state.db.log_action(&audit);
        let _ = on_event.send(AskEvent::ActionResult {
            kind: kind.to_string(),
            description: description.to_string(),
            outcome: outcome.to_string(),
            message: message.to_string(),
        });
    };

    let launchable = if settings.app_awareness {
        super::dashboard::cached_catalogue(app, false).await
    } else {
        Default::default()
    };
    let windows = if settings.app_awareness {
        tauri::async_runtime::spawn_blocking(crate::desktop::open_windows)
            .await
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let env = ToolEnv {
        allowed_apps: &settings.allowed_apps,
        launchable: &launchable,
        windows: &windows,
    };
    let validated =
        wisp_core::tools::split_delay(&call.name, &call.args).and_then(|(args, delay)| {
            Ok((
                validate_call_in(&call.name, &args, &env, Utc::now())?,
                delay,
            ))
        });
    let (action, delay) = match validated {
        Ok(a) => a,
        Err(e) => {
            let kind = summarize_text(&call.name, 40);
            report(&kind, "", "Proposed action", "rejected", &e.message, false);
            return json!({ "ok": false, "error": e.message });
        }
    };
    let kind = action.kind();
    let description = match delay {
        Some(secs) => format!(
            "{} {}",
            action.describe(),
            wisp_core::tools::delay_phrase(secs)
        ),
        None => action.describe(),
    };
    let summary = action.audit_summary();

    let mut confirmed = false;
    if action.needs_confirmation(origin, delay, settings.confirm_actions) {
        let id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel::<bool>();
        state
            .confirmations
            .lock()
            .expect("confirmations")
            .insert(id.clone(), tx);
        let _ = on_event.send(AskEvent::ActionProposed {
            id: id.clone(),
            kind: kind.to_string(),
            description: description.clone(),
        });
        let approved = tokio::select! {
            _ = cancel.cancelled() => false,
            r = tokio::time::timeout(CONFIRM_TIMEOUT, rx) => matches!(r, Ok(Ok(true))),
        };
        state
            .confirmations
            .lock()
            .expect("confirmations")
            .remove(&id);
        if !approved {
            report(
                kind,
                &summary,
                &description,
                "declined",
                "You chose not to do this.",
                false,
            );
            return json!({ "ok": false, "error": "The user declined this action. Don't retry it." });
        }
        confirmed = true;
    }

    if let Some(secs) = delay {
        let message = schedule(app, action, description.clone(), secs);
        report(
            kind,
            &summary,
            &description,
            "scheduled",
            &message,
            confirmed,
        );
        return json!({ "ok": true, "result": message });
    }

    match actions::run(app, &action).await {
        Ok(message) => {
            report(kind, &summary, &description, "done", &message, confirmed);
            json!({ "ok": true, "result": message })
        }
        Err(e) => {
            report(
                kind,
                &summary,
                &description,
                "failed",
                &e.message,
                confirmed,
            );
            json!({ "ok": false, "error": e.message })
        }
    }
}

/// Run a confirmed action after `secs`; it can be cancelled until then.
fn schedule(
    app: &AppHandle,
    action: wisp_core::tools::ValidatedAction,
    description: String,
    secs: u32,
) -> String {
    use tauri::{Emitter, Manager};
    let id = uuid::Uuid::new_v4().to_string();
    let token = CancellationToken::new();
    let run_at = Utc::now().timestamp_millis() + secs as i64 * 1000;
    let state = app.state::<AppState>();
    state
        .scheduled
        .lock()
        .expect("scheduled")
        .push(crate::state::ScheduledAction {
            id: id.clone(),
            description: description.clone(),
            run_at,
            cancel: token.clone(),
        });
    let _ = app.emit("wisp://data-changed", "scheduled");
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let fire = tokio::select! {
            _ = token.cancelled() => false,
            _ = tokio::time::sleep(Duration::from_secs(secs as u64)) => true,
        };
        let state = app.state::<AppState>();
        state
            .scheduled
            .lock()
            .expect("scheduled")
            .retain(|s| s.id != id);
        let _ = app.emit("wisp://data-changed", "scheduled");
        if !fire {
            return;
        }
        let result = actions::run(&app, &action).await;
        let (outcome, message) = match &result {
            Ok(m) => ("done", m.clone()),
            Err(e) => ("failed", e.message.clone()),
        };
        let mut audit = ActionAudit::new(action.kind(), &action.audit_summary(), outcome, true);
        if outcome != "done" {
            audit.detail = Some(message.clone());
        }
        let _ = state.db.log_action(&audit);
        let _ = app.emit(
            "wisp://scheduled-ran",
            json!({ "description": description, "outcome": outcome, "message": message }),
        );
    });
    format!("Scheduled: {}.", lower_first(&description_after(secs)))
}

fn description_after(secs: u32) -> String {
    format!("it will run {}", wisp_core::tools::delay_phrase(secs))
}

fn lower_first(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_lowercase().chain(c).collect(),
        None => String::new(),
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledInfo {
    pub id: String,
    pub description: String,
    pub run_at: i64,
}

#[tauri::command]
pub fn scheduled_list(state: State<'_, AppState>) -> Vec<ScheduledInfo> {
    state
        .scheduled
        .lock()
        .expect("scheduled")
        .iter()
        .map(|s| ScheduledInfo {
            id: s.id.clone(),
            description: s.description.clone(),
            run_at: s.run_at,
        })
        .collect()
}

#[tauri::command]
pub fn scheduled_cancel(state: State<'_, AppState>, id: String) -> bool {
    match state
        .scheduled
        .lock()
        .expect("scheduled")
        .iter()
        .find(|s| s.id == id)
    {
        Some(s) => {
            s.cancel.cancel();
            true
        }
        None => false,
    }
}

/// The user's answer to an `ActionProposed` event.
#[tauri::command]
pub fn action_respond(state: State<'_, AppState>, id: String, approved: bool) -> bool {
    match state
        .confirmations
        .lock()
        .expect("confirmations")
        .remove(&id)
    {
        Some(tx) => tx.send(approved).is_ok(),
        None => false,
    }
}

/// Record request metadata (and content only if the user opted in).
fn log_request(
    state: &AppState,
    settings: &Settings,
    request: &AskRequest,
    result: Result<&GenerateOutcome, &WispError>,
    model: &str,
    started: Instant,
) {
    if settings.history_retention_days == 0 {
        return;
    }
    let kind = serde_json::to_value(&request.context_kind)
        .ok()
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_else(|| "text".into());
    let mut entry = RequestLog::new("ask", model, &kind);
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
