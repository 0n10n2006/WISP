//! Geometry for Select-to-Ask: map a selection drawn in the overlay (CSS
//! pixels) onto the captured monitor image (physical pixels).

use crate::error::{Result, WispError};
use serde::{Deserialize, Serialize};

/// Smallest selection (in image pixels) worth sending.
pub const MIN_SIDE: u32 = 8;
/// Longest side of the image sent to Gemini; larger crops are downscaled.
pub const MAX_SEND_SIDE: u32 = 2048;

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    /// Size of the overlay viewport the selection was drawn in.
    pub view_width: f64,
    pub view_height: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PixelRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

/// Convert and clamp a viewport selection to image pixel coordinates.
pub fn to_pixels(sel: &SelectionRect, img_w: u32, img_h: u32) -> Result<PixelRect> {
    let vals = [
        sel.x,
        sel.y,
        sel.width,
        sel.height,
        sel.view_width,
        sel.view_height,
    ];
    if vals.iter().any(|v| !v.is_finite()) || sel.view_width <= 0.0 || sel.view_height <= 0.0 {
        return Err(WispError::invalid("Invalid selection"));
    }
    // Normalise rectangles drawn right-to-left or bottom-to-top.
    let (x0, x1) = order(sel.x, sel.x + sel.width);
    let (y0, y1) = order(sel.y, sel.y + sel.height);
    let sx = img_w as f64 / sel.view_width;
    let sy = img_h as f64 / sel.view_height;
    let px0 = (x0 * sx).floor().clamp(0.0, img_w as f64) as u32;
    let py0 = (y0 * sy).floor().clamp(0.0, img_h as f64) as u32;
    let px1 = (x1 * sx).ceil().clamp(0.0, img_w as f64) as u32;
    let py1 = (y1 * sy).ceil().clamp(0.0, img_h as f64) as u32;
    let width = px1.saturating_sub(px0);
    let height = py1.saturating_sub(py0);
    if width < MIN_SIDE || height < MIN_SIDE {
        return Err(WispError::invalid(
            "That selection is too small. Drag to select an area.",
        ));
    }
    Ok(PixelRect {
        x: px0,
        y: py0,
        width,
        height,
    })
}

fn order(a: f64, b: f64) -> (f64, f64) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
}

/// Dimensions that fit within `max` on the longest side, keeping aspect ratio.
pub fn fit_within(width: u32, height: u32, max: u32) -> (u32, u32) {
    let longest = width.max(height);
    if longest <= max {
        return (width, height);
    }
    let scale = max as f64 / longest as f64;
    (
        ((width as f64 * scale).round() as u32).max(1),
        ((height as f64 * scale).round() as u32).max(1),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sel(x: f64, y: f64, w: f64, h: f64) -> SelectionRect {
        SelectionRect {
            x,
            y,
            width: w,
            height: h,
            view_width: 1280.0,
            view_height: 720.0,
        }
    }

    #[test]
    fn scales_to_physical_pixels() {
        // 150% scaling: 1280x720 CSS px over a 1920x1080 image.
        let r = to_pixels(&sel(100.0, 50.0, 200.0, 100.0), 1920, 1080).unwrap();
        assert_eq!(
            r,
            PixelRect {
                x: 150,
                y: 75,
                width: 300,
                height: 150
            }
        );
    }

    #[test]
    fn normalises_reverse_drags_and_clamps() {
        let r = to_pixels(&sel(300.0, 200.0, -200.0, -100.0), 1280, 720).unwrap();
        assert_eq!(
            r,
            PixelRect {
                x: 100,
                y: 100,
                width: 200,
                height: 100
            }
        );
        let r = to_pixels(&sel(1200.0, 650.0, 500.0, 500.0), 1280, 720).unwrap();
        assert_eq!((r.x + r.width, r.y + r.height), (1280, 720));
    }

    #[test]
    fn rejects_tiny_or_invalid() {
        assert!(to_pixels(&sel(10.0, 10.0, 3.0, 50.0), 1280, 720).is_err());
        assert!(to_pixels(&sel(f64::NAN, 10.0, 30.0, 50.0), 1280, 720).is_err());
        let mut s = sel(0.0, 0.0, 10.0, 10.0);
        s.view_width = 0.0;
        assert!(to_pixels(&s, 1280, 720).is_err());
        // Entirely outside the image.
        assert!(to_pixels(&sel(2000.0, 2000.0, 50.0, 50.0), 1280, 720).is_err());
    }

    #[test]
    fn fits_long_side() {
        assert_eq!(fit_within(1000, 500, 2048), (1000, 500));
        assert_eq!(fit_within(4096, 1024, 2048), (2048, 512));
        assert_eq!(fit_within(1000, 3000, 1500), (500, 1500));
    }
}
