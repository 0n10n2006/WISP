//! Windows light/dark theme: read for the tray icon, write for the dashboard toggle.

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemTheme {
    /// Apps use the light theme.
    pub apps_light: bool,
    /// Taskbar/Start use the light theme.
    pub system_light: bool,
}

#[cfg(windows)]
mod imp {
    use super::SystemTheme;
    use windows::core::w;
    use windows::Win32::Foundation::ERROR_SUCCESS;
    use windows::Win32::System::Registry::{
        RegGetValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_DWORD, RRF_RT_REG_DWORD,
    };

    const KEY: windows::core::PCWSTR =
        w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");

    fn read(name: windows::core::PCWSTR) -> Option<u32> {
        let mut data: u32 = 0;
        let mut size = std::mem::size_of::<u32>() as u32;
        let rc = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                KEY,
                name,
                RRF_RT_REG_DWORD,
                None,
                Some(&mut data as *mut u32 as *mut core::ffi::c_void),
                Some(&mut size),
            )
        };
        (rc == ERROR_SUCCESS).then_some(data)
    }

    fn write(name: windows::core::PCWSTR, value: u32) -> bool {
        let rc = unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                KEY,
                name,
                REG_DWORD.0,
                Some(&value as *const u32 as *const core::ffi::c_void),
                std::mem::size_of::<u32>() as u32,
            )
        };
        rc == ERROR_SUCCESS
    }

    pub fn get() -> SystemTheme {
        SystemTheme {
            apps_light: read(w!("AppsUseLightTheme")).unwrap_or(1) != 0,
            system_light: read(w!("SystemUsesLightTheme")).unwrap_or(0) != 0,
        }
    }

    pub fn set(light: bool) -> bool {
        let v = u32::from(light);
        write(w!("AppsUseLightTheme"), v) && write(w!("SystemUsesLightTheme"), v)
    }
}

#[cfg(not(windows))]
mod imp {
    use super::SystemTheme;
    pub fn get() -> SystemTheme {
        SystemTheme {
            apps_light: false,
            system_light: false,
        }
    }
    pub fn set(_light: bool) -> bool {
        false
    }
}

pub use imp::{get, set};
