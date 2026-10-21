//! Hold-to-talk: hold Ctrl+Win to dictate, Win+Alt to ask by voice.
//!
//! A low-level keyboard hook *observes* key events and feeds the pure
//! `wisp_core::hold::HoldDetector`. Keys are never swallowed or recorded;
//! only modifier state is looked at, and any other key turns the chord into
//! a normal shortcut. Windows-only; elsewhere this is a no-op.

use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use wisp_core::hold::{Chord, HoldEvent};

pub static ENABLED: AtomicBool = AtomicBool::new(false);

#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HoldPayload {
    pub chord: &'static str,
    pub phase: &'static str,
}

#[cfg_attr(not(windows), allow(dead_code))]
pub fn payload(e: HoldEvent) -> HoldPayload {
    let (c, phase) = match e {
        HoldEvent::Start(c) => (c, "start"),
        HoldEvent::Stop(c) => (c, "stop"),
        HoldEvent::Cancel(c) => (c, "cancel"),
    };
    HoldPayload {
        chord: match c {
            Chord::Dictate => "dictate",
            Chord::Ask => "ask",
        },
        phase,
    }
}

pub fn set_enabled(on: bool) {
    ENABLED.store(on, Ordering::SeqCst);
}

#[cfg(windows)]
mod imp {
    use super::*;
    use crate::state::AppState;
    use crate::windows::{self as wins, NotchMode};
    use std::sync::mpsc::{channel, Sender};
    use std::sync::{Mutex, OnceLock};
    use std::time::{Duration, Instant};
    use tauri::{AppHandle, Emitter, Manager};
    use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
        VIRTUAL_KEY,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, DispatchMessageW, GetMessageW, SetWindowsHookExW, TranslateMessage,
        KBDLLHOOKSTRUCT, LLKHF_INJECTED, MSG, WH_KEYBOARD_LL, WM_KEYDOWN, WM_SYSKEYDOWN,
    };
    use wisp_core::hold::{HoldDetector, Key, Mod};

    static DETECTOR: OnceLock<Mutex<HoldDetector>> = OnceLock::new();
    static TX: OnceLock<Sender<HoldEvent>> = OnceLock::new();

    fn detector() -> &'static Mutex<HoldDetector> {
        DETECTOR.get_or_init(|| Mutex::new(HoldDetector::new()))
    }

    fn classify(vk: u32) -> Key {
        match vk {
            0x11 | 0xA2 | 0xA3 => Key::Mod(Mod::Ctrl),
            0x12 | 0xA4 | 0xA5 => Key::Mod(Mod::Alt),
            0x10 | 0xA0 | 0xA1 => Key::Mod(Mod::Shift),
            0x5B | 0x5C => Key::Mod(Mod::Win),
            _ => Key::Other,
        }
    }

    unsafe extern "system" fn hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if code >= 0 && ENABLED.load(Ordering::Relaxed) {
            let info = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
            // Ignore synthetic input (including our own Start-menu suppressor).
            if !info.flags.contains(LLKHF_INJECTED) {
                let msg = wparam.0 as u32;
                let down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
                if let Ok(mut d) = detector().try_lock() {
                    if let Some(e) = d.key(classify(info.vkCode), down, Instant::now()) {
                        if let Some(tx) = TX.get() {
                            let _ = tx.send(e);
                        }
                    }
                }
            }
        }
        unsafe { CallNextHookEx(None, code, wparam, lparam) }
    }

    /// Releasing Win on its own opens Start; a harmless unassigned key press
    /// (0xE8) tells Windows the Win key was part of a combination.
    fn suppress_start_menu() {
        let key = |up: bool| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(0xE8),
                    wScan: 0,
                    dwFlags: if up {
                        KEYEVENTF_KEYUP
                    } else {
                        KEYBD_EVENT_FLAGS(0)
                    },
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        let inputs = [key(false), key(true)];
        unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
    }

    fn handle(app: &AppHandle, e: HoldEvent) {
        if let HoldEvent::Start(_) = e {
            suppress_start_menu();
            // Remember where dictated text should go, and show the notch
            // without taking focus away from that app.
            if let Some(t) = crate::foreground::current() {
                *app.state::<AppState>()
                    .insert_target
                    .lock()
                    .expect("target") = Some(t);
            }
            if app.state::<AppState>().island.lock().expect("island").mode != NotchMode::Open {
                wins::set_mode(app, NotchMode::Peek, None);
            }
            wins::set_island_visible(app, true);
        }
        let _ = app.emit_to(wins::ISLAND, "wisp://hold", payload(e));
    }

    pub fn install(app: AppHandle) {
        let (tx, rx) = channel::<HoldEvent>();
        if TX.set(tx.clone()).is_err() {
            return;
        }
        // Event pump to the UI.
        let a = app.clone();
        std::thread::spawn(move || {
            for e in rx {
                let h = a.clone();
                let _ = a.run_on_main_thread(move || handle(&h, e));
            }
        });
        // Promote held chords after the hold delay.
        std::thread::spawn(move || loop {
            std::thread::sleep(Duration::from_millis(40));
            if !ENABLED.load(Ordering::Relaxed) {
                continue;
            }
            if let Some(e) = detector().lock().expect("hold").tick(Instant::now()) {
                let _ = tx.send(e);
            }
        });
        // The hook needs a thread with a message loop.
        std::thread::spawn(|| unsafe {
            if SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook), None, 0).is_err() {
                crate::selftest::mark("hold-hook-failed");
                return;
            }
            crate::selftest::mark("hold-hook");
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        });
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn install(_app: tauri::AppHandle) {}
}

pub use imp::install;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payloads() {
        assert_eq!(
            payload(HoldEvent::Start(Chord::Dictate)),
            HoldPayload {
                chord: "dictate",
                phase: "start"
            }
        );
        assert_eq!(payload(HoldEvent::Cancel(Chord::Ask)).phase, "cancel");
    }
}
