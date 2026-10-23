//! Windows settings Wisp can change: the typed set of changes, how to parse
//! them from a request (the command bar or the model), how to describe them
//! for a confirmation, and the pure helpers the Windows code relies on
//! (accent palettes, the night-light state blob). No shell is ever involved.

use crate::error::{Result, WispError};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskbarAlign {
    Left,
    Center,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PowerMode {
    Efficiency,
    Balanced,
    Performance,
}

impl PowerMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Efficiency => "best power efficiency",
            Self::Balanced => "balanced",
            Self::Performance => "best performance",
        }
    }

    /// The next mode in the cycle used by the control-centre button.
    pub fn next(self) -> Self {
        match self {
            Self::Efficiency => Self::Balanced,
            Self::Balanced => Self::Performance,
            Self::Performance => Self::Efficiency,
        }
    }
}

/// One change to a Windows setting.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "setting", content = "value", rename_all = "camelCase")]
pub enum WinChange {
    /// Apps and Windows together.
    DarkMode(bool),
    AppsDark(bool),
    SystemDark(bool),
    Transparency(bool),
    /// `#rrggbb`.
    AccentColor(String),
    AccentFromWallpaper(bool),
    AccentOnTaskbar(bool),
    TaskbarAlign(TaskbarAlign),
    TaskbarAutoHide(bool),
    DesktopIcons(bool),
    StartRecent(bool),
    /// 0–100.
    Brightness(u8),
    NightLight(bool),
    DoNotDisturb(bool),
    PowerMode(PowerMode),
    /// A device name (or part of it) as the user said it, or a device id.
    OutputDevice(String),
    MicMute(bool),
}

/// Settings names the model and the bar may use, with their value hints.
pub const SETTINGS: &[(&str, &str)] = &[
    ("dark_mode", "on/off"),
    ("apps_dark", "on/off"),
    ("system_dark", "on/off"),
    ("transparency", "on/off"),
    ("accent_color", "#rrggbb or a colour name"),
    ("accent_from_wallpaper", "on/off"),
    ("accent_on_taskbar", "on/off"),
    ("taskbar_align", "left/center"),
    ("taskbar_auto_hide", "on/off"),
    ("desktop_icons", "on/off"),
    ("start_recent", "on/off"),
    ("brightness", "0-100"),
    ("night_light", "on/off"),
    ("do_not_disturb", "on/off"),
    ("power_mode", "efficiency/balanced/performance"),
    ("output_device", "device name, e.g. headphones"),
    ("mic_mute", "on/off"),
];

const COLOURS: &[(&str, &str)] = &[
    ("blue", "#0078d4"),
    ("navy", "#003e92"),
    ("teal", "#00b7c3"),
    ("cyan", "#00b7c3"),
    ("green", "#107c10"),
    ("mint", "#00cc6a"),
    ("lime", "#8cbd18"),
    ("yellow", "#ffb900"),
    ("gold", "#ffb900"),
    ("orange", "#f7630c"),
    ("red", "#e81123"),
    ("pink", "#e3008c"),
    ("magenta", "#c239b3"),
    ("purple", "#744da9"),
    ("violet", "#8764b8"),
    ("lavender", "#8e8cd8"),
    ("grey", "#7a7574"),
    ("gray", "#7a7574"),
    ("black", "#2d2d2d"),
];

fn parse_bool(v: &str) -> Result<bool> {
    match v.trim().to_ascii_lowercase().as_str() {
        "on" | "true" | "yes" | "1" | "enable" | "enabled" | "show" | "dark" => Ok(true),
        "off" | "false" | "no" | "0" | "disable" | "disabled" | "hide" | "light" => Ok(false),
        _ => Err(WispError::invalid("Use on or off")),
    }
}

/// `#RGB`/`#rrggbb`/colour name → lowercase `#rrggbb`.
pub fn parse_colour(v: &str) -> Result<String> {
    let v = v.trim().to_ascii_lowercase();
    if let Some((_, hex)) = COLOURS.iter().find(|(n, _)| *n == v) {
        return Ok((*hex).to_string());
    }
    let hex = v.strip_prefix('#').unwrap_or(&v);
    let full = match hex.len() {
        3 => hex.chars().flat_map(|c| [c, c]).collect::<String>(),
        6 => hex.to_string(),
        _ => return Err(WispError::invalid("Use a colour like #0078d4 or blue")),
    };
    if !full.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(WispError::invalid("Use a colour like #0078d4 or blue"));
    }
    Ok(format!("#{full}"))
}

impl WinChange {
    /// Parse `setting` = `value` from the bar or the model.
    pub fn parse(setting: &str, value: &str) -> Result<Self> {
        let s = setting.trim().to_ascii_lowercase().replace([' ', '-'], "_");
        let v = value.trim();
        Ok(match s.as_str() {
            "dark_mode" | "theme" => Self::DarkMode(parse_bool(v)?),
            "apps_dark" => Self::AppsDark(parse_bool(v)?),
            "system_dark" => Self::SystemDark(parse_bool(v)?),
            "transparency" => Self::Transparency(parse_bool(v)?),
            "accent_color" | "accent_colour" | "accent" => Self::AccentColor(parse_colour(v)?),
            "accent_from_wallpaper" => Self::AccentFromWallpaper(parse_bool(v)?),
            "accent_on_taskbar" => Self::AccentOnTaskbar(parse_bool(v)?),
            "taskbar_align" | "taskbar_alignment" => {
                Self::TaskbarAlign(match v.to_ascii_lowercase().as_str() {
                    "left" | "start" => TaskbarAlign::Left,
                    "center" | "centre" | "middle" => TaskbarAlign::Center,
                    _ => return Err(WispError::invalid("Use left or center")),
                })
            }
            "taskbar_auto_hide" | "auto_hide" => Self::TaskbarAutoHide(parse_bool(v)?),
            "desktop_icons" => Self::DesktopIcons(parse_bool(v)?),
            "start_recent" => Self::StartRecent(parse_bool(v)?),
            "brightness" => {
                let n: u8 = v
                    .trim_end_matches('%')
                    .trim()
                    .parse()
                    .map_err(|_| WispError::invalid("Brightness must be 0 to 100"))?;
                if n > 100 {
                    return Err(WispError::invalid("Brightness must be 0 to 100"));
                }
                Self::Brightness(n)
            }
            "night_light" => Self::NightLight(parse_bool(v)?),
            "do_not_disturb" | "dnd" | "focus" => Self::DoNotDisturb(parse_bool(v)?),
            "power_mode" | "power" => Self::PowerMode(match v.to_ascii_lowercase().as_str() {
                "efficiency" | "best_efficiency" | "battery" | "saver" | "eco" => {
                    PowerMode::Efficiency
                }
                "balanced" | "normal" => PowerMode::Balanced,
                "performance" | "best_performance" | "max" => PowerMode::Performance,
                _ => {
                    return Err(WispError::invalid(
                        "Use efficiency, balanced or performance",
                    ))
                }
            }),
            "output_device" | "audio_output" | "speakers" => {
                let name = v.trim();
                if name.is_empty()
                    || name.chars().count() > 120
                    || name.chars().any(char::is_control)
                {
                    return Err(WispError::invalid("Which output device?"));
                }
                Self::OutputDevice(name.to_string())
            }
            "mic_mute" | "microphone_mute" => Self::MicMute(parse_bool(v)?),
            _ => return Err(WispError::invalid("Wisp can't change that setting")),
        })
    }

    /// Validate a change that arrived from the UI.
    pub fn validated(self) -> Result<Self> {
        match &self {
            Self::AccentColor(c) => Ok(Self::AccentColor(parse_colour(c)?)),
            Self::Brightness(n) if *n > 100 => {
                Err(WispError::invalid("Brightness must be 0 to 100"))
            }
            Self::OutputDevice(n)
                if n.trim().is_empty() || n.len() > 400 || n.chars().any(char::is_control) =>
            {
                Err(WispError::invalid("Which output device?"))
            }
            _ => Ok(self),
        }
    }

    /// For confirmations and the action log: "Turn on night light".
    pub fn describe(&self) -> String {
        let onoff = |b: bool| if b { "on" } else { "off" };
        match self {
            Self::DarkMode(d) => format!(
                "Switch Windows to {} mode",
                if *d { "dark" } else { "light" }
            ),
            Self::AppsDark(d) => {
                format!("Switch apps to {} mode", if *d { "dark" } else { "light" })
            }
            Self::SystemDark(d) => format!(
                "Switch the taskbar and Start to {} mode",
                if *d { "dark" } else { "light" }
            ),
            Self::Transparency(b) => format!("Turn transparency effects {}", onoff(*b)),
            Self::AccentColor(c) => format!("Set the accent colour to {c}"),
            Self::AccentFromWallpaper(b) => format!(
                "{} the accent colour from the wallpaper",
                if *b { "Pick" } else { "Stop picking" }
            ),
            Self::AccentOnTaskbar(b) => {
                format!(
                    "Turn the accent colour on Start and the taskbar {}",
                    onoff(*b)
                )
            }
            Self::TaskbarAlign(a) => format!(
                "Move the taskbar icons to the {}",
                match a {
                    TaskbarAlign::Left => "left",
                    TaskbarAlign::Center => "centre",
                }
            ),
            Self::TaskbarAutoHide(b) => format!("Turn taskbar auto-hide {}", onoff(*b)),
            Self::DesktopIcons(b) => format!("{} desktop icons", if *b { "Show" } else { "Hide" }),
            Self::StartRecent(b) => format!("Turn recent items in Start {}", onoff(*b)),
            Self::Brightness(n) => format!("Set brightness to {n}%"),
            Self::NightLight(b) => format!("Turn night light {}", onoff(*b)),
            Self::DoNotDisturb(b) => format!("Turn Do Not Disturb {}", onoff(*b)),
            Self::PowerMode(m) => format!("Set the power mode to {}", m.label()),
            Self::OutputDevice(n) => format!("Play sound through {n}"),
            Self::MicMute(b) => format!("{} the microphone", if *b { "Mute" } else { "Unmute" }),
        }
    }

    /// Short machine name for the action log.
    pub fn key(&self) -> &'static str {
        match self {
            Self::DarkMode(_) => "dark_mode",
            Self::AppsDark(_) => "apps_dark",
            Self::SystemDark(_) => "system_dark",
            Self::Transparency(_) => "transparency",
            Self::AccentColor(_) => "accent_color",
            Self::AccentFromWallpaper(_) => "accent_from_wallpaper",
            Self::AccentOnTaskbar(_) => "accent_on_taskbar",
            Self::TaskbarAlign(_) => "taskbar_align",
            Self::TaskbarAutoHide(_) => "taskbar_auto_hide",
            Self::DesktopIcons(_) => "desktop_icons",
            Self::StartRecent(_) => "start_recent",
            Self::Brightness(_) => "brightness",
            Self::NightLight(_) => "night_light",
            Self::DoNotDisturb(_) => "do_not_disturb",
            Self::PowerMode(_) => "power_mode",
            Self::OutputDevice(_) => "output_device",
            Self::MicMute(_) => "mic_mute",
        }
    }
}

/// An audio output device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioDevice {
    pub id: String,
    pub name: String,
    pub default: bool,
}

/// Pick the device the user meant: exact id, exact name, then a name that
/// contains the words (e.g. "headphones").
pub fn match_device<'a>(query: &str, devices: &'a [AudioDevice]) -> Option<&'a AudioDevice> {
    let q = query.trim().to_lowercase();
    devices
        .iter()
        .find(|d| d.id == query || d.name.to_lowercase() == q)
        .or_else(|| {
            let words: Vec<&str> = q.split_whitespace().collect();
            let hits: Vec<&AudioDevice> = devices
                .iter()
                .filter(|d| {
                    let n = d.name.to_lowercase();
                    words.iter().all(|w| n.contains(w))
                        || (q.contains("headphone") && n.contains("headset"))
                        || (q.contains("headset") && n.contains("headphone"))
                })
                .collect();
            (hits.len() == 1).then(|| hits[0])
        })
}

/// Everything the control centre shows; `None` when unknown or unsupported.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WinSettingsState {
    pub supported: bool,
    pub apps_dark: Option<bool>,
    pub system_dark: Option<bool>,
    pub transparency: Option<bool>,
    pub accent_color: Option<String>,
    pub accent_from_wallpaper: Option<bool>,
    pub accent_on_taskbar: Option<bool>,
    pub taskbar_align: Option<TaskbarAlign>,
    pub taskbar_auto_hide: Option<bool>,
    pub desktop_icons: Option<bool>,
    pub start_recent: Option<bool>,
    pub brightness: Option<u8>,
    pub night_light: Option<bool>,
    pub do_not_disturb: Option<bool>,
    pub power_mode: Option<PowerMode>,
    pub outputs: Vec<AudioDevice>,
    pub mic_muted: Option<bool>,
}

// ---------------------------------------------------------------------------
// Accent colours

/// `#rrggbb` → (r, g, b).
pub fn rgb(hex: &str) -> Option<(u8, u8, u8)> {
    let h = hex.strip_prefix('#')?;
    if h.len() != 6 {
        return None;
    }
    let n = u32::from_str_radix(h, 16).ok()?;
    Some(((n >> 16) as u8, (n >> 8) as u8, n as u8))
}

/// Windows stores colours as 0xAABBGGRR.
pub fn abgr(r: u8, g: u8, b: u8) -> u32 {
    0xff00_0000 | (b as u32) << 16 | (g as u32) << 8 | r as u32
}

/// The 8-shade palette Settings writes for an accent (three lighter shades,
/// the colour, three darker, then a grey), each as RGBA bytes.
pub fn accent_palette(r: u8, g: u8, b: u8) -> [u8; 32] {
    let mix = |c: u8, t: u8, f: f32| (c as f32 + (t as f32 - c as f32) * f).round() as u8;
    let shade = |f: f32| -> [u8; 4] {
        if f >= 0.0 {
            [mix(r, 255, f), mix(g, 255, f), mix(b, 255, f), 0]
        } else {
            [mix(r, 0, -f), mix(g, 0, -f), mix(b, 0, -f), 0]
        }
    };
    let steps = [0.6, 0.4, 0.2, 0.0, -0.25, -0.45, -0.65];
    let mut out = [0u8; 32];
    for (i, f) in steps.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&shade(*f));
    }
    out[28..32].copy_from_slice(&[0x88, 0x88, 0x88, 0]);
    out
}

// ---------------------------------------------------------------------------
// Night light

/// The night-light state is a small CloudStore blob. Byte 18 says whether it
/// is on (0x15) or off (0x13); when on, two extra bytes (0x10 0x00) follow at
/// offset 23. Anything else is a format Wisp doesn't know and won't touch.
const NL_FLAG: usize = 18;
const NL_ON: u8 = 0x15;
const NL_OFF: u8 = 0x13;
const NL_EXTRA: usize = 23;

pub fn night_light_is_on(blob: &[u8]) -> Option<bool> {
    if blob.len() < 30 || blob[0] != 0x43 || blob[1] != 0x42 {
        return None;
    }
    match blob[NL_FLAG] {
        NL_ON if blob[NL_EXTRA] == 0x10 && blob[NL_EXTRA + 1] == 0x00 => Some(true),
        NL_OFF => Some(false),
        _ => None,
    }
}

/// The blob with night light switched `on`, or `None` for an unknown format.
pub fn night_light_set(blob: &[u8], on: bool) -> Option<Vec<u8>> {
    let current = night_light_is_on(blob)?;
    let mut out = blob.to_vec();
    if current != on {
        if on {
            out[NL_FLAG] = NL_ON;
            out.splice(NL_EXTRA..NL_EXTRA, [0x10, 0x00]);
        } else {
            out[NL_FLAG] = NL_OFF;
            out.drain(NL_EXTRA..NL_EXTRA + 2);
        }
    }
    // Bump the change timestamp so Windows notices the new state.
    for b in out.iter_mut().take(15).skip(10) {
        if *b != 0xff {
            *b += 1;
            break;
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_what_people_say() {
        assert_eq!(
            WinChange::parse("night light", "on").unwrap(),
            WinChange::NightLight(true)
        );
        assert_eq!(
            WinChange::parse("taskbar_align", "Left").unwrap(),
            WinChange::TaskbarAlign(TaskbarAlign::Left)
        );
        assert_eq!(
            WinChange::parse("brightness", "40%").unwrap(),
            WinChange::Brightness(40)
        );
        assert!(WinChange::parse("brightness", "140").is_err());
        assert_eq!(
            WinChange::parse("accent", "Blue").unwrap(),
            WinChange::AccentColor("#0078d4".into())
        );
        assert_eq!(
            WinChange::parse("accent", "#ABC").unwrap(),
            WinChange::AccentColor("#aabbcc".into())
        );
        assert!(WinChange::parse("accent", "#12345z").is_err());
        assert_eq!(
            WinChange::parse("power_mode", "best_performance").unwrap(),
            WinChange::PowerMode(PowerMode::Performance)
        );
        assert!(WinChange::parse("registry", "HKLM").is_err());
        assert!(WinChange::parse("output_device", "a\u{7}b").is_err());
        assert_eq!(
            WinChange::parse("dnd", "on").unwrap().describe(),
            "Turn Do Not Disturb on"
        );
    }

    #[test]
    fn serialises_for_the_ui() {
        let v = serde_json::to_value(WinChange::Brightness(40)).unwrap();
        assert_eq!(
            v,
            serde_json::json!({ "setting": "brightness", "value": 40 })
        );
        let c: WinChange =
            serde_json::from_value(serde_json::json!({ "setting": "nightLight", "value": true }))
                .unwrap();
        assert_eq!(c, WinChange::NightLight(true));
        assert!(WinChange::AccentColor("red".into()).validated().is_ok());
        assert!(WinChange::AccentColor("nope".into()).validated().is_err());
    }

    #[test]
    fn finds_the_device_meant() {
        let d = vec![
            AudioDevice {
                id: "1".into(),
                name: "Speakers (Realtek Audio)".into(),
                default: true,
            },
            AudioDevice {
                id: "2".into(),
                name: "Headset (WH-1000XM5)".into(),
                default: false,
            },
        ];
        assert_eq!(match_device("headphones", &d).unwrap().id, "2");
        assert_eq!(match_device("speakers", &d).unwrap().id, "1");
        assert!(match_device("hdmi", &d).is_none());
    }

    #[test]
    fn accent_colours_and_palettes() {
        assert_eq!(rgb("#0078d4"), Some((0, 0x78, 0xd4)));
        assert_eq!(abgr(0, 0x78, 0xd4), 0xffd4_7800);
        let p = accent_palette(0, 0x78, 0xd4);
        assert_eq!(&p[12..15], &[0, 0x78, 0xd4]);
        assert!(p[0] > p[12] && p[16] < p[12] + 1);
    }

    fn blob(on: bool) -> Vec<u8> {
        let mut b = vec![0x43, 0x42, 0x01, 0x00, 0x0a, 0x02, 0x01, 0x00, 0x2a, 0x06];
        b.extend([0x10, 0x20, 0x30, 0x40, 0x50, 0x2a, 0x2b, 0x0e]);
        b.push(if on { NL_ON } else { NL_OFF });
        b.extend([0x43, 0x42, 0x01, 0x00]);
        if on {
            b.extend([0x10, 0x00]);
        }
        b.extend([0xd0, 0x0a, 0x02, 0xc6, 0x14, 0x00, 0x00, 0x00]);
        b
    }

    #[test]
    fn night_light_blob_round_trips() {
        let off = blob(false);
        assert_eq!(night_light_is_on(&off), Some(false));
        let on = night_light_set(&off, true).unwrap();
        assert_eq!(night_light_is_on(&on), Some(true));
        assert_eq!(on.len(), off.len() + 2);
        assert_eq!(on[10], off[10] + 1);
        let back = night_light_set(&on, false).unwrap();
        assert_eq!(night_light_is_on(&back), Some(false));
        assert_eq!(back.len(), off.len());
        // Unknown formats are left alone.
        let mut odd = off.clone();
        odd[NL_FLAG] = 0x99;
        assert!(night_light_set(&odd, true).is_none());
        assert!(night_light_set(&[1, 2, 3], true).is_none());
    }
}
