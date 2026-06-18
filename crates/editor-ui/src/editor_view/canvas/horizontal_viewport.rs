//! Horizontal viewport slicing for very long single lines.

use gpui::{px, Font, Pixels, SharedString, TextRun, Window};

pub(crate) use crate::editor_view::state::LONG_LINE_COL_THRESHOLD;

const VIEWPORT_MARGIN_CHARS: usize = 32;

/// Average glyph width for Consolas-like monospace (used for scroll/col mapping).
pub(crate) fn measure_avg_char_width(
    window: &mut Window,
    font: &Font,
    font_size: Pixels,
) -> Pixels {
    let run = TextRun {
        len: 1,
        font: font.clone(),
        color: gpui::rgb(0xffffff).into(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    window
        .text_system()
        .shape_line(SharedString::from("M"), font_size, &[run], None)
        .width
        .max(px(1.0))
}

/// Column range `[start, end)` within a line that covers the horizontal viewport.
pub(crate) fn viewport_col_range(
    scroll_x: Pixels,
    view_w: Pixels,
    char_width: Pixels,
    line_len_chars: usize,
) -> (usize, usize) {
    if line_len_chars == 0 {
        return (0, 0);
    }
    let cw = f32::from(char_width).max(1.0);
    let scroll_cols = (f32::from(scroll_x) / cw).floor() as usize;
    let visible_cols = (f32::from(view_w) / cw).ceil() as usize + 1;
    let col_start = scroll_cols.saturating_sub(VIEWPORT_MARGIN_CHARS);
    let col_end = (scroll_cols + visible_cols + VIEWPORT_MARGIN_CHARS).min(line_len_chars);
    (col_start, col_end.max(col_start))
}
