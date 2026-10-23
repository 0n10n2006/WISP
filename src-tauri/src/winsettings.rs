//! Windows personalisation and comfort settings for the control centre and
//! `change_setting`: look & feel, taskbar & desktop, display & comfort, and
//! sound devices. Every change is a typed `WinChange` (see wisp-core); each
//! one uses a documented API where Windows has one, or a single per-user
//! registry value followed by the standard change broadcast. No shell.

use wisp_core::winsettings::{WinChange, WinSettingsState};
use wisp_core::{ErrorCode, Result, WispError};

/// Where to send the user when Windows won't take a change directly.
pub fn settings_page(change: &WinChange) -> &'static str {
    match change {
        WinChange::DarkMode(_)
        | WinChange::AppsDark(_)
        | WinChange::SystemDark(_)
        | WinChange::Transparency(_)
        | WinChange::AccentColor(_)
        | WinChange::AccentFromWallpaper(_)
        | WinChange::AccentOnTaskbar(_) => "ms-settings:colors",
        WinChange::TaskbarAlign(_) | WinChange::TaskbarAutoHide(_) => "ms-settings:taskbar",
        WinChange::DesktopIcons(_) => "ms-settings:themes",
        WinChange::StartRecent(_) => "ms-settings:personalization-start",
        WinChange::Brightness(_) => "ms-settings:display",
        WinChange::NightLight(_) => "ms-settings:nightlight",
        WinChange::DoNotDisturb(_) => "ms-settings:notifications",
        WinChange::PowerMode(_) => "ms-settings:powersleep",
        WinChange::OutputDevice(_) | WinChange::MicMute(_) => "ms-settings:sound",
    }
}

fn unsupported() -> WispError {
    WispError::new(
        ErrorCode::Unsupported,
        "Changing Windows settings is only available on Windows.",
    )
}

#[cfg(windows)]
#[allow(non_snake_case)] // COM method names
mod imp {
    use super::*;
    use std::cell::RefCell;
    use std::time::{Duration, Instant};
    use windows::core::{w, GUID, HRESULT, HSTRING, PCWSTR};
    use windows::Win32::Foundation::{
        CloseHandle, ERROR_SUCCESS, HANDLE, HWND, LPARAM, POINT, WPARAM,
    };
    use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
    use windows::Win32::Media::Audio::{
        eCapture, eCommunications, eConsole, eMultimedia, eRender, IMMDeviceEnumerator,
        MMDeviceEnumerator, DEVICE_STATE_ACTIVE,
    };
    use windows::Win32::System::Com::{CoCreateInstance, CLSCTX_ALL, STGM_READ};
    use windows::Win32::System::Registry::{
        RegGetValueW, RegSetKeyValueW, HKEY_CURRENT_USER, REG_BINARY, REG_DWORD, REG_VALUE_TYPE,
        RRF_RT_REG_BINARY, RRF_RT_REG_DWORD,
    };
    use windows::Win32::UI::Shell::{SHAppBarMessage, ABM_GETSTATE, ABM_SETSTATE, APPBARDATA};
    use windows::Win32::UI::WindowsAndMessaging::{
        FindWindowExW, FindWindowW, IsWindowVisible, SendMessageTimeoutW, SendMessageW,
        HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_COMMAND, WM_SETTINGCHANGE,
    };

    const PERSONALIZE: PCWSTR =
        w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize");
    const ADVANCED: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Advanced");
    const DWM: PCWSTR = w!("Software\\Microsoft\\Windows\\DWM");
    const ACCENT: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\Accent");
    const DESKTOP: PCWSTR = w!("Control Panel\\Desktop");
    const NOTIFICATIONS: PCWSTR =
        w!("Software\\Microsoft\\Windows\\CurrentVersion\\Notifications\\Settings");
    const NIGHT_LIGHT: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\CloudStore\\Store\\DefaultAccount\\Current\\default$windows.data.bluelightreduction.bluelightreductionstate\\windows.data.bluelightreduction.bluelightreductionstate");

    fn get_dword(key: PCWSTR, name: PCWSTR) -> Option<u32> {
        let mut data: u32 = 0;
        let mut size = 4u32;
        let rc = unsafe {
            RegGetValueW(
                HKEY_CURRENT_USER,
                key,
                name,
                RRF_RT_REG_DWORD,
                None,
                Some(&mut data as *mut u32 as *mut core::ffi::c_void),
                Some(&mut size),
            )
        };
        (rc == ERROR_SUCCESS).then_some(data)
    }

    fn set_value(key: PCWSTR, name: PCWSTR, kind: REG_VALUE_TYPE, bytes: &[u8]) -> Result<()> {
        let rc = unsafe {
            RegSetKeyValueW(
                HKEY_CURRENT_USER,
                key,
                name,
                kind.0,
                Some(bytes.as_ptr() as *const core::ffi::c_void),
                bytes.len() as u32,
            )
        };
        if rc == ERROR_SUCCESS {
            Ok(())
        } else {
            Err(refused(format!("registry error {}", rc.0)))
        }
    }

    fn set_dword(key: PCWSTR, name: PCWSTR, v: u32) -> Result<()> {
        set_value(key, name, REG_DWORD, &v.to_le_bytes())
    }

    fn get_binary(key: PCWSTR, name: PCWSTR) -> Option<Vec<u8>> {
        let mut size = 0u32;
        unsafe {
            if RegGetValueW(
                HKEY_CURRENT_USER,
                key,
                name,
                RRF_RT_REG_BINARY,
                None,
                None,
                Some(&mut size),
            ) != ERROR_SUCCESS
            {
                return None;
            }
            let mut buf = vec![0u8; size as usize];
            if RegGetValueW(
                HKEY_CURRENT_USER,
                key,
                name,
                RRF_RT_REG_BINARY,
                None,
                Some(buf.as_mut_ptr() as *mut core::ffi::c_void),
                Some(&mut size),
            ) != ERROR_SUCCESS
            {
                return None;
            }
            buf.truncate(size as usize);
            Some(buf)
        }
    }

    fn refused(detail: String) -> WispError {
        WispError::new(
            ErrorCode::Internal,
            format!("Windows didn't accept the change ({detail})."),
        )
    }

    /// Tell running apps (Explorer, the taskbar) that a setting changed.
    fn broadcast(area: &str) {
        let wide: Vec<u16> = area.encode_utf16().chain(Some(0)).collect();
        unsafe {
            let _ = SendMessageTimeoutW(
                HWND_BROADCAST,
                WM_SETTINGCHANGE,
                WPARAM(0),
                LPARAM(wide.as_ptr() as isize),
                SMTO_ABORTIFHUNG,
                200,
                None,
            );
        }
    }

    // ---- Taskbar & desktop -------------------------------------------------

    fn appbar() -> APPBARDATA {
        APPBARDATA {
            cbSize: std::mem::size_of::<APPBARDATA>() as u32,
            hWnd: unsafe { FindWindowW(w!("Shell_TrayWnd"), None) }.unwrap_or_default(),
            ..Default::default()
        }
    }

    fn auto_hide() -> Option<bool> {
        let mut d = appbar();
        let state = unsafe { SHAppBarMessage(ABM_GETSTATE, &mut d) };
        Some(state & 1 != 0)
    }

    fn set_auto_hide(on: bool) {
        let mut d = appbar();
        d.lParam = LPARAM(if on { 1 } else { 2 }); // ABS_AUTOHIDE / ABS_ALWAYSONTOP
        unsafe { SHAppBarMessage(ABM_SETSTATE, &mut d) };
    }

    /// The desktop's icon view (it lives under Progman or a WorkerW).
    fn desktop_view() -> Option<HWND> {
        unsafe {
            let progman = FindWindowW(w!("Progman"), None).ok()?;
            if let Ok(v) = FindWindowExW(Some(progman), None, w!("SHELLDLL_DefView"), None) {
                return Some(v);
            }
            let mut worker = None;
            while let Ok(wk) = FindWindowExW(None, worker, w!("WorkerW"), None) {
                if let Ok(v) = FindWindowExW(Some(wk), None, w!("SHELLDLL_DefView"), None) {
                    return Some(v);
                }
                worker = Some(wk);
            }
            None
        }
    }

    fn desktop_icons_visible() -> Option<bool> {
        let view = desktop_view()?;
        let list = unsafe { FindWindowExW(Some(view), None, w!("SysListView32"), None) }.ok()?;
        Some(unsafe { IsWindowVisible(list) }.as_bool())
    }

    fn set_desktop_icons(show: bool) -> Result<()> {
        set_dword(ADVANCED, w!("HideIcons"), u32::from(!show))?;
        if desktop_icons_visible() != Some(show) {
            if let Some(view) = desktop_view() {
                // Explorer's own "Show desktop icons" command toggles them live.
                unsafe { SendMessageW(view, WM_COMMAND, Some(WPARAM(0x7402)), Some(LPARAM(0))) };
            }
        }
        Ok(())
    }

    // ---- Accent colours ----------------------------------------------------

    fn accent_hex() -> Option<String> {
        let v = get_dword(DWM, w!("AccentColor"))?;
        let (r, g, b) = (v & 0xff, (v >> 8) & 0xff, (v >> 16) & 0xff);
        Some(format!("#{r:02x}{g:02x}{b:02x}"))
    }

    fn set_accent(hex: &str) -> Result<()> {
        use wisp_core::winsettings::{abgr, accent_palette, rgb};
        let (r, g, b) = rgb(hex).ok_or_else(|| WispError::invalid("Use a colour like #0078d4"))?;
        let palette = accent_palette(r, g, b);
        let dark = (palette[20], palette[21], palette[22]);
        set_value(ACCENT, w!("AccentPalette"), REG_BINARY, &palette)?;
        set_dword(ACCENT, w!("AccentColorMenu"), abgr(r, g, b))?;
        set_dword(ACCENT, w!("StartColorMenu"), abgr(dark.0, dark.1, dark.2))?;
        set_dword(DWM, w!("AccentColor"), abgr(r, g, b))?;
        let argb = 0xc400_0000 | (r as u32) << 16 | (g as u32) << 8 | b as u32;
        set_dword(DWM, w!("ColorizationColor"), argb)?;
        set_dword(DWM, w!("ColorizationAfterglow"), argb)?;
        set_dword(DESKTOP, w!("AutoColorization"), 0)?;
        broadcast("ImmersiveColorSet");
        Ok(())
    }

    // ---- Brightness ---------------------------------------------------------

    // The laptop panel answers these documented display IOCTLs on \\.\LCD.
    const IOCTL_QUERY_BRIGHTNESS: u32 = 0x0023_0498;
    const IOCTL_SET_BRIGHTNESS: u32 = 0x0023_049c;

    #[repr(C)]
    #[derive(Default)]
    struct DisplayBrightness {
        policy: u8,
        ac: u8,
        dc: u8,
    }

    fn lcd() -> Option<HANDLE> {
        use windows::Win32::Storage::FileSystem::{
            CreateFileW, FILE_FLAGS_AND_ATTRIBUTES, FILE_SHARE_READ, FILE_SHARE_WRITE,
            OPEN_EXISTING,
        };
        unsafe {
            CreateFileW(
                w!("\\\\.\\LCD"),
                0x8000_0000 | 0x4000_0000, // GENERIC_READ | GENERIC_WRITE
                FILE_SHARE_READ | FILE_SHARE_WRITE,
                None,
                OPEN_EXISTING,
                FILE_FLAGS_AND_ATTRIBUTES(0),
                None,
            )
        }
        .ok()
    }

    fn lcd_brightness() -> Option<u8> {
        use windows::Win32::System::IO::DeviceIoControl;
        let h = lcd()?;
        let mut out = DisplayBrightness::default();
        let mut got = 0u32;
        let ok = unsafe {
            DeviceIoControl(
                h,
                IOCTL_QUERY_BRIGHTNESS,
                None,
                0,
                Some(&mut out as *mut _ as *mut core::ffi::c_void),
                std::mem::size_of::<DisplayBrightness>() as u32,
                Some(&mut got),
                None,
            )
        };
        unsafe {
            let _ = CloseHandle(h);
        }
        ok.ok()?;
        Some(out.ac.max(out.dc).min(100))
    }

    fn set_lcd_brightness(v: u8) -> bool {
        use windows::Win32::System::IO::DeviceIoControl;
        let Some(h) = lcd() else { return false };
        let input = DisplayBrightness {
            policy: 3, // DISPLAYPOLICY_BOTH
            ac: v,
            dc: v,
        };
        let ok = unsafe {
            DeviceIoControl(
                h,
                IOCTL_SET_BRIGHTNESS,
                Some(&input as *const _ as *const core::ffi::c_void),
                std::mem::size_of::<DisplayBrightness>() as u32,
                None,
                0,
                None,
                None,
            )
        };
        unsafe {
            let _ = CloseHandle(h);
        }
        ok.is_ok()
    }

    /// External monitors over DDC/CI (slow: cached by the caller).
    fn ddc(set: Option<u8>) -> Option<u8> {
        use windows::Win32::Devices::Display::{
            DestroyPhysicalMonitors, GetMonitorBrightness, GetNumberOfPhysicalMonitorsFromHMONITOR,
            GetPhysicalMonitorsFromHMONITOR, SetMonitorBrightness, PHYSICAL_MONITOR,
        };
        use windows::Win32::Graphics::Gdi::{MonitorFromPoint, MONITOR_DEFAULTTOPRIMARY};
        unsafe {
            let mon = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
            let mut n = 0u32;
            GetNumberOfPhysicalMonitorsFromHMONITOR(mon, &mut n).ok()?;
            if n == 0 {
                return None;
            }
            let mut list = vec![PHYSICAL_MONITOR::default(); n as usize];
            GetPhysicalMonitorsFromHMONITOR(mon, &mut list).ok()?;
            let mut result = None;
            for m in &list {
                let (mut min, mut cur, mut max) = (0u32, 0u32, 0u32);
                if GetMonitorBrightness(m.hPhysicalMonitor, &mut min, &mut cur, &mut max) == 0
                    || max <= min
                {
                    continue;
                }
                if let Some(v) = set {
                    let raw = min + (max - min) * v as u32 / 100;
                    if SetMonitorBrightness(m.hPhysicalMonitor, raw) != 0 {
                        result = Some(v);
                    }
                } else {
                    result = Some(((cur - min) * 100 / (max - min)) as u8);
                    break;
                }
            }
            let _ = DestroyPhysicalMonitors(&list);
            result
        }
    }

    thread_local! {
        static BRIGHTNESS: RefCell<Option<(Option<u8>, Instant)>> = const { RefCell::new(None) };
    }

    fn brightness() -> Option<u8> {
        BRIGHTNESS.with(|c| {
            if let Some((v, at)) = *c.borrow() {
                if at.elapsed() < Duration::from_secs(15) {
                    return v;
                }
            }
            let v = lcd_brightness().or_else(|| ddc(None));
            *c.borrow_mut() = Some((v, Instant::now()));
            v
        })
    }

    fn set_brightness(v: u8) -> Result<()> {
        let lcd = set_lcd_brightness(v);
        let external = ddc(Some(v)).is_some();
        BRIGHTNESS.with(|c| *c.borrow_mut() = Some((Some(v), Instant::now())));
        if lcd || external {
            Ok(())
        } else {
            Err(WispError::new(
                ErrorCode::Unsupported,
                "This screen doesn't let apps change its brightness.",
            ))
        }
    }

    // ---- Power mode ---------------------------------------------------------

    #[link(name = "powrprof")]
    extern "system" {
        fn PowerGetEffectiveOverlayScheme(id: *mut GUID) -> u32;
        fn PowerSetActiveOverlayScheme(id: *const GUID) -> u32;
    }

    const EFFICIENCY: GUID = GUID::from_u128(0x961cc777_2547_4f9d_8174_7d86181b8a7a);
    const BALANCED: GUID = GUID::zeroed();
    const PERFORMANCE: GUID = GUID::from_u128(0xded574b5_45a0_4f42_8737_46345c09c238);

    fn power_mode() -> Option<wisp_core::winsettings::PowerMode> {
        use wisp_core::winsettings::PowerMode;
        let mut g = GUID::zeroed();
        if unsafe { PowerGetEffectiveOverlayScheme(&mut g) } != 0 {
            return None;
        }
        Some(if g == EFFICIENCY {
            PowerMode::Efficiency
        } else if g == PERFORMANCE {
            PowerMode::Performance
        } else {
            PowerMode::Balanced
        })
    }

    fn set_power_mode(m: wisp_core::winsettings::PowerMode) -> Result<()> {
        use wisp_core::winsettings::PowerMode;
        let g = match m {
            PowerMode::Efficiency => EFFICIENCY,
            PowerMode::Balanced => BALANCED,
            PowerMode::Performance => PERFORMANCE,
        };
        match unsafe { PowerSetActiveOverlayScheme(&g) } {
            0 => Ok(()),
            rc => Err(WispError::new(
                ErrorCode::Unsupported,
                format!("Windows didn't change the power mode (error {rc}). On some PCs it's only available on battery or when plugged in."),
            )),
        }
    }

    // ---- Sound devices -----------------------------------------------------

    /// Undocumented but long-stable interface Windows' own sound settings
    /// use to change the default device.
    #[windows::core::interface("f8679f50-850a-41cf-9c72-430f290290c8")]
    unsafe trait IPolicyConfig: windows::core::IUnknown {
        fn GetMixFormat(&self, a: PCWSTR, b: *mut *mut core::ffi::c_void) -> HRESULT;
        fn GetDeviceFormat(&self, a: PCWSTR, b: i32, c: *mut *mut core::ffi::c_void) -> HRESULT;
        fn ResetDeviceFormat(&self, a: PCWSTR) -> HRESULT;
        fn SetDeviceFormat(
            &self,
            a: PCWSTR,
            b: *mut core::ffi::c_void,
            c: *mut core::ffi::c_void,
        ) -> HRESULT;
        fn GetProcessingPeriod(&self, a: PCWSTR, b: i32, c: *mut i64, d: *mut i64) -> HRESULT;
        fn SetProcessingPeriod(&self, a: PCWSTR, b: *mut i64) -> HRESULT;
        fn GetShareMode(&self, a: PCWSTR, b: *mut core::ffi::c_void) -> HRESULT;
        fn SetShareMode(&self, a: PCWSTR, b: *mut core::ffi::c_void) -> HRESULT;
        fn GetPropertyValue(
            &self,
            a: PCWSTR,
            b: *const core::ffi::c_void,
            c: *mut core::ffi::c_void,
        ) -> HRESULT;
        fn SetPropertyValue(
            &self,
            a: PCWSTR,
            b: *const core::ffi::c_void,
            c: *mut core::ffi::c_void,
        ) -> HRESULT;
        fn SetDefaultEndpoint(&self, id: PCWSTR, role: i32) -> HRESULT;
    }

    const POLICY_CONFIG_CLIENT: GUID = GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2bc9);

    fn enumerator() -> Option<IMMDeviceEnumerator> {
        unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok() }
    }

    fn outputs() -> Vec<wisp_core::winsettings::AudioDevice> {
        use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
        let Some(e) = enumerator() else {
            return Vec::new();
        };
        unsafe {
            let default_id = e
                .GetDefaultAudioEndpoint(eRender, eConsole)
                .and_then(|d| d.GetId())
                .ok()
                .and_then(|p| p.to_string().ok())
                .unwrap_or_default();
            let Ok(list) = e.EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE) else {
                return Vec::new();
            };
            let n = list.GetCount().unwrap_or(0);
            (0..n.min(16))
                .filter_map(|i| {
                    let d = list.Item(i).ok()?;
                    let id = d.GetId().ok()?.to_string().ok()?;
                    let store = d.OpenPropertyStore(STGM_READ).ok()?;
                    let name = store.GetValue(&PKEY_Device_FriendlyName).ok()?.to_string();
                    Some(wisp_core::winsettings::AudioDevice {
                        default: id == default_id,
                        id,
                        name,
                    })
                })
                .collect()
        }
    }

    fn set_output(query: &str) -> Result<String> {
        let devices = outputs();
        let d = wisp_core::winsettings::match_device(query, &devices).ok_or_else(|| {
            WispError::invalid(format!(
                "No single sound output matches \"{}\". Available: {}.",
                wisp_core::redact::summarize_text(query, 40),
                devices
                    .iter()
                    .map(|d| d.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        })?;
        let id = HSTRING::from(d.id.as_str());
        unsafe {
            let policy: IPolicyConfig = CoCreateInstance(&POLICY_CONFIG_CLIENT, None, CLSCTX_ALL)
                .map_err(|e| refused(e.code().to_string()))?;
            for role in [eConsole, eMultimedia, eCommunications] {
                policy
                    .SetDefaultEndpoint(PCWSTR(id.as_ptr()), role.0)
                    .ok()
                    .map_err(|e| refused(e.code().to_string()))?;
            }
        }
        Ok(d.name.clone())
    }

    fn mic() -> Option<IAudioEndpointVolume> {
        unsafe {
            enumerator()?
                .GetDefaultAudioEndpoint(eCapture, eConsole)
                .ok()?
                .Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None)
                .ok()
        }
    }

    // ---- Public ------------------------------------------------------------

    fn init_com() {
        unsafe {
            use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        }
    }

    pub fn state() -> WinSettingsState {
        use wisp_core::winsettings::TaskbarAlign;
        init_com();
        let night = get_binary(NIGHT_LIGHT, w!("Data"))
            .and_then(|b| wisp_core::winsettings::night_light_is_on(&b));
        WinSettingsState {
            supported: true,
            apps_dark: get_dword(PERSONALIZE, w!("AppsUseLightTheme")).map(|v| v == 0),
            system_dark: get_dword(PERSONALIZE, w!("SystemUsesLightTheme")).map(|v| v == 0),
            transparency: get_dword(PERSONALIZE, w!("EnableTransparency")).map(|v| v != 0),
            accent_color: accent_hex(),
            accent_from_wallpaper: Some(get_dword(DESKTOP, w!("AutoColorization")) == Some(1)),
            accent_on_taskbar: get_dword(PERSONALIZE, w!("ColorPrevalence")).map(|v| v != 0),
            taskbar_align: Some(match get_dword(ADVANCED, w!("TaskbarAl")) {
                Some(0) => TaskbarAlign::Left,
                _ => TaskbarAlign::Center,
            }),
            taskbar_auto_hide: auto_hide(),
            desktop_icons: desktop_icons_visible()
                .or_else(|| get_dword(ADVANCED, w!("HideIcons")).map(|v| v == 0)),
            start_recent: Some(get_dword(ADVANCED, w!("Start_TrackDocs")) != Some(0)),
            brightness: brightness(),
            night_light: night,
            do_not_disturb: Some(
                get_dword(NOTIFICATIONS, w!("NOC_GLOBAL_SETTING_TOASTS_ENABLED")) == Some(0),
            ),
            power_mode: power_mode(),
            outputs: outputs(),
            mic_muted: mic()
                .and_then(|m| unsafe { m.GetMute() }.ok())
                .map(|b| b.as_bool()),
        }
    }

    pub fn apply(change: &WinChange) -> Result<String> {
        init_com();
        match change {
            WinChange::DarkMode(d) => {
                set_dword(PERSONALIZE, w!("AppsUseLightTheme"), u32::from(!d))?;
                set_dword(PERSONALIZE, w!("SystemUsesLightTheme"), u32::from(!d))?;
                broadcast("ImmersiveColorSet");
            }
            WinChange::AppsDark(d) => {
                set_dword(PERSONALIZE, w!("AppsUseLightTheme"), u32::from(!d))?;
                broadcast("ImmersiveColorSet");
            }
            WinChange::SystemDark(d) => {
                set_dword(PERSONALIZE, w!("SystemUsesLightTheme"), u32::from(!d))?;
                broadcast("ImmersiveColorSet");
            }
            WinChange::Transparency(on) => {
                set_dword(PERSONALIZE, w!("EnableTransparency"), u32::from(*on))?;
                broadcast("ImmersiveColorSet");
            }
            WinChange::AccentColor(hex) => set_accent(hex)?,
            WinChange::AccentFromWallpaper(on) => {
                set_dword(DESKTOP, w!("AutoColorization"), u32::from(*on))?;
                broadcast("ImmersiveColorSet");
            }
            WinChange::AccentOnTaskbar(on) => {
                set_dword(PERSONALIZE, w!("ColorPrevalence"), u32::from(*on))?;
                broadcast("ImmersiveColorSet");
            }
            WinChange::TaskbarAlign(a) => {
                let v = u32::from(*a == wisp_core::winsettings::TaskbarAlign::Center);
                set_dword(ADVANCED, w!("TaskbarAl"), v)?;
                broadcast("TraySettings");
            }
            WinChange::TaskbarAutoHide(on) => set_auto_hide(*on),
            WinChange::DesktopIcons(show) => set_desktop_icons(*show)?,
            WinChange::StartRecent(on) => {
                set_dword(ADVANCED, w!("Start_TrackDocs"), u32::from(*on))?;
                broadcast("TraySettings");
            }
            WinChange::Brightness(v) => set_brightness(*v)?,
            WinChange::NightLight(on) => {
                let blob = get_binary(NIGHT_LIGHT, w!("Data")).ok_or_else(|| {
                    WispError::new(
                        ErrorCode::Unsupported,
                        "Turn night light on once in Windows Settings first, then Wisp can switch it.",
                    )
                })?;
                let next = wisp_core::winsettings::night_light_set(&blob, *on).ok_or_else(|| {
                    WispError::new(
                        ErrorCode::Unsupported,
                        "This version of Windows stores night light differently. Use Settings → Display → Night light.",
                    )
                })?;
                set_value(NIGHT_LIGHT, w!("Data"), REG_BINARY, &next)?;
            }
            WinChange::DoNotDisturb(on) => {
                set_dword(
                    NOTIFICATIONS,
                    w!("NOC_GLOBAL_SETTING_TOASTS_ENABLED"),
                    u32::from(!on),
                )?;
                broadcast("Notifications");
            }
            WinChange::PowerMode(m) => set_power_mode(*m)?,
            WinChange::OutputDevice(q) => {
                let name = set_output(q)?;
                return Ok(format!("Sound now plays through {name}."));
            }
            WinChange::MicMute(on) => {
                let m = mic().ok_or_else(|| WispError::invalid("No microphone found"))?;
                unsafe { m.SetMute(*on, std::ptr::null()) }
                    .map_err(|e| refused(e.code().to_string()))?;
            }
        }
        Ok(format!("Done: {}.", change.describe().to_lowercase()))
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;
    pub fn state() -> WinSettingsState {
        WinSettingsState::default()
    }
    pub fn apply(_change: &WinChange) -> Result<String> {
        Err(unsupported())
    }
}

pub use imp::{apply, state};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_change_has_a_settings_page() {
        for c in [
            WinChange::NightLight(true),
            WinChange::TaskbarAutoHide(true),
            WinChange::OutputDevice("x".into()),
        ] {
            assert!(settings_page(&c).starts_with("ms-settings:"));
        }
        #[cfg(not(windows))]
        assert!(apply(&WinChange::NightLight(true)).is_err());
    }
}
