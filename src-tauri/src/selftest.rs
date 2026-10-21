//! `wisp.exe --self-test`: start normally, verify the core subsystems and that
//! the UI booted, write a JSON report, and exit 0 (pass) or 1 (fail). Used by
//! Windows CI to prove the packaged app actually launches.

use crate::state::AppState;
use crate::windows;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

pub static UI_READY: AtomicBool = AtomicBool::new(false);
static START: OnceLock<Instant> = OnceLock::new();
static MARKS: Mutex<Vec<(String, u128)>> = Mutex::new(Vec::new());

/// Record the process start (call first thing in `run`).
pub fn start() -> Instant {
    *START.get_or_init(Instant::now)
}

/// Record how long after process start a startup phase finished.
pub fn mark(phase: &str) {
    let ms = START
        .get()
        .map(|s| s.elapsed().as_millis())
        .unwrap_or_default();
    if let Ok(mut m) = MARKS.lock() {
        m.push((phase.to_string(), ms));
    }
}

/// Setup must finish within this budget (PRD §13: start in under 3 s).
const STARTUP_BUDGET_MS: u128 = 3_000;
const UI_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Check {
    name: &'static str,
    ok: bool,
    detail: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Report {
    version: String,
    passed: bool,
    setup_ms: u128,
    ui_ready_ms: Option<u128>,
    /// Startup phases (name, ms since process start).
    phases: Vec<(String, u128)>,
    checks: Vec<Check>,
}

pub fn requested() -> bool {
    std::env::args().any(|a| a == "--self-test")
}

pub fn spawn(app: AppHandle, process_start: Instant, setup_ms: u128) {
    tauri::async_runtime::spawn(async move {
        let waited = Instant::now();
        while !UI_READY.load(Ordering::SeqCst) && waited.elapsed() < UI_TIMEOUT {
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
        let ui_ready = UI_READY.load(Ordering::SeqCst);
        let ui_ready_ms = ui_ready.then(|| process_start.elapsed().as_millis());

        let state = app.state::<AppState>();
        let mut checks = vec![
            Check {
                name: "setup-time",
                ok: setup_ms <= STARTUP_BUDGET_MS,
                detail: format!("{setup_ms} ms (budget {STARTUP_BUDGET_MS} ms)"),
            },
            Check {
                name: "ui-ready",
                ok: ui_ready,
                detail: format!("{ui_ready_ms:?} ms after process start"),
            },
            Check {
                name: "island-window",
                ok: windows::island(&app).is_some(),
                detail: String::new(),
            },
        ];
        match state.db.stats() {
            Ok(s) => checks.push(Check {
                name: "database",
                ok: true,
                detail: format!("schema v{}", s.schema_version),
            }),
            Err(e) => checks.push(Check {
                name: "database",
                ok: false,
                detail: e.message,
            }),
        }
        match state.secrets.get() {
            Ok(k) => checks.push(Check {
                name: "credential-vault",
                ok: true,
                detail: if k.is_some() {
                    "key present".into()
                } else {
                    "no key".into()
                },
            }),
            Err(e) => checks.push(Check {
                name: "credential-vault",
                ok: false,
                detail: e.message,
            }),
        }
        let issues = state.shortcut_issues.lock().expect("issues").len();
        // Informational: CI desktops may already own some hotkeys.
        checks.push(Check {
            name: "global-shortcuts",
            ok: true,
            detail: format!("{issues} could not be registered"),
        });
        // v0.2 notch and dashboard.
        let layout = *state.island.lock().expect("island");
        checks.push(Check {
            name: "notch-mode",
            ok: layout.mode == windows::NotchMode::Sliver,
            detail: format!("{:?}", layout.mode),
        });
        let snap = tauri::async_runtime::spawn_blocking({
            let app = app.clone();
            move || app.state::<AppState>().stats.snapshot()
        })
        .await;
        checks.push(match snap {
            Ok(s) => Check {
                name: "system-stats",
                ok: s.memory_total > 0 && !s.cores.is_empty(),
                detail: format!(
                    "{} cores, {} MB RAM",
                    s.cores.len(),
                    s.memory_total / 1_048_576
                ),
            },
            Err(e) => Check {
                name: "system-stats",
                ok: false,
                detail: e.to_string(),
            },
        });
        let catalogue = tauri::async_runtime::spawn_blocking(crate::apps::catalogue)
            .await
            .unwrap_or_default();
        checks.push(Check {
            name: "launcher",
            ok: catalogue.len() >= wisp_core::launcher::BUILTIN.len(),
            detail: format!("{} items", catalogue.len()),
        });
        // Informational: whether the hold-to-talk hook installed (Windows only).
        let hook = MARKS
            .lock()
            .map(|m| m.iter().any(|(p, _)| p == "hold-hook"))
            .unwrap_or(false);
        checks.push(Check {
            name: "hold-to-talk-hook",
            ok: true,
            detail: if cfg!(windows) {
                format!("installed: {hook}")
            } else {
                "not applicable".into()
            },
        });
        let warnings = state.startup_warnings.lock().expect("warnings").clone();
        checks.push(Check {
            name: "startup-warnings",
            ok: warnings.iter().all(|w| !w.contains("database")),
            detail: warnings.join(" | "),
        });

        let passed = checks.iter().all(|c| c.ok);
        let report = Report {
            version: app.package_info().version.to_string(),
            passed,
            setup_ms,
            ui_ready_ms,
            phases: MARKS.lock().map(|m| m.clone()).unwrap_or_default(),
            checks,
        };
        let json = serde_json::to_string_pretty(&report).unwrap_or_default();
        println!("{json}");
        if let Some(path) = std::env::var_os("WISP_SELFTEST_OUT") {
            let _ = std::fs::write(path, &json);
        }
        app.exit(if passed { 0 } else { 1 });
    });
}
