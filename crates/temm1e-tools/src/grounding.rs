//! Tem Gaze — shared grounding utilities for vision-based interaction.
//!
//! Provides coordinate transform math used by both browser (Prowl V2) and
//! future desktop (Tem Gaze) grounding pipelines. All functions are pure
//! and model-agnostic — they operate on coordinates and dimensions only.

/// Compute scaled dimensions that satisfy VLM API resolution constraints.
///
/// Returns `(new_width, new_height, scale_factor)` where `scale_factor` is
/// the multiplier applied (`new = original * scale_factor`).
///
/// If the image already fits within constraints, returns the original
/// dimensions with `scale_factor = 1.0`.
///
/// # Arguments
/// - `width`, `height` — original image dimensions in pixels
/// - `max_longest_edge` — API constraint on the longest dimension (e.g., 1568 for Anthropic)
/// - `max_total_pixels` — API constraint on total pixel count (e.g., 1_150_000 for Anthropic)
pub fn scale_for_api(
    width: u32,
    height: u32,
    max_longest_edge: u32,
    max_total_pixels: u32,
) -> (u32, u32, f64) {
    if width == 0 || height == 0 {
        return (width, height, 1.0);
    }

    let longest = width.max(height) as f64;
    let total = (width as f64) * (height as f64);

    let s_edge = if longest > max_longest_edge as f64 {
        max_longest_edge as f64 / longest
    } else {
        1.0
    };

    let s_pixels = if total > max_total_pixels as f64 {
        (max_total_pixels as f64 / total).sqrt()
    } else {
        1.0
    };

    let scale = s_edge.min(s_pixels).min(1.0);

    if (scale - 1.0).abs() < f64::EPSILON {
        return (width, height, 1.0);
    }

    let new_w = ((width as f64) * scale).round().max(1.0) as u32;
    let new_h = ((height as f64) * scale).round().max(1.0) as u32;

    (new_w, new_h, scale)
}

/// Transform coordinates from a zoomed (cropped) image space back to the
/// original screenshot space.
///
/// # Arguments
/// - `x_zoomed`, `y_zoomed` — coordinates in the zoomed image (pixels)
/// - `zoom_width`, `zoom_height` — dimensions of the zoomed image (pixels)
/// - `region` — `[x1, y1, x2, y2]` bounding box of the crop in original space
///
/// # Returns
/// `(x_original, y_original)` — coordinates in the original screenshot space
pub fn zoom_to_original(
    x_zoomed: f64,
    y_zoomed: f64,
    zoom_width: u32,
    zoom_height: u32,
    region: [u32; 4],
) -> (f64, f64) {
    let [x1, y1, x2, y2] = region;

    if zoom_width == 0 || zoom_height == 0 {
        return (x1 as f64, y1 as f64);
    }

    let region_w = (x2.saturating_sub(x1)) as f64;
    let region_h = (y2.saturating_sub(y1)) as f64;

    let x_orig = (x1 as f64) + (x_zoomed / zoom_width as f64) * region_w;
    let y_orig = (y1 as f64) + (y_zoomed / zoom_height as f64) * region_h;

    (x_orig, y_orig)
}

/// Convert physical pixel coordinates to logical coordinates using DPI scale factor.
///
/// On Retina/HiDPI displays, the screen captures at physical pixel resolution
/// but input simulation operates at logical point resolution. This converts
/// physical → logical.
///
/// # Arguments
/// - `x_physical`, `y_physical` — coordinates in physical pixels
/// - `scale_factor` — DPI scale (2.0 for Retina, 1.0 for standard)
///
/// # Returns
/// `(x_logical, y_logical)` — coordinates for input simulation
pub fn dpi_to_logical(x_physical: f64, y_physical: f64, scale_factor: f64) -> (f64, f64) {
    if scale_factor <= 0.0 || scale_factor == 1.0 {
        return (x_physical, y_physical);
    }
    (x_physical / scale_factor, y_physical / scale_factor)
}

/// Scale coordinates returned by a VLM (in API-scaled image space) back to
/// the original screenshot space.
///
/// This is the inverse of `scale_for_api` applied to coordinates.
///
/// # Arguments
/// - `x_api`, `y_api` — coordinates from the VLM response
/// - `scale_factor` — the scale factor returned by `scale_for_api`
///
/// # Returns
/// `(x_screen, y_screen)` — coordinates in the original screenshot space
pub fn api_to_screen(x_api: f64, y_api: f64, scale_factor: f64) -> (f64, f64) {
    if scale_factor <= 0.0 || (scale_factor - 1.0).abs() < f64::EPSILON {
        return (x_api, y_api);
    }
    (x_api / scale_factor, y_api / scale_factor)
}

/// Clamp coordinates to valid screen bounds.
pub fn clamp_coords(x: f64, y: f64, width: u32, height: u32) -> (f64, f64) {
    let cx = x.max(0.0).min((width.saturating_sub(1)) as f64);
    let cy = y.max(0.0).min((height.saturating_sub(1)) as f64);
    (cx, cy)
}

/// Validate a zoom region: x2 > x1, y2 > y1, and within bounds.
/// Returns a clamped region or an error message.
pub fn validate_zoom_region(
    x1: u32,
    y1: u32,
    x2: u32,
    y2: u32,
    page_width: u32,
    page_height: u32,
) -> Result<[u32; 4], String> {
    if x2 <= x1 {
        return Err(format!(
            "Invalid zoom region: x2 ({x2}) must be greater than x1 ({x1})"
        ));
    }
    if y2 <= y1 {
        return Err(format!(
            "Invalid zoom region: y2 ({y2}) must be greater than y1 ({y1})"
        ));
    }

    // Clamp to page bounds
    let cx1 = x1.min(page_width.saturating_sub(1));
    let cy1 = y1.min(page_height.saturating_sub(1));
    let cx2 = x2.min(page_width);
    let cy2 = y2.min(page_height);

    if cx2 <= cx1 || cy2 <= cy1 {
        return Err("Zoom region collapses to zero after clamping to page bounds".into());
    }

    Ok([cx1, cy1, cx2, cy2])
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- scale_for_api ---

    #[test]
    fn scale_for_api_no_scaling_needed() {
        let (w, h, s) = scale_for_api(800, 600, 1568, 1_150_000);
        assert_eq!(w, 800);
        assert_eq!(h, 600);
        assert!((s - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn scale_for_api_edge_limited() {
        // 3000px wide — should be scaled down to fit 1568 max edge
        let (w, h, s) = scale_for_api(3000, 1000, 1568, 10_000_000);
        assert!(w <= 1568, "Width {w} should be <= 1568");
        assert!(s < 1.0, "Scale factor {s} should be < 1.0");
        // Proportional
        let ratio_orig = 3000.0 / 1000.0;
        let ratio_scaled = w as f64 / h as f64;
        assert!(
            (ratio_orig - ratio_scaled).abs() < 0.05,
            "Aspect ratio should be preserved"
        );
    }

    #[test]
    fn scale_for_api_pixel_limited() {
        // 2000x2000 = 4M pixels, limit 1.15M
        let (w, h, s) = scale_for_api(2000, 2000, 5000, 1_150_000);
        let total = (w as u64) * (h as u64);
        assert!(
            total <= 1_200_000,
            "Total pixels {total} should be near 1.15M"
        );
        assert!(s < 1.0);
    }

    #[test]
    fn scale_for_api_zero_dimensions() {
        let (w, h, s) = scale_for_api(0, 0, 1568, 1_150_000);
        assert_eq!(w, 0);
        assert_eq!(h, 0);
        assert!((s - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn scale_for_api_exact_boundary() {
        let (w, h, s) = scale_for_api(1568, 800, 1568, 1_254_400);
        assert_eq!(w, 1568);
        assert_eq!(h, 800);
        assert!((s - 1.0).abs() < f64::EPSILON);
    }

    // --- zoom_to_original ---

    #[test]
    fn zoom_to_original_full_region_identity() {
        // Zoomed image covers the entire original — identity transform
        let (x, y) = zoom_to_original(500.0, 300.0, 1000, 600, [0, 0, 1000, 600]);
        assert!((x - 500.0).abs() < 0.01);
        assert!((y - 300.0).abs() < 0.01);
    }

    #[test]
    fn zoom_to_original_top_left_quadrant() {
        // Region is top-left quarter: (0,0)-(500,300) of a 1000x600 image
        // Click at center of zoomed image (500, 300) → maps to (250, 150)
        let (x, y) = zoom_to_original(500.0, 300.0, 1000, 600, [0, 0, 500, 300]);
        assert!((x - 250.0).abs() < 0.01);
        assert!((y - 150.0).abs() < 0.01);
    }

    #[test]
    fn zoom_to_original_offset_region() {
        // Region starts at (200, 100) and is 400x300
        // Click at (0, 0) in zoomed → maps to (200, 100) in original
        let (x, y) = zoom_to_original(0.0, 0.0, 800, 600, [200, 100, 600, 400]);
        assert!((x - 200.0).abs() < 0.01);
        assert!((y - 100.0).abs() < 0.01);
    }

    #[test]
    fn zoom_to_original_zero_dimensions() {
        let (x, y) = zoom_to_original(100.0, 50.0, 0, 0, [100, 200, 300, 400]);
        assert!((x - 100.0).abs() < 0.01);
        assert!((y - 200.0).abs() < 0.01);
    }

    // --- dpi_to_logical ---

    #[test]
    fn dpi_retina() {
        let (x, y) = dpi_to_logical(200.0, 400.0, 2.0);
        assert!((x - 100.0).abs() < f64::EPSILON);
        assert!((y - 200.0).abs() < f64::EPSILON);
    }

    #[test]
    fn dpi_standard() {
        let (x, y) = dpi_to_logical(200.0, 400.0, 1.0);
        assert!((x - 200.0).abs() < f64::EPSILON);
        assert!((y - 400.0).abs() < f64::EPSILON);
    }

    #[test]
    fn dpi_zero_scale() {
        let (x, y) = dpi_to_logical(200.0, 400.0, 0.0);
        assert!((x - 200.0).abs() < f64::EPSILON);
        assert!((y - 400.0).abs() < f64::EPSILON);
    }

    // --- api_to_screen ---

    #[test]
    fn api_to_screen_with_scaling() {
        // If scale_for_api returned scale=0.5, coordinates from VLM need to be doubled
        let (x, y) = api_to_screen(250.0, 150.0, 0.5);
        assert!((x - 500.0).abs() < 0.01);
        assert!((y - 300.0).abs() < 0.01);
    }

    #[test]
    fn api_to_screen_no_scaling() {
        let (x, y) = api_to_screen(250.0, 150.0, 1.0);
        assert!((x - 250.0).abs() < f64::EPSILON);
        assert!((y - 150.0).abs() < f64::EPSILON);
    }

    // --- clamp_coords ---

    #[test]
    fn clamp_within_bounds() {
        let (x, y) = clamp_coords(500.0, 300.0, 1000, 600);
        assert!((x - 500.0).abs() < f64::EPSILON);
        assert!((y - 300.0).abs() < f64::EPSILON);
    }

    #[test]
    fn clamp_negative() {
        let (x, y) = clamp_coords(-50.0, -100.0, 1000, 600);
        assert!(x >= 0.0);
        assert!(y >= 0.0);
    }

    #[test]
    fn clamp_overflow() {
        let (x, y) = clamp_coords(2000.0, 1500.0, 1000, 600);
        assert!(x <= 999.0);
        assert!(y <= 599.0);
    }

    // --- validate_zoom_region ---

    #[test]
    fn validate_zoom_valid() {
        let result = validate_zoom_region(100, 100, 500, 400, 1280, 900);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), [100, 100, 500, 400]);
    }

    #[test]
    fn validate_zoom_x2_less_than_x1() {
        let result = validate_zoom_region(500, 100, 100, 400, 1280, 900);
        assert!(result.is_err());
    }

    #[test]
    fn validate_zoom_clamped_to_bounds() {
        let result = validate_zoom_region(100, 100, 2000, 2000, 1280, 900);
        assert!(result.is_ok());
        let [_, _, x2, y2] = result.unwrap();
        assert!(x2 <= 1280);
        assert!(y2 <= 900);
    }
}
