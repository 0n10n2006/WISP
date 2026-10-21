//! Quick controls for the dashboard: volume, media, Wi-Fi/Bluetooth radios and
//! light/dark mode. Windows-only; other platforms report "unsupported".

use serde::{Deserialize, Serialize};
use wisp_core::{ErrorCode, Result, WispError};

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NowPlaying {
    pub title: String,
    pub artist: String,
    pub playing: bool,
    pub app: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RadioInfo {
    /// "wifi" | "bluetooth"
    pub kind: String,
    pub name: String,
    pub on: bool,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ControlsState {
    pub supported: bool,
    /// 0–100, or `None` when no output device is available.
    pub volume: Option<u8>,
    pub muted: bool,
    pub now_playing: Option<NowPlaying>,
    pub radios: Vec<RadioInfo>,
    pub dark_mode: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MediaCommand {
    PlayPause,
    Next,
    Previous,
}

fn unsupported() -> WispError {
    WispError::new(
        ErrorCode::Unsupported,
        "This control isn't available on this system.",
    )
}

#[cfg(windows)]
mod imp {
    use super::*;
    use windows::Devices::Radios::{Radio, RadioKind, RadioState};
    use windows::Media::Control::{
        GlobalSystemMediaTransportControlsSessionManager as Manager,
        GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
    };
    use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
    use windows::Win32::Media::Audio::{
        eConsole, eRender, IMMDeviceEnumerator, MMDeviceEnumerator,
    };
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED,
    };

    fn endpoint() -> Option<IAudioEndpointVolume> {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let e: IMMDeviceEnumerator =
                CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL).ok()?;
            let device = e.GetDefaultAudioEndpoint(eRender, eConsole).ok()?;
            device
                .Activate::<IAudioEndpointVolume>(CLSCTX_ALL, None)
                .ok()
        }
    }

    fn com_err(e: windows::core::Error) -> WispError {
        WispError::new(
            ErrorCode::Internal,
            format!("Windows refused the change ({})", e.code()),
        )
    }

    pub fn set_volume(percent: u8) -> Result<()> {
        let v = endpoint().ok_or_else(|| {
            WispError::new(ErrorCode::Unsupported, "No audio output device found.")
        })?;
        unsafe {
            v.SetMasterVolumeLevelScalar(percent.min(100) as f32 / 100.0, std::ptr::null())
                .map_err(com_err)?;
            if percent > 0 {
                let _ = v.SetMute(0, std::ptr::null());
            }
        }
        Ok(())
    }

    pub fn set_muted(muted: bool) -> Result<()> {
        let v = endpoint().ok_or_else(|| {
            WispError::new(ErrorCode::Unsupported, "No audio output device found.")
        })?;
        unsafe {
            v.SetMute(u8::from(muted), std::ptr::null())
                .map_err(com_err)
        }
    }

    fn now_playing() -> Option<NowPlaying> {
        let mgr = Manager::RequestAsync().ok()?.join().ok()?;
        let session = mgr.GetCurrentSession().ok()?;
        let props = session.TryGetMediaPropertiesAsync().ok()?.join().ok()?;
        let playing = session
            .GetPlaybackInfo()
            .and_then(|i| i.PlaybackStatus())
            .map(|s| s == Status::Playing)
            .unwrap_or(false);
        let app = session
            .SourceAppUserModelId()
            .map(|s| s.to_string())
            .unwrap_or_default();
        Some(NowPlaying {
            title: props.Title().map(|s| s.to_string()).unwrap_or_default(),
            artist: props.Artist().map(|s| s.to_string()).unwrap_or_default(),
            playing,
            app: app
                .rsplit(['!', '\\'])
                .next()
                .unwrap_or(&app)
                .trim_end_matches(".exe")
                .to_string(),
        })
    }

    pub fn media(cmd: MediaCommand) -> Result<()> {
        let session = Manager::RequestAsync()
            .and_then(|op| op.join())
            .and_then(|m| m.GetCurrentSession())
            .map_err(|_| WispError::new(ErrorCode::Unsupported, "Nothing is playing right now."))?;
        let op = match cmd {
            MediaCommand::PlayPause => session.TryTogglePlayPauseAsync(),
            MediaCommand::Next => session.TrySkipNextAsync(),
            MediaCommand::Previous => session.TrySkipPreviousAsync(),
        };
        op.and_then(|o| o.join()).map(|_| ()).map_err(com_err)
    }

    fn radios() -> Vec<(Radio, String)> {
        let Ok(list) = Radio::GetRadiosAsync().and_then(|op| op.join()) else {
            return Vec::new();
        };
        let mut out = Vec::new();
        for i in 0..list.Size().unwrap_or(0) {
            let Ok(r) = list.GetAt(i) else { continue };
            let kind = match r.Kind() {
                Ok(RadioKind::WiFi) => "wifi",
                Ok(RadioKind::Bluetooth) => "bluetooth",
                _ => continue,
            };
            out.push((r, kind.to_string()));
        }
        out
    }

    pub fn set_radio(kind: &str, on: bool) -> Result<()> {
        let _ = Radio::RequestAccessAsync().and_then(|op| op.join());
        let target = if on { RadioState::On } else { RadioState::Off };
        let mut found = false;
        for (r, k) in radios() {
            if k == kind {
                found = true;
                r.SetStateAsync(target)
                    .and_then(|op| op.join())
                    .map_err(com_err)?;
            }
        }
        if found {
            Ok(())
        } else {
            Err(WispError::new(
                ErrorCode::Unsupported,
                "That radio isn't available on this PC.",
            ))
        }
    }

    pub fn state() -> ControlsState {
        let (volume, muted) = endpoint()
            .and_then(|v| unsafe {
                let level = v.GetMasterVolumeLevelScalar().ok()?;
                let muted = v.GetMute().map(|m| m != 0).unwrap_or(false);
                Some((Some((level * 100.0).round() as u8), muted))
            })
            .unwrap_or((None, false));
        ControlsState {
            supported: true,
            volume,
            muted,
            now_playing: now_playing(),
            radios: radios()
                .into_iter()
                .map(|(r, kind)| RadioInfo {
                    name: r
                        .Name()
                        .map(|n| n.to_string())
                        .unwrap_or_else(|_| kind.clone()),
                    on: r.State().map(|s| s == RadioState::On).unwrap_or(false),
                    kind,
                })
                .collect(),
            dark_mode: !crate::system_theme::get().apps_light,
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::*;
    pub fn set_volume(_p: u8) -> Result<()> {
        Err(unsupported())
    }
    pub fn set_muted(_m: bool) -> Result<()> {
        Err(unsupported())
    }
    pub fn media(_c: MediaCommand) -> Result<()> {
        Err(unsupported())
    }
    pub fn set_radio(_k: &str, _on: bool) -> Result<()> {
        Err(unsupported())
    }
    pub fn state() -> ControlsState {
        ControlsState::default()
    }
}

pub use imp::{media, set_muted, set_radio, set_volume, state};

/// Switch Windows between light and dark mode.
pub fn set_dark_mode(dark: bool) -> Result<()> {
    if crate::system_theme::set(!dark) {
        Ok(())
    } else {
        Err(unsupported())
    }
}
