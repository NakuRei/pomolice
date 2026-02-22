// Renders 16x16 RGBA tray icons showing remaining minutes with status-colored backgrounds.
// Uses a hand-coded 5x7 bitmap font for digits 0-9.

use crate::types::TimerStatus;

const ICON_SIZE: u32 = 16;
const DIGIT_WIDTH: u32 = 5;
const DIGIT_HEIGHT: u32 = 7;

/// 5x7 bitmap font for digits 0-9.
/// Each digit is represented as 7 rows of 5 bits (stored as u8 bitmasks, MSB = left pixel).
const DIGIT_BITMAPS: [[u8; 7]; 10] = [
    // 0
    [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
    // 1
    [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
    // 2
    [0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b10000, 0b11111],
    // 3
    [0b01110, 0b10001, 0b00001, 0b00110, 0b00001, 0b10001, 0b01110],
    // 4
    [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
    // 5
    [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
    // 6
    [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110],
    // 7
    [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
    // 8
    [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
    // 9
    [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100],
];

/// Background color for each timer status: [R, G, B, A]
fn background_color(status: TimerStatus) -> [u8; 4] {
    match status {
        TimerStatus::Working => [200, 50, 50, 255],   // Red
        TimerStatus::OnBreak => [50, 170, 80, 255],    // Green
        TimerStatus::Idle => [128, 128, 128, 255],     // Grey
    }
}

const FONT_COLOR: [u8; 4] = [255, 255, 255, 255]; // White text

/// Render a tray icon showing the remaining minutes with a status-colored background.
///
/// Returns a 16x16 RGBA pixel buffer (1024 bytes).
///
/// # Examples
/// ```ignore
/// let rgba = render_tray_icon(18, TimerStatus::Working);  // Red background, "18"
/// let rgba = render_tray_icon(5, TimerStatus::OnBreak);   // Green background, "5"
/// ```
pub fn render_tray_icon(remaining_minutes: u32, status: TimerStatus) -> Vec<u8> {
    let bg = background_color(status);
    let mut buf = vec![0u8; (ICON_SIZE * ICON_SIZE * 4) as usize];

    // Fill background
    for pixel in buf.chunks_exact_mut(4) {
        pixel.copy_from_slice(&bg);
    }

    // Clamp display to 99
    let minutes = remaining_minutes.min(99);

    if minutes < 10 {
        // Single digit: centered
        let x_offset = (ICON_SIZE - DIGIT_WIDTH) / 2;
        let y_offset = (ICON_SIZE - DIGIT_HEIGHT) / 2;
        draw_digit(&mut buf, minutes as usize, x_offset, y_offset);
    } else {
        // Two digits: 1px gap between them, centered as a group
        // Group width = 5 + 1 + 5 = 11
        let group_width = DIGIT_WIDTH * 2 + 1;
        let x_offset = (ICON_SIZE - group_width) / 2;
        let y_offset = (ICON_SIZE - DIGIT_HEIGHT) / 2;
        let tens = (minutes / 10) as usize;
        let ones = (minutes % 10) as usize;
        draw_digit(&mut buf, tens, x_offset, y_offset);
        draw_digit(&mut buf, ones, x_offset + DIGIT_WIDTH + 1, y_offset);
    }

    buf
}

/// Render an idle tray icon (grey background, no number).
///
/// Returns a 16x16 RGBA pixel buffer (1024 bytes).
pub fn render_idle_icon() -> Vec<u8> {
    let bg = background_color(TimerStatus::Idle);
    let mut buf = vec![0u8; (ICON_SIZE * ICON_SIZE * 4) as usize];
    for pixel in buf.chunks_exact_mut(4) {
        pixel.copy_from_slice(&bg);
    }

    // Draw a dash in the center to indicate idle (5 pixels wide at center)
    let y = ICON_SIZE / 2;
    let x_start = (ICON_SIZE - DIGIT_WIDTH) / 2;
    for dx in 0..DIGIT_WIDTH {
        set_pixel(&mut buf, x_start + dx, y, &FONT_COLOR);
    }

    buf
}

/// Icon dimensions (always 16x16).
pub fn icon_size() -> u32 {
    ICON_SIZE
}

/// Draw a single digit at the given pixel offset using the 5x7 bitmap font.
fn draw_digit(buf: &mut [u8], digit: usize, x_offset: u32, y_offset: u32) {
    let bitmap = &DIGIT_BITMAPS[digit];
    for (row, &bits) in bitmap.iter().enumerate() {
        for col in 0..DIGIT_WIDTH {
            if bits & (1 << (DIGIT_WIDTH - 1 - col)) != 0 {
                set_pixel(buf, x_offset + col, y_offset + row as u32, &FONT_COLOR);
            }
        }
    }
}

/// Set a pixel in the RGBA buffer at (x, y).
fn set_pixel(buf: &mut [u8], x: u32, y: u32, color: &[u8; 4]) {
    if x < ICON_SIZE && y < ICON_SIZE {
        let idx = ((y * ICON_SIZE + x) * 4) as usize;
        buf[idx..idx + 4].copy_from_slice(color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_icon_buffer_size() {
        let buf = render_tray_icon(25, TimerStatus::Working);
        assert_eq!(buf.len(), (ICON_SIZE * ICON_SIZE * 4) as usize);
    }

    #[test]
    fn test_idle_icon_buffer_size() {
        let buf = render_idle_icon();
        assert_eq!(buf.len(), (ICON_SIZE * ICON_SIZE * 4) as usize);
    }

    #[test]
    fn test_icon_size() {
        assert_eq!(icon_size(), 16);
    }

    #[test]
    fn test_background_colors_differ_by_status() {
        let working = render_tray_icon(10, TimerStatus::Working);
        let on_break = render_tray_icon(10, TimerStatus::OnBreak);
        let idle = render_tray_icon(10, TimerStatus::Idle);

        // First pixel (top-left corner) should reflect the background color
        assert_ne!(&working[0..4], &on_break[0..4]);
        assert_ne!(&working[0..4], &idle[0..4]);
        assert_ne!(&on_break[0..4], &idle[0..4]);
    }

    #[test]
    fn test_single_digit_has_white_pixels() {
        let buf = render_tray_icon(5, TimerStatus::Working);
        // There should be at least some white pixels (the digit)
        let white_count = buf
            .chunks_exact(4)
            .filter(|p| p == &FONT_COLOR)
            .count();
        assert!(white_count > 0, "Expected white pixels for digit rendering");
    }

    #[test]
    fn test_two_digit_has_more_white_than_single() {
        let single = render_tray_icon(5, TimerStatus::Working);
        let double = render_tray_icon(25, TimerStatus::Working);

        let single_white = single.chunks_exact(4).filter(|p| p == &FONT_COLOR).count();
        let double_white = double.chunks_exact(4).filter(|p| p == &FONT_COLOR).count();

        assert!(
            double_white > single_white,
            "Two-digit icon should have more white pixels than single-digit"
        );
    }

    #[test]
    fn test_clamps_to_99() {
        // Should not panic for values > 99
        let buf = render_tray_icon(120, TimerStatus::Working);
        assert_eq!(buf.len(), (ICON_SIZE * ICON_SIZE * 4) as usize);
    }
}
