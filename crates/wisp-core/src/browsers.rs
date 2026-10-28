//! Browser history and bookmarks for the bar: found in the browsers' own
//! profile folders and read from copies (the browser keeps its files
//! locked). Read on demand, kept in memory only, never sent anywhere.

use crate::error::Result;
use rusqlite::{params, Connection, OpenFlags};
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Most history rows read from one profile (the most recent ones).
pub const MAX_HISTORY_PER_PROFILE: usize = 20_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Engine {
    Chromium,
    Firefox,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    /// "Chrome", "Edge", "Firefox"…
    pub browser: String,
    pub engine: Engine,
    pub dir: PathBuf,
}

/// Chromium browsers: name and `User Data` folder under `%LOCALAPPDATA%`.
const CHROMIUM: &[(&str, &str)] = &[
    ("Chrome", "Google/Chrome/User Data"),
    ("Edge", "Microsoft/Edge/User Data"),
    ("Brave", "BraveSoftware/Brave-Browser/User Data"),
    ("Vivaldi", "Vivaldi/User Data"),
    ("Chromium", "Chromium/User Data"),
];

/// Profiles of the browsers installed for this user. `local` is
/// `%LOCALAPPDATA%`, `roaming` is `%APPDATA%`.
pub fn find_profiles(local: &Path, roaming: &Path) -> Vec<Profile> {
    let mut out = Vec::new();
    for (name, rel) in CHROMIUM {
        let root = local.join(rel);
        let Ok(entries) = std::fs::read_dir(&root) else {
            continue;
        };
        let mut dirs: Vec<PathBuf> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                let n = p
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                (n == "Default" || n.starts_with("Profile ")) && p.join("History").is_file()
            })
            .collect();
        dirs.sort();
        out.extend(dirs.into_iter().map(|dir| Profile {
            browser: name.to_string(),
            engine: Engine::Chromium,
            dir,
        }));
    }
    // Opera keeps its single profile in Roaming.
    let opera = roaming.join("Opera Software/Opera Stable");
    if opera.join("History").is_file() {
        out.push(Profile {
            browser: "Opera".into(),
            engine: Engine::Chromium,
            dir: opera,
        });
    }
    let ff = roaming.join("Mozilla/Firefox");
    if let Ok(ini) = std::fs::read_to_string(ff.join("profiles.ini")) {
        for dir in firefox_profile_dirs(&ini, &ff) {
            if dir.join("places.sqlite").is_file() {
                out.push(Profile {
                    browser: "Firefox".into(),
                    engine: Engine::Firefox,
                    dir,
                });
            }
        }
    }
    out
}

/// Profile folders listed in Firefox's `profiles.ini`.
pub fn firefox_profile_dirs(ini: &str, base: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut path: Option<String> = None;
    let mut relative = true;
    let mut flush = |path: &mut Option<String>, relative: bool| {
        if let Some(p) = path.take() {
            let p = p.replace('/', std::path::MAIN_SEPARATOR_STR);
            let dir = if relative {
                base.join(p)
            } else {
                PathBuf::from(p)
            };
            if !out.contains(&dir) {
                out.push(dir);
            }
        }
    };
    for line in ini.lines().map(str::trim) {
        if line.starts_with('[') {
            flush(&mut path, relative);
            relative = true;
        } else if let Some(v) = line.strip_prefix("Path=") {
            path = Some(v.to_string());
        } else if let Some(v) = line.strip_prefix("IsRelative=") {
            relative = v.trim() != "0";
        }
    }
    flush(&mut path, relative);
    out
}

/// A page from history or bookmarks.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    /// Stable for the same URL (so a search result can be opened by id).
    pub id: i64,
    pub title: String,
    pub url: String,
    pub browser: String,
    /// "history" | "bookmark"
    pub kind: &'static str,
    pub visits: u32,
    /// Unix milliseconds (0 for bookmarks never visited).
    pub last_visit: i64,
}

/// A JavaScript-safe id for a URL (FNV-1a, 53 bits).
pub fn page_id(url: &str) -> i64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in url.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x0100_0000_01b3);
    }
    (h & 0x1F_FFFF_FFFF_FFFF) as i64
}

fn web_url(url: &str) -> bool {
    (url.starts_with("https://") || url.starts_with("http://")) && url.len() <= 2048
}

fn page(
    url: String,
    title: String,
    browser: &str,
    kind: &'static str,
    visits: u32,
    last: i64,
) -> Page {
    Page {
        id: page_id(&url),
        title: title.chars().take(300).collect(),
        url,
        browser: browser.to_string(),
        kind,
        visits,
        last_visit: last,
    }
}

/// Copy a database the browser has open (and its write-ahead log) to `dir`
/// and open the copy read-only.
fn open_copy(src: &Path, dir: &Path, name: &str) -> Result<(Connection, Vec<PathBuf>)> {
    std::fs::create_dir_all(dir)?;
    let db = dir.join(name);
    std::fs::copy(src, &db)?;
    let mut copies = vec![db.clone()];
    let wal = PathBuf::from(format!("{}-wal", src.display()));
    if wal.is_file() {
        let to = PathBuf::from(format!("{}-wal", db.display()));
        if std::fs::copy(&wal, &to).is_ok() {
            copies.push(to);
        }
    }
    copies.push(PathBuf::from(format!("{}-shm", db.display())));
    let conn = Connection::open_with_flags(&db, OpenFlags::SQLITE_OPEN_READ_WRITE)?;
    Ok((conn, copies))
}

fn remove_all(paths: &[PathBuf]) {
    for p in paths {
        let _ = std::fs::remove_file(p);
    }
}

/// Chromium stores times as microseconds since 1601.
fn chromium_ms(t: i64) -> i64 {
    if t <= 0 {
        0
    } else {
        t / 1000 - 11_644_473_600_000
    }
}

/// History and bookmarks of one profile. Copies go to `tmp` and are deleted
/// before returning.
pub fn read_profile(p: &Profile, tmp: &Path) -> Vec<Page> {
    let tag = format!("{:x}", page_id(&p.dir.to_string_lossy()));
    match p.engine {
        Engine::Chromium => {
            let mut out = Vec::new();
            if let Ok((conn, copies)) =
                open_copy(&p.dir.join("History"), tmp, &format!("h-{tag}.db"))
            {
                out = chromium_history(&conn, &p.browser).unwrap_or_default();
                drop(conn);
                remove_all(&copies);
            }
            if let Ok(text) = std::fs::read_to_string(p.dir.join("Bookmarks")) {
                out.extend(chromium_bookmarks(&text, &p.browser));
            }
            out
        }
        Engine::Firefox => {
            match open_copy(&p.dir.join("places.sqlite"), tmp, &format!("f-{tag}.db")) {
                Ok((conn, copies)) => {
                    let out = firefox_places(&conn, &p.browser).unwrap_or_default();
                    drop(conn);
                    remove_all(&copies);
                    out
                }
                Err(_) => Vec::new(),
            }
        }
    }
}

pub fn chromium_history(conn: &Connection, browser: &str) -> rusqlite::Result<Vec<Page>> {
    let mut stmt = conn.prepare(
        "SELECT url, title, visit_count, last_visit_time FROM urls
         WHERE hidden = 0 ORDER BY last_visit_time DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![MAX_HISTORY_PER_PROFILE as i64], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, Option<String>>(1)?.unwrap_or_default(),
            r.get::<_, i64>(2)?,
            r.get::<_, i64>(3)?,
        ))
    })?;
    Ok(rows
        .flatten()
        .filter(|(u, ..)| web_url(u))
        .map(|(u, t, v, l)| page(u, t, browser, "history", v.max(0) as u32, chromium_ms(l)))
        .collect())
}

/// Every URL in a Chromium `Bookmarks` file.
pub fn chromium_bookmarks(json: &str, browser: &str) -> Vec<Page> {
    fn walk(v: &serde_json::Value, browser: &str, out: &mut Vec<Page>) {
        if out.len() >= 5000 {
            return;
        }
        if v["type"] == "url" {
            if let Some(url) = v["url"].as_str().filter(|u| web_url(u)) {
                let title = v["name"].as_str().unwrap_or_default().to_string();
                out.push(page(url.to_string(), title, browser, "bookmark", 0, 0));
            }
        }
        if let Some(children) = v["children"].as_array() {
            for c in children {
                walk(c, browser, out);
            }
        }
        if let Some(roots) = v["roots"].as_object() {
            for r in roots.values() {
                walk(r, browser, out);
            }
        }
    }
    let mut out = Vec::new();
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(json) {
        walk(&v, browser, &mut out);
    }
    out
}

pub fn firefox_places(conn: &Connection, browser: &str) -> rusqlite::Result<Vec<Page>> {
    let mut out = Vec::new();
    let mut stmt = conn.prepare(
        "SELECT url, title, visit_count, last_visit_date FROM moz_places
         WHERE hidden = 0 AND visit_count > 0 ORDER BY last_visit_date DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![MAX_HISTORY_PER_PROFILE as i64], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, Option<String>>(1)?.unwrap_or_default(),
            r.get::<_, i64>(2)?,
            r.get::<_, Option<i64>>(3)?.unwrap_or(0),
        ))
    })?;
    out.extend(
        rows.flatten()
            .filter(|(u, ..)| web_url(u))
            .map(|(u, t, v, l)| page(u, t, browser, "history", v.max(0) as u32, l / 1000)),
    );
    let mut stmt = conn.prepare(
        "SELECT p.url, b.title FROM moz_bookmarks b JOIN moz_places p ON b.fk = p.id
         WHERE b.type = 1 LIMIT 5000",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok((
            r.get::<_, String>(0)?,
            r.get::<_, Option<String>>(1)?.unwrap_or_default(),
        ))
    })?;
    out.extend(
        rows.flatten()
            .filter(|(u, _)| web_url(u))
            .map(|(u, t)| page(u, t, browser, "bookmark", 0, 0)),
    );
    Ok(out)
}

/// One entry per URL: a bookmark wins over history, and visits add up.
pub fn merge(pages: Vec<Page>) -> Vec<Page> {
    let mut by_id: std::collections::HashMap<i64, Page> = std::collections::HashMap::new();
    for p in pages {
        match by_id.get_mut(&p.id) {
            Some(e) => {
                e.visits = e.visits.saturating_add(p.visits);
                e.last_visit = e.last_visit.max(p.last_visit);
                if p.kind == "bookmark" {
                    e.kind = "bookmark";
                    if !p.title.is_empty() {
                        e.title = p.title;
                    }
                } else if e.title.is_empty() {
                    e.title = p.title;
                }
            }
            None => {
                by_id.insert(p.id, p);
            }
        }
    }
    by_id.into_values().collect()
}

fn words(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_owned)
        .collect()
}

/// "https://www.github.com/x" → "github.com".
pub fn host(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let h = rest.split(['/', '?', '#']).next().unwrap_or("");
    h.strip_prefix("www.").unwrap_or(h)
}

/// Pages whose title or address match every word of `query`, best first.
/// `now` in unix milliseconds.
pub fn search(pages: &[Page], query: &str, limit: usize, now: i64) -> Vec<Page> {
    let q = words(query);
    if q.is_empty() {
        return Vec::new();
    }
    let joined = q.join(" ");
    let mut scored: Vec<(f64, &Page)> = pages
        .iter()
        .filter_map(|p| {
            let title = words(&p.title);
            let h = host(&p.url).to_lowercase();
            let addr = words(&h);
            let path = words(&p.url);
            let all_in = |set: &[String]| {
                q.iter()
                    .all(|w| set.iter().any(|t| t.starts_with(w.as_str())))
            };
            let mut s = if h.starts_with(&joined) {
                700.0
            } else if all_in(&title) {
                if p.title.to_lowercase().starts_with(&joined) {
                    600.0
                } else {
                    450.0
                }
            } else if all_in(&addr)
                || q.iter()
                    .all(|w| title.iter().chain(&addr).any(|t| t.starts_with(w.as_str())))
            {
                400.0
            } else if all_in(&path) {
                200.0
            } else {
                return None;
            };
            s += 25.0 * (1.0 + p.visits as f64).ln();
            if p.last_visit > 0 {
                let days = ((now - p.last_visit).max(0) as f64) / 86_400_000.0;
                s += 80.0 * (-days / 14.0).exp();
            }
            if p.kind == "bookmark" {
                s += 60.0;
            }
            Some((s, p))
        })
        .collect();
    scored.sort_by(|a, b| b.0.total_cmp(&a.0));
    scored
        .into_iter()
        .take(limit)
        .map(|(_, p)| p.clone())
        .collect()
}

/// Browsers whose windows can hold tabs, by process name.
pub fn browser_for_process(process: &str) -> Option<&'static str> {
    Some(match process {
        "chrome" => "Chrome",
        "msedge" => "Edge",
        "brave" => "Brave",
        "vivaldi" => "Vivaldi",
        "opera" => "Opera",
        "firefox" => "Firefox",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_profiles_of_each_browser() {
        let dir = tempfile::tempdir().unwrap();
        let (local, roaming) = (dir.path().join("Local"), dir.path().join("Roaming"));
        for p in [
            "Google/Chrome/User Data/Default",
            "Google/Chrome/User Data/Profile 2",
        ] {
            std::fs::create_dir_all(local.join(p)).unwrap();
            std::fs::write(local.join(p).join("History"), "").unwrap();
        }
        // A folder without history isn't a profile.
        std::fs::create_dir_all(local.join("Microsoft/Edge/User Data/System Profile")).unwrap();
        let ff = roaming.join("Mozilla/Firefox");
        std::fs::create_dir_all(ff.join("Profiles/abc.default")).unwrap();
        std::fs::write(ff.join("Profiles/abc.default/places.sqlite"), "").unwrap();
        std::fs::write(
            ff.join("profiles.ini"),
            "[General]\nStartWithLastProfile=1\n\n[Profile0]\nName=default\nIsRelative=1\nPath=Profiles/abc.default\n\n[Profile1]\nIsRelative=1\nPath=Profiles/missing\n",
        )
        .unwrap();
        let found = find_profiles(&local, &roaming);
        let names: Vec<_> = found.iter().map(|p| p.browser.as_str()).collect();
        assert_eq!(names, vec!["Chrome", "Chrome", "Firefox"]);
        assert!(found[2].dir.ends_with("abc.default"));
        let abs = firefox_profile_dirs("[Profile0]\nIsRelative=0\nPath=D:/ff\n", Path::new("/x"));
        assert_eq!(
            abs,
            vec![PathBuf::from(
                "D:/ff".replace('/', std::path::MAIN_SEPARATOR_STR)
            )]
        );
    }

    #[test]
    fn reads_chromium_and_firefox_data() {
        let dir = tempfile::tempdir().unwrap();
        // Chromium history on a copy of a "locked" file.
        let profile = dir.path().join("Default");
        std::fs::create_dir_all(&profile).unwrap();
        {
            let c = Connection::open(profile.join("History")).unwrap();
            c.execute_batch(
                "CREATE TABLE urls (id INTEGER PRIMARY KEY, url TEXT, title TEXT, visit_count INTEGER, last_visit_time INTEGER, hidden INTEGER DEFAULT 0);
                 INSERT INTO urls (url, title, visit_count, last_visit_time) VALUES
                   ('https://github.com/0n10n2006/WISP', 'Wisp on GitHub', 12, 13370000000000000),
                   ('chrome://settings', 'Settings', 3, 13370000000000000),
                   ('file:///C:/secret.txt', 'secret', 1, 13370000000000000);",
            )
            .unwrap();
        }
        std::fs::write(
            profile.join("Bookmarks"),
            r#"{"roots":{"bookmark_bar":{"type":"folder","children":[{"type":"url","name":"Rust","url":"https://www.rust-lang.org/"},{"type":"folder","children":[{"type":"url","name":"JS","url":"javascript:alert(1)"}]}]}}}"#,
        )
        .unwrap();
        let tmp = dir.path().join("tmp");
        let p = Profile {
            browser: "Chrome".into(),
            engine: Engine::Chromium,
            dir: profile.clone(),
        };
        let pages = read_profile(&p, &tmp);
        assert_eq!(pages.len(), 2, "{pages:?}");
        assert_eq!(pages[0].title, "Wisp on GitHub");
        assert_eq!(pages[0].visits, 12);
        // 13370000000000000 µs since 1601 ≈ 2024-09.
        assert!(pages[0].last_visit > 1_700_000_000_000 && pages[0].last_visit < 1_800_000_000_000);
        assert_eq!(pages[1].kind, "bookmark");
        // The copies are gone.
        assert_eq!(std::fs::read_dir(&tmp).unwrap().count(), 0);

        // Firefox places.
        let ff = dir.path().join("ff");
        std::fs::create_dir_all(&ff).unwrap();
        {
            let c = Connection::open(ff.join("places.sqlite")).unwrap();
            c.execute_batch(
                "CREATE TABLE moz_places (id INTEGER PRIMARY KEY, url TEXT, title TEXT, visit_count INTEGER, last_visit_date INTEGER, hidden INTEGER DEFAULT 0);
                 CREATE TABLE moz_bookmarks (id INTEGER PRIMARY KEY, type INTEGER, fk INTEGER, title TEXT);
                 INSERT INTO moz_places (url, title, visit_count, last_visit_date) VALUES ('https://news.ycombinator.com/', 'Hacker News', 40, 1760000000000000);
                 INSERT INTO moz_places (url, title, visit_count, last_visit_date) VALUES ('https://docs.rs/', 'Docs.rs', 0, NULL);
                 INSERT INTO moz_bookmarks (type, fk, title) VALUES (1, 2, 'Docs');",
            )
            .unwrap();
        }
        let p = Profile {
            browser: "Firefox".into(),
            engine: Engine::Firefox,
            dir: ff,
        };
        let pages = read_profile(&p, &tmp);
        assert_eq!(pages.len(), 2);
        assert_eq!(pages[0].last_visit, 1_760_000_000_000);
        assert_eq!(pages[1].title, "Docs");
        assert_eq!(pages[1].kind, "bookmark");
    }

    fn pg(url: &str, title: &str, kind: &'static str, visits: u32, last: i64) -> Page {
        page(url.into(), title.into(), "Chrome", kind, visits, last)
    }

    #[test]
    fn merges_and_ranks_pages() {
        let now = 1_760_000_000_000;
        let merged = merge(vec![
            pg("https://github.com/", "GitHub", "history", 50, now - 1000),
            pg("https://github.com/", "", "history", 5, now - 5000),
            pg("https://github.com/", "My GitHub", "bookmark", 0, 0),
            pg(
                "https://example.com/github-tips",
                "Tips",
                "history",
                1,
                now - 86_400_000 * 300,
            ),
            pg(
                "https://news.ycombinator.com/",
                "Hacker News",
                "history",
                30,
                now,
            ),
            pg(
                "https://www.rust-lang.org/learn",
                "Learn Rust",
                "history",
                2,
                now,
            ),
        ]);
        assert_eq!(merged.len(), 4);
        let gh = merged
            .iter()
            .find(|p| p.url == "https://github.com/")
            .unwrap();
        assert_eq!(
            (gh.visits, gh.kind, gh.title.as_str()),
            (55, "bookmark", "My GitHub")
        );

        let r = search(&merged, "git", 5, now);
        assert_eq!(r[0].url, "https://github.com/");
        assert_eq!(r.len(), 2);
        assert_eq!(search(&merged, "hacker", 5, now)[0].title, "Hacker News");
        assert_eq!(search(&merged, "rust learn", 5, now)[0].title, "Learn Rust");
        assert!(search(&merged, "nothing here", 5, now).is_empty());
        assert!(search(&merged, "  ", 5, now).is_empty());
        assert_eq!(host("https://www.rust-lang.org/learn?x"), "rust-lang.org");
        assert_eq!(page_id("a"), page_id("a"));
        assert!(page_id("https://x") < (1i64 << 53));
        assert_eq!(browser_for_process("msedge"), Some("Edge"));
        assert_eq!(browser_for_process("notepad"), None);
    }
}
