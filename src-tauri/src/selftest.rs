//! `wisp.exe --self-test`: start normally, verify the core subsystems, then
//! drive the real UI through a scripted scenario (hover, open, type, every
//! dashboard and Settings tab, shortcut, capture) and check the real windows
//! after each step. Writes a JSON report and exits 0 (pass) or 1 (fail).
//! With `WISP_SELFTEST_MOUSE=1` it also waits for a real mouse hover and
//! click on the notch (Windows CI moves the cursor). Used by CI to prove the
//! packaged app works, not just that it launches.

use crate::commands::CmdResult;
use crate::state::AppState;
use crate::windows;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tauri::Emitter;
use tauri::{AppHandle, Manager};
use wisp_core::WispError;

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
    name: String,
    ok: bool,
    detail: String,
}

fn check(name: &str, ok: bool, detail: impl Into<String>) -> Check {
    Check {
        name: name.to_string(),
        ok,
        detail: detail.into(),
    }
}

/// One step of a UI scenario, as reported by a window.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiStep {
    pub name: String,
    pub ok: bool,
    #[serde(default)]
    pub detail: String,
    /// Steps that can't work on this machine (e.g. no screen to capture).
    #[serde(default)]
    pub optional: bool,
}

static UI_REPORTS: Mutex<Vec<(String, Vec<UiStep>)>> = Mutex::new(Vec::new());
static RUNNING: AtomicBool = AtomicBool::new(false);

/// What the real windows look like right now (for the scenario's checks).
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Probe {
    mode: windows::NotchMode,
    visible: bool,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    settings_open: bool,
    overlays: usize,
    focused: bool,
}

pub fn probe(app: &AppHandle) -> Probe {
    let mode = app.state::<AppState>().island.lock().expect("island").mode;
    let win = windows::island(app);
    let (x, y) = win
        .as_ref()
        .and_then(|w| w.outer_position().ok())
        .map(|p| (p.x, p.y))
        .unwrap_or_default();
    let (width, height) = win
        .as_ref()
        .and_then(|w| w.inner_size().ok())
        .map(|s| (s.width, s.height))
        .unwrap_or_default();
    Probe {
        mode,
        visible: win
            .as_ref()
            .and_then(|w| w.is_visible().ok())
            .unwrap_or(false),
        x,
        y,
        width,
        height,
        settings_open: app.get_webview_window(windows::SETTINGS).is_some(),
        // Overlays stay loaded (hidden) between captures: count shown ones.
        overlays: app
            .webview_windows()
            .iter()
            .filter(|(l, w)| l.starts_with("overlay-") && w.is_visible().unwrap_or(false))
            .count(),
        focused: win
            .as_ref()
            .and_then(|w| w.is_focused().ok())
            .unwrap_or(false),
    }
}

/// The island's real window state, for the self-test scenario.
#[tauri::command]
pub fn selftest_probe(app: AppHandle) -> Probe {
    probe(&app)
}

/// Host-side steps the scenario can't trigger from the page itself.
#[tauri::command]
pub fn selftest_action(app: AppHandle, action: String) -> CmdResult<()> {
    if !RUNNING.load(Ordering::SeqCst) {
        return Err(WispError::invalid("Only available during --self-test"));
    }
    match action.as_str() {
        "summon" => windows::summon_island(&app),
        "close" => windows::close_bar(&app),
        "capture" => crate::capture::start(&app),
        "capture-cancel" => crate::capture::cancel(&app),
        _ => return Err(WispError::invalid("Unknown self-test action")),
    }
    Ok(())
}

/// A window finished its part of the scenario.
#[tauri::command]
pub fn selftest_report(section: String, steps: Vec<UiStep>) {
    if !RUNNING.load(Ordering::SeqCst) {
        return;
    }
    if let Ok(mut r) = UI_REPORTS.lock() {
        r.push((
            section.chars().take(40).collect(),
            steps.into_iter().take(100).collect(),
        ));
    }
}

async fn wait_report(section: &str, timeout: Duration) -> Option<Vec<UiStep>> {
    let start = Instant::now();
    while start.elapsed() < timeout {
        if let Some(steps) = UI_REPORTS.lock().ok().and_then(|r| {
            r.iter()
                .find(|(s, _)| s == section)
                .map(|(_, st)| st.clone())
        }) {
            return Some(steps);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    None
}

/// Run a window's scenario and turn its steps into checks.
async fn run_section(app: &AppHandle, window: &str, section: &str, checks: &mut Vec<Check>) {
    let _ = app.emit_to(window, "wisp://selftest-run", section);
    match wait_report(section, UI_SCENARIO_TIMEOUT).await {
        Some(steps) => {
            for s in steps {
                // Optional steps are reported but never fail the run.
                let ok = s.ok || s.optional;
                checks.push(check(&format!("ui:{section}:{}", s.name), ok, s.detail));
            }
        }
        None => checks.push(check(
            &format!("ui:{section}"),
            false,
            "the window never finished its scenario",
        )),
    }
}

const UI_SCENARIO_TIMEOUT: Duration = Duration::from_secs(90);

/// Wait for a real mouse hover and click on the notch (driven from outside).
async fn real_mouse(app: &AppHandle, checks: &mut Vec<Check>) {
    windows::close_bar(app);
    tokio::time::sleep(Duration::from_millis(500)).await;
    let p = probe(app);
    let target = serde_json::json!({
        "x": p.x, "y": p.y, "width": p.width, "height": p.height, "visible": p.visible,
    });
    if let Some(out) = std::env::var_os("WISP_SELFTEST_OUT") {
        let mut path = std::path::PathBuf::from(out);
        path.set_extension("notch.json");
        let _ = std::fs::write(path, target.to_string());
    }
    let (mut peeked, mut opened) = (false, false);
    let start = Instant::now();
    while start.elapsed() < Duration::from_secs(45) && !opened {
        let mode = app.state::<AppState>().island.lock().expect("island").mode;
        peeked |= mode == windows::NotchMode::Peek;
        opened |= mode == windows::NotchMode::Open;
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    checks.push(check(
        "real-mouse:hover-peeks",
        peeked || opened,
        format!("notch at {target}"),
    ));
    checks.push(check("real-mouse:click-opens", opened, ""));
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
                name: "setup-time".into(),
                ok: setup_ms <= STARTUP_BUDGET_MS,
                detail: format!("{setup_ms} ms (budget {STARTUP_BUDGET_MS} ms)"),
            },
            Check {
                name: "ui-ready".into(),
                ok: ui_ready,
                detail: format!("{ui_ready_ms:?} ms after process start"),
            },
            Check {
                name: "island-window".into(),
                ok: windows::island(&app).is_some(),
                detail: String::new(),
            },
        ];
        match state.db.stats() {
            Ok(s) => checks.push(Check {
                name: "database".into(),
                ok: true,
                detail: format!("schema v{}", s.schema_version),
            }),
            Err(e) => checks.push(Check {
                name: "database".into(),
                ok: false,
                detail: e.message,
            }),
        }
        match state.secrets.get() {
            Ok(k) => checks.push(Check {
                name: "credential-vault".into(),
                ok: true,
                detail: if k.is_some() {
                    "key present".into()
                } else {
                    "no key".into()
                },
            }),
            Err(e) => checks.push(Check {
                name: "credential-vault".into(),
                ok: false,
                detail: e.message,
            }),
        }
        let issues = state.shortcut_issues.lock().expect("issues").len();
        // Informational: CI desktops may already own some hotkeys.
        checks.push(Check {
            name: "global-shortcuts".into(),
            ok: true,
            detail: format!("{issues} could not be registered"),
        });
        // v0.2 notch and dashboard.
        let layout = *state.island.lock().expect("island");
        checks.push(Check {
            name: "notch-mode".into(),
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
                name: "system-stats".into(),
                ok: s.memory_total > 0 && !s.cores.is_empty(),
                detail: format!(
                    "{} cores, {} MB RAM",
                    s.cores.len(),
                    s.memory_total / 1_048_576
                ),
            },
            Err(e) => Check {
                name: "system-stats".into(),
                ok: false,
                detail: e.to_string(),
            },
        });
        let catalogue = tauri::async_runtime::spawn_blocking(crate::apps::catalogue)
            .await
            .unwrap_or_default();
        checks.push(Check {
            name: "launcher".into(),
            ok: catalogue.len() >= wisp_core::launcher::BUILTIN.len(),
            detail: format!("{} items", catalogue.len()),
        });
        // Informational: whether the hold-to-talk hook installed (Windows only).
        let hook = MARKS
            .lock()
            .map(|m| m.iter().any(|(p, _)| p == "hold-keys"))
            .unwrap_or(false);
        checks.push(Check {
            name: "hold-to-talk-hook".into(),
            ok: true,
            detail: if cfg!(windows) {
                format!("installed: {hook}")
            } else {
                "not applicable".into()
            },
        });
        let warnings = state.startup_warnings.lock().expect("warnings").clone();
        checks.push(Check {
            name: "startup-warnings".into(),
            ok: warnings.iter().all(|w| !w.contains("database")),
            detail: warnings.join(" | "),
        });

        // Drive the real UI (skip with WISP_SELFTEST_UI=0).
        if ui_ready && std::env::var("WISP_SELFTEST_UI").map_or(true, |v| v != "0") {
            RUNNING.store(true, Ordering::SeqCst);
            run_section(&app, windows::ISLAND, "island", &mut checks).await;
            windows::open_settings(&app, None);
            // Let the Settings window boot before driving it.
            let opened = Instant::now();
            while app.get_webview_window(windows::SETTINGS).is_none()
                && opened.elapsed() < Duration::from_secs(10)
            {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            tokio::time::sleep(Duration::from_millis(1500)).await;
            run_section(&app, windows::SETTINGS, "settings", &mut checks).await;
            if std::env::var("WISP_SELFTEST_MOUSE").is_ok_and(|v| v == "1") {
                real_mouse(&app, &mut checks).await;
            }
            RUNNING.store(false, Ordering::SeqCst);
        }
        let errors = crate::errlog::recent(20);
        checks.push(check(
            "error-log",
            true,
            if errors.is_empty() {
                "empty".to_string()
            } else {
                errors.join(" | ")
            },
        ));

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
