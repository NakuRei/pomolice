// Renders 16x16 RGBA tray icons showing remaining minutes with status-colored backgrounds.
// Uses a hand-coded 5x7 bitmap font for digits 0-9.

use crate::types::TimerStatus;

const ICON_SIZE: u32 = 16;
const DIGIT_WIDTH: u32 = 5;
const DIGIT_HEIGHT: u32 = 7;
const IDLE_GLYPH_ORIGIN_X: u32 = 4;
const IDLE_GLYPH_ORIGIN_Y: u32 = 4;
const IDLE_BACKGROUND_COLOR: [u8; 4] = [96, 96, 96, 255];
#[cfg(test)]
const MIN_IDLE_CONTRAST_RATIO: f64 = 4.5;

/// Available visual variants for the idle tray glyph.
///
/// # Examples
/// ```ignore
/// use crate::tray_icon_renderer::{render_idle_icon_for_variant, IdleGlyphVariant};
///
/// let icon = render_idle_icon_for_variant(IdleGlyphVariant::HollowCircle);
/// assert_eq!(icon.len(), 16 * 16 * 4);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IdleGlyphVariant {
    HollowCircle,
    PowerStandby,
    PauseBars,
}

/// All selectable idle glyph variants for preview and fixture generation.
pub(crate) const IDLE_GLYPH_VARIANTS: [IdleGlyphVariant; 3] = [
    IdleGlyphVariant::HollowCircle,
    IdleGlyphVariant::PowerStandby,
    IdleGlyphVariant::PauseBars,
];

const ACTIVE_IDLE_GLYPH_INDEX: usize = 0;

/// 5x7 bitmap font for digits 0-9.
/// Each digit is represented as 7 rows of 5 bits (stored as u8 bitmasks, MSB = left pixel).
const DIGIT_BITMAPS: [[u8; 7]; 10] = [
    // 0
    [
        0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110,
    ],
    // 1
    [
        0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110,
    ],
    // 2
    [
        0b01110, 0b10001, 0b00001, 0b00110, 0b01000, 0b10000, 0b11111,
    ],
    // 3
    [
        0b01110, 0b10001, 0b00001, 0b00110, 0b00001, 0b10001, 0b01110,
    ],
    // 4
    [
        0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010,
    ],
    // 5
    [
        0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110,
    ],
    // 6
    [
        0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110,
    ],
    // 7
    [
        0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000,
    ],
    // 8
    [
        0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110,
    ],
    // 9
    [
        0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100,
    ],
];

/// Background color for each timer status: [R, G, B, A]
fn background_color(status: TimerStatus) -> [u8; 4] {
    match status {
        TimerStatus::Working => [200, 50, 50, 255], // Red
        TimerStatus::OnBreak => [50, 170, 80, 255], // Green
        TimerStatus::Idle => IDLE_BACKGROUND_COLOR, // Grey
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
pub(crate) fn render_tray_icon(remaining_minutes: u32, status: TimerStatus) -> Vec<u8> {
    let bg = background_color(status);
    let mut buf = vec![0u8; (ICON_SIZE * ICON_SIZE * 4) as usize];

    fill_background(&mut buf, bg);
    draw_minutes_glyph(&mut buf, remaining_minutes);

    buf
}

/// Render an idle tray icon (grey background, no number).
///
/// Returns a 16x16 RGBA pixel buffer (1024 bytes).
pub(crate) fn render_idle_icon() -> Vec<u8> {
    render_idle_icon_for_variant(IDLE_GLYPH_VARIANTS[ACTIVE_IDLE_GLYPH_INDEX])
}

/// Render an idle icon for a specific glyph variant.
///
/// This is intended for design review and fixture generation workflows.
pub(crate) fn render_idle_icon_for_variant(variant: IdleGlyphVariant) -> Vec<u8> {
    let bg = background_color(TimerStatus::Idle);
    let mut buf = vec![0u8; (ICON_SIZE * ICON_SIZE * 4) as usize];
    fill_background(&mut buf, bg);
    draw_idle_glyph_with_variant(&mut buf, variant);

    buf
}

/// Icon dimensions (always 16x16).
pub(crate) fn icon_size() -> u32 {
    ICON_SIZE
}

/// Draw a single digit at the given pixel offset using the 5x7 bitmap font.
fn draw_digit(buf: &mut [u8], digit: usize, x_offset: u32, y_offset: u32) {
    let bitmap = &DIGIT_BITMAPS[digit];
    for (row, &bits) in (0_u32..).zip(bitmap.iter()) {
        for col in 0..DIGIT_WIDTH {
            if bits & (1 << (DIGIT_WIDTH - 1 - col)) != 0 {
                set_pixel(buf, x_offset + col, y_offset + row, FONT_COLOR);
            }
        }
    }
}

/// Fill all icon pixels with the provided RGBA color.
fn fill_background(buf: &mut [u8], color: [u8; 4]) {
    for pixel in buf.chunks_exact_mut(4) {
        pixel.copy_from_slice(&color);
    }
}

/// Draw minutes using one or two center-aligned 5x7 digits.
fn draw_minutes_glyph(buf: &mut [u8], remaining_minutes: u32) {
    // Clamp display to 99
    let minutes = remaining_minutes.min(99);

    if minutes < 10 {
        // Single digit: centered
        let x_offset = (ICON_SIZE - DIGIT_WIDTH) / 2;
        let y_offset = (ICON_SIZE - DIGIT_HEIGHT) / 2;
        draw_digit(buf, minutes as usize, x_offset, y_offset);
    } else {
        // Two digits: 1px gap between them, centered as a group
        // Group width = 5 + 1 + 5 = 11
        let group_width = DIGIT_WIDTH * 2 + 1;
        let x_offset = (ICON_SIZE - group_width) / 2;
        let y_offset = (ICON_SIZE - DIGIT_HEIGHT) / 2;
        let tens = (minutes / 10) as usize;
        let ones = (minutes % 10) as usize;
        draw_digit(buf, tens, x_offset, y_offset);
        draw_digit(buf, ones, x_offset + DIGIT_WIDTH + 1, y_offset);
    }
}

fn draw_idle_glyph_with_variant(buf: &mut [u8], variant: IdleGlyphVariant) {
    match variant {
        IdleGlyphVariant::HollowCircle => draw_idle_hollow_circle(buf),
        IdleGlyphVariant::PowerStandby => draw_idle_power_standby(buf),
        IdleGlyphVariant::PauseBars => draw_idle_pause_bars(buf),
    }
}

fn draw_idle_hollow_circle(buf: &mut [u8]) {
    let points: &[(u32, u32)] = &[
        (3, 0),
        (4, 0),
        (2, 1),
        (5, 1),
        (1, 2),
        (6, 2),
        (0, 3),
        (7, 3),
        (0, 4),
        (7, 4),
        (0, 5),
        (7, 5),
        (1, 6),
        (6, 6),
        (2, 7),
        (5, 7),
        (3, 8),
        (4, 8),
    ];

    draw_offset_points(buf, points);
}

fn draw_idle_power_standby(buf: &mut [u8]) {
    let ring_points: &[(u32, u32)] = &[
        (3, 0),
        (5, 0),
        (2, 1),
        (6, 1),
        (1, 2),
        (7, 2),
        (1, 3),
        (7, 3),
        (1, 4),
        (7, 4),
        (1, 5),
        (7, 5),
        (2, 6),
        (6, 6),
        (3, 7),
        (4, 7),
        (5, 7),
    ];
    let line_points: &[(u32, u32)] = &[(4, 0), (4, 1), (4, 2), (4, 3)];

    draw_offset_points(buf, ring_points);
    draw_offset_points(buf, line_points);
}

fn draw_idle_pause_bars(buf: &mut [u8]) {
    for y in 1..=6 {
        set_pixel(
            buf,
            IDLE_GLYPH_ORIGIN_X + 1,
            IDLE_GLYPH_ORIGIN_Y + y,
            FONT_COLOR,
        );
        set_pixel(
            buf,
            IDLE_GLYPH_ORIGIN_X + 2,
            IDLE_GLYPH_ORIGIN_Y + y,
            FONT_COLOR,
        );
        set_pixel(
            buf,
            IDLE_GLYPH_ORIGIN_X + 5,
            IDLE_GLYPH_ORIGIN_Y + y,
            FONT_COLOR,
        );
        set_pixel(
            buf,
            IDLE_GLYPH_ORIGIN_X + 6,
            IDLE_GLYPH_ORIGIN_Y + y,
            FONT_COLOR,
        );
    }
}

fn draw_offset_points(buf: &mut [u8], points: &[(u32, u32)]) {
    for &(x, y) in points {
        set_pixel(
            buf,
            IDLE_GLYPH_ORIGIN_X + x,
            IDLE_GLYPH_ORIGIN_Y + y,
            FONT_COLOR,
        );
    }
}

/// Set a pixel in the RGBA buffer at (x, y).
fn set_pixel(buf: &mut [u8], x: u32, y: u32, color: [u8; 4]) {
    if x < ICON_SIZE && y < ICON_SIZE {
        let idx = ((y * ICON_SIZE + x) * 4) as usize;
        buf[idx..idx + 4].copy_from_slice(&color);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    fn count_white_pixels(buf: &[u8]) -> usize {
        buf.chunks_exact(4)
            .filter(|pixel| *pixel == FONT_COLOR)
            .count()
    }

    fn count_connected_white_components(buf: &[u8]) -> usize {
        let mut visited = vec![false; (ICON_SIZE * ICON_SIZE) as usize];
        let mut components = 0usize;

        for y in 0..ICON_SIZE {
            for x in 0..ICON_SIZE {
                let index = (y * ICON_SIZE + x) as usize;
                if visited[index] || !is_white_pixel(buf, x, y) {
                    continue;
                }

                components += 1;
                let mut queue = VecDeque::new();
                queue.push_back((x, y));
                visited[index] = true;

                while let Some((cx, cy)) = queue.pop_front() {
                    for (nx, ny) in neighbors(cx, cy) {
                        let nindex = (ny * ICON_SIZE + nx) as usize;
                        if visited[nindex] || !is_white_pixel(buf, nx, ny) {
                            continue;
                        }
                        visited[nindex] = true;
                        queue.push_back((nx, ny));
                    }
                }
            }
        }

        components
    }

    fn is_white_pixel(buf: &[u8], x: u32, y: u32) -> bool {
        let start = ((y * ICON_SIZE + x) * 4) as usize;
        buf[start..start + 4] == FONT_COLOR
    }

    fn neighbors(x: u32, y: u32) -> Vec<(u32, u32)> {
        let mut result = Vec::new();
        for dx in [-1_i32, 0, 1] {
            for dy in [-1_i32, 0, 1] {
                if dx == 0 && dy == 0 {
                    continue;
                }
                let nx = i64::from(x) + i64::from(dx);
                let ny = i64::from(y) + i64::from(dy);
                if nx >= 0 && ny >= 0 && nx < i64::from(ICON_SIZE) && ny < i64::from(ICON_SIZE) {
                    let Some(nx) = u32::try_from(nx).ok() else {
                        continue;
                    };
                    let Some(ny) = u32::try_from(ny).ok() else {
                        continue;
                    };
                    result.push((nx, ny));
                }
            }
        }
        result
    }

    fn white_pixel_bounds(buf: &[u8]) -> Option<(u32, u32, u32, u32)> {
        let mut min_x = ICON_SIZE;
        let mut max_x = 0;
        let mut min_y = ICON_SIZE;
        let mut max_y = 0;
        let mut found = false;

        for y in 0..ICON_SIZE {
            for x in 0..ICON_SIZE {
                if is_white_pixel(buf, x, y) {
                    found = true;
                    min_x = min_x.min(x);
                    max_x = max_x.max(x);
                    min_y = min_y.min(y);
                    max_y = max_y.max(y);
                }
            }
        }

        if found {
            Some((min_x, max_x, min_y, max_y))
        } else {
            None
        }
    }

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
        let white_count = buf.chunks_exact(4).filter(|p| p == &FONT_COLOR).count();
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

    #[test]
    fn test_idle_variants_render_visible_glyphs() {
        for variant in IDLE_GLYPH_VARIANTS {
            let icon = render_idle_icon_for_variant(variant);

            assert!(
                count_white_pixels(&icon) > 0,
                "Idle variant {variant:?} should render a visible glyph"
            );
        }
    }

    #[test]
    fn test_idle_variants_are_visually_distinct() {
        let hollow = render_idle_icon_for_variant(IdleGlyphVariant::HollowCircle);
        let standby = render_idle_icon_for_variant(IdleGlyphVariant::PowerStandby);
        let pause = render_idle_icon_for_variant(IdleGlyphVariant::PauseBars);

        assert_ne!(hollow, standby);
        assert_ne!(hollow, pause);
        assert_ne!(standby, pause);
    }

    #[test]
    fn test_pause_idle_variant_renders_two_separated_bars() {
        let pause = render_idle_icon_for_variant(IdleGlyphVariant::PauseBars);

        assert_eq!(count_connected_white_components(&pause), 2);
    }

    fn relative_luminance(color: [u8; 4]) -> f64 {
        fn channel(value: u8) -> f64 {
            let normalized = f64::from(value) / 255.0;
            if normalized <= 0.03928 {
                normalized / 12.92
            } else {
                ((normalized + 0.055) / 1.055).powf(2.4)
            }
        }

        let red = channel(color[0]);
        let green = channel(color[1]);
        let blue = channel(color[2]);
        (0.2126 * red) + (0.7152 * green) + (0.0722 * blue)
    }

    fn contrast_ratio(a: [u8; 4], b: [u8; 4]) -> f64 {
        let a_luma = relative_luminance(a);
        let b_luma = relative_luminance(b);
        let (bright, dark) = if a_luma >= b_luma {
            (a_luma, b_luma)
        } else {
            (b_luma, a_luma)
        };
        (bright + 0.05) / (dark + 0.05)
    }

    #[test]
    fn test_idle_text_contrast_ratio_meets_threshold() {
        let ratio = contrast_ratio(IDLE_BACKGROUND_COLOR, FONT_COLOR);
        assert!(
            ratio >= MIN_IDLE_CONTRAST_RATIO,
            "Idle contrast ratio {ratio:.2} is below threshold {MIN_IDLE_CONTRAST_RATIO:.2}"
        );
    }

    #[test]
    fn test_idle_glyph_origin_uses_even_coordinates() {
        assert_eq!(IDLE_GLYPH_ORIGIN_X % 2, 0);
        assert_eq!(IDLE_GLYPH_ORIGIN_Y % 2, 0);
    }

    #[test]
    fn test_idle_glyph_bounds_are_center_aligned() {
        let icon = render_idle_icon();
        let bounds = white_pixel_bounds(&icon).expect("Idle icon must include white pixels");
        let horizontal_center_twice = bounds.0 + bounds.1;
        let vertical_center_twice = bounds.2 + bounds.3;
        let icon_center_twice = ICON_SIZE - 1;

        assert!(
            horizontal_center_twice.abs_diff(icon_center_twice) <= 1,
            "Idle glyph horizontal center is misaligned"
        );
        assert!(
            vertical_center_twice.abs_diff(icon_center_twice) <= 1,
            "Idle glyph vertical center is misaligned"
        );
    }
}
