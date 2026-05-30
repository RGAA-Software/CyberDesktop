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

pub mod language;
mod syntax_languages;
pub mod buffer;
pub mod document;
pub mod encoding;
pub mod global_search;
pub mod history;
pub mod loader;
mod rope_scan;
pub mod search;
pub mod selection;
pub mod fold;
pub mod syntax;

pub use buffer::{BytePoint, EditSummary, Position, TextBuffer};
pub use document::{Document, SaveSnapshot};
pub use encoding::{EncodingInfo, LineEnding};
pub use global_search::{
    search_directory, search_directory_with_progress, FileMatches, GlobalSearchOptions,
    LineMatch, SearchProgress,
};
pub use history::{Edit, History, Transaction};
pub use language::{language_for_path, line_comment_prefix};
pub use loader::{load_file, load_file_with_progress, LoadedFile, LoadProgress};
pub use search::{
    FindInLinesOutcome, LineSearchHit, Match, SearchOptions, Searcher, FIND_IN_FILE_MAX_MATCHES,
};
pub use selection::{Cursor, SelectionSet};
pub use fold::{build_display_lines, crease_at_line, display_line_count, fold_header, is_folded_header, is_line_hidden, line_indent, starts_indent, FoldRange};
pub use syntax::{HighlightKind, HighlightSpan, SyntaxState, is_supported, parse_rope, supported_language_ids};
