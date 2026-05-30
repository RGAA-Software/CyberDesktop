//! Custom GPUI element: virtualized text surface with syntax highlighting.

mod element;
mod fold_icon;
mod horizontal_viewport;
mod paint;
mod prepaint;
mod prepaint_wrapped;
mod syntax_paint;

pub(crate) use horizontal_viewport::LONG_LINE_COL_THRESHOLD;

pub(crate) use element::EditorCanvas;
