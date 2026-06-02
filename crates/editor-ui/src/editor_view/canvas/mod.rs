//! Custom GPUI element: virtualized text surface with syntax highlighting.

mod element;
mod fold_icon;
mod horizontal_viewport;
mod paint;
mod prepaint;
mod prepaint_wrapped;
mod syntax_paint;
mod wrap_virtual;

pub(crate) use horizontal_viewport::measure_avg_char_width;
pub(crate) use wrap_virtual::{cols_per_row, estimated_wrap_rows, wrapped_block_height};

pub(crate) use element::EditorCanvas;
pub(crate) use fold_icon::fold_hit_bounds;
