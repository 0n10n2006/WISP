//! Keeps the file index in step with the user's folders: one background
//! pass builds it (at low priority), a folder watcher keeps it current, and
//! a full pass every few hours catches anything the watcher missed.
//! Only names, places, sizes and dates are stored, never file contents.

use crate::state::AppState;
use serde::Serialize;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Mutex};
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_opener::OpenerExt;
use wisp_core::files::{self, Entry, FileHit, FileIndex, Root};
use wisp_core::storage::ActionAudit;
use wisp_core::{Result, WispError};

/// A full pass this often, in case the watcher missed something.
const FULL_EVERY: Duration = Duration::from_secs(6 * 3600);
/// Changes are applied once the folder has been quiet this long.
const DEBOUNCE: Duration = Duration::from_secs(2);
/// Wait this long after startup before the first pass.
const START_DELAY: Duration = Duration::from_secs(4);
const BATCH: usize = 1000;

enum Msg {
    /// Walk every folder again (settings changed, rebuild, watcher overflow).
    Rescan,
    /// Forget everything (indexing turned off).
    Clear,
    Changed(Vec<PathBuf>),
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FilesStatus {
    pub enabled: bool,
    pub indexing: bool,
    pub count: u64,
    /// Entries seen so far in the current pass.
    pub scanned: u64,
    /// When the last full pass finished (unix ms).
    pub updated_at: Option<i64>,
    pub folders: Vec<Root>,
}

pub struct Indexer {
    db: Option<FileIndex>,
    tx: Mutex<Option<mpsc::Sender<Msg>>>,
    roots: Mutex<Vec<Root>>,
    indexing: AtomicBool,
    scanned: Mutex<u64>,
    /// Set to stop a pass in progress.
    stop: AtomicBool,
}

fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

impl Indexer {
    pub fn open(path: &Path) -> Self {
        let db = match FileIndex::open(path) {
            Ok(db) => Some(db),
            Err(e) => {
                crate::errlog::log("file index", &e.message);
                None
            }
        };
        Self {
            db,
            tx: Mutex::new(None),
            roots: Mutex::new(Vec::new()),
            indexing: AtomicBool::new(false),
            scanned: Mutex::new(0),
            stop: AtomicBool::new(false),
        }
    }

    fn send(&self, msg: Msg) {
        if let Some(tx) = self.tx.lock().expect("files tx").as_ref() {
            let _ = tx.send(msg);
        }
    }

    /// Settings changed: index again, or forget it all when turned off.
    pub fn reconfigure(&self, enabled: bool) {
        self.stop.store(true, Ordering::SeqCst);
        self.send(if enabled { Msg::Rescan } else { Msg::Clear });
    }

    pub fn rebuild(&self) {
        self.reconfigure(true);
    }

    /// Forget the index (Delete everything); it fills again at the next pass.
    pub fn clear(&self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(db) = &self.db {
            let _ = db.clear();
        }
    }

    pub fn roots(&self) -> Vec<Root> {
        self.roots.lock().expect("roots").clone()
    }

    pub fn status(&self, enabled: bool, folders: Vec<Root>) -> FilesStatus {
        let db = self.db.as_ref();
        FilesStatus {
            enabled,
            indexing: self.indexing.load(Ordering::SeqCst),
            count: db.and_then(|d| d.count().ok()).unwrap_or(0),
            scanned: *self.scanned.lock().expect("scanned"),
            updated_at: db.and_then(|d| d.last_full().ok().flatten()),
            folders,
        }
    }

    pub fn search(&self, query: &str, limit: usize) -> Vec<FileHit> {
        let Some(db) = &self.db else {
            return Vec::new();
        };
        db.search(query, limit, chrono::Utc::now().timestamp())
            .unwrap_or_default()
    }

    pub fn get(&self, id: i64) -> Option<FileHit> {
        self.db.as_ref()?.get(id).ok().flatten()
    }
}

/// The folders to index: the standard Windows ones and the user's own.
pub fn roots_for(app: &AppHandle, settings: &wisp_core::settings::Settings) -> Vec<Root> {
    let p = app.path();
    let mut roots: Vec<Root> = [
        p.desktop_dir(),
        p.document_dir(),
        p.download_dir(),
        p.picture_dir(),
        p.audio_dir(),
        p.video_dir(),
    ]
    .into_iter()
    .flatten()
    .map(|d| Root::new(d, false))
    .collect();
    #[cfg(windows)]
    if let Some(od) = std::env::var_os("OneDrive").filter(|v| !v.is_empty()) {
        roots.push(Root::new(PathBuf::from(od), false));
    }
    roots.extend(
        settings
            .index_folders
            .iter()
            .filter(|f| crate::wallpaper::is_local_folder(f))
            .map(|f| Root::new(PathBuf::from(f), true)),
    );
    files::tidy_roots(roots)
}

/// The index's state, with the folders it covers.
pub fn status(app: &AppHandle) -> FilesStatus {
    let state = app.state::<AppState>();
    let settings = state.settings();
    let folders = if settings.index_files {
        roots_for(app, &settings)
    } else {
        Vec::new()
    };
    state.files.status(settings.index_files, folders)
}

fn emit(app: &AppHandle) {
    let _ = app.emit("wisp://files-index", status(app));
}

/// Start the indexer's worker thread.
pub fn spawn(app: AppHandle) {
    let (tx, rx) = mpsc::channel();
    *app.state::<AppState>().files.tx.lock().expect("files tx") = Some(tx.clone());
    let _ = std::thread::Builder::new()
        .name("wisp-files".into())
        .spawn(move || worker(app, tx, rx));
}

#[cfg(windows)]
fn background_priority() {
    use windows::Win32::System::Threading::{
        GetCurrentThread, SetThreadPriority, THREAD_MODE_BACKGROUND_BEGIN,
    };
    // Low CPU and disk priority: indexing never gets in the user's way.
    unsafe {
        let _ = SetThreadPriority(GetCurrentThread(), THREAD_MODE_BACKGROUND_BEGIN);
    }
}

#[cfg(not(windows))]
fn background_priority() {}

fn worker(app: AppHandle, tx: mpsc::Sender<Msg>, rx: mpsc::Receiver<Msg>) {
    background_priority();
    std::thread::sleep(START_DELAY);
    let state = app.state::<AppState>();
    let Some(db) = state.files.db.as_ref() else {
        return;
    };
    let mut _watcher: Option<notify::RecommendedWatcher> = None;
    let mut pending: HashSet<PathBuf> = HashSet::new();
    let mut quiet_at: Option<Instant> = None;
    let mut next_full = Instant::now();
    let mut first = true;
    loop {
        let enabled = state.settings().index_files;
        let mut rescan = false;
        if first {
            first = false;
            if enabled {
                // Watch first, so nothing changed during the pass is lost.
                _watcher = watch(&app, &tx);
                // A recent full pass is enough: just catch up on changes.
                let fresh = db
                    .last_full()
                    .ok()
                    .flatten()
                    .is_some_and(|t| now_ms() - t < FULL_EVERY.as_millis() as i64);
                rescan = !fresh || db.count().unwrap_or(0) == 0;
                if !rescan {
                    *state.files.roots.lock().expect("roots") = roots_for(&app, &state.settings());
                    next_full = Instant::now() + Duration::from_secs(15 * 60);
                }
            }
        } else {
            let now = Instant::now();
            let wake = quiet_at.map_or(next_full, |q| q.min(next_full));
            match rx.recv_timeout(wake.saturating_duration_since(now)) {
                Ok(Msg::Rescan) => rescan = true,
                Ok(Msg::Clear) => {
                    _watcher = None;
                    pending.clear();
                    quiet_at = None;
                    let _ = db.clear();
                    state.files.roots.lock().expect("roots").clear();
                    emit(&app);
                    continue;
                }
                Ok(Msg::Changed(paths)) => {
                    pending.extend(paths);
                    quiet_at = Some(Instant::now() + DEBOUNCE);
                    continue;
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
            }
            if !rescan && quiet_at.is_some_and(|q| Instant::now() >= q) {
                quiet_at = None;
                if enabled {
                    apply_changes(db, &state.files.roots(), pending.drain());
                    emit(&app);
                } else {
                    pending.clear();
                }
            }
            if Instant::now() >= next_full && enabled {
                rescan = true;
            }
        }
        if rescan && state.settings().index_files {
            // Coalesce a burst of requests into one pass.
            while let Ok(m) = rx.try_recv() {
                match m {
                    Msg::Rescan => {}
                    Msg::Changed(p) => pending.extend(p),
                    Msg::Clear => {
                        let _ = tx.send(Msg::Clear);
                        break;
                    }
                }
            }
            state.files.stop.store(false, Ordering::SeqCst);
            let roots = roots_for(&app, &state.settings());
            *state.files.roots.lock().expect("roots") = roots.clone();
            _watcher = watch(&app, &tx);
            full_pass(&app, db, &roots);
            next_full = Instant::now() + FULL_EVERY;
        }
    }
}

/// Watch every root for changes (the OS tells us; nothing is polled).
fn watch(app: &AppHandle, tx: &mpsc::Sender<Msg>) -> Option<notify::RecommendedWatcher> {
    use notify::{EventKind, RecursiveMode, Watcher};
    let tx = tx.clone();
    let mut w = notify::recommended_watcher(move |res: notify::Result<notify::Event>| match res {
        Ok(ev) if ev.need_rescan() => {
            let _ = tx.send(Msg::Rescan);
        }
        Ok(ev) if !matches!(ev.kind, EventKind::Access(_)) && !ev.paths.is_empty() => {
            let _ = tx.send(Msg::Changed(ev.paths));
        }
        _ => {}
    })
    .ok()?;
    let state = app.state::<AppState>();
    for r in roots_for(app, &state.settings()) {
        let _ = w.watch(&r.path, RecursiveMode::Recursive);
    }
    Some(w)
}

fn full_pass(app: &AppHandle, db: &FileIndex, roots: &[Root]) {
    let files = &app.state::<AppState>().files;
    files.indexing.store(true, Ordering::SeqCst);
    *files.scanned.lock().expect("scanned") = 0;
    emit(app);
    let seen = now_ms();
    let bulk = db.count().unwrap_or(0) == 0;
    if bulk {
        let _ = db.begin_bulk();
    }
    let mut batch: Vec<Entry> = Vec::with_capacity(BATCH);
    let mut total: u64 = 0;
    let mut complete = true;
    let mut last_emit = Instant::now();
    for root in roots {
        // Roots inside this one are walked on their own, with their own name.
        let exclude: Vec<PathBuf> = roots
            .iter()
            .filter(|o| o.path != root.path && o.path.starts_with(&root.path))
            .map(|o| o.path.clone())
            .collect();
        let finished = files::walk(root, &exclude, &mut |e| {
            batch.push(e);
            total += 1;
            if batch.len() >= BATCH {
                let _ = db.upsert(&batch, seen);
                batch.clear();
                *files.scanned.lock().expect("scanned") = total;
                if last_emit.elapsed() > Duration::from_millis(500) {
                    last_emit = Instant::now();
                    emit(app);
                }
            }
            total < files::MAX_ENTRIES as u64 && !files.stop.load(Ordering::SeqCst)
        });
        if !finished {
            complete = total >= files::MAX_ENTRIES as u64;
            break;
        }
    }
    let _ = db.upsert(&batch, seen);
    if bulk {
        let _ = db.end_bulk();
    }
    if complete {
        let keys: Vec<String> = roots
            .iter()
            .map(|r| r.path.to_string_lossy().into_owned())
            .collect();
        let _ = db.finish_pass(seen, &keys);
    }
    *files.scanned.lock().expect("scanned") = total;
    files.indexing.store(false, Ordering::SeqCst);
    emit(app);
}

/// The root a path belongs to (the innermost one).
fn root_of<'a>(path: &Path, roots: &'a [Root]) -> Option<&'a Root> {
    roots
        .iter()
        .filter(|r| path.starts_with(&r.path))
        .max_by_key(|r| r.path.as_os_str().len())
}

/// Bring the index up to date for paths the watcher reported.
fn apply_changes(db: &FileIndex, roots: &[Root], paths: impl Iterator<Item = PathBuf>) {
    let seen = now_ms();
    let mut batch = Vec::new();
    for path in paths {
        let Some(root) = root_of(&path, roots) else {
            continue;
        };
        let key = path.to_string_lossy();
        let meta = match std::fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(_) => {
                let _ = db.remove(&key);
                continue;
            }
        };
        let is_dir = meta.is_dir();
        if meta.file_type().is_symlink()
            || files::is_hidden(&meta)
            || !files::allowed_below(&path, root, is_dir)
        {
            let _ = db.remove(&key);
            continue;
        }
        if is_dir {
            // A new or renamed folder: take in what's inside it.
            let inner: Vec<PathBuf> = roots
                .iter()
                .filter(|o| o.path != root.path && o.path.starts_with(&path))
                .map(|o| o.path.clone())
                .collect();
            files::walk_from(root, &path, &inner, &mut |e| {
                batch.push(e);
                batch.len() < files::MAX_ENTRIES
            });
        } else if let Some(e) = files::entry_for(&path, &meta, root) {
            batch.push(e);
        }
    }
    for chunk in batch.chunks(BATCH) {
        let _ = db.upsert(chunk, seen);
    }
}

/// Open (or show in its folder) an indexed file, after checking it again.
pub fn open(app: &AppHandle, id: i64, reveal: bool) -> Result<String> {
    let state = app.state::<AppState>();
    if !state.settings().index_files {
        return Err(WispError::invalid(
            "File search is off. Turn it on in Settings → Search.",
        ));
    }
    let hit = state
        .files
        .get(id)
        .ok_or_else(|| WispError::invalid("That file isn't in the index any more."))?;
    let path = files::check_openable(&hit, &state.files.roots())?;
    let res = if reveal {
        app.opener().reveal_item_in_dir(&path)
    } else {
        app.opener().open_path(path.to_string_lossy(), None::<&str>)
    };
    let kind = hit.kind.as_str();
    let action = if reveal { "show_file" } else { "open_file" };
    if let Err(e) = res {
        let mut audit = ActionAudit::new(action, &format!("kind={kind}"), "failed", false);
        audit.detail = Some(e.to_string());
        let _ = state.db.log_action(&audit);
        return Err(WispError::internal(format!(
            "Couldn't open \"{}\": {e}",
            hit.name
        )));
    }
    if let Some(db) = &state.files.db {
        let _ = db.record_open(id, chrono::Utc::now().timestamp());
    }
    let _ = state.db.log_action(&ActionAudit::new(
        action,
        &format!("kind={kind}"),
        "done",
        false,
    ));
    Ok(if reveal {
        format!("Showed {} in its folder.", hit.name)
    } else {
        format!("Opened {}.", hit.name)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changes_are_applied_from_the_watcher() {
        let dir = std::env::temp_dir().join(format!("wisp-fi-{}", uuid::Uuid::new_v4()));
        let docs = dir.join("Documents");
        std::fs::create_dir_all(docs.join("Trip")).unwrap();
        std::fs::write(docs.join("Trip/plan.pdf"), "x").unwrap();
        std::fs::write(docs.join("new.txt"), "x").unwrap();
        std::fs::write(docs.join("tool.exe"), "x").unwrap();
        let roots = vec![Root::new(&docs, false)];
        let db = FileIndex::open_in_memory().unwrap();
        apply_changes(
            &db,
            &roots,
            [
                docs.join("Trip"),
                docs.join("new.txt"),
                docs.join("tool.exe"),
                dir.join("outside.txt"),
            ]
            .into_iter(),
        );
        assert_eq!(db.search("plan", 5, 0).unwrap().len(), 1);
        assert_eq!(db.search("new", 5, 0).unwrap().len(), 1);
        assert!(db.search("tool", 5, 0).unwrap().is_empty());
        // Deleted: gone from the index, with what was inside.
        std::fs::remove_dir_all(docs.join("Trip")).unwrap();
        apply_changes(&db, &roots, [docs.join("Trip")].into_iter());
        assert!(db.search("plan", 5, 0).unwrap().is_empty());
        assert_eq!(db.count().unwrap(), 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn the_innermost_root_wins() {
        let roots = vec![
            Root::new("/home/u/OneDrive", false),
            Root::new("/home/u/OneDrive/Desktop", false),
        ];
        let r = root_of(Path::new("/home/u/OneDrive/Desktop/a.txt"), &roots).unwrap();
        assert_eq!(r.label, "Desktop");
        assert!(root_of(Path::new("/home/u/elsewhere"), &roots).is_none());
    }
}
