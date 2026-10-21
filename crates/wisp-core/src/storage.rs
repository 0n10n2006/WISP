//! Local SQLite storage: request history, action audit, reminders and memory.
//!
//! Every write is a single statement or an explicit transaction. Schema
//! changes are versioned migrations (`PRAGMA user_version`) and an existing
//! database is copied aside before migrating.

use crate::error::{Result, WispError};
use crate::tools;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const MIGRATIONS: &[&str] = &[
    // v1
    r#"
    CREATE TABLE request_log (
        id TEXT PRIMARY KEY,
        timestamp TEXT NOT NULL,
        request_type TEXT NOT NULL,
        model TEXT NOT NULL,
        status TEXT NOT NULL,
        latency_ms INTEGER NOT NULL,
        error_code TEXT,
        context_kind TEXT NOT NULL,
        total_tokens INTEGER,
        prompt TEXT,
        response TEXT
    );
    CREATE INDEX request_log_ts ON request_log(timestamp);
    CREATE TABLE action_audit (
        id TEXT PRIMARY KEY,
        timestamp TEXT NOT NULL,
        action_type TEXT NOT NULL,
        summary TEXT NOT NULL,
        outcome TEXT NOT NULL,
        user_confirmed INTEGER NOT NULL,
        detail TEXT
    );
    CREATE INDEX action_audit_ts ON action_audit(timestamp);
    CREATE TABLE tasks (
        id TEXT PRIMARY KEY,
        title TEXT NOT NULL,
        notes TEXT,
        due_at TEXT,
        created_at TEXT NOT NULL,
        completed_at TEXT,
        notification_enabled INTEGER NOT NULL DEFAULT 1,
        notified_at TEXT
    );
    CREATE INDEX tasks_due ON tasks(due_at);
    CREATE TABLE memory (
        id TEXT PRIMARY KEY,
        content TEXT NOT NULL,
        category TEXT NOT NULL,
        created_at TEXT NOT NULL,
        updated_at TEXT NOT NULL,
        user_approved INTEGER NOT NULL,
        enabled INTEGER NOT NULL DEFAULT 1
    );
    "#,
];

pub use crate::tools::MAX_MEMORY_CHARS;
pub const MAX_MEMORY_ENTRIES: usize = 200;
const MAX_LOG_TEXT: usize = 20_000;

fn now() -> DateTime<Utc> {
    Utc::now()
}

fn ts(d: &DateTime<Utc>) -> String {
    d.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn parse_ts(s: &str) -> DateTime<Utc> {
    DateTime::parse_from_rfc3339(s)
        .map(|d| d.with_timezone(&Utc))
        .unwrap_or_else(|_| Utc::now())
}

fn opt_ts(s: Option<String>) -> Option<DateTime<Utc>> {
    s.as_deref().map(parse_ts)
}

fn new_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

fn clip(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestLog {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub request_type: String,
    pub model: String,
    /// "ok" | "error" | "cancelled"
    pub status: String,
    pub latency_ms: u64,
    pub error_code: Option<String>,
    pub context_kind: String,
    pub total_tokens: Option<u64>,
    /// Only stored when the user opted into content history.
    pub prompt: Option<String>,
    pub response: Option<String>,
}

impl RequestLog {
    pub fn new(request_type: &str, model: &str, context_kind: &str) -> Self {
        Self {
            id: new_id(),
            timestamp: now(),
            request_type: request_type.into(),
            model: model.into(),
            status: "ok".into(),
            latency_ms: 0,
            error_code: None,
            context_kind: context_kind.into(),
            total_tokens: None,
            prompt: None,
            response: None,
        }
    }

    fn from_row(r: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: r.get(0)?,
            timestamp: parse_ts(&r.get::<_, String>(1)?),
            request_type: r.get(2)?,
            model: r.get(3)?,
            status: r.get(4)?,
            latency_ms: r.get::<_, i64>(5)? as u64,
            error_code: r.get(6)?,
            context_kind: r.get(7)?,
            total_tokens: r.get::<_, Option<i64>>(8)?.map(|v| v as u64),
            prompt: r.get(9)?,
            response: r.get(10)?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionAudit {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub action_type: String,
    pub summary: String,
    /// "done" | "rejected" | "declined" | "failed"
    pub outcome: String,
    pub user_confirmed: bool,
    pub detail: Option<String>,
}

impl ActionAudit {
    pub fn new(action_type: &str, summary: &str, outcome: &str, user_confirmed: bool) -> Self {
        Self {
            id: new_id(),
            timestamp: now(),
            action_type: action_type.into(),
            summary: crate::redact::summarize_text(summary, 160),
            outcome: outcome.into(),
            user_confirmed,
            detail: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: String,
    pub title: String,
    pub notes: Option<String>,
    pub due_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub notification_enabled: bool,
    pub notified_at: Option<DateTime<Utc>>,
}

impl Task {
    fn from_row(r: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: r.get(0)?,
            title: r.get(1)?,
            notes: r.get(2)?,
            due_at: opt_ts(r.get(3)?),
            created_at: parse_ts(&r.get::<_, String>(4)?),
            completed_at: opt_ts(r.get(5)?),
            notification_enabled: r.get::<_, i64>(6)? != 0,
            notified_at: opt_ts(r.get(7)?),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewTask {
    pub title: String,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub due_at: Option<DateTime<Utc>>,
    #[serde(default = "yes")]
    pub notification_enabled: bool,
}

fn yes() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryEntry {
    pub id: String,
    pub content: String,
    pub category: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub user_approved: bool,
    pub enabled: bool,
}

impl MemoryEntry {
    fn from_row(r: &Row) -> rusqlite::Result<Self> {
        Ok(Self {
            id: r.get(0)?,
            content: r.get(1)?,
            category: r.get(2)?,
            created_at: parse_ts(&r.get::<_, String>(3)?),
            updated_at: parse_ts(&r.get::<_, String>(4)?),
            user_approved: r.get::<_, i64>(5)? != 0,
            enabled: r.get::<_, i64>(6)? != 0,
        })
    }
}

pub const MEMORY_CATEGORIES: [&str; 5] = ["preference", "fact", "project", "people", "other"];

fn validate_memory(content: &str, category: &str) -> Result<(String, String)> {
    let c = content.trim();
    if c.is_empty() {
        return Err(WispError::invalid("Memory can't be empty"));
    }
    if c.chars().count() > MAX_MEMORY_CHARS {
        return Err(WispError::invalid(format!(
            "Keep memories under {MAX_MEMORY_CHARS} characters"
        )));
    }
    let cat = category.trim().to_lowercase();
    let cat = if MEMORY_CATEGORIES.contains(&cat.as_str()) {
        cat
    } else {
        "other".to_string()
    };
    Ok((c.to_string(), cat))
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageStats {
    pub requests: u64,
    pub actions: u64,
    pub tasks: u64,
    pub open_tasks: u64,
    pub memories: u64,
    pub schema_version: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DataScope {
    History,
    Memory,
    Tasks,
    All,
}

// ---------------------------------------------------------------------------

pub struct Db {
    conn: Mutex<Connection>,
    pub backup_path: Option<PathBuf>,
}

impl Db {
    /// Open (or create) the database at `path`, migrating if needed.
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let existed = path.exists();
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        let version: u32 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        let mut backup_path = None;
        if existed && (version as usize) < MIGRATIONS.len() {
            let bak = path.with_extension(format!("v{version}.bak"));
            conn.execute("VACUUM INTO ?1", params![bak.to_string_lossy()])?;
            backup_path = Some(bak);
        }
        let mut db = Self {
            conn: Mutex::new(conn),
            backup_path: None,
        };
        db.migrate()?;
        db.backup_path = backup_path;
        Ok(db)
    }

    pub fn open_in_memory() -> Result<Self> {
        let db = Self {
            conn: Mutex::new(Connection::open_in_memory()?),
            backup_path: None,
        };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> Result<()> {
        let mut conn = self.conn.lock().expect("db");
        conn.pragma_update(None, "foreign_keys", "ON")?;
        let version: u32 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version as usize > MIGRATIONS.len() {
            return Err(WispError::storage(
                "This database was created by a newer version of Wisp. Update Wisp to open it.",
            ));
        }
        for (i, sql) in MIGRATIONS.iter().enumerate().skip(version as usize) {
            let tx = conn.transaction()?;
            tx.execute_batch(sql)?;
            tx.pragma_update(None, "user_version", (i + 1) as u32)?;
            tx.commit()?;
        }
        Ok(())
    }

    fn with<T>(&self, f: impl FnOnce(&mut Connection) -> rusqlite::Result<T>) -> Result<T> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|_| WispError::storage("Database is unavailable"))?;
        f(&mut conn).map_err(Into::into)
    }

    // ---- request log ----------------------------------------------------

    pub fn log_request(&self, entry: &RequestLog) -> Result<()> {
        self.with(|c| {
            c.execute(
                "INSERT INTO request_log (id, timestamp, request_type, model, status, latency_ms, error_code, context_kind, total_tokens, prompt, response)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
                params![
                    entry.id,
                    ts(&entry.timestamp),
                    entry.request_type,
                    entry.model,
                    entry.status,
                    entry.latency_ms as i64,
                    entry.error_code,
                    entry.context_kind,
                    entry.total_tokens.map(|v| v as i64),
                    entry.prompt.as_deref().map(|s| clip(&crate::redact::redact(s), MAX_LOG_TEXT)),
                    entry.response.as_deref().map(|s| clip(s, MAX_LOG_TEXT)),
                ],
            )
            .map(|_| ())
        })
    }

    pub fn list_requests(&self, limit: u32) -> Result<Vec<RequestLog>> {
        self.with(|c| {
            let mut stmt = c.prepare(
                "SELECT id, timestamp, request_type, model, status, latency_ms, error_code, context_kind, total_tokens, prompt, response
                 FROM request_log ORDER BY timestamp DESC LIMIT ?1",
            )?;
            let rows = stmt.query_map(params![limit.min(1000)], RequestLog::from_row)?;
            rows.collect()
        })
    }

    pub fn clear_requests(&self) -> Result<u64> {
        self.with(|c| c.execute("DELETE FROM request_log", []).map(|n| n as u64))
    }

    /// Remove history older than `days`, and stored content when content history is off.
    pub fn prune_history(&self, days: u32, keep_content: bool) -> Result<()> {
        let cutoff = ts(&(now() - chrono::Duration::days(days as i64)));
        self.with(|c| {
            let tx = c.transaction()?;
            tx.execute("DELETE FROM request_log WHERE timestamp < ?1", params![cutoff])?;
            tx.execute("DELETE FROM action_audit WHERE timestamp < ?1", params![cutoff])?;
            if !keep_content {
                tx.execute(
                    "UPDATE request_log SET prompt = NULL, response = NULL WHERE prompt IS NOT NULL OR response IS NOT NULL",
                    [],
                )?;
            }
            tx.commit()
        })
    }

    // ---- action audit ---------------------------------------------------

    pub fn log_action(&self, a: &ActionAudit) -> Result<()> {
        self.with(|c| {
            c.execute(
                "INSERT INTO action_audit (id, timestamp, action_type, summary, outcome, user_confirmed, detail)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    a.id,
                    ts(&a.timestamp),
                    a.action_type,
                    a.summary,
                    a.outcome,
                    a.user_confirmed as i64,
                    a.detail.as_deref().map(crate::redact::redact)
                ],
            )
            .map(|_| ())
        })
    }

    pub fn list_actions(&self, limit: u32) -> Result<Vec<ActionAudit>> {
        self.with(|c| {
            let mut stmt = c.prepare(
                "SELECT id, timestamp, action_type, summary, outcome, user_confirmed, detail
                 FROM action_audit ORDER BY timestamp DESC LIMIT ?1",
            )?;
            let rows = stmt.query_map(params![limit.min(1000)], |r| {
                Ok(ActionAudit {
                    id: r.get(0)?,
                    timestamp: parse_ts(&r.get::<_, String>(1)?),
                    action_type: r.get(2)?,
                    summary: r.get(3)?,
                    outcome: r.get(4)?,
                    user_confirmed: r.get::<_, i64>(5)? != 0,
                    detail: r.get(6)?,
                })
            })?;
            rows.collect()
        })
    }

    pub fn clear_actions(&self) -> Result<u64> {
        self.with(|c| c.execute("DELETE FROM action_audit", []).map(|n| n as u64))
    }

    // ---- tasks ----------------------------------------------------------

    const TASK_COLS: &'static str =
        "id, title, notes, due_at, created_at, completed_at, notification_enabled, notified_at";

    pub fn create_task(&self, t: &NewTask) -> Result<Task> {
        let title = tools::validate_title(&t.title)?;
        let notes = tools::validate_notes(t.notes.clone())?;
        if let Some(due) = t.due_at {
            tools::validate_due(due, now())?;
        }
        let task = Task {
            id: new_id(),
            title,
            notes,
            due_at: t.due_at,
            created_at: now(),
            completed_at: None,
            notification_enabled: t.notification_enabled,
            notified_at: None,
        };
        self.with(|c| {
            c.execute(
                &format!(
                    "INSERT INTO tasks ({}) VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, NULL)",
                    Self::TASK_COLS
                ),
                params![
                    task.id,
                    task.title,
                    task.notes,
                    task.due_at.as_ref().map(ts),
                    ts(&task.created_at),
                    task.notification_enabled as i64
                ],
            )
        })?;
        Ok(task)
    }

    pub fn get_task(&self, id: &str) -> Result<Option<Task>> {
        self.with(|c| {
            c.query_row(
                &format!("SELECT {} FROM tasks WHERE id = ?1", Self::TASK_COLS),
                params![id],
                Task::from_row,
            )
            .optional()
        })
    }

    fn require_task(&self, id: &str) -> Result<Task> {
        self.get_task(id)?
            .ok_or_else(|| WispError::invalid("That reminder no longer exists"))
    }

    pub fn list_tasks(&self, include_completed: bool) -> Result<Vec<Task>> {
        let filter = if include_completed {
            ""
        } else {
            "WHERE completed_at IS NULL"
        };
        self.with(|c| {
            let mut stmt = c.prepare(&format!(
                "SELECT {} FROM tasks {filter}
                 ORDER BY completed_at IS NOT NULL, due_at IS NULL, due_at, created_at",
                Self::TASK_COLS
            ))?;
            let rows = stmt.query_map([], Task::from_row)?;
            rows.collect()
        })
    }

    pub fn update_task(&self, id: &str, t: &NewTask) -> Result<Task> {
        let existing = self.require_task(id)?;
        let title = tools::validate_title(&t.title)?;
        let notes = tools::validate_notes(t.notes.clone())?;
        if let Some(due) = t.due_at {
            if Some(due) != existing.due_at {
                tools::validate_due(due, now())?;
            }
        }
        let due_changed = t.due_at != existing.due_at;
        self.with(|c| {
            c.execute(
                "UPDATE tasks SET title = ?2, notes = ?3, due_at = ?4, notification_enabled = ?5,
                 notified_at = CASE WHEN ?6 THEN NULL ELSE notified_at END WHERE id = ?1",
                params![
                    id,
                    title,
                    notes,
                    t.due_at.as_ref().map(ts),
                    t.notification_enabled as i64,
                    due_changed
                ],
            )
        })?;
        self.require_task(id)
    }

    pub fn set_task_completed(&self, id: &str, completed: bool) -> Result<Task> {
        self.require_task(id)?;
        let at = completed.then(|| ts(&now()));
        self.with(|c| {
            c.execute(
                "UPDATE tasks SET completed_at = ?2 WHERE id = ?1",
                params![id, at],
            )
        })?;
        self.require_task(id)
    }

    pub fn snooze_task(&self, id: &str, until: DateTime<Utc>) -> Result<Task> {
        self.require_task(id)?;
        tools::validate_due(until, now())?;
        self.with(|c| {
            c.execute(
                "UPDATE tasks SET due_at = ?2, notified_at = NULL, completed_at = NULL WHERE id = ?1",
                params![id, ts(&until)],
            )
        })?;
        self.require_task(id)
    }

    pub fn delete_task(&self, id: &str) -> Result<bool> {
        self.with(|c| {
            c.execute("DELETE FROM tasks WHERE id = ?1", params![id])
                .map(|n| n > 0)
        })
    }

    /// Open reminders whose time has come and that haven't been notified yet.
    pub fn due_tasks(&self, at: DateTime<Utc>) -> Result<Vec<Task>> {
        self.with(|c| {
            let mut stmt = c.prepare(&format!(
                "SELECT {} FROM tasks WHERE completed_at IS NULL AND notified_at IS NULL
                 AND notification_enabled = 1 AND due_at IS NOT NULL AND due_at <= ?1 ORDER BY due_at",
                Self::TASK_COLS
            ))?;
            let rows = stmt.query_map(params![ts(&at)], Task::from_row)?;
            rows.collect()
        })
    }

    pub fn mark_notified(&self, id: &str) -> Result<()> {
        self.with(|c| {
            c.execute(
                "UPDATE tasks SET notified_at = ?2 WHERE id = ?1",
                params![id, ts(&now())],
            )
            .map(|_| ())
        })
    }

    // ---- memory ---------------------------------------------------------

    pub fn add_memory(
        &self,
        content: &str,
        category: &str,
        user_approved: bool,
    ) -> Result<MemoryEntry> {
        if !user_approved {
            return Err(WispError::invalid(
                "Memories are only saved with your approval",
            ));
        }
        let (content, category) = validate_memory(content, category)?;
        let count: i64 =
            self.with(|c| c.query_row("SELECT COUNT(*) FROM memory", [], |r| r.get(0)))?;
        if count as usize >= MAX_MEMORY_ENTRIES {
            return Err(WispError::invalid(
                "Memory is full. Delete some entries first.",
            ));
        }
        let m = MemoryEntry {
            id: new_id(),
            content,
            category,
            created_at: now(),
            updated_at: now(),
            user_approved: true,
            enabled: true,
        };
        self.with(|c| {
            c.execute(
                "INSERT INTO memory (id, content, category, created_at, updated_at, user_approved, enabled)
                 VALUES (?1, ?2, ?3, ?4, ?5, 1, 1)",
                params![m.id, m.content, m.category, ts(&m.created_at), ts(&m.updated_at)],
            )
        })?;
        Ok(m)
    }

    pub fn list_memory(&self) -> Result<Vec<MemoryEntry>> {
        self.with(|c| {
            let mut stmt = c.prepare(
                "SELECT id, content, category, created_at, updated_at, user_approved, enabled
                 FROM memory ORDER BY created_at DESC",
            )?;
            let rows = stmt.query_map([], MemoryEntry::from_row)?;
            rows.collect()
        })
    }

    pub fn update_memory(
        &self,
        id: &str,
        content: &str,
        category: &str,
        enabled: bool,
    ) -> Result<MemoryEntry> {
        let (content, category) = validate_memory(content, category)?;
        let n = self.with(|c| {
            c.execute(
                "UPDATE memory SET content = ?2, category = ?3, enabled = ?4, updated_at = ?5 WHERE id = ?1",
                params![id, content, category, enabled as i64, ts(&now())],
            )
        })?;
        if n == 0 {
            return Err(WispError::invalid("That memory no longer exists"));
        }
        self.list_memory()?
            .into_iter()
            .find(|m| m.id == id)
            .ok_or_else(|| WispError::invalid("That memory no longer exists"))
    }

    pub fn delete_memory(&self, id: &str) -> Result<bool> {
        self.with(|c| {
            c.execute("DELETE FROM memory WHERE id = ?1", params![id])
                .map(|n| n > 0)
        })
    }

    /// Content of enabled memories, newest first, for prompt context.
    pub fn enabled_memories(&self) -> Result<Vec<String>> {
        Ok(self
            .list_memory()?
            .into_iter()
            .filter(|m| m.enabled && m.user_approved)
            .map(|m| m.content)
            .collect())
    }

    // ---- lifecycle ------------------------------------------------------

    pub fn delete_data(&self, scope: DataScope) -> Result<()> {
        self.with(|c| {
            let tx = c.transaction()?;
            if matches!(scope, DataScope::History | DataScope::All) {
                tx.execute("DELETE FROM request_log", [])?;
                tx.execute("DELETE FROM action_audit", [])?;
            }
            if matches!(scope, DataScope::Memory | DataScope::All) {
                tx.execute("DELETE FROM memory", [])?;
            }
            if matches!(scope, DataScope::Tasks | DataScope::All) {
                tx.execute("DELETE FROM tasks", [])?;
            }
            tx.commit()?;
            // Reclaim space so deleted text doesn't linger in the file.
            c.execute_batch("VACUUM")
        })
    }

    pub fn stats(&self) -> Result<StorageStats> {
        self.with(|c| {
            let count = |sql: &str| {
                c.query_row(sql, [], |r| r.get::<_, i64>(0))
                    .map(|v| v as u64)
            };
            Ok(StorageStats {
                requests: count("SELECT COUNT(*) FROM request_log")?,
                actions: count("SELECT COUNT(*) FROM action_audit")?,
                tasks: count("SELECT COUNT(*) FROM tasks")?,
                open_tasks: count("SELECT COUNT(*) FROM tasks WHERE completed_at IS NULL")?,
                memories: count("SELECT COUNT(*) FROM memory")?,
                schema_version: c.pragma_query_value(None, "user_version", |r| r.get(0))?,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn db() -> Db {
        Db::open_in_memory().unwrap()
    }

    #[test]
    fn migrates_fresh_db() {
        let d = db();
        assert_eq!(d.stats().unwrap().schema_version, MIGRATIONS.len() as u32);
    }

    #[test]
    fn reopening_is_idempotent_and_persistent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wisp.db");
        {
            let d = Db::open(&path).unwrap();
            assert!(d.backup_path.is_none());
            d.add_memory("Likes tea", "preference", true).unwrap();
        }
        let d = Db::open(&path).unwrap();
        assert_eq!(d.list_memory().unwrap().len(), 1);
        assert!(d.backup_path.is_none());
    }

    #[test]
    fn backs_up_before_migrating_existing_db() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wisp.db");
        // An existing but unmigrated database (e.g. from an older version).
        Connection::open(&path)
            .unwrap()
            .execute_batch("CREATE TABLE legacy (x INTEGER);")
            .unwrap();
        let d = Db::open(&path).unwrap();
        let bak = d.backup_path.clone().unwrap();
        assert!(bak.exists());
        assert_eq!(d.stats().unwrap().schema_version, MIGRATIONS.len() as u32);
    }

    #[test]
    fn refuses_newer_schema() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("wisp.db");
        Connection::open(&path)
            .unwrap()
            .pragma_update(None, "user_version", 99)
            .unwrap();
        assert!(Db::open(&path).is_err());
    }

    #[test]
    fn request_log_roundtrip_and_redaction() {
        let d = db();
        let mut e = RequestLog::new("ask", "gemini-2.5-flash", "text");
        e.latency_ms = 420;
        e.prompt = Some("my key is AIzaSyDdI0hCZtE6vySjMm-WEfRq3CPzqKqqsHI".into());
        d.log_request(&e).unwrap();
        let got = d.list_requests(10).unwrap();
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].latency_ms, 420);
        assert!(!got[0].prompt.as_ref().unwrap().contains("AIza"));
        assert_eq!(d.clear_requests().unwrap(), 1);
        assert!(d.list_requests(10).unwrap().is_empty());
    }

    #[test]
    fn prune_removes_old_entries_and_content() {
        let d = db();
        let mut old = RequestLog::new("ask", "m", "text");
        old.timestamp = Utc::now() - Duration::days(40);
        d.log_request(&old).unwrap();
        let mut fresh = RequestLog::new("ask", "m", "text");
        fresh.prompt = Some("hello".into());
        d.log_request(&fresh).unwrap();
        d.prune_history(30, false).unwrap();
        let rows = d.list_requests(10).unwrap();
        assert_eq!(rows.len(), 1);
        assert!(rows[0].prompt.is_none());
    }

    #[test]
    fn task_lifecycle() {
        let d = db();
        let due = Utc::now() + Duration::minutes(30);
        let t = d
            .create_task(&NewTask {
                title: "Stretch".into(),
                notes: None,
                due_at: Some(due),
                notification_enabled: true,
            })
            .unwrap();
        assert_eq!(d.list_tasks(false).unwrap().len(), 1);
        assert!(d.due_tasks(Utc::now()).unwrap().is_empty());
        assert_eq!(d.due_tasks(due + Duration::seconds(1)).unwrap().len(), 1);
        d.mark_notified(&t.id).unwrap();
        assert!(d.due_tasks(due + Duration::seconds(1)).unwrap().is_empty());

        let snoozed = d
            .snooze_task(&t.id, Utc::now() + Duration::hours(1))
            .unwrap();
        assert!(snoozed.notified_at.is_none());

        let done = d.set_task_completed(&t.id, true).unwrap();
        assert!(done.completed_at.is_some());
        assert!(d.list_tasks(false).unwrap().is_empty());
        assert_eq!(d.list_tasks(true).unwrap().len(), 1);
        assert!(d.delete_task(&t.id).unwrap());
        assert!(!d.delete_task(&t.id).unwrap());
    }

    #[test]
    fn task_validation() {
        let d = db();
        let bad = |t: NewTask| d.create_task(&t).is_err();
        assert!(bad(NewTask {
            title: " ".into(),
            notes: None,
            due_at: None,
            notification_enabled: true
        }));
        assert!(bad(NewTask {
            title: "x".into(),
            notes: None,
            due_at: Some(Utc::now() - Duration::days(1)),
            notification_enabled: true
        }));
        // Undated tasks are fine.
        assert!(d
            .create_task(&NewTask {
                title: "Someday".into(),
                notes: Some("  ".into()),
                due_at: None,
                notification_enabled: true
            })
            .unwrap()
            .notes
            .is_none());
        assert!(d.set_task_completed("missing", true).is_err());
    }

    #[test]
    fn update_task_resets_notification_when_due_changes() {
        let d = db();
        let t = d
            .create_task(&NewTask {
                title: "A".into(),
                notes: None,
                due_at: Some(Utc::now() + Duration::minutes(5)),
                notification_enabled: true,
            })
            .unwrap();
        d.mark_notified(&t.id).unwrap();
        let u = d
            .update_task(
                &t.id,
                &NewTask {
                    title: "B".into(),
                    notes: Some("n".into()),
                    due_at: Some(Utc::now() + Duration::hours(2)),
                    notification_enabled: true,
                },
            )
            .unwrap();
        assert_eq!(u.title, "B");
        assert!(u.notified_at.is_none());
    }

    #[test]
    fn memory_requires_approval_and_validates() {
        let d = db();
        assert!(d.add_memory("inferred", "fact", false).is_err());
        assert!(d.add_memory("", "fact", true).is_err());
        assert!(d
            .add_memory(&"x".repeat(MAX_MEMORY_CHARS + 1), "fact", true)
            .is_err());
        let m = d
            .add_memory("Works in UTC+1", "weird-category", true)
            .unwrap();
        assert_eq!(m.category, "other");
        let m2 = d
            .update_memory(&m.id, "Works in UTC+2", "fact", false)
            .unwrap();
        assert!(!m2.enabled);
        assert!(d.enabled_memories().unwrap().is_empty());
        assert!(d.delete_memory(&m.id).unwrap());
    }

    #[test]
    fn delete_scopes() {
        let d = db();
        d.add_memory("a", "fact", true).unwrap();
        d.create_task(&NewTask {
            title: "t".into(),
            notes: None,
            due_at: None,
            notification_enabled: true,
        })
        .unwrap();
        d.log_request(&RequestLog::new("ask", "m", "text")).unwrap();
        d.log_action(&ActionAudit::new("copy_text", "chars=3", "done", false))
            .unwrap();
        d.delete_data(DataScope::History).unwrap();
        let s = d.stats().unwrap();
        assert_eq!((s.requests, s.actions, s.memories, s.tasks), (0, 0, 1, 1));
        d.delete_data(DataScope::All).unwrap();
        let s = d.stats().unwrap();
        assert_eq!((s.memories, s.tasks), (0, 0));
    }
}
