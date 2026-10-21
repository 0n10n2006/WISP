//! Discover installed apps from Start-menu shortcuts so users can allowlist
//! them without typing paths. Only `.lnk` files that pass the same validation
//! as manual entries are offered.

use serde::Serialize;
use std::path::{Path, PathBuf};

const MAX_DEPTH: usize = 4;
const MAX_APPS: usize = 600;
const SKIP_WORDS: [&str; 6] = [
    "uninstall",
    "readme",
    "help",
    "license",
    "release notes",
    "website",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledApp {
    pub name: String,
    pub target: String,
}

pub fn start_menu_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    for var in ["ProgramData", "APPDATA"] {
        if let Some(base) = std::env::var_os(var) {
            dirs.push(
                PathBuf::from(base)
                    .join("Microsoft")
                    .join("Windows")
                    .join("Start Menu")
                    .join("Programs"),
            );
        }
    }
    dirs
}

fn walk(dir: &Path, depth: usize, out: &mut Vec<PathBuf>) {
    if depth > MAX_DEPTH || out.len() >= MAX_APPS * 2 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        match e.file_type() {
            Ok(t) if t.is_dir() => walk(&p, depth + 1, out),
            Ok(t)
                if t.is_file() && p.extension().is_some_and(|x| x.eq_ignore_ascii_case("lnk")) =>
            {
                out.push(p);
            }
            _ => {}
        }
    }
}

/// Scan `dirs` for shortcuts accepted by `valid`, deduplicated by name.
pub fn scan(dirs: &[PathBuf], valid: impl Fn(&str) -> bool) -> Vec<InstalledApp> {
    let mut files = Vec::new();
    for d in dirs {
        walk(d, 0, &mut files);
    }
    let mut apps: Vec<InstalledApp> = files
        .into_iter()
        .filter_map(|p| {
            let name = p.file_stem()?.to_string_lossy().trim().to_lowercase();
            let target = p.to_string_lossy().into_owned();
            let skip = name.is_empty()
                || name.chars().count() > 60
                || SKIP_WORDS.iter().any(|w| name.contains(w));
            (!skip && valid(&target)).then_some(InstalledApp { name, target })
        })
        .collect();
    apps.sort_by(|a, b| a.name.cmp(&b.name));
    apps.dedup_by(|a, b| a.name == b.name);
    apps.truncate(MAX_APPS);
    apps
}

pub fn list() -> Vec<InstalledApp> {
    scan(&start_menu_dirs(), |t| {
        wisp_core::tools::validate_app_target(t).is_ok()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn finds_shortcuts_and_skips_noise() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("Tools/Deep")).unwrap();
        for f in [
            "Notepad++.lnk",
            "Tools/Spotify.lnk",
            "Tools/Deep/Paint.NET.LNK",
            "Uninstall Foo.lnk",
            "Read Me.txt",
            "Tools/Q&A.lnk",
        ] {
            fs::write(root.join(f), b"").unwrap();
        }
        // Second directory with a duplicate name.
        let other = tempfile::tempdir().unwrap();
        fs::write(other.path().join("spotify.lnk"), b"").unwrap();

        let apps = scan(&[root.to_path_buf(), other.path().to_path_buf()], |t| {
            !t.contains('&')
        });
        let names: Vec<_> = apps.iter().map(|a| a.name.as_str()).collect();
        assert_eq!(names, ["notepad++", "paint.net", "spotify"]);
        assert!(apps
            .iter()
            .all(|a| a.target.to_lowercase().ends_with(".lnk")));
    }

    #[test]
    fn missing_dirs_are_ignored() {
        assert!(scan(&[PathBuf::from("/definitely/not/here")], |_| true).is_empty());
    }
}
