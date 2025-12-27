use image::{ImageBuffer, ImageEncoder, Rgba, RgbaImage};

// Compact 3x5 bitmap font optimized for small sizes (macOS menubar)
// Each digit is 3 pixels wide, 5 pixels tall - perfect for 22x22 icons
const DIGIT_WIDTH: u32 = 3;
const DIGIT_HEIGHT: u32 = 5;

// Clean, readable digit patterns (3x5 each) - SF Mono inspired
const DIGITS: [[u8; 5]; 10] = [
    // 0
    [0b111, 0b101, 0b101, 0b101, 0b111],
    // 1
    [0b010, 0b110, 0b010, 0b010, 0b111],
    // 2
    [0b111, 0b001, 0b111, 0b100, 0b111],
    // 3
    [0b111, 0b001, 0b111, 0b001, 0b111],
    // 4
    [0b101, 0b101, 0b111, 0b001, 0b001],
    // 5
    [0b111, 0b100, 0b111, 0b001, 0b111],
    // 6
    [0b111, 0b100, 0b111, 0b101, 0b111],
    // 7
    [0b111, 0b001, 0b001, 0b001, 0b001],
    // 8
    [0b111, 0b101, 0b111, 0b101, 0b111],
    // 9
    [0b111, 0b101, 0b111, 0b001, 0b111],
];

/// Draw a single digit at position (x, y) with given scale
fn draw_digit(img: &mut RgbaImage, digit: u8, x: i32, y: i32, scale: u32, color: Rgba<u8>) {
    if digit > 9 {
        return;
    }
    let pattern = &DIGITS[digit as usize];

    for (row, &bits) in pattern.iter().enumerate() {
        for col in 0..DIGIT_WIDTH {
            if (bits >> (DIGIT_WIDTH - 1 - col)) & 1 == 1 {
                for sy in 0..scale {
                    for sx in 0..scale {
                        let px = x + (col * scale) as i32 + sx as i32;
                        let py = y + (row as u32 * scale) as i32 + sy as i32;
                        if px >= 0 && py >= 0 && (px as u32) < img.width() && (py as u32) < img.height() {
                            img.put_pixel(px as u32, py as u32, color);
                        }
                    }
                }
            }
        }
    }
}

/// Professional macOS-style circular progress ring with centered number
/// Follows Apple Human Interface Guidelines for menubar icons
pub fn generate_tray_icon_with_ring(percentage: u8, size: u32) -> Vec<u8> {
    let mut img: RgbaImage = ImageBuffer::new(size, size);

    let center = size as f32 / 2.0;
    let pct = percentage.min(99);

    // Dynamic color based on usage percentage
    // Low (0-50%): Green, Medium (50-80%): Yellow/Orange, High (80-100%): Red
    let (progress_r, progress_g, progress_b) = if pct <= 50 {
        (76u8, 175u8, 80u8)    // Green - safe
    } else if pct <= 80 {
        (255u8, 193u8, 7u8)    // Yellow/Amber - warning
    } else {
        (244u8, 67u8, 54u8)    // Red - critical
    };

    // Gray for background ring
    let gray = 128u8;

    // Ring dimensions - BOLD ring for maximum visibility
    // 44x44 (@2x): ring_width = 7.0, 22x22 (@1x): ring_width = 3.5
    let ring_width = if size >= 44 { 7.0 } else { 3.5 };
    let outer_radius = center;  // Full edge-to-edge
    let inner_radius = outer_radius - ring_width;

    // Progress starts from top (12 o'clock), goes clockwise
    let start_angle = -std::f32::consts::FRAC_PI_2;
    let progress_ratio = pct as f32 / 100.0;
    let progress_angle = start_angle + 2.0 * std::f32::consts::PI * progress_ratio;

    // Draw ring with smooth anti-aliasing
    for y in 0..size {
        for x in 0..size {
            let dx = x as f32 - center + 0.5;
            let dy = y as f32 - center + 0.5;
            let dist = (dx * dx + dy * dy).sqrt();

            // Calculate ring mask with anti-aliased edges
            let inner_edge = smooth_step(inner_radius - 0.5, inner_radius + 0.5, dist);
            let outer_edge = smooth_step(outer_radius + 0.5, outer_radius - 0.5, dist);
            let ring_mask = inner_edge * outer_edge;

            if ring_mask > 0.01 {
                let angle = dy.atan2(dx);

                // Normalize angle to [start_angle, start_angle + 2π)
                let normalized = if angle < start_angle {
                    angle + 2.0 * std::f32::consts::PI
                } else {
                    angle
                };

                // Determine if this point is in the progress portion
                let in_progress = normalized <= progress_angle;

                let final_alpha = (255.0 * ring_mask) as u8;

                if final_alpha > 0 {
                    if in_progress {
                        // Dynamic color based on percentage
                        img.put_pixel(x, y, Rgba([progress_r, progress_g, progress_b, final_alpha]));
                    } else {
                        // Gray for background
                        img.put_pixel(x, y, Rgba([gray, gray, gray, (100.0 * ring_mask) as u8]));
                    }
                }
            }
        }
    }

    // Draw centered number with LARGER scaling
    // 44x44 (@2x): scale = 3, 22x22 (@1x): scale = 1
    let scale = if size >= 44 { 3 } else { 1 };
    let digit_w = DIGIT_WIDTH * scale;
    let digit_h = DIGIT_HEIGHT * scale;
    let spacing = if size >= 44 { 2 } else { 1 };  // 2px gap at @2x

    let d1 = pct / 10;
    let d2 = pct % 10;

    let total_width = 2 * digit_w + spacing;
    let start_x = ((size as i32 - total_width as i32) / 2).max(0);
    let start_y = ((size as i32 - digit_h as i32) / 2).max(0);

    // White text for visibility on both light and dark menubar
    let text_color = Rgba([255, 255, 255, 255]);
    draw_digit(&mut img, d1, start_x, start_y, scale, text_color);
    draw_digit(&mut img, d2, start_x + (digit_w + spacing) as i32, start_y, scale, text_color);

    encode_png(&img, size)
}

/// Smooth step function for anti-aliasing (like GLSL smoothstep)
fn smooth_step(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn encode_png(img: &RgbaImage, size: u32) -> Vec<u8> {
    let mut png_bytes: Vec<u8> = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut png_bytes);
    encoder
        .write_image(img.as_raw(), size, size, image::ExtendedColorType::Rgba8)
        .expect("Failed to encode PNG");
    png_bytes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_ring_44() {
        let bytes = generate_tray_icon_with_ring(75, 44);
        assert!(!bytes.is_empty());
    }

    #[test]
    fn test_generate_ring_22() {
        let bytes = generate_tray_icon_with_ring(50, 22);
        assert!(!bytes.is_empty());
    }
}
