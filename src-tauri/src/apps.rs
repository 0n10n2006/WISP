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

// ---------------------------------------------------------------------------
// Launcher catalogue: Start-menu apps, Steam games and built-in Windows apps.

use wisp_core::launcher::{self, LaunchItem};

#[cfg(windows)]
fn steam_from_registry() -> Option<PathBuf> {
    use windows::core::w;
    use windows::Win32::Foundation::ERROR_SUCCESS;
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_SZ};
    let mut buf = [0u16; 520];
    let mut size = (buf.len() * 2) as u32;
    let rc = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Valve\\Steam"),
            w!("SteamPath"),
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr() as *mut core::ffi::c_void),
            Some(&mut size),
        )
    };
    if rc != ERROR_SUCCESS {
        return None;
    }
    let len = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
    Some(PathBuf::from(String::from_utf16_lossy(&buf[..len])))
}

#[cfg(not(windows))]
fn steam_from_registry() -> Option<PathBuf> {
    None
}

fn steam_roots() -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = steam_from_registry().into_iter().collect();
    for var in ["ProgramFiles(x86)", "ProgramFiles"] {
        if let Some(base) = std::env::var_os(var) {
            roots.push(PathBuf::from(base).join("Steam"));
        }
    }
    roots
}

/// Installed Steam games from every library folder under `roots`.
pub fn steam_games(roots: &[PathBuf]) -> Vec<LaunchItem> {
    let mut libraries: Vec<PathBuf> = Vec::new();
    for root in roots {
        let vdf = root.join("steamapps").join("libraryfolders.vdf");
        if let Ok(text) = std::fs::read_to_string(&vdf) {
            libraries.push(root.clone());
            libraries.extend(
                launcher::steam_libraries(&text)
                    .into_iter()
                    .map(PathBuf::from),
            );
        }
    }
    libraries.sort();
    libraries.dedup();
    let mut games = Vec::new();
    for lib in libraries {
        let Ok(entries) = std::fs::read_dir(lib.join("steamapps")) else {
            continue;
        };
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().to_lowercase();
            if name.starts_with("appmanifest_") && name.ends_with(".acf") {
                if let Some(g) = std::fs::read_to_string(e.path())
                    .ok()
                    .and_then(|t| launcher::steam_game(&t))
                {
                    games.push(g);
                }
            }
        }
    }
    games.sort_by_key(|g| g.name.to_lowercase());
    games.dedup_by(|a, b| a.id == b.id);
    games
}

/// Everything the launcher can open, validated.
pub fn catalogue() -> Vec<LaunchItem> {
    let mut items: Vec<LaunchItem> = list()
        .into_iter()
        .map(|a| LaunchItem {
            id: format!("lnk:{}", a.target),
            name: title_case(&a.name),
            kind: "app".into(),
            target: a.target,
        })
        .collect();
    items.extend(steam_games(&steam_roots()));
    items.extend(launcher::builtin_items());
    items.retain(|i| launcher::validate_target(&i.target).is_ok());
    items
}

/// "visual studio code" → "Visual Studio Code" (Start-menu names are lowered for matching).
pub fn title_case(s: &str) -> String {
    s.split(' ')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
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
    fn finds_steam_games_across_libraries() {
        let root = tempfile::tempdir().unwrap();
        let lib2 = tempfile::tempdir().unwrap();
        let apps = root.path().join("steamapps");
        fs::create_dir_all(&apps).unwrap();
        fs::create_dir_all(lib2.path().join("steamapps")).unwrap();
        let vdf = format!(
            "\"libraryfolders\" {{ \"1\" {{ \"path\" \"{}\" }} }}",
            lib2.path().display().to_string().replace('\\', "\\\\")
        );
        fs::write(apps.join("libraryfolders.vdf"), vdf).unwrap();
        fs::write(
            apps.join("appmanifest_570.acf"),
            r#""AppState" { "appid" "570" "name" "Dota 2" }"#,
        )
        .unwrap();
        fs::write(
            lib2.path().join("steamapps").join("appmanifest_620.acf"),
            r#""AppState" { "appid" "620" "name" "Portal 2" }"#,
        )
        .unwrap();
        let games = steam_games(&[root.path().to_path_buf()]);
        let names: Vec<_> = games.iter().map(|g| g.name.as_str()).collect();
        assert_eq!(names, ["Dota 2", "Portal 2"]);
        assert_eq!(title_case("visual studio code"), "Visual Studio Code");
    }

    #[test]
    fn missing_dirs_are_ignored() {
        assert!(scan(&[PathBuf::from("/definitely/not/here")], |_| true).is_empty());
    }
}
