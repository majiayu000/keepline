use image::{ImageBuffer, ImageEncoder, Rgba, RgbaImage};

// Simple 5x7 bitmap font for digits 0-9 and % symbol
// Each digit is represented as a 5-wide, 7-tall bitmap (35 bits)
const DIGIT_WIDTH: u32 = 5;
const DIGIT_HEIGHT: u32 = 7;

// Bitmap patterns for digits 0-9 (5x7 each, row by row)
const DIGITS: [[u8; 7]; 10] = [
    // 0
    [
        0b01110,
        0b10001,
        0b10011,
        0b10101,
        0b11001,
        0b10001,
        0b01110,
    ],
    // 1
    [
        0b00100,
        0b01100,
        0b00100,
        0b00100,
        0b00100,
        0b00100,
        0b01110,
    ],
    // 2
    [
        0b01110,
        0b10001,
        0b00001,
        0b00110,
        0b01000,
        0b10000,
        0b11111,
    ],
    // 3
    [
        0b01110,
        0b10001,
        0b00001,
        0b00110,
        0b00001,
        0b10001,
        0b01110,
    ],
    // 4
    [
        0b00010,
        0b00110,
        0b01010,
        0b10010,
        0b11111,
        0b00010,
        0b00010,
    ],
    // 5
    [
        0b11111,
        0b10000,
        0b11110,
        0b00001,
        0b00001,
        0b10001,
        0b01110,
    ],
    // 6
    [
        0b00110,
        0b01000,
        0b10000,
        0b11110,
        0b10001,
        0b10001,
        0b01110,
    ],
    // 7
    [
        0b11111,
        0b00001,
        0b00010,
        0b00100,
        0b01000,
        0b01000,
        0b01000,
    ],
    // 8
    [
        0b01110,
        0b10001,
        0b10001,
        0b01110,
        0b10001,
        0b10001,
        0b01110,
    ],
    // 9
    [
        0b01110,
        0b10001,
        0b10001,
        0b01111,
        0b00001,
        0b00010,
        0b01100,
    ],
];

// Percent symbol (5x7)
const PERCENT: [u8; 7] = [
    0b11001,
    0b11010,
    0b00100,
    0b00100,
    0b00100,
    0b01011,
    0b10011,
];

/// Draw a single digit at position (x, y) with given scale
fn draw_digit(img: &mut RgbaImage, digit: u8, x: u32, y: u32, scale: u32, color: Rgba<u8>) {
    let pattern = if digit <= 9 {
        &DIGITS[digit as usize]
    } else {
        &PERCENT
    };

    for (row, &bits) in pattern.iter().enumerate() {
        for col in 0..DIGIT_WIDTH {
            if (bits >> (DIGIT_WIDTH - 1 - col)) & 1 == 1 {
                // Draw scaled pixel
                for sy in 0..scale {
                    for sx in 0..scale {
                        let px = x + col * scale + sx;
                        let py = y + (row as u32) * scale + sy;
                        if px < img.width() && py < img.height() {
                            img.put_pixel(px, py, color);
                        }
                    }
                }
            }
        }
    }
}

/// Generate a tray icon with the percentage displayed
/// Returns PNG bytes
pub fn generate_tray_icon(percentage: u8, size: u32) -> Vec<u8> {
    // Create transparent image
    let mut img: RgbaImage = ImageBuffer::new(size, size);

    // For macOS template icons, use black color (system will handle dark mode)
    let text_color = Rgba([0, 0, 0, 255]);

    // Determine scale based on icon size
    let scale = if size >= 32 { 2 } else { 1 };

    // Calculate digit dimensions
    let digit_w = DIGIT_WIDTH * scale;
    let digit_h = DIGIT_HEIGHT * scale;
    let spacing = scale; // 1px or 2px spacing between digits

    // Determine what to render
    let percentage = percentage.min(100);
    let digits: Vec<u8> = if percentage == 100 {
        vec![1, 0, 0]
    } else if percentage >= 10 {
        vec![percentage / 10, percentage % 10]
    } else {
        vec![percentage]
    };

    // Calculate total width
    let total_width = digits.len() as u32 * digit_w + (digits.len() as u32 - 1) * spacing;

    // Center the text
    let start_x = (size - total_width) / 2;
    let start_y = (size - digit_h) / 2;

    // Draw each digit
    let mut x = start_x;
    for digit in digits {
        draw_digit(&mut img, digit, x, start_y, scale, text_color);
        x += digit_w + spacing;
    }

    // Encode as PNG
    let mut png_bytes: Vec<u8> = Vec::new();
    let encoder = image::codecs::png::PngEncoder::new(&mut png_bytes);
    encoder
        .write_image(
            img.as_raw(),
            size,
            size,
            image::ExtendedColorType::Rgba8,
        )
        .expect("Failed to encode PNG");

    png_bytes
}

/// Generate a tray icon with percentage and optional visual indicator
pub fn generate_tray_icon_with_bar(percentage: u8, size: u32) -> Vec<u8> {
    let mut img: RgbaImage = ImageBuffer::new(size, size);

    let text_color = Rgba([0, 0, 0, 255]);
    let bar_color = Rgba([0, 0, 0, 180]);
    let bar_bg = Rgba([0, 0, 0, 60]);

    let scale = if size >= 32 { 2 } else { 1 };
    let digit_w = DIGIT_WIDTH * scale;
    let digit_h = DIGIT_HEIGHT * scale;
    let spacing = scale;

    let percentage = percentage.min(100);
    let digits: Vec<u8> = if percentage == 100 {
        vec![1, 0, 0]
    } else if percentage >= 10 {
        vec![percentage / 10, percentage % 10]
    } else {
        vec![percentage]
    };

    let total_width = digits.len() as u32 * digit_w + (digits.len() as u32 - 1) * spacing;

    // Position text a bit higher to make room for bar
    let start_x = (size - total_width) / 2;
    let text_y = (size - digit_h - 3 * scale) / 2;

    // Draw digits
    let mut x = start_x;
    for digit in digits {
        draw_digit(&mut img, digit, x, text_y, scale, text_color);
        x += digit_w + spacing;
    }

    // Draw progress bar at bottom
    let bar_height = scale;
    let bar_y = size - bar_height - scale;
    let bar_margin = 2 * scale;
    let bar_width = size - 2 * bar_margin;
    let filled_width = (bar_width as f32 * percentage as f32 / 100.0) as u32;

    // Draw bar background
    for bx in bar_margin..(bar_margin + bar_width) {
        for by in bar_y..(bar_y + bar_height) {
            img.put_pixel(bx, by, bar_bg);
        }
    }

    // Draw filled portion
    for bx in bar_margin..(bar_margin + filled_width) {
        for by in bar_y..(bar_y + bar_height) {
            img.put_pixel(bx, by, bar_color);
        }
    }

    // Encode as PNG
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
    fn test_generate_icon_16() {
        let bytes = generate_tray_icon(50, 16);
        assert!(!bytes.is_empty());
    }

    #[test]
    fn test_generate_icon_32() {
        let bytes = generate_tray_icon(100, 32);
        assert!(!bytes.is_empty());
    }

    #[test]
    fn test_generate_icon_with_bar() {
        let bytes = generate_tray_icon_with_bar(75, 32);
        assert!(!bytes.is_empty());
    }
}
