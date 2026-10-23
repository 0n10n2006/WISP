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

/// A point in overlay (CSS pixel) coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

/// Bounding selection for a freeform lasso path drawn in the overlay.
pub fn lasso_bounds(points: &[Point], view_width: f64, view_height: f64) -> Result<SelectionRect> {
    if points.len() < 3 || points.len() > 5000 {
        return Err(WispError::invalid(
            "Draw around the area you want to ask about",
        ));
    }
    if points.iter().any(|p| !p.x.is_finite() || !p.y.is_finite()) {
        return Err(WispError::invalid("Invalid selection"));
    }
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for p in points {
        x0 = x0.min(p.x);
        y0 = y0.min(p.y);
        x1 = x1.max(p.x);
        y1 = y1.max(p.y);
    }
    Ok(SelectionRect {
        x: x0,
        y: y0,
        width: x1 - x0,
        height: y1 - y0,
        view_width,
        view_height,
    })
}

/// Map overlay points into the crop's pixel space (for masking).
pub fn polygon_in_crop(
    points: &[Point],
    sel: &SelectionRect,
    img_w: u32,
    img_h: u32,
    crop: PixelRect,
) -> Vec<(f64, f64)> {
    let sx = img_w as f64 / sel.view_width;
    let sy = img_h as f64 / sel.view_height;
    points
        .iter()
        .map(|p| (p.x * sx - crop.x as f64, p.y * sy - crop.y as f64))
        .collect()
}

/// Even-odd point-in-polygon test.
pub fn point_in_polygon(x: f64, y: f64, poly: &[(f64, f64)]) -> bool {
    let mut inside = false;
    let mut j = poly.len().wrapping_sub(1);
    for i in 0..poly.len() {
        let (xi, yi) = poly[i];
        let (xj, yj) = poly[j];
        if (yi > y) != (yj > y) && x < (xj - xi) * (y - yi) / (yj - yi) + xi {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// A region the model pointed at ("Show me").
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Highlight {
    pub label: String,
    /// Fractions of the image (0..1): x, y, width, height.
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// Parse Gemini's `[{"box_2d": [ymin, xmin, ymax, xmax], "label": ".."}]`
/// (0–1000 normalised), tolerating code fences. Invalid boxes are dropped.
pub fn parse_highlights(text: &str) -> Vec<Highlight> {
    let body = text.trim();
    let body = body
        .strip_prefix("```json")
        .or_else(|| body.strip_prefix("```"))
        .map(|b| b.trim_end_matches("```"))
        .unwrap_or(body)
        .trim();
    // Smaller models wrap the JSON in prose; take the outermost array.
    let parsed = serde_json::from_str::<serde_json::Value>(body)
        .ok()
        .or_else(|| {
            let (a, b) = (body.find('[')?, body.rfind(']')?);
            (a < b)
                .then(|| serde_json::from_str(&body[a..=b]).ok())
                .flatten()
        });
    let Some(serde_json::Value::Array(items)) = parsed else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|it| {
            let b = it.get("box_2d")?.as_array()?;
            if b.len() != 4 {
                return None;
            }
            let v: Vec<f64> = b.iter().filter_map(|n| n.as_f64()).collect();
            if v.len() != 4 || v.iter().any(|n| !n.is_finite()) {
                return None;
            }
            let c = |n: f64| (n / 1000.0).clamp(0.0, 1.0);
            let (y0, x0, y1, x1) = (c(v[0]), c(v[1]), c(v[2]), c(v[3]));
            let (w, h) = (x1 - x0, y1 - y0);
            if w <= 0.002 || h <= 0.002 {
                return None;
            }
            let label = it
                .get("label")
                .and_then(|l| l.as_str())
                .unwrap_or("")
                .chars()
                .take(80)
                .collect();
            Some(Highlight {
                label,
                x: x0,
                y: y0,
                width: w,
                height: h,
            })
        })
        .take(5)
        .collect()
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
    fn lasso_and_polygon() {
        let pts = [
            Point { x: 10.0, y: 10.0 },
            Point { x: 110.0, y: 20.0 },
            Point { x: 60.0, y: 90.0 },
        ];
        let sel = lasso_bounds(&pts, 1280.0, 720.0).unwrap();
        assert_eq!(
            (sel.x, sel.y, sel.width, sel.height),
            (10.0, 10.0, 100.0, 80.0)
        );
        assert!(lasso_bounds(&pts[..2], 1280.0, 720.0).is_err());
        let crop = to_pixels(&sel, 1280, 720).unwrap();
        let poly = polygon_in_crop(&pts, &sel, 1280, 720, crop);
        assert!(point_in_polygon(50.0, 30.0, &poly));
        assert!(!point_in_polygon(1.0, 70.0, &poly));
    }

    #[test]
    fn parses_model_boxes() {
        let h = parse_highlights(
            "```json\n[{\"box_2d\": [100, 200, 300, 600], \"label\": \"Save button\"}, {\"box_2d\": [5, 5]}, {\"box_2d\": [500, 500, 400, 600]}]\n```",
        );
        assert_eq!(h.len(), 1);
        assert_eq!(h[0].label, "Save button");
        assert!((h[0].x - 0.2).abs() < 1e-9 && (h[0].width - 0.4).abs() < 1e-9);
        assert!(parse_highlights("not json").is_empty());
        let clamped = parse_highlights("[{\"box_2d\": [-50, 900, 2000, 1500]}]");
        assert_eq!((clamped[0].y, clamped[0].x + clamped[0].width), (0.0, 1.0));
    }

    #[test]
    fn fits_long_side() {
        assert_eq!(fit_within(1000, 500, 2048), (1000, 500));
        assert_eq!(fit_within(4096, 1024, 2048), (2048, 512));
        assert_eq!(fit_within(1000, 3000, 1500), (500, 1500));
    }
}
