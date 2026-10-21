//! Remember which app the user was in before Wisp took focus, so dictated
//! text can be pasted back there. Windows-only; other platforms report
//! insertion as unsupported and the UI falls back to Copy.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Target(isize);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TargetInfo {
    pub available: bool,
    pub title: Option<String>,
}

#[cfg(windows)]
mod imp {
    use super::Target;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
        VIRTUAL_KEY, VK_CONTROL, VK_V,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId, IsWindow,
        SetForegroundWindow,
    };

    fn hwnd(t: Target) -> HWND {
        HWND(t.0 as *mut core::ffi::c_void)
    }

    pub fn current() -> Option<Target> {
        let h = unsafe { GetForegroundWindow() };
        if h.0.is_null() {
            return None;
        }
        // Ignore our own windows.
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(h, Some(&mut pid)) };
        if pid == std::process::id() {
            return None;
        }
        Some(Target(h.0 as isize))
    }

    pub fn is_valid(t: Target) -> bool {
        unsafe { IsWindow(Some(hwnd(t))).as_bool() }
    }

    pub fn title(t: Target) -> Option<String> {
        let mut buf = [0u16; 256];
        let n = unsafe { GetWindowTextW(hwnd(t), &mut buf) };
        (n > 0).then(|| String::from_utf16_lossy(&buf[..n as usize]))
    }

    pub fn focus(t: Target) -> bool {
        unsafe { SetForegroundWindow(hwnd(t)).as_bool() }
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

    /// Simulate Ctrl+V in the focused window.
    pub fn paste() -> bool {
        let inputs = [
            key(VK_CONTROL, false),
            key(VK_V, false),
            key(VK_V, true),
            key(VK_CONTROL, true),
        ];
        let sent = unsafe { SendInput(&inputs, std::mem::size_of::<INPUT>() as i32) };
        sent as usize == inputs.len()
    }
}

#[cfg(not(windows))]
mod imp {
    use super::Target;
    pub fn current() -> Option<Target> {
        None
    }
    pub fn is_valid(_t: Target) -> bool {
        false
    }
    pub fn title(_t: Target) -> Option<String> {
        None
    }
    pub fn focus(_t: Target) -> bool {
        false
    }
    pub fn paste() -> bool {
        false
    }
}

pub use imp::{current, focus, is_valid, paste, title};

pub fn info(t: Option<Target>) -> TargetInfo {
    match t.filter(|t| is_valid(*t)) {
        Some(t) => TargetInfo {
            available: true,
            title: title(t),
        },
        None => TargetInfo {
            available: false,
            title: None,
        },
    }
}
