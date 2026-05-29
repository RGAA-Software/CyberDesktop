//! Custom GPUI element: virtualized text surface with syntax highlighting.

mod element;
mod paint;
mod prepaint;
mod prepaint_wrapped;
mod syntax_paint;

pub(crate) use element::EditorCanvas;
