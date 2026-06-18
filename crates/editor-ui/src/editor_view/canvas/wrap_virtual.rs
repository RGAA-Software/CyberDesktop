//! Virtual sub-row estimates for soft-wrapped long lines.

use gpui::{px, Pixels};

/// Approximate character columns per visual row at `view_w`.
pub(crate) fn cols_per_row(char_width: Pixels, view_w: Pixels) -> usize {
    (f32::from(view_w) / f32::from(char_width).max(1.0))
        .floor()
        .max(1.0) as usize
}

pub(crate) fn estimated_wrap_rows(line_len_chars: usize, cols_per_row: usize) -> usize {
    if line_len_chars == 0 {
        1
    } else {
        line_len_chars.div_ceil(cols_per_row)
    }
}

/// Character column range covering wrap sub-rows `[first, last)` with margin.
pub(crate) fn char_range_for_wrap_subrows(
    first_subrow: usize,
    last_subrow: usize,
    cols_per_row: usize,
    line_len: usize,
) -> (usize, usize) {
    let margin = 1usize;
    let first = first_subrow
        .saturating_sub(margin)
        .saturating_mul(cols_per_row);
    let last = (last_subrow + margin + 1)
        .saturating_mul(cols_per_row)
        .min(line_len);
    (first.min(line_len), last.max(first))
}

/// Visible wrap sub-row range within a document-line block.
pub(crate) fn visible_subrow_range(
    block_top: Pixels,
    viewport_top: Pixels,
    viewport_bottom: Pixels,
    line_height: Pixels,
    total_subrows: usize,
) -> (usize, usize) {
    if total_subrows == 0 {
        return (0, 0);
    }
    let lh = f32::from(line_height).max(1.0);
    let rel_top = f32::from((viewport_top - block_top).max(px(0.0)));
    let rel_bottom = f32::from((viewport_bottom - block_top).max(px(0.0)));
    let first = (rel_top / lh).floor() as usize;
    let last = ((rel_bottom / lh).ceil() as usize + 1).min(total_subrows);
    (first.min(total_subrows), last.max(first).min(total_subrows))
}

/// Block height in pixels for a wrapped row entry.
pub(crate) fn wrapped_block_height(
    line_height: Pixels,
    wrap_row_count: usize,
    shaped_rows: usize,
) -> Pixels {
    let rows = if wrap_row_count > 0 {
        wrap_row_count
    } else {
        shaped_rows.max(1)
    };
    line_height * rows as f32
}
