//! Voice typing: Windows 11's own dictation (Win+H) types what you say into
//! the focused text box, live and punctuated. Wisp only puts focus where the
//! words should go and presses Win+H; it never records audio itself.

use std::time::{Duration, Instant};

/// Wait (up to `limit`) until `held` says no modifier key is down, so Win+H
/// isn't merged with keys the user is still holding from a shortcut.
pub fn wait_for_release(mut held: impl FnMut() -> bool, limit: Duration, step: Duration) -> bool {
    let start = Instant::now();
    while held() {
        if start.elapsed() >= limit {
            return false;
        }
        std::thread::sleep(step);
    }
    true
}

#[cfg(windows)]
mod imp {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS,
        KEYEVENTF_KEYUP, VIRTUAL_KEY, VK_CONTROL, VK_H, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
    };

    /// Whether Ctrl, Alt, Shift or a Win key is physically down.
    pub fn modifiers_down() -> bool {
        [VK_CONTROL, VK_MENU, VK_SHIFT, VK_LWIN, VK_RWIN]
            .iter()
            .any(|vk| unsafe { GetAsyncKeyState(i32::from(vk.0)) } as u16 & 0x8000 != 0)
    }

    fn key(vk: VIRTUAL_KEY, up: bool) -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
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
        }
    }

    /// Press Win+H: Windows opens voice typing in the focused text box.
    pub fn press_win_h() -> bool {
        let inputs = [
            key(VK_LWIN, false),
            key(VK_H, false),
            key(VK_H, true),
            key(VK_LWIN, true),
        ];
        let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
        sent as usize == inputs.len()
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn modifiers_down() -> bool {
        false
    }
    pub fn press_win_h() -> bool {
        false
    }
}

pub use imp::{modifiers_down, press_win_h};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waits_for_keys_to_come_up() {
        let mut polls = 0;
        let released = wait_for_release(
            || {
                polls += 1;
                polls < 3
            },
            Duration::from_secs(1),
            Duration::from_millis(1),
        );
        assert!(released);
        assert_eq!(polls, 3);
    }

    #[test]
    fn gives_up_when_keys_stay_down() {
        let started = Instant::now();
        let released =
            wait_for_release(|| true, Duration::from_millis(20), Duration::from_millis(5));
        assert!(!released);
        assert!(started.elapsed() >= Duration::from_millis(20));
    }
}
