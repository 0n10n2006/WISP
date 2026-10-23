//! Background agents. Each job runs only when the user starts it, shows its
//! progress in the Agents tab (and as a dot on the notch), can be cancelled,
//! and writes its result to Documents\Wisp. Nothing is executed: outputs are
//! plain HTML/Markdown files the user opens themselves.

use crate::state::AppState;
use serde::Deserialize;
use std::path::PathBuf;
use tauri::{AppHandle, Emitter, Manager};
use tokio_util::sync::CancellationToken;
use wisp_core::gemini::prompt;
use wisp_core::storage::AgentJob;
use wisp_core::{ErrorCode, Result, WispError};

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AgentKind {
    /// Recreate what's on screen as a standalone HTML page.
    Replica,
    /// Web-grounded research brief.
    Research { topic: String },
}

fn changed(app: &AppHandle) {
    let _ = app.emit("wisp://data-changed", "jobs");
}

/// `Documents\Wisp`, created on demand.
pub fn output_dir(app: &AppHandle) -> Result<PathBuf> {
    let base = app
        .path()
        .document_dir()
        .or_else(|_| app.path().home_dir())
        .map_err(|e| WispError::internal(e.to_string()))?;
    let dir = base.join("Wisp");
    std::fs::create_dir_all(&dir)
        .map_err(|e| WispError::internal(format!("Couldn't create {}: {e}", dir.display())))?;
    Ok(dir)
}

/// Safe file name from a title: letters, digits, spaces and dashes only.
pub fn file_stem(title: &str, stamp: &str) -> String {
    let cleaned: String = title
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' || c == '-' {
                c
            } else {
                ' '
            }
        })
        .collect();
    let words: Vec<&str> = cleaned.split_whitespace().take(8).collect();
    let base = if words.is_empty() {
        "wisp".to_string()
    } else {
        words.join("-")
    };
    format!(
        "{}-{stamp}",
        base.to_lowercase().chars().take(60).collect::<String>()
    )
}

fn progress(app: &AppHandle, id: &str, p: f32, detail: &str) {
    let _ = app
        .state::<AppState>()
        .db
        .update_job(id, "running", p, Some(detail), None);
    changed(app);
}

pub async fn start(app: AppHandle, kind: AgentKind) -> Result<AgentJob> {
    let state = app.state::<AppState>();
    let key = state.secrets.require()?;
    let title = match &kind {
        AgentKind::Replica => "Recreate this screen as HTML".to_string(),
        AgentKind::Research { topic } => {
            let t = topic.trim();
            if t.chars().count() < 3 || t.chars().count() > 500 {
                return Err(WispError::invalid("Describe what to research"));
            }
            format!("Research: {t}")
        }
    };
    let running = state
        .db
        .list_jobs(50)?
        .iter()
        .filter(|j| j.status == "running")
        .count();
    if running >= 3 {
        return Err(WispError::invalid(
            "Three agents are already running. Wait for one to finish.",
        ));
    }
    // Replica needs the screenshot now, before anything else changes.
    let shot = match kind {
        AgentKind::Replica => Some(crate::capture::screenshot_here(&app).await?),
        _ => None,
    };
    let job = state.db.create_job(
        match kind {
            AgentKind::Replica => "replica",
            AgentKind::Research { .. } => "research",
        },
        &title,
    )?;
    let token = CancellationToken::new();
    state
        .requests
        .lock()
        .expect("requests")
        .insert(job.id.clone(), token.clone());
    changed(&app);
    let id = job.id.clone();
    let a = app.clone();
    tauri::async_runtime::spawn(async move {
        let result = run(&a, &id, &key, kind, shot, &token).await;
        let state = a.state::<AppState>();
        state.requests.lock().expect("requests").remove(&id);
        let _ = match result {
            Ok(path) => {
                let _ = state
                    .db
                    .update_job(&id, "done", 1.0, Some("Saved"), Some(&path));
                notify_done(&a, &path);
                Ok(())
            }
            Err(e) if e.code == ErrorCode::Cancelled || token.is_cancelled() => state
                .db
                .update_job(&id, "cancelled", 0.0, Some("Cancelled"), None)
                .map(|_| ()),
            Err(e) => state
                .db
                .update_job(&id, "failed", 0.0, Some(&e.message), None)
                .map(|_| ()),
        };
        changed(&a);
    });
    Ok(job)
}

fn notify_done(app: &AppHandle, path: &str) {
    use tauri_plugin_notification::NotificationExt;
    if !app.state::<AppState>().settings().notifications_enabled {
        return;
    }
    let name = std::path::Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let _ = app
        .notification()
        .builder()
        .title("Wisp agent finished")
        .body(format!("Saved {name} to Documents\\Wisp"))
        .show();
}

async fn run(
    app: &AppHandle,
    id: &str,
    key: &str,
    kind: AgentKind,
    shot: Option<crate::capture::Shot>,
    cancel: &CancellationToken,
) -> Result<String> {
    let state = app.state::<AppState>();
    let model = state.settings().selected_model;
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M").to_string();
    let dir = output_dir(app)?;
    match kind {
        AgentKind::Replica => {
            let shot = shot.ok_or_else(|| WispError::internal("missing screenshot"))?;
            progress(app, id, 0.2, "Studying the layout…");
            let req = prompt::replica_request(&shot.png_b64, &model);
            let out = state.gemini.generate(key, &model, &req, cancel).await?;
            progress(app, id, 0.85, "Writing the page…");
            let html = prompt::extract_code_block(&out.text, "html");
            if !html.to_ascii_lowercase().contains("<html") && !html.contains("<body") {
                return Err(WispError::new(
                    ErrorCode::MalformedResponse,
                    "The model didn't return a web page. Try again.",
                ));
            }
            let html = wisp_core::sanitize::sanitize_html(&html);
            let path = dir.join(format!("{}.html", file_stem("screen replica", &stamp)));
            std::fs::write(&path, html).map_err(|e| WispError::internal(e.to_string()))?;
            Ok(path.display().to_string())
        }
        AgentKind::Research { topic } => {
            progress(app, id, 0.15, "Searching the web…");
            let req = prompt::research_request(&topic, &model);
            let out = state.gemini.generate(key, &model, &req, cancel).await?;
            progress(app, id, 0.9, "Writing the brief…");
            let md = prompt::research_markdown(
                &topic,
                &wisp_core::sanitize::sanitize_markdown(&out.text),
                &out.sources,
                &chrono::Local::now().format("%-d %B %Y").to_string(),
            );
            let path = dir.join(format!("{}.md", file_stem(&topic, &stamp)));
            std::fs::write(&path, md).map_err(|e| WispError::internal(e.to_string()))?;
            Ok(path.display().to_string())
        }
    }
}

pub fn cancel(app: &AppHandle, id: &str) {
    if let Some(t) = app
        .state::<AppState>()
        .requests
        .lock()
        .expect("requests")
        .get(id)
    {
        t.cancel();
    }
}

/// `path` if it is an `.html`/`.md` file really inside Documents\\Wisp
/// (links and `..` resolved), i.e. something an agent wrote.
pub fn owned_output(app: &AppHandle, path: &str) -> Option<std::path::PathBuf> {
    let dir = std::fs::canonicalize(output_dir(app).ok()?).ok()?;
    let real = std::fs::canonicalize(path).ok()?;
    let ext_ok = real
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("html") || e.eq_ignore_ascii_case("md"));
    (real.starts_with(&dir) && ext_ok && real.is_file()).then_some(real)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_file_names() {
        assert_eq!(
            file_stem(
                "Research: ../../Windows\\System32 <script>",
                "20261021-0900"
            ),
            "research-windows-system32-script-20261021-0900"
        );
        assert_eq!(file_stem("???", "x"), "wisp-x");
        let k: AgentKind = serde_json::from_str(r#"{"kind":"research","topic":"AI"}"#).unwrap();
        assert!(matches!(k, AgentKind::Research { .. }));
        assert!(serde_json::from_str::<AgentKind>(r#"{"kind":"shell"}"#).is_err());
    }
}
