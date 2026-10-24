//! Picks which model answers each request, and falls back to the next one
//! when a free quota runs out.
//!
//! Gemini (with the user's key) comes first, then Google's open Gemma models
//! on the same key, then an optional OpenRouter key for text. Simple jobs go
//! to OpenRouter first, to save the Gemini quota for what only Gemini can do
//! (web search, voice, screenshots). Everything here is pure, so it's tested
//! without a network.

use crate::error::{ErrorCode, Result, WispError};
use crate::gemini::types::{AskRequest, ContextKind, GenerateContentRequest, Part};
use chrono::{DateTime, Datelike, Duration as ChronoDuration, NaiveDate, TimeZone, Utc, Weekday};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::future::Future;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Provider {
    Gemini,
    OpenRouter,
}

/// What a request needs, which decides who can answer it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Task {
    /// An ordinary question.
    Chat,
    /// A request that may need desktop actions (function calling).
    Actions,
    /// Rewrite, summarise or explain supplied text; short plain questions.
    Simple,
    /// Needs Google Search.
    Web,
    /// Includes an image (circled area, screenshot).
    Vision,
    /// Includes audio (dictation, voice).
    Audio,
}

/// What a model can be sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Caps {
    pub tools: bool,
    pub search: bool,
    pub system_prompt: bool,
    pub json_mode: bool,
    pub images: bool,
    pub audio: bool,
}

const GEMINI_CAPS: Caps = Caps {
    tools: true,
    search: true,
    system_prompt: true,
    json_mode: true,
    images: true,
    audio: true,
};
const GEMMA_CAPS: Caps = Caps {
    tools: false,
    search: false,
    system_prompt: false,
    json_mode: false,
    images: true,
    audio: false,
};
const OPENROUTER_CAPS: Caps = Caps {
    tools: false,
    search: false,
    system_prompt: true,
    json_mode: false,
    images: false,
    audio: false,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Route {
    pub provider: Provider,
    pub model: String,
    pub caps: Caps,
}

impl Route {
    pub fn gemini(model: impl Into<String>) -> Self {
        let model = model.into();
        let caps = if is_gemma(&model) {
            GEMMA_CAPS
        } else {
            GEMINI_CAPS
        };
        Self {
            provider: Provider::Gemini,
            model,
            caps,
        }
    }

    pub fn openrouter(model: impl Into<String>) -> Self {
        Self {
            provider: Provider::OpenRouter,
            model: model.into(),
            caps: OPENROUTER_CAPS,
        }
    }

    /// Key used for quota bookkeeping.
    pub fn key(&self) -> String {
        let p = match self.provider {
            Provider::Gemini => "gemini",
            Provider::OpenRouter => "openrouter",
        };
        format!("{p}:{}", self.model)
    }

    /// A short name for the UI ("Gemma 3 27B", "OpenRouter · Llama 3.3 70B").
    pub fn label(&self) -> String {
        match self.provider {
            Provider::Gemini => pretty_model(&self.model),
            Provider::OpenRouter => {
                let name = self.model.rsplit('/').next().unwrap_or(&self.model);
                let name = name.trim_end_matches(":free");
                format!("OpenRouter · {}", pretty_model(name))
            }
        }
    }
}

/// "gemini-2.5-flash-lite" → "Gemini 2.5 Flash Lite", "gemma-3-27b-it" → "Gemma 3 27B".
pub fn pretty_model(id: &str) -> String {
    id.split(['-', '_'])
        .filter(|w| !w.is_empty() && *w != "it" && *w != "instruct" && *w != "latest")
        .map(|w| {
            if w.len() <= 4 && w.ends_with('b') && w[..w.len() - 1].parse::<f32>().is_ok() {
                w.to_uppercase()
            } else {
                let mut c = w.chars();
                match c.next() {
                    Some(f) => f.to_uppercase().chain(c).collect(),
                    None => String::new(),
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn is_gemma(model: &str) -> bool {
    model.to_ascii_lowercase().starts_with("gemma")
}

/// Words that suggest the user wants Wisp to *do* something on the PC.
const ACTION_WORDS: &[&str] = &[
    "open",
    "launch",
    "start",
    "close",
    "remind",
    "reminder",
    "timer",
    "alarm",
    "volume",
    "mute",
    "unmute",
    "brightness",
    "play",
    "pause",
    "skip",
    "copy",
    "paste",
    "type",
    "remember",
    "forget",
    "note",
    "schedule",
    "set",
    "turn",
    "switch",
    "lock",
    "sleep",
    "shutdown",
    "restart",
    "wallpaper",
    "focus",
    "save",
    "create",
    "delete",
    "move",
];

/// Prompts that are obviously light work on supplied text.
const SIMPLE_WORDS: &[&str] = &[
    "summarise",
    "summarize",
    "rewrite",
    "rephrase",
    "shorten",
    "translate",
    "explain",
    "define",
    "proofread",
    "fix grammar",
    "spell",
    "tl;dr",
    "tldr",
    "simplify",
];

/// Longest prompt that still counts as a short, simple question.
const SIMPLE_MAX_CHARS: usize = 280;

pub fn classify(req: &AskRequest) -> Task {
    if req
        .attachments
        .iter()
        .any(|a| a.mime_type.starts_with("audio/"))
    {
        return Task::Audio;
    }
    if req
        .attachments
        .iter()
        .any(|a| a.mime_type.starts_with("image/"))
        || req.context_kind == ContextKind::Screen
    {
        return Task::Vision;
    }
    if req.web {
        return Task::Web;
    }
    let lower = req.prompt.to_lowercase();
    let words: Vec<&str> = lower
        .split(|c: char| !c.is_alphanumeric() && c != '\'')
        .filter(|w| !w.is_empty())
        .collect();
    let actionable = req.enable_tools && words.iter().any(|w| ACTION_WORDS.contains(w));
    if actionable || req.think {
        return Task::Actions;
    }
    if req.context_kind == ContextKind::Clipboard
        || SIMPLE_WORDS.iter().any(|w| lower.contains(w))
        || lower.chars().count() <= SIMPLE_MAX_CHARS
    {
        return Task::Simple;
    }
    Task::Chat
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Family {
    Pro,
    Flash,
    FlashLite,
    Gemma,
    Other,
}

fn family(id: &str) -> Family {
    let m = id.to_ascii_lowercase();
    if m.starts_with("gemma") {
        Family::Gemma
    } else if m.contains("flash-lite") || m.contains("flash_lite") {
        Family::FlashLite
    } else if m.contains("flash") {
        Family::Flash
    } else if m.contains("pro") {
        Family::Pro
    } else {
        Family::Other
    }
}

/// Models that aren't general chat models (or are unstable) never get picked
/// automatically; the user may still choose them by hand.
fn auto_pickable(id: &str) -> bool {
    let m = id.to_ascii_lowercase();
    const SKIP: &[&str] = &[
        "preview",
        "exp",
        "live",
        "audio",
        "tts",
        "image",
        "thinking",
        "computer",
        "robotics",
        "embedding",
        "aqa",
        "nano",
        "learnlm",
    ];
    !SKIP.iter().any(|s| m.contains(s))
}

/// (version, size) for ordering: newer first, then bigger.
fn rank(id: &str) -> (u32, u32) {
    let mut version = 0;
    let mut size = 0;
    for part in id.split(['-', '_']) {
        if version == 0 {
            if let Ok(v) = part.parse::<f32>() {
                version = (v * 100.0) as u32;
                continue;
            }
        }
        if let Some(n) = part.strip_suffix('b') {
            if let Ok(v) = n.parse::<f32>() {
                size = (v * 10.0) as u32;
            }
        }
    }
    (version, size)
}

fn best_of(models: &[String], fam: Family) -> Option<String> {
    let mut picks: Vec<&String> = models
        .iter()
        .filter(|m| family(m) == fam && auto_pickable(m))
        .collect();
    // Gemma's tiny sizes (1B, the "e" edge models) don't follow instructions well.
    if fam == Family::Gemma {
        picks.retain(|m| rank(m).1 >= 120 || rank(m).1 == 0);
    }
    picks.sort_by_key(|m| std::cmp::Reverse(rank(m)));
    picks.first().map(|m| m.to_string())
}

/// Used before the model list has been fetched.
const FALLBACK_FLASH_LITE: &str = "gemini-2.5-flash-lite";
const FALLBACK_GEMMA: &str = "gemma-3-27b-it";

/// What the user chose that shapes routing.
#[derive(Debug, Clone, Default)]
pub struct Prefs {
    pub selected: String,
    pub auto_fallback: bool,
    /// OpenRouter model to use, when a key is saved ("" = not available).
    pub openrouter_model: String,
    pub simple_via_openrouter: bool,
}

/// The Gemini-key chain: the chosen model, then the best Flash and Flash-Lite,
/// then the best Gemma.
fn gemini_chain(prefs: &Prefs, models: &[String]) -> Vec<Route> {
    let mut ids = vec![prefs.selected.clone()];
    if models.is_empty() {
        ids.push(FALLBACK_FLASH_LITE.into());
        ids.push(FALLBACK_GEMMA.into());
    } else {
        for fam in [Family::Flash, Family::FlashLite, Family::Gemma] {
            if let Some(m) = best_of(models, fam) {
                ids.push(m);
            }
        }
    }
    let mut seen = Vec::new();
    ids.into_iter()
        .filter(|m| !m.is_empty())
        .filter(|m| {
            let new = !seen.contains(m);
            seen.push(m.clone());
            new
        })
        .map(Route::gemini)
        .collect()
}

/// Every model that may answer `task`, best first, skipping any whose quota
/// is used up. With automatic fallback off, only the first.
pub fn candidates(
    task: Task,
    prefs: &Prefs,
    gemini_models: &[String],
    book: &QuotaBook,
    now: i64,
) -> Vec<Route> {
    let chain = gemini_chain(prefs, gemini_models);
    let openrouter =
        (!prefs.openrouter_model.is_empty()).then(|| Route::openrouter(&prefs.openrouter_model));
    let mut out: Vec<Route> = match task {
        Task::Audio => chain.into_iter().filter(|r| r.caps.audio).collect(),
        Task::Vision => chain.into_iter().filter(|r| r.caps.images).collect(),
        Task::Web => {
            // Search-capable first; then answer without search rather than not at all.
            let (search, rest): (Vec<_>, Vec<_>) = chain.into_iter().partition(|r| r.caps.search);
            search.into_iter().chain(rest).chain(openrouter).collect()
        }
        Task::Simple => {
            let mut v = Vec::new();
            if prefs.simple_via_openrouter {
                v.extend(openrouter.clone());
            }
            // Spend the cheapest Gemini quota on simple jobs.
            let (light, heavy): (Vec<_>, Vec<_>) = chain
                .into_iter()
                .partition(|r| matches!(family(&r.model), Family::FlashLite | Family::Gemma));
            v.extend(light);
            v.extend(heavy);
            if !prefs.simple_via_openrouter {
                v.extend(openrouter);
            }
            v
        }
        Task::Chat | Task::Actions => chain.into_iter().chain(openrouter).collect(),
    };
    if !prefs.auto_fallback {
        out.truncate(1);
    }
    out.retain(|r| book.available(&r.key(), now));
    out
}

/// Errors that mean "try the next model" rather than "tell the user".
pub fn should_fall_back(e: &WispError) -> bool {
    matches!(
        e.code,
        ErrorCode::QuotaExhausted
            | ErrorCode::RateLimited
            | ErrorCode::ModelUnavailable
            | ErrorCode::ServerError
    )
}

/// When each model can be used again (unix seconds), after hitting a limit.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct QuotaBook {
    pub until: HashMap<String, i64>,
}

/// How long a per-minute rate limit benches a model when the provider
/// doesn't say.
const RATE_LIMIT_COOLDOWN_SECS: i64 = 60;
/// A model the key can't use is skipped for this long.
const UNAVAILABLE_SECS: i64 = 6 * 3600;

impl QuotaBook {
    pub fn available(&self, key: &str, now: i64) -> bool {
        self.until.get(key).is_none_or(|t| *t <= now)
    }

    /// Record a failure; returns whether the book changed.
    pub fn mark(&mut self, route: &Route, err: &WispError, now: i64) -> bool {
        let until = match err.code {
            ErrorCode::QuotaExhausted => match route.provider {
                Provider::Gemini => next_pacific_midnight(now),
                // OpenRouter's free allowance resets at midnight UTC.
                Provider::OpenRouter => next_utc_midnight(now),
            },
            ErrorCode::RateLimited => {
                now + err
                    .retry_after_secs
                    .map(|s| (s as i64).clamp(5, 3600))
                    .unwrap_or(RATE_LIMIT_COOLDOWN_SECS)
            }
            ErrorCode::ModelUnavailable => now + UNAVAILABLE_SECS,
            _ => return false,
        };
        self.until.insert(route.key(), until);
        true
    }

    /// Drop entries that have expired.
    pub fn prune(&mut self, now: i64) {
        self.until.retain(|_, t| *t > now);
    }

    /// Models that are used up right now, with when they come back.
    pub fn exhausted(&self, now: i64) -> Vec<(String, i64)> {
        let mut v: Vec<(String, i64)> = self
            .until
            .iter()
            .filter(|(_, t)| **t > now)
            .map(|(k, t)| (k.clone(), *t))
            .collect();
        v.sort();
        v
    }
}

fn second_sunday_march(year: i32) -> NaiveDate {
    let mut d = NaiveDate::from_ymd_opt(year, 3, 1).expect("date");
    let mut sundays = 0;
    loop {
        if d.weekday() == Weekday::Sun {
            sundays += 1;
            if sundays == 2 {
                return d;
            }
        }
        d = d.succ_opt().expect("date");
    }
}

fn first_sunday_november(year: i32) -> NaiveDate {
    let mut d = NaiveDate::from_ymd_opt(year, 11, 1).expect("date");
    while d.weekday() != Weekday::Sun {
        d = d.succ_opt().expect("date");
    }
    d
}

/// UTC offset (hours) of US Pacific time at `utc` (PDT −7 or PST −8).
fn pacific_offset_hours(utc: DateTime<Utc>) -> i64 {
    let year = utc.year();
    // DST starts 2:00 PST (10:00 UTC) and ends 2:00 PDT (09:00 UTC).
    let start = Utc.from_utc_datetime(&second_sunday_march(year).and_hms_opt(10, 0, 0).expect("t"));
    let end = Utc.from_utc_datetime(&first_sunday_november(year).and_hms_opt(9, 0, 0).expect("t"));
    if utc >= start && utc < end {
        -7
    } else {
        -8
    }
}

/// Gemini's daily quotas reset at midnight US Pacific time.
pub fn next_pacific_midnight(now: i64) -> i64 {
    let utc = Utc.timestamp_opt(now, 0).single().unwrap_or_else(Utc::now);
    let local = utc + ChronoDuration::hours(pacific_offset_hours(utc));
    let tomorrow = local.date_naive().succ_opt().expect("date");
    let midnight_local = tomorrow.and_hms_opt(0, 0, 0).expect("t");
    // Convert back with the offset in force at that midnight.
    let guess = Utc.from_utc_datetime(&midnight_local) - ChronoDuration::hours(-8);
    let offset = pacific_offset_hours(guess);
    (Utc.from_utc_datetime(&midnight_local) - ChronoDuration::hours(offset)).timestamp()
}

pub fn next_utc_midnight(now: i64) -> i64 {
    (now.div_euclid(86_400) + 1) * 86_400
}

/// Make a request fit a model that lacks some features: fold the system
/// prompt into the first user turn, and drop tools, thinking and JSON mode.
pub fn adapt_for(mut req: GenerateContentRequest, caps: Caps) -> GenerateContentRequest {
    if !caps.system_prompt {
        if let Some(sys) = req.system_instruction.take() {
            let text: String = sys
                .parts
                .iter()
                .filter_map(|p| p.text.as_deref())
                .collect::<Vec<_>>()
                .join("\n");
            if !text.trim().is_empty() {
                let preface = Part::text(format!("{text}\n\n---\n\n"));
                match req.contents.iter_mut().find(|c| c.role != "model") {
                    Some(first) => first.parts.insert(0, preface),
                    None => req.contents.insert(
                        0,
                        crate::gemini::types::Content {
                            role: "user".into(),
                            parts: vec![preface],
                        },
                    ),
                }
            }
        }
    }
    if let Some(tools) = req.tools.take() {
        let kept: Vec<serde_json::Value> = tools
            .as_array()
            .into_iter()
            .flatten()
            .filter(|t| {
                (caps.tools && t.get("functionDeclarations").is_some())
                    || (caps.search && t.get("google_search").is_some())
            })
            .cloned()
            .collect();
        req.tools = (!kept.is_empty()).then_some(serde_json::Value::Array(kept));
    }
    if let Some(cfg) = req
        .generation_config
        .as_mut()
        .and_then(|c| c.as_object_mut())
    {
        if !caps.system_prompt {
            // Gemma (the only model without a system prompt) has no thinking settings.
            cfg.remove("thinkingConfig");
        }
        if !caps.json_mode {
            cfg.remove("responseMimeType");
            cfg.remove("responseSchema");
        }
    }
    req
}

/// One model that was tried and why it was passed over.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Skipped {
    pub model: String,
    pub code: ErrorCode,
}

/// Which model answered, and what was skipped on the way.
#[derive(Debug, Clone)]
pub struct Answered<T> {
    pub value: T,
    pub route: Route,
    pub skipped: Vec<Skipped>,
}

/// Run `attempt` on each route in turn until one succeeds. A route that
/// fails with a quota/rate/availability error is recorded in `book` and
/// the next one is tried, unless it had already started answering (its
/// `started` flag is set), so text is never repeated.
pub async fn run_with_fallback<T, F, Fut>(
    routes: Vec<Route>,
    book: &std::sync::Mutex<QuotaBook>,
    now: impl Fn() -> i64,
    mut attempt: F,
) -> std::result::Result<Answered<T>, (WispError, Vec<Skipped>)>
where
    F: FnMut(Route) -> Fut,
    Fut: Future<Output = (Result<T>, bool)>,
{
    let mut skipped = Vec::new();
    let mut last: Option<WispError> = None;
    for route in routes {
        let (res, started) = attempt(route.clone()).await;
        match res {
            Ok(value) => {
                return Ok(Answered {
                    value,
                    route,
                    skipped,
                })
            }
            Err(e) => {
                book.lock().expect("quota").mark(&route, &e, now());
                if started || !should_fall_back(&e) {
                    return Err((e, skipped));
                }
                skipped.push(Skipped {
                    model: route.label(),
                    code: e.code,
                });
                last = Some(e);
            }
        }
    }
    Err((
        match last {
            Some(e) if e.code == ErrorCode::QuotaExhausted || e.code == ErrorCode::RateLimited => {
                all_used_up(&e)
            }
            Some(e) => e,
            None => WispError::new(
                ErrorCode::QuotaExhausted,
                "Every free model is resting after hitting its limit. Try again in a minute, \
or add an OpenRouter key in Settings → Model for more free requests.",
            ),
        },
        skipped,
    ))
}

fn all_used_up(last: &WispError) -> WispError {
    WispError::new(
        ErrorCode::QuotaExhausted,
        "All of today's free models are used up. Gemini's limits reset at midnight Pacific time; \
add an OpenRouter key in Settings → Model for more free requests.",
    )
    .with_retry_after(last.retry_after_secs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gemini::types::{Attachment, Content};
    use serde_json::json;

    fn ask(prompt: &str) -> AskRequest {
        AskRequest {
            request_id: "r".into(),
            prompt: prompt.into(),
            history: vec![],
            attachments: vec![],
            context_kind: ContextKind::Text,
            use_memory: true,
            enable_tools: true,
            think: false,
            web: false,
            outside_content: false,
        }
    }

    fn models() -> Vec<String> {
        [
            "gemini-2.0-flash",
            "gemini-2.5-flash",
            "gemini-2.5-flash-lite",
            "gemini-2.5-flash-preview-09-2025",
            "gemini-2.5-pro",
            "gemma-3-1b-it",
            "gemma-3-12b-it",
            "gemma-3-27b-it",
            "gemma-3n-e4b-it",
        ]
        .map(String::from)
        .to_vec()
    }

    fn prefs(openrouter: bool) -> Prefs {
        Prefs {
            selected: "gemini-2.5-flash".into(),
            auto_fallback: true,
            openrouter_model: if openrouter {
                "meta-llama/llama-3.3-70b-instruct:free".into()
            } else {
                String::new()
            },
            simple_via_openrouter: true,
        }
    }

    fn ids(routes: &[Route]) -> Vec<String> {
        routes.iter().map(|r| r.key()).collect()
    }

    #[test]
    fn classifies_each_kind_of_request() {
        let mut r = ask("hello");
        r.attachments.push(Attachment {
            mime_type: "audio/wav".into(),
            data_base64: "AA".into(),
        });
        assert_eq!(classify(&r), Task::Audio);
        let mut r = ask("what is this");
        r.attachments.push(Attachment {
            mime_type: "image/png".into(),
            data_base64: "AA".into(),
        });
        assert_eq!(classify(&r), Task::Vision);
        assert_eq!(
            classify(&AskRequest {
                web: true,
                ..ask("news today")
            }),
            Task::Web
        );
        assert_eq!(classify(&ask("open spotify and play jazz")), Task::Actions);
        assert_eq!(classify(&ask("remind me to stretch at 5")), Task::Actions);
        assert_eq!(
            classify(&AskRequest {
                think: true,
                ..ask("why is the sky blue")
            }),
            Task::Actions
        );
        assert_eq!(classify(&ask("why is the sky blue?")), Task::Simple);
        assert_eq!(
            classify(&AskRequest {
                context_kind: ContextKind::Clipboard,
                ..ask(&"long text ".repeat(100))
            }),
            Task::Simple
        );
        assert_eq!(
            classify(&ask(&"tell me a long story about a lighthouse ".repeat(10))),
            Task::Chat
        );
        // Without tools, action words don't matter.
        assert_eq!(
            classify(&AskRequest {
                enable_tools: false,
                ..ask("open the pod bay doors")
            }),
            Task::Simple
        );
    }

    #[test]
    fn gemini_and_gemma_come_first_and_tiny_or_preview_models_are_skipped() {
        let book = QuotaBook::default();
        let r = candidates(Task::Chat, &prefs(false), &models(), &book, 0);
        assert_eq!(
            ids(&r),
            [
                "gemini:gemini-2.5-flash",
                "gemini:gemini-2.5-flash-lite",
                "gemini:gemma-3-27b-it"
            ]
        );
        let r = candidates(Task::Actions, &prefs(true), &models(), &book, 0);
        assert_eq!(
            r.last().unwrap().key(),
            "openrouter:meta-llama/llama-3.3-70b-instruct:free"
        );
        assert!(!r[2].caps.tools && r[0].caps.tools);
    }

    #[test]
    fn simple_jobs_go_to_openrouter_then_the_lightest_gemini() {
        let book = QuotaBook::default();
        let r = candidates(Task::Simple, &prefs(true), &models(), &book, 0);
        assert_eq!(
            ids(&r),
            [
                "openrouter:meta-llama/llama-3.3-70b-instruct:free",
                "gemini:gemini-2.5-flash-lite",
                "gemini:gemma-3-27b-it",
                "gemini:gemini-2.5-flash"
            ]
        );
        let mut p = prefs(true);
        p.simple_via_openrouter = false;
        let r = candidates(Task::Simple, &p, &models(), &book, 0);
        assert_eq!(r[0].key(), "gemini:gemini-2.5-flash-lite");
        assert_eq!(r.last().unwrap().provider, Provider::OpenRouter);
    }

    #[test]
    fn screenshots_and_audio_never_reach_openrouter() {
        let book = QuotaBook::default();
        let v = candidates(Task::Vision, &prefs(true), &models(), &book, 0);
        assert!(v.iter().all(|r| r.provider == Provider::Gemini));
        assert_eq!(v.last().unwrap().model, "gemma-3-27b-it");
        let a = candidates(Task::Audio, &prefs(true), &models(), &book, 0);
        assert_eq!(
            ids(&a),
            ["gemini:gemini-2.5-flash", "gemini:gemini-2.5-flash-lite"]
        );
        // Web: search-capable first, then without search.
        let w = candidates(Task::Web, &prefs(true), &models(), &book, 0);
        assert!(w[0].caps.search && w[1].caps.search && !w[2].caps.search);
    }

    #[test]
    fn used_up_models_are_skipped_until_their_reset() {
        let mut book = QuotaBook::default();
        let now = 1_790_000_000; // 2026-09-21
        let flash = Route::gemini("gemini-2.5-flash");
        let daily = WispError::new(ErrorCode::QuotaExhausted, "q");
        assert!(book.mark(&flash, &daily, now));
        let r = candidates(Task::Chat, &prefs(false), &models(), &book, now);
        assert_eq!(r[0].model, "gemini-2.5-flash-lite");
        let reset = book.until[&flash.key()];
        assert!(reset > now && reset - now <= 86_400);
        assert_eq!(
            candidates(Task::Chat, &prefs(false), &models(), &book, reset)[0].model,
            "gemini-2.5-flash"
        );
        // A per-minute limit benches the model briefly.
        let lite = Route::gemini("gemini-2.5-flash-lite");
        book.mark(
            &lite,
            &WispError::new(ErrorCode::RateLimited, "r").with_retry_after(Some(30)),
            now,
        );
        assert!(!book.available(&lite.key(), now + 29));
        assert!(book.available(&lite.key(), now + 30));
        // Other errors don't change anything.
        assert!(!book.mark(&lite, &WispError::invalid("x"), now));
        // Fallback off: only the chosen model, even if it's used up.
        let mut p = prefs(false);
        p.auto_fallback = false;
        assert!(candidates(Task::Chat, &p, &models(), &book, now).is_empty());
    }

    #[test]
    fn falls_back_to_defaults_before_the_model_list_arrives() {
        let r = candidates(Task::Chat, &prefs(false), &[], &QuotaBook::default(), 0);
        assert_eq!(
            ids(&r),
            [
                "gemini:gemini-2.5-flash",
                "gemini:gemini-2.5-flash-lite",
                "gemini:gemma-3-27b-it"
            ]
        );
    }

    #[test]
    fn pacific_midnight_follows_daylight_saving() {
        // 2026-07-01 12:00 UTC (PDT): next reset is 2026-07-02 07:00 UTC.
        let summer = Utc
            .with_ymd_and_hms(2026, 7, 1, 12, 0, 0)
            .unwrap()
            .timestamp();
        assert_eq!(
            next_pacific_midnight(summer),
            Utc.with_ymd_and_hms(2026, 7, 2, 7, 0, 0)
                .unwrap()
                .timestamp()
        );
        // 2026-12-01 03:00 UTC is still Nov 30 in California (PST).
        let winter = Utc
            .with_ymd_and_hms(2026, 12, 1, 3, 0, 0)
            .unwrap()
            .timestamp();
        assert_eq!(
            next_pacific_midnight(winter),
            Utc.with_ymd_and_hms(2026, 12, 1, 8, 0, 0)
                .unwrap()
                .timestamp()
        );
        assert_eq!(next_utc_midnight(86_400 * 3 + 5), 86_400 * 4);
    }

    #[test]
    fn gemma_requests_lose_what_gemma_cant_take() {
        let req = GenerateContentRequest {
            contents: vec![Content {
                role: "user".into(),
                parts: vec![Part::text("hi")],
            }],
            system_instruction: Some(Content {
                role: String::new(),
                parts: vec![Part::text("Be brief.")],
            }),
            tools: Some(json!([{ "functionDeclarations": [] }])),
            generation_config: Some(json!({
                "temperature": 0.7,
                "thinkingConfig": { "thinkingBudget": 0 },
                "responseMimeType": "application/json"
            })),
        };
        let g = adapt_for(req.clone(), GEMMA_CAPS);
        assert!(g.system_instruction.is_none());
        assert!(g.tools.is_none());
        let cfg = g.generation_config.unwrap();
        assert!(cfg.get("thinkingConfig").is_none() && cfg.get("responseMimeType").is_none());
        assert_eq!(g.contents[0].parts.len(), 2);
        assert!(g.contents[0].parts[0]
            .text
            .as_deref()
            .unwrap()
            .starts_with("Be brief."));
        // Gemini keeps everything.
        let full = adapt_for(req, GEMINI_CAPS);
        assert!(full.system_instruction.is_some() && full.tools.is_some());
    }

    #[test]
    fn labels_read_well() {
        assert_eq!(Route::gemini("gemma-3-27b-it").label(), "Gemma 3 27B");
        assert_eq!(
            Route::gemini("gemini-2.5-flash-lite").label(),
            "Gemini 2.5 Flash Lite"
        );
        assert_eq!(
            Route::openrouter("meta-llama/llama-3.3-70b-instruct:free").label(),
            "OpenRouter · Llama 3.3 70B"
        );
    }

    #[tokio::test]
    async fn fallback_moves_on_only_before_text_started() {
        let book = std::sync::Mutex::new(QuotaBook::default());
        let routes = vec![
            Route::gemini("gemini-2.5-flash"),
            Route::gemini("gemini-2.5-flash-lite"),
        ];
        let ok = run_with_fallback(
            routes.clone(),
            &book,
            || 100,
            |r| async move {
                if r.model == "gemini-2.5-flash" {
                    (Err(WispError::new(ErrorCode::QuotaExhausted, "q")), false)
                } else {
                    (Ok("answer"), true)
                }
            },
        )
        .await
        .unwrap();
        assert_eq!(ok.value, "answer");
        assert_eq!(ok.route.model, "gemini-2.5-flash-lite");
        assert_eq!(ok.skipped.len(), 1);
        assert!(!book
            .lock()
            .unwrap()
            .available("gemini:gemini-2.5-flash", 100));

        // Started streaming, then failed: no second answer.
        let (err, _) = run_with_fallback(
            routes.clone(),
            &book,
            || 100,
            |_| async {
                (
                    Err::<&str, _>(WispError::new(ErrorCode::ServerError, "s")),
                    true,
                )
            },
        )
        .await
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::ServerError);

        // A cancel or bad request is never retried elsewhere.
        let (err, skipped) = run_with_fallback(
            routes.clone(),
            &book,
            || 100,
            |_| async {
                (
                    Err::<&str, _>(WispError::new(ErrorCode::Cancelled, "c")),
                    false,
                )
            },
        )
        .await
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::Cancelled);
        assert!(skipped.is_empty());

        // Everything used up: one clear message.
        let (err, skipped) = run_with_fallback(
            routes,
            &book,
            || 100,
            |_| async {
                (
                    Err::<&str, _>(WispError::new(ErrorCode::QuotaExhausted, "q")),
                    false,
                )
            },
        )
        .await
        .unwrap_err();
        assert_eq!(err.code, ErrorCode::QuotaExhausted);
        assert!(err.message.contains("OpenRouter"));
        assert_eq!(skipped.len(), 2);
    }
}
