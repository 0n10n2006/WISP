//! What's on the desktop: the window in front (is it fullscreen, a game, a
//! presentation?), the user's open windows, and an instant signal when the
//! foreground window changes. Window titles never leave this module unless
//! the user allows sharing them. Windows-only; elsewhere everything is empty.

use wisp_core::screen::ScreenFacts;
use wisp_core::tools::OpenWindow;

#[cfg(windows)]
mod imp {
    use super::*;
    use std::sync::OnceLock;
    use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, RECT};
    use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
    use windows::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MonitorFromWindow, HMONITOR, MONITORINFO, MONITOR_DEFAULTTONEAREST,
    };
    use windows::Win32::System::Threading::{
        OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::Accessibility::{SetWinEventHook, HWINEVENTHOOK};
    use windows::Win32::UI::Shell::{
        SHQueryUserNotificationState, QUNS_BUSY, QUNS_PRESENTATION_MODE,
        QUNS_RUNNING_D3D_FULL_SCREEN,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        DispatchMessageW, EnumWindows, GetClassNameW, GetForegroundWindow, GetMessageW, GetWindow,
        GetWindowLongPtrW, GetWindowRect, GetWindowTextW, GetWindowThreadProcessId, IsIconic,
        IsWindowVisible, SetForegroundWindow, ShowWindow, TranslateMessage,
        EVENT_OBJECT_LOCATIONCHANGE, EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_MINIMIZEEND,
        GWL_EXSTYLE, GW_OWNER, MSG, OBJID_WINDOW, SW_RESTORE, WINEVENT_OUTOFCONTEXT,
        WS_EX_TOOLWINDOW,
    };
    use wisp_core::screen::BusyState;

    fn hwnd(id: isize) -> HWND {
        HWND(id as *mut core::ffi::c_void)
    }

    fn class_of(h: HWND) -> String {
        let mut buf = [0u16; 128];
        let n = unsafe { GetClassNameW(h, &mut buf) };
        String::from_utf16_lossy(&buf[..n.max(0) as usize])
    }

    fn title_of(h: HWND) -> String {
        let mut buf = [0u16; 256];
        let n = unsafe { GetWindowTextW(h, &mut buf) };
        String::from_utf16_lossy(&buf[..n.max(0) as usize])
    }

    fn pid_of(h: HWND) -> u32 {
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(h, Some(&mut pid)) };
        pid
    }

    fn exe_path(pid: u32) -> String {
        if pid == 0 {
            return String::new();
        }
        let Ok(handle) = (unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) })
        else {
            return String::new();
        };
        let mut buf = [0u16; 520];
        let mut len = buf.len() as u32;
        let ok = unsafe {
            QueryFullProcessImageNameW(
                handle,
                PROCESS_NAME_WIN32,
                windows::core::PWSTR(buf.as_mut_ptr()),
                &mut len,
            )
        };
        unsafe {
            let _ = CloseHandle(handle);
        }
        if ok.is_err() {
            return String::new();
        }
        String::from_utf16_lossy(&buf[..len as usize])
    }

    fn monitor_rect(m: HMONITOR) -> Option<RECT> {
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        unsafe { GetMonitorInfoW(m, &mut info) }
            .as_bool()
            .then_some(info.rcMonitor)
    }

    const SHELL_CLASSES: [&str; 5] = [
        "Progman",
        "WorkerW",
        "Shell_TrayWnd",
        "Shell_SecondaryTrayWnd",
        "Windows.UI.Core.CoreWindow",
    ];

    fn busy_state() -> BusyState {
        match unsafe { SHQueryUserNotificationState() } {
            Ok(s) if s == QUNS_RUNNING_D3D_FULL_SCREEN => BusyState::D3dFullscreen,
            Ok(s) if s == QUNS_PRESENTATION_MODE => BusyState::Presentation,
            Ok(s) if s == QUNS_BUSY => BusyState::Busy,
            _ => BusyState::Normal,
        }
    }

    /// Facts about the foreground window, relative to the notch window.
    pub fn facts(notch: Option<isize>) -> ScreenFacts {
        let busy = busy_state();
        let fg = unsafe { GetForegroundWindow() };
        if fg.0.is_null() {
            return ScreenFacts {
                busy,
                ..Default::default()
            };
        }
        let pid = pid_of(fg);
        if pid == std::process::id() {
            // Wisp itself is in front (the bar or Settings).
            return ScreenFacts {
                busy,
                ..Default::default()
            };
        }
        let class = class_of(fg);
        if SHELL_CLASSES.contains(&class.as_str()) {
            return ScreenFacts {
                busy,
                class,
                ..Default::default()
            };
        }
        let fg_mon = unsafe { MonitorFromWindow(fg, MONITOR_DEFAULTTONEAREST) };
        let same_monitor = match notch {
            Some(n) => {
                let notch_mon = unsafe { MonitorFromWindow(hwnd(n), MONITOR_DEFAULTTONEAREST) };
                notch_mon == fg_mon
            }
            None => true,
        };
        let mut r = RECT::default();
        let fullscreen = unsafe { GetWindowRect(fg, &mut r) }.is_ok()
            && !unsafe { IsIconic(fg) }.as_bool()
            && monitor_rect(fg_mon).is_some_and(|m| {
                r.left <= m.left && r.top <= m.top && r.right >= m.right && r.bottom >= m.bottom
            });
        ScreenFacts {
            fullscreen,
            busy,
            exe_path: exe_path(pid),
            class,
            same_monitor,
        }
    }

    fn cloaked(h: HWND) -> bool {
        let mut v: u32 = 0;
        unsafe {
            DwmGetWindowAttribute(
                h,
                DWMWA_CLOAKED,
                &mut v as *mut u32 as *mut core::ffi::c_void,
                std::mem::size_of::<u32>() as u32,
            )
        }
        .is_ok()
            && v != 0
    }

    unsafe extern "system" fn collect(h: HWND, data: LPARAM) -> windows::core::BOOL {
        let list = unsafe { &mut *(data.0 as *mut Vec<OpenWindow>) };
        if list.len() >= 60 {
            return false.into();
        }
        let ex = unsafe { GetWindowLongPtrW(h, GWL_EXSTYLE) } as u32;
        let owned = unsafe { GetWindow(h, GW_OWNER) }.is_ok_and(|o| !o.0.is_null());
        if !unsafe { IsWindowVisible(h) }.as_bool()
            || ex & WS_EX_TOOLWINDOW.0 != 0
            || owned
            || cloaked(h)
        {
            return true.into();
        }
        let title = title_of(h);
        let pid = pid_of(h);
        if title.trim().is_empty() || pid == std::process::id() {
            return true.into();
        }
        if SHELL_CLASSES.contains(&class_of(h).as_str()) {
            return true.into();
        }
        let app = wisp_core::screen::process_name(&exe_path(pid));
        list.push(OpenWindow {
            id: h.0 as i64,
            app: if app.is_empty() { "app".into() } else { app },
            title,
        });
        true.into()
    }

    /// Visible, real top-level windows, front to back.
    pub fn open_windows() -> Vec<OpenWindow> {
        let mut list: Vec<OpenWindow> = Vec::new();
        unsafe {
            let _ = EnumWindows(Some(collect), LPARAM(&mut list as *mut _ as isize));
        }
        list
    }

    /// Bring a window from `open_windows` to the front (restoring if minimised).
    pub fn focus_window(id: i64) -> bool {
        let h = HWND(id as isize as *mut core::ffi::c_void);
        let still_there = open_windows().iter().any(|w| w.id == id);
        if !still_there {
            return false;
        }
        unsafe {
            if IsIconic(h).as_bool() {
                let _ = ShowWindow(h, SW_RESTORE);
            }
            SetForegroundWindow(h).as_bool()
        }
    }

    static NOTIFY: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();

    unsafe extern "system" fn on_event(
        _hook: HWINEVENTHOOK,
        event: u32,
        h: HWND,
        id_object: i32,
        _id_child: i32,
        _thread: u32,
        _time: u32,
    ) {
        // Size/position changes only matter for the window in front.
        if event == EVENT_OBJECT_LOCATIONCHANGE
            && (id_object != OBJID_WINDOW.0 || unsafe { GetForegroundWindow() } != h)
        {
            return;
        }
        if let Some(f) = NOTIFY.get() {
            f();
        }
    }

    /// Call `notify` whenever the foreground window changes, is restored, or
    /// is resized (e.g. a video goes fullscreen). Observe-only.
    pub fn watch_foreground(notify: impl Fn() + Send + Sync + 'static) {
        if NOTIFY.set(Box::new(notify)).is_err() {
            return;
        }
        std::thread::Builder::new()
            .name("wisp-foreground".into())
            .spawn(|| unsafe {
                let flags = WINEVENT_OUTOFCONTEXT;
                SetWinEventHook(
                    EVENT_SYSTEM_FOREGROUND,
                    EVENT_SYSTEM_FOREGROUND,
                    None,
                    Some(on_event),
                    0,
                    0,
                    flags,
                );
                SetWinEventHook(
                    EVENT_SYSTEM_MINIMIZEEND,
                    EVENT_SYSTEM_MINIMIZEEND,
                    None,
                    Some(on_event),
                    0,
                    0,
                    flags,
                );
                SetWinEventHook(
                    EVENT_OBJECT_LOCATIONCHANGE,
                    EVENT_OBJECT_LOCATIONCHANGE,
                    None,
                    Some(on_event),
                    0,
                    0,
                    flags,
                );
                let mut msg = MSG::default();
                while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                    let _ = TranslateMessage(&msg);
                    let _ = DispatchMessageW(&msg);
                }
            })
            .ok();
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;
    pub fn facts(_notch: Option<isize>) -> ScreenFacts {
        ScreenFacts::default()
    }
    pub fn open_windows() -> Vec<OpenWindow> {
        Vec::new()
    }
    pub fn focus_window(_id: i64) -> bool {
        false
    }
    pub fn watch_foreground(_notify: impl Fn() + Send + Sync + 'static) {}
}

pub use imp::{facts, focus_window, open_windows, watch_foreground};
