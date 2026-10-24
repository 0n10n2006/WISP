//! Hold-to-talk: hold Ctrl+Win to dictate, Win+Alt to ask by voice (or
//! the pairs the user picked in Settings → Shortcuts).
//!
//! Key events arrive through Raw Input (the API games use), registered on a
//! hidden window, and feed the pure `wisp_core::hold::HoldDetector`. There is
//! no keyboard hook: keys can't be swallowed or changed, nothing is recorded,
//! only modifier state is looked at, and any other key turns the chord into a
//! normal shortcut. Windows-only; elsewhere this is a no-op.

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

/// Hold-to-talk is switched off while a game is in front.
pub static PAUSED: AtomicBool = AtomicBool::new(false);

pub fn set_enabled(on: bool) {
    ENABLED.store(on, Ordering::SeqCst);
}

pub fn set_paused(paused: bool) {
    PAUSED.store(paused, Ordering::SeqCst);
}

/// Hold-to-talk is also off while the user records a new shortcut.
pub static RECORDING: AtomicBool = AtomicBool::new(false);

pub fn set_recording(on: bool) {
    RECORDING.store(on, Ordering::SeqCst);
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
    use windows::core::w;
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
        VIRTUAL_KEY,
    };
    use windows::Win32::UI::Input::{
        GetRawInputData, RegisterRawInputDevices, HRAWINPUT, RAWINPUT, RAWINPUTDEVICE,
        RAWINPUTHEADER, RIDEV_INPUTSINK, RID_INPUT, RIM_TYPEKEYBOARD,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, RegisterClassW,
        TranslateMessage, MSG, RI_KEY_BREAK, WINDOW_EX_STYLE, WM_INPUT, WNDCLASSW, WS_POPUP,
    };
    use wisp_core::hold::{HoldDetector, Key, Mod};

    static DETECTOR: OnceLock<Mutex<HoldDetector>> = OnceLock::new();
    static TX: OnceLock<Sender<HoldEvent>> = OnceLock::new();
    /// Wakes the timer thread only while a chord is being held.
    static WAKE: OnceLock<Sender<()>> = OnceLock::new();

    pub fn configure(dictate: &str, ask: &str) {
        let d = wisp_core::hold::parse_chord(dictate).flatten();
        let a = wisp_core::hold::parse_chord(ask).flatten();
        detector().lock().expect("hold").set_chords(d, a);
    }

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

    fn feed(vk: u32, down: bool) {
        if !ENABLED.load(Ordering::Relaxed)
            || PAUSED.load(Ordering::Relaxed)
            || RECORDING.load(Ordering::Relaxed)
        {
            return;
        }
        if let Ok(mut d) = detector().try_lock() {
            if let Some(e) = d.key(classify(vk), down, Instant::now()) {
                if let Some(tx) = TX.get() {
                    let _ = tx.send(e);
                }
            }
            if d.waiting() {
                if let Some(w) = WAKE.get() {
                    let _ = w.send(());
                }
            }
        }
    }

    unsafe extern "system" fn wndproc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if msg == WM_INPUT {
            let mut raw = RAWINPUT::default();
            let mut size = std::mem::size_of::<RAWINPUT>() as u32;
            let got = unsafe {
                GetRawInputData(
                    HRAWINPUT(lparam.0 as *mut core::ffi::c_void),
                    RID_INPUT,
                    Some(&mut raw as *mut RAWINPUT as *mut core::ffi::c_void),
                    &mut size,
                    std::mem::size_of::<RAWINPUTHEADER>() as u32,
                )
            };
            // Synthetic input (including our own Start-menu suppressor) has
            // no device; only real key presses count.
            if got != u32::MAX
                && raw.header.dwType == RIM_TYPEKEYBOARD.0
                && !raw.header.hDevice.is_invalid()
            {
                let kb = unsafe { raw.data.keyboard };
                let down = u32::from(kb.Flags) & RI_KEY_BREAK == 0;
                // 0xFF marks the fake keys some keyboards send around
                // extended keys; they aren't presses.
                if kb.VKey != 0xFF {
                    feed(u32::from(kb.VKey), down);
                }
            }
        }
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
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
        if let HoldEvent::Start(chord) = e {
            // Only a chord with the Win key would otherwise open Start.
            let win = detector()
                .lock()
                .map(|d| d.mods(chord).is_some_and(|m| m.win))
                .unwrap_or(true);
            if win {
                suppress_start_menu();
            }
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
        // Promote held chords after the hold delay. The thread sleeps until a
        // chord is actually being held, so it costs nothing while idle.
        let (wake_tx, wake_rx) = channel::<()>();
        let _ = WAKE.set(wake_tx);
        std::thread::spawn(move || {
            while wake_rx.recv().is_ok() {
                loop {
                    std::thread::sleep(Duration::from_millis(30));
                    let (event, waiting) = {
                        let mut d = detector().lock().expect("hold");
                        (d.tick(Instant::now()), d.waiting())
                    };
                    if let Some(e) = event {
                        let _ = tx.send(e);
                    }
                    if !waiting {
                        break;
                    }
                }
                // Drop wake-ups queued while we were ticking.
                while wake_rx.try_recv().is_ok() {}
            }
        });
        // Raw Input arrives as window messages: a hidden, never-shown
        // window on its own thread with a message loop receives them.
        std::thread::spawn(|| unsafe {
            let Ok(module) = GetModuleHandleW(None) else {
                return;
            };
            let class = w!("WispHoldKeys");
            let wc = WNDCLASSW {
                lpfnWndProc: Some(wndproc),
                hInstance: module.into(),
                lpszClassName: class,
                ..Default::default()
            };
            RegisterClassW(&wc);
            let Ok(hwnd) = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                class,
                w!(""),
                WS_POPUP,
                0,
                0,
                0,
                0,
                None,
                None,
                Some(module.into()),
                None,
            ) else {
                crate::selftest::mark("hold-keys-failed");
                return;
            };
            let device = RAWINPUTDEVICE {
                usUsagePage: 0x01, // generic desktop
                usUsage: 0x06,     // keyboard
                dwFlags: RIDEV_INPUTSINK,
                hwndTarget: hwnd,
            };
            if RegisterRawInputDevices(&[device], std::mem::size_of::<RAWINPUTDEVICE>() as u32)
                .is_err()
            {
                crate::selftest::mark("hold-keys-failed");
                return;
            }
            crate::selftest::mark("hold-keys");
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
    pub fn configure(_dictate: &str, _ask: &str) {}
}

pub use imp::{configure, install};

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
