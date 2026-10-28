//! Typing recognised speech into the app in front, as keystrokes (no
//! clipboard), once the user has let go of any shortcut keys.

use std::time::{Duration, Instant};

/// One keystroke to send: a Unicode code unit, or Enter for a line break.
#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stroke {
    Unit(u16),
    Enter,
}

/// The keystrokes that type `text`: UTF-16 units (so emoji and other
/// characters outside the basic plane work), Enter for line breaks, no
/// other control characters.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn strokes(text: &str) -> Vec<Stroke> {
    let mut out = Vec::new();
    for c in text.chars() {
        match c {
            '\n' => out.push(Stroke::Enter),
            c if c.is_control() => {}
            c => {
                let mut buf = [0u16; 2];
                out.extend(c.encode_utf16(&mut buf).iter().map(|u| Stroke::Unit(*u)));
            }
        }
    }
    out
}

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
    use super::{strokes, Stroke};
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS,
        KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, VIRTUAL_KEY, VK_CONTROL, VK_LWIN, VK_MENU, VK_RETURN,
        VK_RWIN, VK_SHIFT,
    };

    /// Whether Ctrl, Alt, Shift or a Win key is physically down.
    pub fn modifiers_down() -> bool {
        [VK_CONTROL, VK_MENU, VK_SHIFT, VK_LWIN, VK_RWIN]
            .iter()
            .any(|vk| unsafe { GetAsyncKeyState(i32::from(vk.0)) } as u16 & 0x8000 != 0)
    }

    fn input(vk: VIRTUAL_KEY, scan: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: vk,
                    wScan: scan,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }

    /// Type `text` into the focused window.
    pub fn type_text(text: &str) -> bool {
        let mut inputs = Vec::new();
        for s in strokes(text) {
            match s {
                Stroke::Unit(u) => {
                    inputs.push(input(VIRTUAL_KEY(0), u, KEYEVENTF_UNICODE));
                    inputs.push(input(
                        VIRTUAL_KEY(0),
                        u,
                        KEYEVENTF_UNICODE | KEYEVENTF_KEYUP,
                    ));
                }
                Stroke::Enter => {
                    inputs.push(input(VK_RETURN, 0, KEYBD_EVENT_FLAGS(0)));
                    inputs.push(input(VK_RETURN, 0, KEYEVENTF_KEYUP));
                }
            }
        }
        if inputs.is_empty() {
            return true;
        }
        let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
        sent as usize == inputs.len()
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn modifiers_down() -> bool {
        false
    }
    pub fn type_text(_text: &str) -> bool {
        false
    }
}

pub use imp::{modifiers_down, type_text};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strokes_cover_any_text() {
        assert_eq!(
            strokes("Hi\n"),
            vec![
                Stroke::Unit(b'H' as u16),
                Stroke::Unit(b'i' as u16),
                Stroke::Enter
            ]
        );
        // An emoji is two UTF-16 units; tabs and other control characters are dropped.
        assert_eq!(
            strokes("👍"),
            vec![Stroke::Unit(0xD83D), Stroke::Unit(0xDC4D)]
        );
        assert_eq!(
            strokes("a\tb\u{7}"),
            vec![Stroke::Unit(97), Stroke::Unit(98)]
        );
        assert_eq!(strokes("é").len(), 1);
    }

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
