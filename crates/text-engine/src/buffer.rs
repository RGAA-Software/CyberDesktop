//! Rope-backed text buffer.
//!
//! All size-related queries ([`TextBuffer::len_chars`], [`TextBuffer::line_count`],
//! position conversions, …) are O(log n) thanks to [`ropey`]. This is what lets
//! the editor edit multi-hundred-MB files without recomputing metrics over the
//! whole document on every keystroke.

use ropey::Rope;

/// A zero-based `(line, column)` position. `column` counts **characters** (not
/// bytes) from the start of the line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

impl Position {
    pub fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}

/// A `(row, column)` position in **byte** coordinates, matching the convention
/// tree-sitter's `Point` uses for [`EditSummary`] interop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BytePoint {
    pub row: usize,
    /// Byte offset from the start of `row`.
    pub column: usize,
}

/// The result of applying an edit, expressed in byte offsets (and byte-based
/// row/column points) so it can be fed directly into an incremental parser
/// (e.g. tree-sitter's `InputEdit`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditSummary {
    /// Byte offset where the edit started.
    pub start_byte: usize,
    /// Byte offset of the end of the removed range (before the edit).
    pub old_end_byte: usize,
    /// Byte offset of the end of the inserted text (after the edit).
    pub new_end_byte: usize,
    /// Row/column of `start_byte` (identical in old and new text).
    pub start_point: BytePoint,
    /// Row/column of the end of the removed range, in the **pre-edit** text.
    pub old_end_point: BytePoint,
    /// Row/column of the end of the inserted text, in the **post-edit** text.
    pub new_end_point: BytePoint,
}

/// A rope-backed text buffer with a monotonically increasing revision counter.
#[derive(Clone, Debug)]
pub struct TextBuffer {
    rope: Rope,
    revision: u64,
}

impl Default for TextBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl TextBuffer {
    /// Creates an empty buffer.
    pub fn new() -> Self {
        Self {
            rope: Rope::new(),
            revision: 0,
        }
    }

    /// Creates a buffer from an in-memory string.
    pub fn from_str(text: &str) -> Self {
        Self {
            rope: Rope::from_str(text),
            revision: 0,
        }
    }

    /// Creates a buffer from an already-built [`Rope`] (used by the streaming
    /// loader, which fills a `RopeBuilder` directly).
    pub fn from_rope(rope: Rope) -> Self {
        Self { rope, revision: 0 }
    }

    /// Borrows the underlying rope (read-only).
    pub fn rope(&self) -> &Rope {
        &self.rope
    }

    /// The current revision. Bumped on every mutation.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    /// Total number of characters.
    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }

    /// Total number of bytes (UTF-8).
    pub fn len_bytes(&self) -> usize {
        self.rope.len_bytes()
    }

    /// Number of lines. An empty buffer reports `1`. A trailing newline does
    /// **not** add a phantom empty line to this count.
    pub fn line_count(&self) -> usize {
        // ropey counts a trailing line break as starting a new (empty) line.
        // For an editor we want "1 for empty" and to not over-count the final
        // newline, matching what users expect in the status bar.
        if self.rope.len_chars() == 0 {
            return 1;
        }
        let lines = self.rope.len_lines();
        if self.rope.char(self.rope.len_chars() - 1) == '\n' {
            lines.saturating_sub(1).max(1)
        } else {
            lines.max(1)
        }
    }

    /// Returns the text of `line` (0-based) without the trailing line break.
    pub fn line_text(&self, line: usize) -> String {
        if line >= self.rope.len_lines() {
            return String::new();
        }
        let slice = self.rope.line(line);
        let mut s = slice.to_string();
        while s.ends_with('\n') || s.ends_with('\r') {
            s.pop();
        }
        s
    }

    /// Number of characters on `line`, excluding the trailing line break.
    pub fn line_len_chars(&self, line: usize) -> usize {
        if line >= self.rope.len_lines() {
            return 0;
        }
        let slice = self.rope.line(line);
        let mut len = slice.len_chars();
        // Trim trailing CR/LF.
        let mut idx = slice.len_chars();
        while idx > 0 {
            let c = slice.char(idx - 1);
            if c == '\n' || c == '\r' {
                idx -= 1;
                len -= 1;
            } else {
                break;
            }
        }
        len
    }

    // ---- Conversions -----------------------------------------------------

    /// Converts a character offset to a [`Position`].
    pub fn char_to_position(&self, char_idx: usize) -> Position {
        let char_idx = char_idx.min(self.rope.len_chars());
        let line = self.rope.char_to_line(char_idx);
        let line_start = self.rope.line_to_char(line);
        Position::new(line, char_idx - line_start)
    }

    /// Converts a [`Position`] to a character offset, clamping out-of-range
    /// lines/columns to valid values.
    pub fn position_to_char(&self, pos: Position) -> usize {
        let last_line = self.rope.len_lines().saturating_sub(1);
        let line = pos.line.min(last_line);
        let line_start = self.rope.line_to_char(line);
        let line_chars = self.line_len_chars(line);
        line_start + pos.column.min(line_chars)
    }

    /// Converts a character offset to a byte offset.
    pub fn char_to_byte(&self, char_idx: usize) -> usize {
        self.rope.char_to_byte(char_idx.min(self.rope.len_chars()))
    }

    /// Converts a byte offset to a character offset.
    pub fn byte_to_char(&self, byte_idx: usize) -> usize {
        self.rope.byte_to_char(byte_idx.min(self.rope.len_bytes()))
    }

    /// Total length in UTF-16 code units (for platform IME interop).
    pub fn len_utf16(&self) -> usize {
        self.rope.len_utf16_cu()
    }

    /// Converts a character offset to a UTF-16 code-unit offset.
    pub fn char_to_utf16(&self, char_idx: usize) -> usize {
        self.rope.char_to_utf16_cu(char_idx.min(self.rope.len_chars()))
    }

    /// Converts a UTF-16 code-unit offset to a character offset.
    pub fn utf16_to_char(&self, utf16_idx: usize) -> usize {
        self.rope.utf16_cu_to_char(utf16_idx.min(self.rope.len_utf16_cu()))
    }

    // ---- Editing ---------------------------------------------------------

    /// Inserts `text` at `char_idx`. Returns a byte-based [`EditSummary`].
    pub fn insert(&mut self, char_idx: usize, text: &str) -> EditSummary {
        self.replace(char_idx, char_idx, text)
    }

    /// Removes the character range `start..end`. Returns a byte-based
    /// [`EditSummary`].
    pub fn remove(&mut self, start: usize, end: usize) -> EditSummary {
        self.replace(start, end, "")
    }

    /// Replaces the character range `start..end` with `text` in a single edit.
    pub fn replace(&mut self, start: usize, end: usize, text: &str) -> EditSummary {
        let len = self.rope.len_chars();
        let start = start.min(len);
        let end = end.min(len).max(start);
        let start_byte = self.rope.char_to_byte(start);
        let old_end_byte = self.rope.char_to_byte(end);
        // Points are derived while the rope still holds the pre-edit text.
        let start_point = self.byte_point(start_byte);
        let old_end_point = self.byte_point(old_end_byte);

        if start != end {
            self.rope.remove(start..end);
        }
        if !text.is_empty() {
            self.rope.insert(start, text);
        }
        self.revision = self.revision.wrapping_add(1);

        let new_end_byte = start_byte + text.len();
        let new_end_point = self.byte_point(new_end_byte);
        EditSummary {
            start_byte,
            old_end_byte,
            new_end_byte,
            start_point,
            old_end_point,
            new_end_point,
        }
    }

    /// Byte-coordinate `(row, column)` of a byte offset in the current rope.
    fn byte_point(&self, byte_idx: usize) -> BytePoint {
        let byte_idx = byte_idx.min(self.rope.len_bytes());
        let row = self.rope.byte_to_line(byte_idx);
        let line_start = self.rope.line_to_byte(row);
        BytePoint {
            row,
            column: byte_idx - line_start,
        }
    }

    /// Materializes the whole buffer as a `String`. Avoid on the hot path for
    /// large files; prefer line/slice access.
    pub fn to_string(&self) -> String {
        self.rope.to_string()
    }

    /// Returns the text in the given character range.
    pub fn slice_text(&self, range: std::ops::Range<usize>) -> String {
        let len = self.rope.len_chars();
        let start = range.start.min(len);
        let end = range.end.min(len).max(start);
        self.rope.slice(start..end).chars().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_buffer_metrics() {
        let b = TextBuffer::new();
        assert_eq!(b.len_chars(), 0);
        assert_eq!(b.line_count(), 1);
    }

    #[test]
    fn line_count_ignores_trailing_newline() {
        assert_eq!(TextBuffer::from_str("a\nb\nc").line_count(), 3);
        assert_eq!(TextBuffer::from_str("a\nb\nc\n").line_count(), 3);
    }

    #[test]
    fn position_round_trip_with_multibyte() {
        let b = TextBuffer::from_str("héllo\nwörld");
        let pos = Position::new(1, 3);
        let ci = b.position_to_char(pos);
        assert_eq!(b.char_to_position(ci), pos);
    }

    #[test]
    fn line_helpers() {
        let b = TextBuffer::from_str("abc\nde\n");
        assert_eq!(b.line_text(0), "abc");
        assert_eq!(b.line_len_chars(0), 3);
        assert_eq!(b.line_text(1), "de");
        assert_eq!(b.line_len_chars(1), 2);
    }

    #[test]
    fn edits_report_byte_summary() {
        let mut b = TextBuffer::from_str("hello");
        let s = b.insert(5, " world");
        assert_eq!(b.to_string(), "hello world");
        assert_eq!(s.start_byte, 5);
        assert_eq!(s.old_end_byte, 5);
        assert_eq!(s.new_end_byte, 11);

        let r = b.remove(0, 6);
        assert_eq!(b.to_string(), "world");
        assert_eq!(r.start_byte, 0);
        assert_eq!(r.old_end_byte, 6);
        assert_eq!(r.new_end_byte, 0);

        let rep = b.replace(0, 5, "earth");
        assert_eq!(b.to_string(), "earth");
        assert_eq!(rep.new_end_byte, 5);
    }

    #[test]
    fn revision_advances_on_mutation() {
        let mut b = TextBuffer::from_str("x");
        let r0 = b.revision();
        b.insert(1, "y");
        assert_ne!(b.revision(), r0);
    }
}
