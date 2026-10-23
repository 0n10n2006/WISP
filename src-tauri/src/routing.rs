//! Which model answers each request (see `wisp_core::router`), with the
//! state that needs: which models are used up until when, today's request
//! counts, and cached model lists. Saved to `quota.json` in the app data
//! folder so a restart doesn't hammer a model that's already used up.

use crate::state::AppState;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};
use tokio_util::sync::CancellationToken;
use wisp_core::gemini::types::{GenerateContentRequest, GenerateOutcome};
use wisp_core::openrouter::{pick_model, FreeModel, OpenRouterClient};
use wisp_core::router::{
    self, candidates, next_pacific_midnight, Prefs, Provider, QuotaBook, Route, Task,
};
use wisp_core::settings::Settings;
use wisp_core::{ErrorCode, Result, WispError};

/// Used until OpenRouter's free list has been fetched once.
const DEFAULT_OPENROUTER_MODEL: &str = "meta-llama/llama-3.3-70b-instruct:free";
/// How long fetched model lists are trusted.
const LIST_TTL: Duration = Duration::from_secs(3600);

#[derive(Default, Serialize, Deserialize)]
struct Saved {
    book: QuotaBook,
    /// Route key → (start of the Pacific day it counts, requests that day).
    used: HashMap<String, (i64, u32)>,
}

pub struct Routing {
    path: Option<PathBuf>,
    pub book: Mutex<QuotaBook>,
    used: Mutex<HashMap<String, (i64, u32)>>,
    gemini_models: Mutex<Option<(Instant, Vec<String>)>>,
    openrouter_models: Mutex<Option<(Instant, Vec<FreeModel>)>>,
    refreshing: AtomicBool,
    pub openrouter: OpenRouterClient,
}

pub fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

/// Start of the current Gemini day (midnight Pacific), as unix seconds.
fn day_start(now: i64) -> i64 {
    next_pacific_midnight(now) - 86_400
}

impl Routing {
    pub fn load(path: Option<PathBuf>) -> Self {
        let saved: Saved = path
            .as_ref()
            .and_then(|p| std::fs::read(p).ok())
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        let mut book = saved.book;
        book.prune(now());
        Self {
            path,
            book: Mutex::new(book),
            used: Mutex::new(saved.used),
            gemini_models: Mutex::new(None),
            openrouter_models: Mutex::new(None),
            refreshing: AtomicBool::new(false),
            openrouter: OpenRouterClient::new(),
        }
    }

    fn save(&self) {
        let Some(path) = &self.path else { return };
        let saved = Saved {
            book: self.book.lock().expect("quota").clone(),
            used: self.used.lock().expect("used").clone(),
        };
        if let Ok(json) = serde_json::to_vec(&saved) {
            let _ = std::fs::write(path, json);
        }
    }

    /// Forget everything (Settings → Data → Delete everything).
    pub fn clear(&self) {
        *self.book.lock().expect("quota") = QuotaBook::default();
        self.used.lock().expect("used").clear();
        *self.openrouter_models.lock().expect("models") = None;
        if let Some(p) = &self.path {
            let _ = std::fs::remove_file(p);
        }
    }

    fn count(&self, route: &Route) {
        let today = day_start(now());
        let mut used = self.used.lock().expect("used");
        let entry = used.entry(route.key()).or_insert((today, 0));
        if entry.0 != today {
            *entry = (today, 0);
        }
        entry.1 += 1;
        used.retain(|_, (d, _)| *d == today);
    }

    pub fn gemini_models(&self) -> Vec<String> {
        self.gemini_models
            .lock()
            .expect("models")
            .as_ref()
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    }

    pub fn set_gemini_models(&self, ids: Vec<String>) {
        *self.gemini_models.lock().expect("models") = Some((Instant::now(), ids));
    }

    pub fn openrouter_models(&self) -> Vec<FreeModel> {
        self.openrouter_models
            .lock()
            .expect("models")
            .as_ref()
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    }

    pub fn set_openrouter_models(&self, models: Vec<FreeModel>) {
        *self.openrouter_models.lock().expect("models") = Some((Instant::now(), models));
    }

    fn stale<T>(slot: &Mutex<Option<(Instant, T)>>) -> bool {
        slot.lock()
            .expect("models")
            .as_ref()
            .is_none_or(|(t, _)| t.elapsed() > LIST_TTL)
    }
}

/// The two keys, read once per request.
#[derive(Clone, Default)]
pub struct Keys {
    pub gemini: Option<String>,
    pub openrouter: Option<String>,
}

impl Keys {
    pub fn load(state: &AppState) -> Self {
        Self {
            gemini: state.secrets.get().ok().flatten(),
            openrouter: state.openrouter_key.get().ok().flatten(),
        }
    }

    fn has(&self, p: Provider) -> bool {
        match p {
            Provider::Gemini => self.gemini.is_some(),
            Provider::OpenRouter => self.openrouter.is_some(),
        }
    }
}

pub fn prefs(state: &AppState, settings: &Settings, keys: &Keys) -> Prefs {
    let openrouter_model = if keys.openrouter.is_some() {
        pick_model(
            &settings.openrouter_model,
            &state.routing.openrouter_models(),
        )
        .unwrap_or_else(|| DEFAULT_OPENROUTER_MODEL.to_string())
    } else {
        String::new()
    };
    Prefs {
        selected: settings.selected_model.clone(),
        auto_fallback: settings.auto_fallback,
        openrouter_model,
        simple_via_openrouter: settings.simple_via_openrouter,
    }
}

/// Fetch model lists in the background when they're missing or old, so
/// routing knows which Flash-Lite and Gemma models this key has.
pub fn refresh_lists(app: &AppHandle) {
    let state = app.state::<AppState>();
    let r = &state.routing;
    let need = Routing::stale(&r.gemini_models) || Routing::stale(&r.openrouter_models);
    if !need || r.refreshing.swap(true, Ordering::SeqCst) {
        return;
    }
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let state = app.state::<AppState>();
        let keys = Keys::load(&state);
        if let Some(k) = keys.gemini.as_deref() {
            if Routing::stale(&state.routing.gemini_models) {
                if let Ok(models) = state.gemini.list_models(k).await {
                    state
                        .routing
                        .set_gemini_models(models.into_iter().map(|m| m.id).collect());
                }
            }
        }
        if let Some(k) = keys.openrouter.as_deref() {
            if Routing::stale(&state.routing.openrouter_models) {
                if let Ok(models) = state.routing.openrouter.list_free_models(k).await {
                    state.routing.set_openrouter_models(models);
                }
            }
        }
        state.routing.refreshing.store(false, Ordering::SeqCst);
    });
}

/// Models that may answer `task`, best first.
pub fn routes(app: &AppHandle, task: Task, keys: &Keys) -> Vec<Route> {
    refresh_lists(app);
    let state = app.state::<AppState>();
    let settings = state.settings();
    let prefs = prefs(&state, &settings, keys);
    let book = state.routing.book.lock().expect("quota").clone();
    candidates(task, &prefs, &state.routing.gemini_models(), &book, now())
        .into_iter()
        .filter(|r| keys.has(r.provider))
        .collect()
}

/// Who answered, and what the user should know about it.
pub struct Outcome<T> {
    pub value: T,
    pub route: Route,
    /// Models that were tried first and skipped (used up, busy…).
    pub skipped: Vec<String>,
}

/// Try each route until one answers; record limits and today's counts.
pub async fn run<T, F, Fut>(
    app: &AppHandle,
    task: Task,
    keys: &Keys,
    attempt: F,
) -> Result<Outcome<T>>
where
    F: FnMut(Route) -> Fut,
    Fut: Future<Output = (Result<T>, bool)>,
{
    let routes = routes(app, task, keys);
    let state = app.state::<AppState>();
    if routes.is_empty() {
        if keys.gemini.is_none() && keys.openrouter.is_none() {
            return Err(WispError::new(
                ErrorCode::MissingKey,
                "Add your Gemini API key in Settings → Model to ask Wisp.",
            ));
        }
        if keys.gemini.is_none() && matches!(task, Task::Audio | Task::Vision) {
            return Err(WispError::new(
                ErrorCode::MissingKey,
                "Voice and screen questions need a Gemini key (OpenRouter is text only).",
            ));
        }
        return Err(WispError::new(
            ErrorCode::QuotaExhausted,
            "Every free model that can do this is used up for now. Gemini's limits reset at midnight Pacific time.",
        ));
    }
    // Count every attempt, as providers do.
    let mut attempt = attempt;
    let counted = |route: Route| {
        state.routing.count(&route);
        attempt(route)
    };
    let result = router::run_with_fallback(routes, &state.routing.book, now, counted).await;
    state.routing.save();
    match result {
        Ok(a) => Ok(Outcome {
            value: a.value,
            route: a.route,
            skipped: a.skipped.into_iter().map(|s| s.model).collect(),
        }),
        Err((e, _)) => Err(e),
    }
}

/// Stream from whichever provider `route` names.
pub async fn stream(
    state: &AppState,
    keys: &Keys,
    route: &Route,
    body: &GenerateContentRequest,
    cancel: &CancellationToken,
    on_delta: impl FnMut(&str),
) -> Result<GenerateOutcome> {
    let body = router::adapt_for(body.clone(), route.caps);
    match route.provider {
        Provider::Gemini => {
            let key = keys.gemini.as_deref().unwrap_or_default();
            state
                .gemini
                .stream(key, &route.model, &body, cancel, on_delta)
                .await
        }
        Provider::OpenRouter => {
            let key = keys.openrouter.as_deref().unwrap_or_default();
            state
                .routing
                .openrouter
                .stream(key, &route.model, &body, cancel, on_delta)
                .await
        }
    }
}

/// A whole answer at once (transcription, show me, agents).
pub async fn complete(
    state: &AppState,
    keys: &Keys,
    route: &Route,
    body: &GenerateContentRequest,
    cancel: &CancellationToken,
) -> Result<GenerateOutcome> {
    match route.provider {
        Provider::Gemini => {
            let body = router::adapt_for(body.clone(), route.caps);
            let key = keys.gemini.as_deref().unwrap_or_default();
            state
                .gemini
                .generate(key, &route.model, &body, cancel)
                .await
        }
        Provider::OpenRouter => stream(state, keys, route, body, cancel, |_| {}).await,
    }
}

/// Today's use per model, and which are resting until when (for Settings).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelUsage {
    pub model: String,
    pub label: String,
    pub provider: Provider,
    pub requests_today: u32,
    /// Unix seconds when it can be used again; absent when available.
    pub resting_until: Option<i64>,
}

pub fn usage(state: &AppState) -> Vec<ModelUsage> {
    let now = now();
    let today = day_start(now);
    let book = state.routing.book.lock().expect("quota").clone();
    let used = state.routing.used.lock().expect("used").clone();
    let mut keys: Vec<String> = used
        .iter()
        .filter(|(_, (d, _))| *d == today)
        .map(|(k, _)| k.clone())
        .chain(book.exhausted(now).into_iter().map(|(k, _)| k))
        .collect();
    keys.sort();
    keys.dedup();
    keys.into_iter()
        .filter_map(|k| {
            let (provider, model) = k.split_once(':')?;
            let route = match provider {
                "gemini" => Route::gemini(model),
                "openrouter" => Route::openrouter(model),
                _ => return None,
            };
            Some(ModelUsage {
                label: route.label(),
                provider: route.provider,
                model: model.to_string(),
                requests_today: used
                    .get(&k)
                    .filter(|(d, _)| *d == today)
                    .map_or(0, |(_, n)| *n),
                resting_until: book.until.get(&k).copied().filter(|t| *t > now),
            })
        })
        .collect()
}
