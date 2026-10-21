//! Launcher catalogue: what the command bar and Apps tab can open.
//!
//! Every item carries a `target` that is re-validated before launching, so a
//! tampered settings file or UI can only ever start: an allowlisted-style
//! program/shortcut path, a Steam game by numeric id, or one of the built-in
//! Windows app URIs listed here.

use crate::error::{Result, WispError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchItem {
    /// Stable id: `lnk:<path>`, `steam:<appid>` or `uri:<uri>`.
    pub id: String,
    pub name: String,
    /// "app" | "game" | "system"
    pub kind: String,
    pub target: String,
}

/// Built-in Windows apps and settings pages (protocol URIs, no paths).
pub const BUILTIN: &[(&str, &str)] = &[
    ("Calculator", "ms-calculator:"),
    ("Settings", "ms-settings:"),
    ("Photos", "ms-photos:"),
    ("Clock", "ms-clock:"),
    ("Camera", "microsoft.windows.camera:"),
    ("Microsoft Store", "ms-windows-store:"),
    ("Snipping Tool", "ms-screenclip:"),
    ("Display settings", "ms-settings:display"),
    ("Night light", "ms-settings:nightlight"),
    ("Sound settings", "ms-settings:sound"),
    ("Bluetooth & devices", "ms-settings:bluetooth"),
    ("Wi-Fi settings", "ms-settings:network-wifi"),
    ("Personalization", "ms-settings:personalization"),
    ("Background", "ms-settings:personalization-background"),
    ("Apps & features", "ms-settings:appsfeatures"),
    ("Windows Update", "ms-settings:windowsupdate"),
    ("Privacy: microphone", "ms-settings:privacy-microphone"),
    ("Battery & power", "ms-settings:powersleep"),
    ("Focus", "ms-settings:quiethours"),
    ("Notifications", "ms-settings:notifications"),
];

pub fn builtin_items() -> Vec<LaunchItem> {
    BUILTIN
        .iter()
        .map(|(name, uri)| LaunchItem {
            id: format!("uri:{uri}"),
            name: (*name).into(),
            kind: "system".into(),
            target: (*uri).into(),
        })
        .collect()
}

/// Validate a launch target before it is handed to the OS.
pub fn validate_target(target: &str) -> Result<()> {
    if BUILTIN.iter().any(|(_, uri)| *uri == target) {
        return Ok(());
    }
    if let Some(id) = target.strip_prefix("steam://rungameid/") {
        if !id.is_empty() && id.len() <= 20 && id.chars().all(|c| c.is_ascii_digit()) {
            return Ok(());
        }
        return Err(WispError::invalid("Invalid Steam game id"));
    }
    crate::tools::validate_app_target(target)
}

// ---------------------------------------------------------------------------
// Steam: libraryfolders.vdf + appmanifest_<id>.acf (Valve KeyValues text).

/// Minimal Valve KeyValues parser: returns flattened `a/b/c` → value pairs.
pub fn parse_vdf(text: &str) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    let mut path: Vec<String> = Vec::new();
    let mut pending_key: Option<String> = None;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '"' => {
                let mut s = String::new();
                while let Some(ch) = chars.next() {
                    match ch {
                        '\\' => {
                            if let Some(n) = chars.next() {
                                s.push(n);
                            }
                        }
                        '"' => break,
                        _ => s.push(ch),
                    }
                }
                match pending_key.take() {
                    None => pending_key = Some(s),
                    Some(k) => {
                        let mut full = path.clone();
                        full.push(k.to_ascii_lowercase());
                        out.insert(full.join("/"), s);
                    }
                }
            }
            '{' => {
                if let Some(k) = pending_key.take() {
                    path.push(k.to_ascii_lowercase());
                }
            }
            '}' => {
                path.pop();
                pending_key = None;
            }
            '/' if chars.peek() == Some(&'/') => {
                for ch in chars.by_ref() {
                    if ch == '\n' {
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// Library folder paths listed in `libraryfolders.vdf`.
pub fn steam_libraries(vdf: &str) -> Vec<String> {
    parse_vdf(vdf)
        .into_iter()
        .filter(|(k, _)| k.starts_with("libraryfolders/") && k.ends_with("/path"))
        .map(|(_, v)| v)
        .collect()
}

/// A game from an `appmanifest_*.acf` file, skipping tools and redistributables.
pub fn steam_game(acf: &str) -> Option<LaunchItem> {
    let kv = parse_vdf(acf);
    let appid = kv.get("appstate/appid")?.trim().to_string();
    let name = kv.get("appstate/name")?.trim().to_string();
    if appid.is_empty() || !appid.chars().all(|c| c.is_ascii_digit()) || name.is_empty() {
        return None;
    }
    let lower = name.to_ascii_lowercase();
    let skip = [
        "steamworks",
        "redistributable",
        "proton",
        "steam linux runtime",
        "dedicated server",
    ];
    if skip.iter().any(|s| lower.contains(s)) || appid == "228980" {
        return None;
    }
    Some(LaunchItem {
        id: format!("steam:{appid}"),
        name,
        kind: "game".into(),
        target: format!("steam://rungameid/{appid}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIBRARIES: &str = r#"
"libraryfolders"
{
	"0"
	{
		"path"		"C:\\Program Files (x86)\\Steam"
		"apps" { "228980" "0" "570" "123" }
	}
	"1"
	{
		"path"		"D:\\SteamLibrary"
	}
}"#;

    #[test]
    fn parses_libraries() {
        assert_eq!(
            steam_libraries(LIBRARIES),
            vec![r"C:\Program Files (x86)\Steam", r"D:\SteamLibrary"]
        );
    }

    #[test]
    fn parses_games_and_skips_tools() {
        let acf = r#""AppState" { "appid" "570" "name" "Dota 2" // comment
        "StateFlags" "4" }"#;
        let g = steam_game(acf).unwrap();
        assert_eq!(g.target, "steam://rungameid/570");
        assert_eq!(g.name, "Dota 2");
        assert!(steam_game(
            r#""AppState" { "appid" "228980" "name" "Steamworks Common Redistributables" }"#
        )
        .is_none());
        assert!(steam_game(r#""AppState" { "appid" "abc" "name" "X" }"#).is_none());
        assert!(steam_game("garbage").is_none());
    }

    #[test]
    fn validates_targets() {
        assert!(validate_target("ms-calculator:").is_ok());
        assert!(validate_target("steam://rungameid/570").is_ok());
        assert!(validate_target("steam://rungameid/570;calc").is_err());
        assert!(validate_target("ms-settings:../../x").is_err());
        assert!(validate_target("notepad.exe").is_ok());
        assert!(validate_target("cmd.exe").is_err());
        assert!(validate_target("https://example.com").is_err());
        assert!(builtin_items()
            .iter()
            .all(|i| validate_target(&i.target).is_ok()));
    }
}
