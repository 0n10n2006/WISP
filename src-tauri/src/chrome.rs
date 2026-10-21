//! Window chrome details: no OS border or rounded corners on the notch
//! windows, and a Settings title bar that matches Wisp's theme.

use tauri::WebviewWindow;
use wisp_core::settings::Theme;

/// Ask Windows 11 not to draw its own border or round the corners of a
/// borderless, transparent window (it would show as a faint outline).
#[cfg(windows)]
pub fn borderless(win: &WebviewWindow) {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE,
        DWMWA_WINDOW_CORNER_PREFERENCE, DWMWCP_DONOTROUND,
    };
    let Ok(h) = win.hwnd() else { return };
    let hwnd = HWND(h.0 as *mut core::ffi::c_void);
    let none: u32 = DWMWA_COLOR_NONE;
    let corner = DWMWCP_DONOTROUND;
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR,
            &none as *const u32 as *const core::ffi::c_void,
            std::mem::size_of::<u32>() as u32,
        );
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &corner as *const _ as *const core::ffi::c_void,
            std::mem::size_of_val(&corner) as u32,
        );
    }
}

#[cfg(not(windows))]
pub fn borderless(_win: &WebviewWindow) {}

/// The native title bar follows Wisp's theme setting (System = Windows).
pub fn apply_theme(win: &WebviewWindow, theme: Theme) {
    let t = match theme {
        Theme::Dark => Some(tauri::Theme::Dark),
        Theme::Light => Some(tauri::Theme::Light),
        Theme::System => None,
    };
    let _ = win.set_theme(t);
}
