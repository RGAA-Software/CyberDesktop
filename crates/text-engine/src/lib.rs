//! Core text-editing engine for CyberEditor.
//!
//! This crate is intentionally free of any UI/GPUI dependency. It owns the
//! pieces that decide whether the editor can open and edit large files at
//! Notepad++-class speed:
//!
//! - [`buffer`]: a rope-backed [`buffer::TextBuffer`] with O(log n) edits and
//!   line/char/byte conversions.
//! - [`encoding`]: encoding detection (BOM + `chardetng`) and decode/encode via
//!   `encoding_rs`, plus line-ending detection.
//! - [`loader`]: memory-mapped, streaming-decode file loading straight into the
//!   rope (no giant intermediate `String`).

pub mod buffer;
pub mod document;
pub mod encoding;
pub mod global_search;
pub mod history;
pub mod loader;
pub mod search;
pub mod selection;
pub mod syntax;

pub use buffer::{BytePoint, EditSummary, Position, TextBuffer};
pub use document::Document;
pub use encoding::{EncodingInfo, LineEnding};
pub use global_search::{search_directory, FileMatches, GlobalSearchOptions, LineMatch};
pub use history::{Edit, History, Transaction};
pub use loader::{load_file, LoadedFile};
pub use search::{Match, SearchOptions, Searcher};
pub use selection::{Cursor, SelectionSet};
pub use syntax::{HighlightKind, HighlightSpan, SyntaxState};
