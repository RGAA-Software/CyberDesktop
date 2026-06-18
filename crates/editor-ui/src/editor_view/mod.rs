//! Engine-backed editor view (module root).

mod canvas;
mod editor;
mod r#impl;
mod imports;
mod input_handler;
mod language;
mod render;
mod state;
mod text_util;
mod ui;

pub use editor::EngineEditor;
pub use language::language_for_path;
