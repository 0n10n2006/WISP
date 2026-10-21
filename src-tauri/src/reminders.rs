//! Background reminder delivery. Checks the database every 15 seconds and
//! posts a Windows notification for each due reminder, exactly once. Also
//! applies history retention once an hour.

use crate::state::AppState;
use crate::windows;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_notification::NotificationExt;

pub fn spawn(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let mut ticks: u32 = 0;
        loop {
            deliver_due(&app);
            // Apply history retention hourly, not just at startup.
            if ticks.is_multiple_of(240) {
                let state = app.state::<AppState>();
                let s = state.settings();
                let _ = state
                    .db
                    .prune_history(s.history_retention_days.max(1), s.content_history);
            }
            ticks = ticks.wrapping_add(1);
            tokio::time::sleep(Duration::from_secs(15)).await;
        }
    });
}

fn deliver_due(app: &AppHandle) {
    let state = app.state::<AppState>();
    let notify = state.settings().notifications_enabled;
    let Ok(due) = state.db.due_tasks(chrono::Utc::now()) else {
        return;
    };
    for task in due {
        if state.db.mark_notified(&task.id).is_err() {
            continue;
        }
        if notify {
            let _ = app
                .notification()
                .builder()
                .title(format!("Reminder: {}", task.title))
                .body(task.notes.clone().unwrap_or_else(|| "From Wisp".into()))
                .show();
        }
        let _ = app.emit_to(windows::ISLAND, "wisp://reminder-due", &task);
    }
}
