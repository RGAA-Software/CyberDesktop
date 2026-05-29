//! Engine-backed editor view (module root).

mod canvas;
mod editor;
mod imports;
mod language;
mod r#impl;
mod input_handler;
mod render;
mod state;
mod text_util;
mod ui;

pub use editor::EngineEditor;
pub use language::language_for_path;
