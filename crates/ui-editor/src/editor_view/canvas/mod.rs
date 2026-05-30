//! Custom GPUI element: virtualized text surface with syntax highlighting.

mod element;
mod fold_icon;
mod horizontal_viewport;
mod paint;
mod prepaint;
mod prepaint_wrapped;
mod syntax_paint;
mod wrap_virtual;

pub(crate) use horizontal_viewport::LONG_LINE_COL_THRESHOLD;
pub(crate) use horizontal_viewport::measure_avg_char_width;
pub(crate) use wrap_virtual::{
    char_range_for_wrap_subrows, cols_per_row, estimated_wrap_rows, visible_subrow_range,
    wrapped_block_height,
};

pub(crate) use element::EditorCanvas;
