//! Remember which app the user was in before Wisp took focus, so voice
//! typing can go back there. Windows-only; elsewhere nothing is tracked.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Target(isize);

#[cfg(windows)]
mod imp {
    use super::Target;
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetForegroundWindow, GetWindowTextW, GetWindowThreadProcessId, IsWindow,
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

    /// Executable name of the window's process, e.g. "code" (local only).
    pub fn process_name(t: Target) -> Option<String> {
        use windows::Win32::Foundation::CloseHandle;
        use windows::Win32::System::Threading::{
            OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
            PROCESS_QUERY_LIMITED_INFORMATION,
        };
        let mut pid = 0u32;
        unsafe { GetWindowThreadProcessId(hwnd(t), Some(&mut pid)) };
        if pid == 0 {
            return None;
        }
        let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
        let mut buf = [0u16; 512];
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
        ok.ok()?;
        let path = String::from_utf16_lossy(&buf[..len as usize]);
        let file = path.rsplit('\\').next()?.to_lowercase();
        Some(file.trim_end_matches(".exe").to_string())
    }

    pub fn title(t: Target) -> Option<String> {
        let mut buf = [0u16; 256];
        let n = unsafe { GetWindowTextW(hwnd(t), &mut buf) };
        (n > 0).then(|| String::from_utf16_lossy(&buf[..n as usize]))
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
    pub fn process_name(_t: Target) -> Option<String> {
        None
    }
    pub fn title(_t: Target) -> Option<String> {
        None
    }
}

pub use imp::{current, is_valid, process_name, title};
