//! What's in front of the user, and whether Wisp may appear over it.
//!
//! The host gathers facts about the foreground window (fullscreen or not,
//! Windows' own "user is busy" state, the program's path and window class)
//! and this module decides. Pure and unit-tested so the rules are explicit.

use serde::Serialize;

/// Windows' `SHQueryUserNotificationState` result, simplified.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum BusyState {
    #[default]
    Normal,
    /// A full-screen app (often a game or video) is running.
    Busy,
    /// An exclusive-mode Direct3D app (a game) is running.
    D3dFullscreen,
    /// Presentation settings are on (slideshow, meeting share).
    Presentation,
}

#[derive(Debug, Clone, Default)]
pub struct ScreenFacts {
    /// The foreground window covers its whole monitor.
    pub fullscreen: bool,
    pub busy: BusyState,
    /// Full path of the foreground program (lower-case is fine).
    pub exe_path: String,
    /// Window class of the foreground window.
    pub class: String,
    /// The foreground window is on the same monitor as the notch.
    pub same_monitor: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Cover {
    Show,
    Fullscreen,
    Game,
    Presentation,
    /// The app is on the user's "never show over" list.
    Excluded,
}

impl Cover {
    pub fn hidden(self) -> bool {
        self != Cover::Show
    }
}

/// Install folders of game stores and launchers.
const GAME_DIRS: &[&str] = &[
    "\\steamapps\\common\\",
    "\\epic games\\",
    "\\riot games\\",
    "\\xboxgames\\",
    "\\windowsapps\\microsoft.minecraft",
    "\\gog galaxy\\games\\",
    "\\gog games\\",
    "\\ubisoft game launcher\\games\\",
    "\\ea games\\",
    "\\electronic arts\\",
    "\\battle.net\\",
    "\\blizzard\\",
    "\\rockstar games\\",
    "\\itch\\apps\\",
    "\\amazon games\\library\\",
    "\\games\\",
];

/// Window classes used by game engines.
const GAME_CLASSES: &[&str] = &[
    "unitywndclass",
    "unrealwindow",
    "sdl_app",
    "glfw30",
    "cryengine",
    "valve001",
    "riotwindowclass",
    "lwjgl",
    "godot",
    "ue4",
    "d3d window",
];

/// Process file name without extension, lower-case ("C:\\x\\Game.exe" → "game").
pub fn process_name(exe_path: &str) -> String {
    let file = exe_path.rsplit(['\\', '/']).next().unwrap_or(exe_path);
    let lower = file.to_lowercase();
    lower.trim_end_matches(".exe").to_string()
}

pub fn looks_like_game(exe_path: &str, class: &str) -> bool {
    let p = exe_path.to_lowercase();
    let c = class.to_lowercase();
    GAME_DIRS.iter().any(|d| p.contains(d)) || GAME_CLASSES.iter().any(|g| c.starts_with(g))
}

pub struct Rules<'a> {
    pub hide_fullscreen: bool,
    pub hide_in_games: bool,
    /// Process names (without .exe) Wisp must never appear over.
    pub never_over: &'a [String],
}

/// Should the notch stay out of the way right now?
pub fn decide(f: &ScreenFacts, r: &Rules) -> Cover {
    let name = process_name(&f.exe_path);
    if f.same_monitor && !name.is_empty() && r.never_over.iter().any(|n| n == &name) {
        return Cover::Excluded;
    }
    if r.hide_in_games {
        if f.busy == BusyState::D3dFullscreen {
            return Cover::Game;
        }
        if f.same_monitor && looks_like_game(&f.exe_path, &f.class) {
            return Cover::Game;
        }
    }
    if r.hide_fullscreen {
        if f.busy == BusyState::Presentation {
            return Cover::Presentation;
        }
        if f.same_monitor && f.fullscreen {
            return Cover::Fullscreen;
        }
    }
    Cover::Show
}

/// A game is in front anywhere: hold-to-talk and shortcuts stay quiet.
pub fn game_active(f: &ScreenFacts, r: &Rules) -> bool {
    r.hide_in_games
        && (f.busy == BusyState::D3dFullscreen || looks_like_game(&f.exe_path, &f.class))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(list: &[String]) -> Rules<'_> {
        Rules {
            hide_fullscreen: true,
            hide_in_games: true,
            never_over: list,
        }
    }

    fn facts(path: &str, class: &str, fullscreen: bool, same: bool) -> ScreenFacts {
        ScreenFacts {
            fullscreen,
            busy: BusyState::Normal,
            exe_path: path.into(),
            class: class.into(),
            same_monitor: same,
        }
    }

    #[test]
    fn windowed_games_are_detected_by_folder_or_engine() {
        let r = rules(&[]);
        let steam = facts(
            r"C:\Program Files (x86)\Steam\steamapps\common\Hades\Hades.exe",
            "Hades",
            false,
            true,
        );
        assert_eq!(decide(&steam, &r), Cover::Game);
        let unity = facts(r"D:\Indie\thing.exe", "UnityWndClass", false, true);
        assert_eq!(decide(&unity, &r), Cover::Game);
        assert!(game_active(&unity, &r));
        // On another monitor a windowed game doesn't hide the notch…
        let other = facts(r"D:\Indie\thing.exe", "UnityWndClass", false, false);
        assert_eq!(decide(&other, &r), Cover::Show);
        // …but hold-to-talk still stays quiet.
        assert!(game_active(&other, &r));
    }

    #[test]
    fn exclusive_fullscreen_and_presentations_hide_everywhere() {
        let r = rules(&[]);
        let mut f = facts(r"C:\x\app.exe", "X", false, false);
        f.busy = BusyState::D3dFullscreen;
        assert_eq!(decide(&f, &r), Cover::Game);
        f.busy = BusyState::Presentation;
        assert_eq!(decide(&f, &r), Cover::Presentation);
    }

    #[test]
    fn borderless_fullscreen_on_this_monitor() {
        let r = rules(&[]);
        let video = facts(r"C:\x\chrome.exe", "Chrome_WidgetWin_1", true, true);
        assert_eq!(decide(&video, &r), Cover::Fullscreen);
        assert!(!game_active(&video, &r));
        let elsewhere = facts(r"C:\x\chrome.exe", "Chrome_WidgetWin_1", true, false);
        assert_eq!(decide(&elsewhere, &r), Cover::Show);
        let normal = facts(r"C:\x\notepad.exe", "Notepad", false, true);
        assert_eq!(decide(&normal, &r), Cover::Show);
    }

    #[test]
    fn user_rules_and_switches() {
        let list = vec!["obs64".to_string()];
        let obs = facts(
            r"C:\Program Files\obs-studio\bin\64bit\obs64.exe",
            "Qt",
            false,
            true,
        );
        assert_eq!(decide(&obs, &rules(&list)), Cover::Excluded);
        let off = Rules {
            hide_fullscreen: false,
            hide_in_games: false,
            never_over: &[],
        };
        let game = facts(r"C:\Games\x.exe", "UnrealWindow", true, true);
        assert_eq!(decide(&game, &off), Cover::Show);
        assert!(!game_active(&game, &off));
        assert_eq!(process_name(r"C:\A\B\Game.EXE"), "game");
    }
}
