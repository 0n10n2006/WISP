//! The local file index: names and places of the files in the user's
//! folders (never their contents), so "open my resume" is instant.
//!
//! What gets indexed is decided here (skip rules, blocked types), as is the
//! ranking. The index lives in its own SQLite file with a full-text table
//! over names and folders, so a lookup over hundreds of thousands of files
//! takes a few milliseconds.

use crate::error::{Result, WispError};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Deepest folder level walked below a root.
pub const MAX_DEPTH: usize = 12;
/// Most entries kept in the index.
pub const MAX_ENTRIES: usize = 300_000;
/// Most results a search returns.
pub const MAX_RESULTS: usize = 50;
/// Folders the user may add on top of the standard ones.
pub const MAX_USER_FOLDERS: usize = 20;

/// Folders never walked into (build output, caches, app data, VCS).
const SKIP_DIRS: &[&str] = &[
    "node_modules",
    "appdata",
    "$recycle.bin",
    "system volume information",
    "target",
    "venv",
    "__pycache__",
    "site-packages",
    "bower_components",
    "obj",
    "dist",
    "temp",
    "tmp",
    "cache",
    "caches",
    "my music",
    "my pictures",
    "my videos",
];

/// File types Wisp never lists or opens: anything that runs code when opened.
const BLOCKED_EXTS: &[&str] = &[
    "exe",
    "msi",
    "msix",
    "msixbundle",
    "msp",
    "mst",
    "appx",
    "appxbundle",
    "appref-ms",
    "application",
    "bat",
    "cmd",
    "com",
    "scr",
    "pif",
    "cpl",
    "msc",
    "ps1",
    "psm1",
    "psd1",
    "ps1xml",
    "vb",
    "vbs",
    "vbe",
    "js",
    "jse",
    "mjs",
    "wsf",
    "wsh",
    "ws",
    "hta",
    "jar",
    "py",
    "pyw",
    "pyc",
    "lnk",
    "url",
    "website",
    "scf",
    "settingcontent-ms",
    "library-ms",
    "search-ms",
    "searchconnector-ms",
    "dll",
    "sys",
    "drv",
    "ocx",
    "reg",
    "inf",
    "gadget",
    "chm",
    "diagcab",
    "xll",
    "xbap",
    "sh",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FileKind {
    Folder,
    Pdf,
    Document,
    Sheet,
    Slides,
    Image,
    Video,
    Audio,
    Archive,
    Other,
}

impl FileKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Folder => "folder",
            Self::Pdf => "pdf",
            Self::Document => "document",
            Self::Sheet => "sheet",
            Self::Slides => "slides",
            Self::Image => "image",
            Self::Video => "video",
            Self::Audio => "audio",
            Self::Archive => "archive",
            Self::Other => "other",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "folder" => Self::Folder,
            "pdf" => Self::Pdf,
            "document" => Self::Document,
            "sheet" => Self::Sheet,
            "slides" => Self::Slides,
            "image" => Self::Image,
            "video" => Self::Video,
            "audio" => Self::Audio,
            "archive" => Self::Archive,
            "other" => Self::Other,
            _ => return None,
        })
    }

    /// The kind a word in a query asks for ("photos", "pdf", "songs").
    fn from_word(w: &str) -> Option<Self> {
        Some(match w {
            "folder" | "folders" | "directory" => Self::Folder,
            "pdf" | "pdfs" => Self::Pdf,
            "doc" | "docs" | "document" | "documents" | "word" => Self::Document,
            "sheet" | "sheets" | "spreadsheet" | "spreadsheets" | "excel" => Self::Sheet,
            "slides" | "deck" | "decks" | "presentation" | "presentations" | "powerpoint" => {
                Self::Slides
            }
            "photo" | "photos" | "picture" | "pictures" | "image" | "images" | "pic" | "pics" => {
                Self::Image
            }
            "video" | "videos" | "movie" | "movies" => Self::Video,
            "song" | "songs" | "music" | "audio" | "track" | "tracks" => Self::Audio,
            "zip" | "zips" | "archive" | "archives" => Self::Archive,
            _ => return None,
        })
    }
}

/// The kind of a file with extension `ext` (lowercase, no dot).
pub fn kind_for(ext: &str, is_dir: bool) -> FileKind {
    if is_dir {
        return FileKind::Folder;
    }
    match ext {
        "pdf" => FileKind::Pdf,
        "doc" | "docx" | "odt" | "rtf" | "txt" | "md" | "pages" | "epub" => FileKind::Document,
        "xls" | "xlsx" | "xlsm" | "ods" | "csv" | "numbers" => FileKind::Sheet,
        "ppt" | "pptx" | "odp" | "key" => FileKind::Slides,
        "jpg" | "jpeg" | "png" | "gif" | "bmp" | "webp" | "heic" | "heif" | "tif" | "tiff"
        | "svg" | "raw" | "cr2" | "nef" | "arw" | "dng" | "ico" | "avif" => FileKind::Image,
        "mp4" | "mkv" | "mov" | "avi" | "wmv" | "webm" | "m4v" | "flv" | "mpg" | "mpeg" | "3gp" => {
            FileKind::Video
        }
        "mp3" | "wav" | "flac" | "aac" | "m4a" | "ogg" | "wma" | "opus" | "aiff" | "mid"
        | "midi" => FileKind::Audio,
        "zip" | "rar" | "7z" | "tar" | "gz" | "bz2" | "xz" | "tgz" | "iso" => FileKind::Archive,
        _ => FileKind::Other,
    }
}

/// Lowercase extension of a file name ("" when there is none).
pub fn extension(name: &str) -> String {
    match name.rsplit_once('.') {
        Some((stem, ext)) if !stem.is_empty() && !ext.is_empty() && ext.len() <= 20 => {
            ext.to_ascii_lowercase()
        }
        _ => String::new(),
    }
}

/// Whether files of this type run code when opened (never listed or opened).
pub fn is_blocked_ext(ext: &str) -> bool {
    BLOCKED_EXTS.contains(&ext)
}

/// Whether a folder with this name is skipped.
pub fn skip_dir(name: &str) -> bool {
    name.starts_with('.')
        || name.starts_with('$')
        || name.starts_with('~')
        || SKIP_DIRS.contains(&name.to_ascii_lowercase().as_str())
}

/// Whether a file with this name is skipped (temp files, Windows' own).
pub fn skip_file(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    name.starts_with('.')
        || name.starts_with("~$")
        || lower == "desktop.ini"
        || lower == "thumbs.db"
        || lower.ends_with(".tmp")
        || lower.ends_with(".crdownload")
        || lower.ends_with(".partial")
        || is_blocked_ext(&extension(name))
}

/// A folder that is indexed, shown by its label ("Downloads").
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Root {
    pub path: PathBuf,
    pub label: String,
    /// Added by the user (as opposed to a standard Windows folder).
    pub user: bool,
}

impl Root {
    pub fn new(path: impl Into<PathBuf>, user: bool) -> Self {
        let path = path.into();
        let label = path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| path.to_string_lossy().into_owned());
        Self { path, label, user }
    }
}

/// Drop roots that don't exist or repeat another; keep order.
pub fn tidy_roots(roots: Vec<Root>) -> Vec<Root> {
    let mut out: Vec<Root> = Vec::new();
    for r in roots {
        let key = r.path.to_string_lossy().to_lowercase();
        if !r.path.is_dir()
            || out
                .iter()
                .any(|o| o.path.to_string_lossy().to_lowercase() == key)
        {
            continue;
        }
        out.push(r);
    }
    out
}

/// One indexed file or folder.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub path: String,
    pub name: String,
    /// Where it is, for display: root label and subfolders ("Downloads/Invoices").
    pub folder: String,
    pub root: String,
    pub kind: FileKind,
    pub size: u64,
    /// Unix seconds.
    pub modified: i64,
}

fn modified_secs(meta: &std::fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Hidden or system item (Windows attributes; dot-names elsewhere).
#[cfg(windows)]
pub fn is_hidden(meta: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const HIDDEN: u32 = 0x2;
    const SYSTEM: u32 = 0x4;
    meta.file_attributes() & (HIDDEN | SYSTEM) != 0
}

#[cfg(not(windows))]
pub fn is_hidden(_meta: &std::fs::Metadata) -> bool {
    false
}

/// The entry for `path` (a root itself, or anything below one).
pub fn entry_for(path: &Path, meta: &std::fs::Metadata, root: &Root) -> Option<Entry> {
    let name = path.file_name()?.to_string_lossy().into_owned();
    let is_dir = meta.is_dir();
    let ext = if is_dir {
        String::new()
    } else {
        extension(&name)
    };
    let rel = path.strip_prefix(&root.path).ok()?;
    let folder = match rel.parent() {
        // The root itself: shown by where it lives.
        None => String::new(),
        Some(p) if p.as_os_str().is_empty() => root.label.clone(),
        Some(p) => {
            let mut f = root.label.clone();
            for c in p.components() {
                f.push('/');
                f.push_str(&c.as_os_str().to_string_lossy());
            }
            f
        }
    };
    Some(Entry {
        path: path.to_string_lossy().into_owned(),
        name,
        folder,
        root: root.path.to_string_lossy().into_owned(),
        kind: kind_for(&ext, is_dir),
        size: if is_dir { 0 } else { meta.len() },
        modified: modified_secs(meta),
    })
}

/// Whether `path` (below `root`) may be in the index, by its names alone.
pub fn allowed_below(path: &Path, root: &Root, is_dir: bool) -> bool {
    let Ok(rel) = path.strip_prefix(&root.path) else {
        return false;
    };
    let parts: Vec<String> = rel
        .components()
        .map(|c| c.as_os_str().to_string_lossy().into_owned())
        .collect();
    if parts.len() > MAX_DEPTH + 1 {
        return false;
    }
    let Some((last, dirs)) = parts.split_last() else {
        return true; // the root itself
    };
    if dirs.iter().any(|d| skip_dir(d)) {
        return false;
    }
    if is_dir {
        !skip_dir(last)
    } else {
        !skip_file(last)
    }
}

fn same_path(a: &Path, b: &Path) -> bool {
    a.to_string_lossy().to_lowercase() == b.to_string_lossy().to_lowercase()
}

/// Walk `root`, calling `visit` for each entry (the root first) until it
/// returns false. Skips hidden and system items, the skip-list folders,
/// links and junctions, and folders listed in `exclude` (other roots,
/// walked on their own).
pub fn walk(root: &Root, exclude: &[PathBuf], visit: &mut dyn FnMut(Entry) -> bool) -> bool {
    walk_from(root, &root.path, exclude, visit)
}

/// [`walk`], starting at `start` (a folder inside `root`).
pub fn walk_from(
    root: &Root,
    start: &Path,
    exclude: &[PathBuf],
    visit: &mut dyn FnMut(Entry) -> bool,
) -> bool {
    let Ok(meta) = std::fs::metadata(start) else {
        return true;
    };
    let Ok(rel) = start.strip_prefix(&root.path) else {
        return true;
    };
    let depth = rel.components().count();
    if depth > MAX_DEPTH {
        return true;
    }
    if let Some(e) = entry_for(start, &meta, root) {
        if !visit(e) {
            return false;
        }
    }
    let mut stack = vec![(start.to_path_buf(), depth)];
    while let Some((dir, depth)) = stack.pop() {
        let Ok(read) = std::fs::read_dir(&dir) else {
            continue;
        };
        for item in read.flatten() {
            let Ok(ft) = item.file_type() else { continue };
            if ft.is_symlink() {
                continue;
            }
            let name = item.file_name().to_string_lossy().into_owned();
            let is_dir = ft.is_dir();
            if (is_dir && skip_dir(&name)) || (!is_dir && skip_file(&name)) {
                continue;
            }
            let Ok(meta) = item.metadata() else { continue };
            if is_hidden(&meta) {
                continue;
            }
            let path = item.path();
            if is_dir && exclude.iter().any(|x| same_path(x, &path)) {
                continue;
            }
            if let Some(e) = entry_for(&path, &meta, root) {
                if !visit(e) {
                    return false;
                }
            }
            if is_dir && depth < MAX_DEPTH {
                stack.push((path, depth + 1));
            }
        }
    }
    true
}

// ---------------------------------------------------------------------------
// Queries and ranking.

/// Words that say nothing about which file ("open my resume").
const FILLER: &[&str] = &[
    "my", "the", "a", "an", "file", "files", "called", "named", "of",
];

#[derive(Debug, Clone, PartialEq)]
pub struct Query {
    pub words: Vec<String>,
    pub kind: Option<FileKind>,
}

fn words_of(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Split a query into name words and an optional kind ("budget sheet" →
/// "budget", sheets only). A query of only kind words searches names
/// instead, so "music" still finds the Music folder.
pub fn parse_query(q: &str) -> Query {
    let all: Vec<String> = words_of(q)
        .into_iter()
        .filter(|w| !FILLER.contains(&w.as_str()))
        .take(8)
        .collect();
    let mut kind = None;
    let mut words = Vec::new();
    for w in &all {
        match FileKind::from_word(w) {
            Some(k) if kind.is_none() => kind = Some(k),
            _ => words.push(w.clone()),
        }
    }
    if words.is_empty() {
        return Query {
            words: all,
            kind: None,
        };
    }
    Query { words, kind }
}

/// A search result. The path stays in the host: the UI and the model only
/// ever see the id, the name and where it is.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileHit {
    pub id: i64,
    pub name: String,
    pub folder: String,
    pub kind: FileKind,
    pub size: u64,
    /// Unix milliseconds.
    pub modified: i64,
    #[serde(skip)]
    pub path: String,
    #[serde(skip)]
    pub root: String,
    #[serde(skip)]
    pub opened: u32,
    #[serde(skip)]
    pub last_opened: Option<i64>,
}

fn stem(name: &str) -> &str {
    match name.rsplit_once('.') {
        Some((s, _)) if !s.is_empty() => s,
        _ => name,
    }
}

/// How well `hit` matches `q` (higher is better); `now` in unix seconds.
pub fn score(hit: &FileHit, q: &Query, now: i64) -> f64 {
    let name = hit.name.to_lowercase();
    let st = stem(&name);
    let joined = q.words.join(" ");
    let name_words = words_of(&name);
    let in_name = |w: &String| name_words.iter().any(|n| n.starts_with(w.as_str()));
    let mut s = if st == joined || name == joined {
        1000.0
    } else if words_of(st).join(" ") == joined {
        900.0
    } else if st.starts_with(&joined) || words_of(st).join(" ").starts_with(&joined) {
        600.0
    } else if q.words.iter().all(in_name) {
        400.0
    } else {
        // Some words only match the folder it's in.
        150.0 + 50.0 * q.words.iter().filter(|w| in_name(w)).count() as f64
    };
    if hit.opened > 0 {
        s += 60.0 * (1.0 + hit.opened as f64).ln();
    }
    if let Some(at) = hit.last_opened {
        let days = ((now - at).max(0) as f64) / 86_400.0;
        s += 120.0 * (-days / 7.0).exp();
    }
    let age_days = ((now - hit.modified / 1000).max(0) as f64) / 86_400.0;
    s += 60.0 * (-age_days / 30.0).exp();
    let top = hit.folder.split('/').next().unwrap_or("");
    if matches!(top, "Desktop" | "Downloads" | "Documents") {
        s += 20.0;
    }
    s - (name.chars().count() as f64) * 0.5
}

/// FTS5 query: every word as a prefix ("res inv" → `"res"* "inv"*`).
fn fts_query(words: &[String]) -> String {
    words
        .iter()
        .map(|w| format!("\"{}\"*", w.replace('"', "")))
        .collect::<Vec<_>>()
        .join(" ")
}

// ---------------------------------------------------------------------------
// Storage.

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS files (
    id INTEGER PRIMARY KEY,
    path TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    folder TEXT NOT NULL,
    root TEXT NOT NULL,
    kind TEXT NOT NULL,
    size INTEGER NOT NULL,
    modified INTEGER NOT NULL,
    seen INTEGER NOT NULL,
    opened INTEGER NOT NULL DEFAULT 0,
    last_opened INTEGER
);
CREATE INDEX IF NOT EXISTS files_root ON files(root);
CREATE INDEX IF NOT EXISTS files_kind ON files(kind, modified);
CREATE VIRTUAL TABLE IF NOT EXISTS files_fts USING fts5(
    name, folder,
    content='files', content_rowid='id',
    tokenize='unicode61 remove_diacritics 2',
    prefix='2 3'
);
CREATE TRIGGER IF NOT EXISTS files_ad AFTER DELETE ON files BEGIN
    INSERT INTO files_fts(files_fts, rowid, name, folder) VALUES ('delete', old.id, old.name, old.folder);
END;
CREATE TABLE IF NOT EXISTS meta (key TEXT PRIMARY KEY, value INTEGER NOT NULL);
"#;

/// Keeps the full-text table in step with new rows (dropped while bulk loading).
const INSERT_TRIGGER: &str = r#"
CREATE TRIGGER IF NOT EXISTS files_ai AFTER INSERT ON files BEGIN
    INSERT INTO files_fts(rowid, name, folder) VALUES (new.id, new.name, new.folder);
END;
"#;

/// Schema version of the index file; a different one is rebuilt from scratch.
const INDEX_VERSION: u32 = 1;

const HIT_COLS: &str =
    "f.id, f.path, f.name, f.folder, f.root, f.kind, f.size, f.modified, f.opened, f.last_opened";

fn hit_from(r: &rusqlite::Row) -> rusqlite::Result<FileHit> {
    Ok(FileHit {
        id: r.get(0)?,
        path: r.get(1)?,
        name: r.get(2)?,
        folder: r.get(3)?,
        root: r.get(4)?,
        kind: FileKind::parse(&r.get::<_, String>(5)?).unwrap_or(FileKind::Other),
        size: r.get::<_, i64>(6)?.max(0) as u64,
        modified: r.get::<_, i64>(7)? * 1000,
        opened: r.get::<_, i64>(8)?.max(0) as u32,
        last_opened: r.get(9)?,
    })
}

pub struct FileIndex {
    conn: Mutex<Connection>,
}

impl FileIndex {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let conn = match Self::prepare(Connection::open(path)?) {
            Ok(c) => c,
            Err(_) => {
                // It's only a cache of names on disk: start over if it's damaged.
                let _ = std::fs::remove_file(path);
                Self::prepare(Connection::open(path)?)?
            }
        };
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn open_in_memory() -> Result<Self> {
        Ok(Self {
            conn: Mutex::new(Self::prepare(Connection::open_in_memory()?)?),
        })
    }

    fn prepare(conn: Connection) -> rusqlite::Result<Connection> {
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        let version: u32 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version != INDEX_VERSION {
            conn.execute_batch(
                "DROP TABLE IF EXISTS files_fts; DROP TABLE IF EXISTS files; DROP TABLE IF EXISTS meta;",
            )?;
        }
        conn.execute_batch(SCHEMA)?;
        let has_trigger: bool = conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'trigger' AND name = 'files_ai'",
            [],
            |r| r.get::<_, i64>(0).map(|n| n > 0),
        )?;
        if !has_trigger {
            // A bulk load was cut short: catch the search table up.
            Self::end_bulk_on(&conn)?;
        }
        conn.pragma_update(None, "user_version", INDEX_VERSION)?;
        Ok(conn)
    }

    fn with<T>(&self, f: impl FnOnce(&mut Connection) -> rusqlite::Result<T>) -> Result<T> {
        let mut conn = self
            .conn
            .lock()
            .map_err(|_| WispError::storage("The file index is unavailable"))?;
        f(&mut conn).map_err(Into::into)
    }

    /// Start loading many new rows: the search table is built once at the
    /// end ([`Self::end_bulk`]), which is several times faster than row by row.
    pub fn begin_bulk(&self) -> Result<()> {
        self.with(|c| c.execute_batch("DROP TRIGGER IF EXISTS files_ai"))
    }

    pub fn end_bulk(&self) -> Result<()> {
        self.with(|c| Self::end_bulk_on(c))
    }

    fn end_bulk_on(c: &Connection) -> rusqlite::Result<()> {
        c.execute_batch("INSERT INTO files_fts(files_fts) VALUES ('rebuild');")?;
        c.execute_batch(INSERT_TRIGGER)
    }

    /// Add or refresh entries, marking them seen in pass `seen`.
    pub fn upsert(&self, entries: &[Entry], seen: i64) -> Result<()> {
        self.with(|c| {
            let tx = c.transaction()?;
            {
                // Name and folder follow from the path, so an existing row
                // only ever changes size and date (and the FTS row stays).
                let mut stmt = tx.prepare_cached(
                    "INSERT INTO files (path, name, folder, root, kind, size, modified, seen)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                     ON CONFLICT(path) DO UPDATE SET size = excluded.size,
                        modified = excluded.modified, seen = excluded.seen",
                )?;
                for e in entries {
                    stmt.execute(params![
                        e.path,
                        e.name,
                        e.folder,
                        e.root,
                        e.kind.as_str(),
                        e.size as i64,
                        e.modified,
                        seen
                    ])?;
                }
            }
            tx.commit()
        })
    }

    /// Forget `path` and everything below it.
    pub fn remove(&self, path: &str) -> Result<u64> {
        let sep = if path.contains('\\') { '\\' } else { '/' };
        let below = format!("{}{sep}%", like_escape(path));
        self.with(|c| {
            c.execute(
                "DELETE FROM files WHERE path = ?1 OR path LIKE ?2 ESCAPE '^'",
                params![path, below],
            )
            .map(|n| n as u64)
        })
    }

    /// After a full pass: forget what that pass didn't see under `roots`,
    /// and anything under roots that are no longer indexed.
    pub fn finish_pass(&self, seen: i64, roots: &[String]) -> Result<u64> {
        self.with(|c| {
            let tx = c.transaction()?;
            let mut removed = 0;
            for r in roots {
                removed += tx.execute(
                    "DELETE FROM files WHERE root = ?1 AND seen < ?2",
                    params![r, seen],
                )?;
            }
            let all: Vec<String> = {
                let mut stmt = tx.prepare("SELECT DISTINCT root FROM files")?;
                let rows = stmt.query_map([], |r| r.get(0))?;
                rows.collect::<rusqlite::Result<_>>()?
            };
            for r in all.iter().filter(|r| !roots.contains(r)) {
                removed += tx.execute("DELETE FROM files WHERE root = ?1", params![r])?;
            }
            tx.execute(
                "INSERT INTO meta (key, value) VALUES ('last_full', ?1)
                 ON CONFLICT(key) DO UPDATE SET value = excluded.value",
                params![seen],
            )?;
            tx.commit()?;
            Ok(removed as u64)
        })
    }

    /// When the last full pass finished (unix milliseconds).
    pub fn last_full(&self) -> Result<Option<i64>> {
        self.with(|c| {
            c.query_row("SELECT value FROM meta WHERE key = 'last_full'", [], |r| {
                r.get(0)
            })
            .optional()
        })
    }

    pub fn count(&self) -> Result<u64> {
        self.with(|c| {
            c.query_row("SELECT COUNT(*) FROM files", [], |r| r.get::<_, i64>(0))
                .map(|n| n as u64)
        })
    }

    pub fn clear(&self) -> Result<()> {
        self.with(|c| {
            c.execute_batch(
                "DELETE FROM files; DELETE FROM meta;
                 INSERT INTO files_fts(files_fts) VALUES ('rebuild'); VACUUM;",
            )
        })
    }

    pub fn get(&self, id: i64) -> Result<Option<FileHit>> {
        self.with(|c| {
            c.query_row(
                &format!("SELECT {HIT_COLS} FROM files f WHERE f.id = ?1"),
                params![id],
                hit_from,
            )
            .optional()
        })
    }

    /// Count an open from Wisp (it ranks higher next time).
    pub fn record_open(&self, id: i64, now: i64) -> Result<()> {
        self.with(|c| {
            c.execute(
                "UPDATE files SET opened = opened + 1, last_opened = ?2 WHERE id = ?1",
                params![id, now],
            )
            .map(|_| ())
        })
    }

    /// The best matches for `query`, best first. `now` in unix seconds.
    pub fn search(&self, query: &str, limit: usize, now: i64) -> Result<Vec<FileHit>> {
        let q = parse_query(query);
        if q.words.is_empty() {
            return Ok(Vec::new());
        }
        let kind = q.kind.map(FileKind::as_str);
        let any = fts_query(&q.words);
        // Names that start with the query come first; ranking every match
        // of a short, common prefix would be slow on a big index.
        let starts = format!("name : ^{any}");
        let mut hits = self.with(|c| {
            let mut out: Vec<FileHit> = Vec::new();
            for (m, extra, cap) in [
                (&starts, "", 300),
                (&any, "AND f.opened > 0", 100),
                (&any, "", 300),
            ] {
                let mut stmt = c.prepare_cached(&format!(
                    "SELECT {HIT_COLS} FROM files_fts JOIN files f ON f.id = files_fts.rowid
                     WHERE files_fts MATCH ?1 AND (?2 IS NULL OR f.kind = ?2) {extra} LIMIT {cap}"
                ))?;
                let rows = stmt.query_map(params![m, kind], hit_from)?;
                for h in rows {
                    let h = h?;
                    if !out.iter().any(|o| o.id == h.id) {
                        out.push(h);
                    }
                }
            }
            Ok(out)
        })?;
        let mut scored: Vec<(f64, FileHit)> =
            hits.drain(..).map(|h| (score(&h, &q, now), h)).collect();
        scored.sort_by(|a, b| b.0.total_cmp(&a.0));
        Ok(scored
            .into_iter()
            .take(limit.min(MAX_RESULTS))
            .map(|(_, h)| h)
            .collect())
    }
}

fn like_escape(s: &str) -> String {
    s.replace('^', "^^").replace('%', "^%").replace('_', "^_")
}

/// Whether `path` lies inside one of `roots` (component-wise, any case).
pub fn inside_roots(path: &Path, roots: &[Root]) -> bool {
    roots.iter().any(|r| {
        let (p, r) = (
            path.to_string_lossy().to_lowercase(),
            r.path.to_string_lossy().to_lowercase(),
        );
        let r = r.trim_end_matches(['/', '\\']);
        p == r
            || p.strip_prefix(r)
                .is_some_and(|rest| rest.starts_with(['/', '\\']))
    })
}

/// Check a hit right before opening it: still inside an indexed folder,
/// not a link, not a blocked type, and still there.
pub fn check_openable(hit: &FileHit, roots: &[Root]) -> Result<PathBuf> {
    let path = PathBuf::from(&hit.path);
    if path.components().any(|c| c.as_os_str() == "..") || !inside_roots(&path, roots) {
        return Err(WispError::invalid(
            "That file isn't in your indexed folders.",
        ));
    }
    let meta = std::fs::symlink_metadata(&path)
        .map_err(|_| WispError::invalid(format!("\"{}\" isn't there any more.", hit.name)))?;
    if meta.file_type().is_symlink() {
        return Err(WispError::invalid("Wisp doesn't open links."));
    }
    if !meta.is_dir() && is_blocked_ext(&extension(&hit.name)) {
        return Err(WispError::invalid("Wisp doesn't open programs or scripts."));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(name: &str, folder: &str) -> FileHit {
        FileHit {
            id: 1,
            name: name.into(),
            folder: folder.into(),
            kind: kind_for(&extension(name), false),
            size: 1,
            modified: 0,
            path: String::new(),
            root: String::new(),
            opened: 0,
            last_opened: None,
        }
    }

    #[test]
    fn kinds_and_rules() {
        assert_eq!(kind_for("pdf", false), FileKind::Pdf);
        assert_eq!(
            kind_for("DOCX".to_lowercase().as_str(), false),
            FileKind::Document
        );
        assert_eq!(kind_for("", true), FileKind::Folder);
        assert_eq!(extension("Resume.PDF"), "pdf");
        assert_eq!(extension(".gitignore"), "");
        assert_eq!(extension("noext"), "");
        for blocked in [
            "setup.exe",
            "run.BAT",
            "x.ps1",
            "a.lnk",
            "b.js",
            "c.hta",
            "d.url",
        ] {
            assert!(skip_file(blocked), "{blocked}");
        }
        for ok in ["Resume.pdf", "photo.JPG", "notes.txt", "data.csv"] {
            assert!(!skip_file(ok), "{ok}");
        }
        assert!(skip_file("~$draft.docx") && skip_file("desktop.ini") && skip_file("a.crdownload"));
        assert!(skip_dir("node_modules") && skip_dir(".git") && skip_dir("AppData"));
        assert!(skip_dir("$RECYCLE.BIN") && !skip_dir("Invoices"));
    }

    #[test]
    fn queries_split_kind_words() {
        let q = parse_query("open my budget sheet");
        assert_eq!(q.words, vec!["open", "budget"]);
        assert_eq!(q.kind, Some(FileKind::Sheet));
        let q = parse_query("Music");
        assert_eq!(q.words, vec!["music"]);
        assert_eq!(q.kind, None);
        let q = parse_query("trip photos 2024");
        assert_eq!(q.words, vec!["trip", "2024"]);
        assert_eq!(q.kind, Some(FileKind::Image));
        assert!(parse_query("  the  ").words.is_empty());
    }

    #[test]
    fn ranking_prefers_exact_then_prefix_then_folder() {
        let q = parse_query("resume");
        let now = 1_800_000_000;
        let exact = score(&hit("Resume.pdf", "Documents"), &q, now);
        let prefix = score(&hit("Resume 2023 old.pdf", "Documents"), &q, now);
        let word = score(&hit("My big resume.docx", "Documents"), &q, now);
        let folder = score(&hit("cover.docx", "Documents/Resume"), &q, now);
        assert!(exact > prefix && prefix > word && word > folder);
        let mut opened = hit("My big resume.docx", "Documents");
        opened.opened = 5;
        opened.last_opened = Some(now - 3600);
        assert!(score(&opened, &q, now) > prefix);
    }

    fn entry(path: &str, name: &str, folder: &str, kind: FileKind) -> Entry {
        Entry {
            path: path.into(),
            name: name.into(),
            folder: folder.into(),
            root: "/home/u/Documents".into(),
            kind,
            size: 10,
            modified: 1_700_000_000,
        }
    }

    #[test]
    fn search_finds_prefixes_and_filters_kinds() {
        let idx = FileIndex::open_in_memory().unwrap();
        idx.begin_bulk().unwrap();
        idx.upsert(
            &[
                entry(
                    "/home/u/Documents/Resume.pdf",
                    "Resume.pdf",
                    "Documents",
                    FileKind::Pdf,
                ),
                entry(
                    "/home/u/Documents/Invoices/Invoice_March.pdf",
                    "Invoice_March.pdf",
                    "Documents/Invoices",
                    FileKind::Pdf,
                ),
                entry(
                    "/home/u/Documents/budget.xlsx",
                    "budget.xlsx",
                    "Documents",
                    FileKind::Sheet,
                ),
                entry(
                    "/home/u/Documents/budget notes.txt",
                    "budget notes.txt",
                    "Documents",
                    FileKind::Document,
                ),
            ],
            1,
        )
        .unwrap();
        idx.end_bulk().unwrap();
        assert_eq!(idx.count().unwrap(), 4);
        let now = 1_700_000_000;
        assert_eq!(idx.search("res", 5, now).unwrap()[0].name, "Resume.pdf");
        assert_eq!(
            idx.search("inv mar", 5, now).unwrap()[0].name,
            "Invoice_March.pdf"
        );
        // Folder names match too.
        assert_eq!(
            idx.search("invoices", 5, now).unwrap()[0].name,
            "Invoice_March.pdf"
        );
        let sheets = idx.search("budget sheet", 5, now).unwrap();
        assert_eq!(sheets.len(), 1);
        assert_eq!(sheets[0].name, "budget.xlsx");
        assert_eq!(idx.search("budget", 5, now).unwrap().len(), 2);
        assert!(idx.search("nothing-like-it", 5, now).unwrap().is_empty());
        // Quotes and FTS syntax in a query are just text.
        assert!(idx.search("\" OR * NEAR(", 5, now).is_ok());

        // Opening a file counts, and it can be looked up by id.
        let id = idx.search("budget notes", 1, now).unwrap()[0].id;
        idx.record_open(id, now).unwrap();
        assert_eq!(idx.get(id).unwrap().unwrap().opened, 1);
        assert!(idx.get(9999).unwrap().is_none());
    }

    #[test]
    fn an_interrupted_bulk_load_is_caught_up_on_open() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("files.db");
        {
            let idx = FileIndex::open(&path).unwrap();
            idx.begin_bulk().unwrap();
            let e = entry(
                "/home/u/Documents/Resume.pdf",
                "Resume.pdf",
                "Documents",
                FileKind::Pdf,
            );
            idx.upsert(&[e], 1).unwrap();
            assert!(idx.search("resume", 5, 0).unwrap().is_empty());
        }
        let idx = FileIndex::open(&path).unwrap();
        assert_eq!(idx.search("resume", 5, 0).unwrap().len(), 1);
        // New rows are searchable straight away again.
        let e = entry(
            "/home/u/Documents/Notes.txt",
            "Notes.txt",
            "Documents",
            FileKind::Document,
        );
        idx.upsert(&[e], 2).unwrap();
        assert_eq!(idx.search("notes", 5, 0).unwrap().len(), 1);
    }

    #[test]
    fn passes_update_and_forget() {
        let idx = FileIndex::open_in_memory().unwrap();
        let root = "/home/u/Documents".to_string();
        let mut a = entry(
            "/home/u/Documents/a.txt",
            "a.txt",
            "Documents",
            FileKind::Document,
        );
        let b = entry(
            "/home/u/Documents/sub",
            "sub",
            "Documents",
            FileKind::Folder,
        );
        let c = entry(
            "/home/u/Documents/sub/c.txt",
            "c.txt",
            "Documents/sub",
            FileKind::Document,
        );
        idx.upsert(&[a.clone(), b, c], 1).unwrap();
        // The next pass sees a changed `a` only: the rest is forgotten.
        a.size = 99;
        idx.upsert(&[a], 2).unwrap();
        assert_eq!(idx.finish_pass(2, std::slice::from_ref(&root)).unwrap(), 2);
        assert_eq!(idx.count().unwrap(), 1);
        assert_eq!(idx.search("a.txt", 1, 0).unwrap()[0].size, 99);
        assert!(idx.last_full().unwrap() == Some(2));
        // The FTS rows went with them.
        assert!(idx.search("c", 5, 0).unwrap().is_empty());

        // Removing a folder removes what's below it, not look-alikes.
        let d = entry(
            "/home/u/Documents/sub/d.txt",
            "d.txt",
            "Documents/sub",
            FileKind::Document,
        );
        let e = entry(
            "/home/u/Documents/sub2.txt",
            "sub2.txt",
            "Documents",
            FileKind::Document,
        );
        idx.upsert(&[d, e], 3).unwrap();
        assert_eq!(idx.remove("/home/u/Documents/sub").unwrap(), 1);
        assert_eq!(idx.count().unwrap(), 2);

        // A root that's no longer indexed is dropped entirely.
        idx.finish_pass(4, &[]).unwrap();
        assert_eq!(idx.count().unwrap(), 0);
        idx.clear().unwrap();
    }

    #[test]
    fn walks_a_tree_with_the_skip_rules() {
        let dir = std::env::temp_dir().join(format!("wisp-walk-{}", uuid::Uuid::new_v4()));
        let docs = dir.join("Documents");
        std::fs::create_dir_all(docs.join("Work/Deep")).unwrap();
        std::fs::create_dir_all(docs.join("node_modules/pkg")).unwrap();
        std::fs::create_dir_all(docs.join(".git")).unwrap();
        std::fs::create_dir_all(docs.join("Other")).unwrap();
        for f in [
            "Resume.pdf",
            "Work/Plan.docx",
            "Work/Deep/notes.txt",
            "node_modules/pkg/index.md",
            ".git/HEAD",
            "setup.exe",
            "script.ps1",
            "~$Plan.docx",
            "Other/inner.txt",
        ] {
            std::fs::write(docs.join(f), "x").unwrap();
        }
        let root = Root::new(&docs, false);
        assert_eq!(root.label, "Documents");
        let mut seen = Vec::new();
        walk(&root, &[docs.join("Other")], &mut |e| {
            seen.push((e.name, e.folder));
            true
        });
        seen.sort();
        let names: Vec<&str> = seen.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "Deep",
                "Documents",
                "Plan.docx",
                "Resume.pdf",
                "Work",
                "notes.txt"
            ]
        );
        let notes = seen.iter().find(|(n, _)| n == "notes.txt").unwrap();
        assert_eq!(notes.1, "Documents/Work/Deep");
        assert!(allowed_below(&docs.join("Work/x.pdf"), &root, false));
        assert!(!allowed_below(
            &docs.join("node_modules/x.pdf"),
            &root,
            false
        ));
        assert!(!allowed_below(&docs.join("x.exe"), &root, false));
        assert!(!allowed_below(&dir.join("elsewhere.pdf"), &root, false));

        // Opening re-checks the file.
        let idx = FileIndex::open_in_memory().unwrap();
        let meta = std::fs::metadata(docs.join("Resume.pdf")).unwrap();
        let e = entry_for(&docs.join("Resume.pdf"), &meta, &root).unwrap();
        idx.upsert(&[e], 1).unwrap();
        let h = idx.search("resume", 1, 0).unwrap().remove(0);
        let roots = vec![root.clone()];
        assert!(check_openable(&h, &roots).is_ok());
        assert!(check_openable(&h, &[Root::new(dir.join("Elsewhere"), true)]).is_err());
        let mut gone = h.clone();
        gone.path = docs.join("missing.pdf").to_string_lossy().into_owned();
        assert!(check_openable(&gone, &roots).is_err());
        let mut exe = h.clone();
        exe.name = "setup.exe".into();
        exe.path = docs.join("setup.exe").to_string_lossy().into_owned();
        assert!(check_openable(&exe, &roots).is_err());
        assert_eq!(tidy_roots(vec![root.clone(), root.clone()]).len(), 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
