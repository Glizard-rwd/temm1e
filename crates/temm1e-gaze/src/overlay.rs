//! SoM (Set-of-Mark) overlay compositing for desktop screenshots.
//!
//! Unlike browser SoM overlays (injected via JavaScript), desktop overlays
//! are composited directly onto the PNG image before sending to the VLM.

use image::{Rgba, RgbaImage};
use std::io::Cursor;
use temm1e_core::types::error::Temm1eError;

/// A candidate element for SoM labeling.
pub struct SomCandidate {
    /// The label index (1-based, matching accessibility tree if available).
    pub index: u32,
    /// Center X coordinate in the screenshot (physical pixels).
    pub center_x: u32,
    /// Center Y coordinate in the screenshot (physical pixels).
    pub center_y: u32,
}

const SOM_RADIUS: i32 = 14;
const SOM_COLOR: Rgba<u8> = Rgba([229, 62, 62, 255]); // #e53e3e red
const SOM_TEXT_COLOR: Rgba<u8> = Rgba([255, 255, 255, 255]); // white

/// Composite numbered SoM labels onto a screenshot PNG.
///
/// Draws red filled circles with white index numbers at each candidate's
/// position. Returns the modified PNG bytes.
///
/// If `candidates` is empty, returns the original PNG unchanged.
pub fn overlay_som_labels(
    screenshot_png: &[u8],
    candidates: &[SomCandidate],
) -> Result<Vec<u8>, Temm1eError> {
    if candidates.is_empty() {
        return Ok(screenshot_png.to_vec());
    }

    let img = image::load_from_memory(screenshot_png)
        .map_err(|e| Temm1eError::Tool(format!("Failed to load screenshot for overlay: {}", e)))?;

    let mut rgba = img.into_rgba8();

    for candidate in candidates {
        draw_som_label(&mut rgba, candidate);
    }

    let mut output = Vec::new();
    image::DynamicImage::ImageRgba8(rgba)
        .write_to(&mut Cursor::new(&mut output), image::ImageFormat::Png)
        .map_err(|e| Temm1eError::Tool(format!("Failed to encode overlay PNG: {}", e)))?;

    Ok(output)
}

/// Draw a single SoM label (red circle with index number) onto the image.
fn draw_som_label(img: &mut RgbaImage, candidate: &SomCandidate) {
    let cx = candidate.center_x as i32;
    let cy = candidate.center_y as i32;
    let w = img.width() as i32;
    let h = img.height() as i32;

    // Draw filled circle
    for dy in -SOM_RADIUS..=SOM_RADIUS {
        for dx in -SOM_RADIUS..=SOM_RADIUS {
            if dx * dx + dy * dy <= SOM_RADIUS * SOM_RADIUS {
                let px = cx + dx;
                let py = cy + dy;
                if px >= 0 && px < w && py >= 0 && py < h {
                    img.put_pixel(px as u32, py as u32, SOM_COLOR);
                }
            }
        }
    }

    // Draw index number using simple bitmap font (no external font dependency)
    let text = candidate.index.to_string();
    let char_w = 5;
    let char_h = 7;
    let text_w = text.len() as i32 * (char_w + 1);
    let start_x = cx - text_w / 2;
    let start_y = cy - char_h / 2;

    for (ci, ch) in text.chars().enumerate() {
        let bitmap = char_bitmap(ch);
        let offset_x = start_x + ci as i32 * (char_w + 1);
        for (row, bits) in bitmap.iter().enumerate() {
            for col in 0..char_w {
                if bits & (1 << (char_w - 1 - col)) != 0 {
                    let px = offset_x + col;
                    let py = start_y + row as i32;
                    if px >= 0 && px < w && py >= 0 && py < h {
                        img.put_pixel(px as u32, py as u32, SOM_TEXT_COLOR);
                    }
                }
            }
        }
    }
}

/// 5x7 bitmap font for digits 0-9. Each u8 encodes one row (5 bits used).
fn char_bitmap(ch: char) -> [u8; 7] {
    match ch {
        '0' => [
            0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
        ],
        '1' => [
            0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
        ],
        '2' => [
            0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b10000, 0b11111,
        ],
        '3' => [
            0b01110, 0b10001, 0b00001, 0b00110, 0b00001, 0b10001, 0b01110,
        ],
        '4' => [
            0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
        ],
        '5' => [
            0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110,
        ],
        '6' => [
            0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
        ],
        '7' => [
            0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
        ],
        '8' => [
            0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
        ],
        '9' => [
            0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100,
        ],
        _ => [
            0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000, 0b00000,
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_png(w: u32, h: u32) -> Vec<u8> {
        let img = RgbaImage::from_pixel(w, h, Rgba([200, 200, 200, 255]));
        let mut buf = Vec::new();
        image::DynamicImage::ImageRgba8(img)
            .write_to(&mut Cursor::new(&mut buf), image::ImageFormat::Png)
            .unwrap();
        buf
    }

    #[test]
    fn overlay_empty_candidates() {
        let png = make_test_png(100, 100);
        let result = overlay_som_labels(&png, &[]).unwrap();
        assert_eq!(result, png, "Empty candidates should return unchanged PNG");
    }

    #[test]
    fn overlay_single_label() {
        let png = make_test_png(200, 200);
        let candidates = vec![SomCandidate {
            index: 1,
            center_x: 100,
            center_y: 100,
        }];
        let result = overlay_som_labels(&png, &candidates).unwrap();
        assert_ne!(result, png, "Overlay should modify the image");
        assert!(result.len() > 100, "Result should be valid PNG");
    }

    #[test]
    fn overlay_multiple_labels() {
        let png = make_test_png(400, 400);
        let candidates: Vec<SomCandidate> = (1..=20)
            .map(|i| SomCandidate {
                index: i,
                center_x: (i * 18) % 380 + 20,
                center_y: (i * 15) % 380 + 20,
            })
            .collect();
        let result = overlay_som_labels(&png, &candidates).unwrap();
        assert!(
            result.len() > 100,
            "Result should be valid PNG with 20 labels"
        );
    }

    #[test]
    fn overlay_edge_coordinates() {
        let png = make_test_png(100, 100);
        let candidates = vec![
            SomCandidate {
                index: 1,
                center_x: 0,
                center_y: 0,
            },
            SomCandidate {
                index: 2,
                center_x: 99,
                center_y: 99,
            },
        ];
        let result = overlay_som_labels(&png, &candidates);
        assert!(result.is_ok(), "Edge coordinates should not crash");
    }

    #[test]
    fn char_bitmap_coverage() {
        for ch in '0'..='9' {
            let bm = char_bitmap(ch);
            let has_pixels = bm.iter().any(|row| *row != 0);
            assert!(has_pixels, "Digit {} should have non-zero bitmap", ch);
        }
    }
}
