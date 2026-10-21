//! Desktop wallpaper: set a local image the user picked, and switch between
//! recently used ones. Only image files are accepted.

use std::path::Path;
use wisp_core::{ErrorCode, Result, WispError};

pub const MAX_RECENT: usize = 12;
const EXTENSIONS: [&str; 4] = ["jpg", "jpeg", "png", "bmp"];

pub fn validate(path: &str) -> Result<()> {
    let p = Path::new(path);
    let ext_ok = p
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| EXTENSIONS.iter().any(|x| x.eq_ignore_ascii_case(e)));
    if !ext_ok || !p.is_absolute() {
        return Err(WispError::invalid("Choose a JPG, PNG or BMP image."));
    }
    if !p.is_file() {
        return Err(WispError::invalid("That image no longer exists."));
    }
    Ok(())
}

/// Most-recent-first list with `path` added, deduplicated and capped.
pub fn push_recent(recent: &[String], path: &str) -> Vec<String> {
    let mut out = vec![path.to_string()];
    out.extend(
        recent
            .iter()
            .filter(|p| !p.eq_ignore_ascii_case(path))
            .cloned(),
    );
    out.truncate(MAX_RECENT);
    out
}

/// Small JPEG preview as a data URL (decoded locally).
pub fn thumbnail(path: &str) -> Result<String> {
    use base64::Engine;
    let img = image::open(path)
        .map_err(|_| WispError::new(ErrorCode::Internal, "Couldn't read that image."))?;
    let thumb = img.thumbnail(320, 180).to_rgb8();
    let mut out = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgb8(thumb)
        .write_to(&mut out, image::ImageFormat::Jpeg)
        .map_err(|e| WispError::internal(e.to_string()))?;
    Ok(format!(
        "data:image/jpeg;base64,{}",
        base64::engine::general_purpose::STANDARD.encode(out.into_inner())
    ))
}

#[cfg(windows)]
pub fn set(path: &str) -> Result<()> {
    use windows::Win32::UI::WindowsAndMessaging::{
        SystemParametersInfoW, SPIF_SENDCHANGE, SPIF_UPDATEINIFILE, SPI_SETDESKWALLPAPER,
    };
    validate(path)?;
    let mut wide: Vec<u16> = path.encode_utf16().chain(Some(0)).collect();
    unsafe {
        SystemParametersInfoW(
            SPI_SETDESKWALLPAPER,
            0,
            Some(wide.as_mut_ptr() as *mut core::ffi::c_void),
            SPIF_UPDATEINIFILE | SPIF_SENDCHANGE,
        )
    }
    .map_err(|e| {
        WispError::new(
            ErrorCode::Internal,
            format!("Windows refused the wallpaper: {e}"),
        )
    })
}

#[cfg(windows)]
pub fn current() -> Option<String> {
    use windows::Win32::UI::WindowsAndMessaging::{
        SystemParametersInfoW, SPI_GETDESKWALLPAPER, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
    };
    let mut buf = [0u16; 520];
    unsafe {
        SystemParametersInfoW(
            SPI_GETDESKWALLPAPER,
            buf.len() as u32,
            Some(buf.as_mut_ptr() as *mut core::ffi::c_void),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    }
    .ok()?;
    let len = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
    let s = String::from_utf16_lossy(&buf[..len]);
    (!s.is_empty()).then_some(s)
}

#[cfg(not(windows))]
pub fn set(path: &str) -> Result<()> {
    validate(path)?;
    Err(WispError::new(
        ErrorCode::Unsupported,
        "Setting the wallpaper is only available on Windows.",
    ))
}

#[cfg(not(windows))]
pub fn current() -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_and_tracks_recent() {
        let dir = tempfile::tempdir().unwrap();
        let img = dir.path().join("a.png");
        image::RgbImage::from_pixel(640, 360, image::Rgb([10, 20, 30]))
            .save(&img)
            .unwrap();
        let p = img.to_string_lossy().to_string();
        assert!(validate(&p).is_ok());
        assert!(validate("relative.png").is_err());
        assert!(validate(&dir.path().join("x.exe").to_string_lossy()).is_err());
        assert!(validate(&dir.path().join("missing.png").to_string_lossy()).is_err());
        assert!(thumbnail(&p)
            .unwrap()
            .starts_with("data:image/jpeg;base64,"));

        let recent: Vec<String> = (0..12).map(|i| format!("/w/{i}.png")).collect();
        let next = push_recent(&recent, "/w/5.png");
        assert_eq!(next[0], "/w/5.png");
        assert_eq!(next.len(), 12);
        assert_eq!(next.iter().filter(|p| *p == "/w/5.png").count(), 1);
        assert_eq!(push_recent(&recent, "/new.png").len(), MAX_RECENT);
    }
}
