//! Parsing, normalisation and conflict detection for global shortcuts.
//!
//! Accelerators use the Tauri syntax, e.g. `Ctrl+Shift+Space`. Wisp requires
//! at least one modifier other than Shift so global shortcuts don't swallow
//! normal typing.

use crate::error::{Result, WispError};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Shortcuts {
    pub toggle_island: String,
    pub select_to_ask: String,
    pub dictation: String,
    pub clipboard_assist: String,
    pub voice: String,
    /// Tuck the notch away / bring it back.
    pub tuck: String,
}

impl Default for Shortcuts {
    fn default() -> Self {
        Self {
            toggle_island: "Ctrl+Alt+Space".into(),
            select_to_ask: "Ctrl+Shift+S".into(),
            dictation: "Ctrl+Shift+D".into(),
            clipboard_assist: "Ctrl+Shift+C".into(),
            voice: "Ctrl+Alt+V".into(),
            tuck: "Ctrl+Alt+H".into(),
        }
    }
}

impl Shortcuts {
    pub fn entries(&self) -> [(&'static str, &str); 6] {
        [
            ("toggleIsland", &self.toggle_island),
            ("selectToAsk", &self.select_to_ask),
            ("dictation", &self.dictation),
            ("clipboardAssist", &self.clipboard_assist),
            ("voice", &self.voice),
            ("tuck", &self.tuck),
        ]
    }

    /// Normalise every accelerator and reject invalid or duplicate ones.
    /// Empty strings mean "disabled" and are allowed.
    pub fn validated(&self) -> Result<Self> {
        let mut out = self.clone();
        let mut seen: BTreeMap<String, &'static str> = BTreeMap::new();
        let fields: [(&'static str, &mut String); 6] = [
            ("toggleIsland", &mut out.toggle_island),
            ("selectToAsk", &mut out.select_to_ask),
            ("dictation", &mut out.dictation),
            ("clipboardAssist", &mut out.clipboard_assist),
            ("voice", &mut out.voice),
            ("tuck", &mut out.tuck),
        ];
        for (name, value) in fields {
            if value.trim().is_empty() {
                value.clear();
                continue;
            }
            let normal = normalize(value)
                .map_err(|e| WispError::invalid(format!("Shortcut for {name}: {}", e.message)))?;
            if let Some(other) = seen.insert(normal.clone(), name) {
                return Err(WispError::invalid(format!(
                    "{normal} is assigned to both {other} and {name}"
                )));
            }
            *value = normal;
        }
        Ok(out)
    }
}

const MODIFIERS: [(&str, &[&str]); 4] = [
    (
        "Ctrl",
        &["ctrl", "control", "cmdorctrl", "commandorcontrol"],
    ),
    ("Alt", &["alt", "option"]),
    ("Shift", &["shift"]),
    ("Super", &["super", "win", "meta", "cmd", "command"]),
];

/// Windows reserves some combinations; registering them fails or is hostile.
const RESERVED: [&str; 6] = [
    "Ctrl+Alt+Delete",
    "Alt+Tab",
    "Alt+F4",
    "Ctrl+Shift+Escape",
    "Super+L",
    "Ctrl+Escape",
];

fn normalize_key(raw: &str) -> Option<String> {
    let k = raw.trim();
    let lower = k.to_ascii_lowercase();
    if k.len() == 1 {
        let c = k.chars().next()?;
        if c.is_ascii_alphanumeric() {
            return Some(c.to_ascii_uppercase().to_string());
        }
        return match c {
            '.' => Some("Period".into()),
            ',' => Some("Comma".into()),
            '/' => Some("Slash".into()),
            ';' => Some("Semicolon".into()),
            '\'' => Some("Quote".into()),
            '[' => Some("BracketLeft".into()),
            ']' => Some("BracketRight".into()),
            '-' => Some("Minus".into()),
            '=' => Some("Equal".into()),
            '`' => Some("Backquote".into()),
            _ => None,
        };
    }
    if let Some(n) = lower.strip_prefix('f') {
        if let Ok(n) = n.parse::<u8>() {
            if (1..=24).contains(&n) {
                return Some(format!("F{n}"));
            }
        }
    }
    let named = [
        "Space",
        "Enter",
        "Tab",
        "Backspace",
        "Escape",
        "Delete",
        "Insert",
        "Home",
        "End",
        "PageUp",
        "PageDown",
        "ArrowUp",
        "ArrowDown",
        "ArrowLeft",
        "ArrowRight",
        "Period",
        "Comma",
        "Slash",
        "Semicolon",
        "Quote",
        "BracketLeft",
        "BracketRight",
        "Minus",
        "Equal",
        "Backquote",
    ];
    let alias = match lower.as_str() {
        "esc" => "escape",
        "return" => "enter",
        "del" => "delete",
        "up" => "arrowup",
        "down" => "arrowdown",
        "left" => "arrowleft",
        "right" => "arrowright",
        other => other,
    };
    named
        .iter()
        .find(|n| n.to_ascii_lowercase() == alias)
        .map(|n| n.to_string())
}

/// Normalise an accelerator to canonical form `Ctrl+Alt+Shift+Super+Key`.
pub fn normalize(accel: &str) -> Result<String> {
    let parts: Vec<&str> = accel.split('+').map(str::trim).collect();
    if parts.iter().any(|p| p.is_empty()) {
        return Err(WispError::invalid("Shortcut is incomplete"));
    }
    let (key_part, mod_parts) = parts
        .split_last()
        .ok_or_else(|| WispError::invalid("Shortcut is empty"))?;
    let mut mods = [false; 4];
    for m in mod_parts {
        let lower = m.to_ascii_lowercase();
        let idx = MODIFIERS
            .iter()
            .position(|(_, aliases)| aliases.contains(&lower.as_str()))
            .ok_or_else(|| WispError::invalid(format!("Unknown modifier \"{m}\"")))?;
        if mods[idx] {
            return Err(WispError::invalid(format!("Modifier \"{m}\" repeated")));
        }
        mods[idx] = true;
    }
    let key = normalize_key(key_part)
        .ok_or_else(|| WispError::invalid(format!("Unsupported key \"{key_part}\"")))?;
    let has_strong_modifier = mods[0] || mods[1] || mods[3];
    let is_function_key = key.starts_with('F') && key.len() > 1 && key[1..].parse::<u8>().is_ok();
    if !has_strong_modifier && !is_function_key {
        return Err(WispError::invalid(
            "Use Ctrl, Alt or Win so the shortcut doesn't interfere with typing",
        ));
    }
    let mut out: Vec<&str> = MODIFIERS
        .iter()
        .zip(mods)
        .filter(|(_, on)| *on)
        .map(|((name, _), _)| *name)
        .collect();
    out.push(&key);
    let joined = out.join("+");
    if RESERVED.contains(&joined.as_str()) {
        return Err(WispError::invalid(format!(
            "{joined} is reserved by Windows"
        )));
    }
    Ok(joined)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_order_and_aliases() {
        assert_eq!(
            normalize("shift+control+space").unwrap(),
            "Ctrl+Shift+Space"
        );
        assert_eq!(normalize("Win+Alt+k").unwrap(), "Alt+Super+K");
        assert_eq!(normalize("CmdOrCtrl+.").unwrap(), "Ctrl+Period");
        assert_eq!(normalize("F9").unwrap(), "F9");
        assert!(normalize("ctrl+esc+").is_err());
    }

    #[test]
    fn rejects_bad_shortcuts() {
        assert!(normalize("A").is_err());
        assert!(normalize("Shift+A").is_err());
        assert!(normalize("Ctrl+Ctrl+A").is_err());
        assert!(normalize("Hyper+A").is_err());
        assert!(normalize("Ctrl+NotAKey").is_err());
        assert!(normalize("Alt+F4").is_err());
        assert!(normalize("").is_err());
    }

    #[test]
    fn detects_conflicts() {
        let s = Shortcuts {
            dictation: "ctrl+alt+space".into(),
            ..Shortcuts::default()
        };
        let err = s.validated().unwrap_err();
        assert!(err.message.contains("both"));
    }

    #[test]
    fn empty_means_disabled() {
        let s = Shortcuts {
            voice: "  ".into(),
            ..Shortcuts::default()
        };
        assert_eq!(s.validated().unwrap().voice, "");
    }

    #[test]
    fn defaults_are_valid() {
        let d = Shortcuts::default();
        assert_eq!(d.validated().unwrap(), d);
    }
}
