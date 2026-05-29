//! The editable document: the single funnel through which all mutations flow.
//!
//! [`Document`] owns the [`TextBuffer`], the [`SelectionSet`], the undo
//! [`History`], and the file metadata (encoding, line ending, language, path).
//! Every mutation goes through [`Document::perform`], which builds a
//! [`Transaction`], applies it, moves the carets, and records undo — so undo,
//! dirty-tracking and (later) incremental highlighting all stay consistent.

use std::ops::Range;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::buffer::{EditSummary, TextBuffer};
use crate::encoding::{self, EncodingInfo, LineEnding};
use crate::history::{Edit, History, Transaction};
use crate::loader::LoadedFile;
use crate::selection::{Cursor, SelectionSet};

/// An editable text document.
pub struct Document {
    buffer: TextBuffer,
    selections: SelectionSet,
    history: History,
    /// A coalescing in-progress transaction (consecutive typed characters) that
    /// has not yet been committed to `history`.
    pending: Option<Transaction>,
    encoding: EncodingInfo,
    line_ending: LineEnding,
    language: String,
    path: Option<PathBuf>,
    /// Buffer revision at the last save (or load). `dirty` compares against it.
    saved_revision: u64,
    /// Byte-range edit summaries since the last [`Document::take_syntax_edits`],
    /// in application order — drives incremental tree-sitter reparsing.
    syntax_edits: Vec<EditSummary>,
}

impl Document {
    /// An empty, untitled document.
    pub fn empty() -> Self {
        Self {
            buffer: TextBuffer::new(),
            selections: SelectionSet::default(),
            history: History::new(),
            pending: None,
            encoding: EncodingInfo::default(),
            line_ending: LineEnding::default(),
            language: "text".to_string(),
            path: None,
            saved_revision: 0,
            syntax_edits: Vec::new(),
        }
    }

    /// Builds a document from a [`LoadedFile`].
    pub fn from_loaded(loaded: LoadedFile, path: Option<PathBuf>, language: impl Into<String>) -> Self {
        let saved_revision = loaded.buffer.revision();
        Self {
            buffer: loaded.buffer,
            selections: SelectionSet::default(),
            history: History::new(),
            pending: None,
            encoding: loaded.encoding,
            line_ending: loaded.line_ending,
            language: language.into(),
            path,
            saved_revision,
            syntax_edits: Vec::new(),
        }
    }

    /// Drains the byte-range edit summaries accumulated since the last call.
    /// The UI feeds these to the incremental highlighter; if it instead does a
    /// full reparse (e.g. after load), it should drain and discard them.
    pub fn take_syntax_edits(&mut self) -> Vec<EditSummary> {
        std::mem::take(&mut self.syntax_edits)
    }

    // ---- Accessors -------------------------------------------------------

    pub fn buffer(&self) -> &TextBuffer {
        &self.buffer
    }

    pub fn selections(&self) -> &SelectionSet {
        &self.selections
    }

    pub fn encoding(&self) -> EncodingInfo {
        self.encoding
    }

    pub fn line_ending(&self) -> LineEnding {
        self.line_ending
    }

    pub fn language(&self) -> &str {
        &self.language
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn revision(&self) -> u64 {
        self.buffer.revision()
    }

    pub fn dirty(&self) -> bool {
        self.buffer.revision() != self.saved_revision
    }

    pub fn can_undo(&self) -> bool {
        self.pending.is_some() || self.history.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.history.can_redo()
    }

    pub fn set_language(&mut self, language: impl Into<String>) {
        self.language = language.into();
    }

    pub fn set_encoding(&mut self, encoding: EncodingInfo) {
        self.encoding = encoding;
    }

    pub fn set_line_ending(&mut self, line_ending: LineEnding) {
        self.line_ending = line_ending;
    }

    // ---- Selection control ----------------------------------------------

    /// Places a single caret at `offset`.
    pub fn set_caret(&mut self, offset: usize) {
        self.flush_pending();
        let max = self.buffer.len_chars();
        self.selections.set_caret(offset.min(max));
    }

    /// Sets a single selection.
    pub fn set_selection(&mut self, anchor: usize, head: usize) {
        self.flush_pending();
        let max = self.buffer.len_chars();
        self.selections
            .set_single(Cursor::new(anchor.min(max), head.min(max)));
    }

    /// Replaces the whole selection set.
    pub fn set_selections(&mut self, selections: SelectionSet) {
        self.flush_pending();
        self.selections = selections;
        self.selections.clamp(self.buffer.len_chars());
    }

    /// Selects the entire document.
    pub fn select_all(&mut self) {
        self.flush_pending();
        self.selections
            .set_single(Cursor::new(0, self.buffer.len_chars()));
    }

    // ---- Editing ---------------------------------------------------------

    /// Inserts `text` at every cursor, replacing any selected ranges. Typed
    /// single characters coalesce into one undo step.
    pub fn insert(&mut self, text: &str) {
        let spans: Vec<(Range<usize>, String)> = self
            .selections
            .cursors()
            .iter()
            .map(|c| (c.range(), text.to_string()))
            .collect();
        let coalesce = is_coalescable_insert(&self.selections, text);
        self.perform(spans, coalesce);
    }

    /// Backspace: deletes the selection, or the character before each caret.
    pub fn delete_backward(&mut self) {
        let spans: Vec<(Range<usize>, String)> = self
            .selections
            .cursors()
            .iter()
            .filter_map(|c| {
                if !c.is_empty() {
                    Some((c.range(), String::new()))
                } else if c.head > 0 {
                    Some((c.head - 1..c.head, String::new()))
                } else {
                    None
                }
            })
            .collect();
        if spans.is_empty() {
            return;
        }
        self.perform(spans, false);
    }

    /// Delete: deletes the selection, or the character after each caret.
    pub fn delete_forward(&mut self) {
        let len = self.buffer.len_chars();
        let spans: Vec<(Range<usize>, String)> = self
            .selections
            .cursors()
            .iter()
            .filter_map(|c| {
                if !c.is_empty() {
                    Some((c.range(), String::new()))
                } else if c.head < len {
                    Some((c.head..c.head + 1, String::new()))
                } else {
                    None
                }
            })
            .collect();
        if spans.is_empty() {
            return;
        }
        self.perform(spans, false);
    }

    /// Replaces an explicit character range and places a caret at its end.
    pub fn replace_range(&mut self, range: Range<usize>, text: &str) {
        self.perform(vec![(range, text.to_string())], false);
    }

    /// Core mutation primitive. `spans` must be non-overlapping (any order).
    fn perform(&mut self, mut spans: Vec<(Range<usize>, String)>, coalesce: bool) {
        if spans.is_empty() {
            return;
        }
        spans.sort_by_key(|(r, _)| r.start);

        let before = self.selections.clone();
        let mut edits = Vec::with_capacity(spans.len());
        let mut carets = Vec::with_capacity(spans.len());
        let mut delta: isize = 0;

        for (range, new) in &spans {
            let old: String = self.buffer.rope().slice(range.clone()).chars().collect();
            edits.push(Edit::replace(range.start, old, new.clone()));
            let new_len = new.chars().count();
            let caret = (range.start as isize + delta) as usize + new_len;
            carets.push(Cursor::caret(caret));
            delta += new_len as isize - (range.end - range.start) as isize;
        }

        let after = SelectionSet::from_cursors(carets);
        let txn = Transaction::new(edits, before, after.clone());
        self.syntax_edits.extend(txn.apply(&mut self.buffer));
        self.selections = after;
        self.record(txn, coalesce);
    }

    fn record(&mut self, txn: Transaction, coalesce: bool) {
        if coalesce {
            if let Some(pending) = self.pending.as_mut() {
                if try_merge(pending, &txn) {
                    return;
                }
            }
            self.flush_pending();
            self.pending = Some(txn);
        } else {
            self.flush_pending();
            self.history.record(txn);
        }
    }

    fn flush_pending(&mut self) {
        if let Some(txn) = self.pending.take() {
            self.history.record(txn);
        }
    }

    /// Undoes the last change. Returns true if anything was undone.
    pub fn undo(&mut self) -> bool {
        self.flush_pending();
        match self.history.undo() {
            Some(inverse) => {
                self.syntax_edits.extend(inverse.apply(&mut self.buffer));
                self.selections = inverse.selections_after.clone();
                self.selections.clamp(self.buffer.len_chars());
                true
            }
            None => false,
        }
    }

    /// Redoes the last undone change. Returns true if anything was redone.
    pub fn redo(&mut self) -> bool {
        self.flush_pending();
        match self.history.redo() {
            Some(txn) => {
                self.syntax_edits.extend(txn.apply(&mut self.buffer));
                self.selections = txn.selections_after.clone();
                self.selections.clamp(self.buffer.len_chars());
                true
            }
            None => false,
        }
    }

    // ---- Persistence -----------------------------------------------------

    /// Encodes the buffer (honoring encoding + line ending) and returns bytes.
    pub fn encoded_bytes(&self) -> Vec<u8> {
        let text = self.normalized_text();
        encoding::encode(&text, self.encoding)
    }

    /// The buffer text with line endings normalized to [`Self::line_ending`].
    pub fn normalized_text(&self) -> String {
        let raw = self.buffer.to_string();
        match self.line_ending {
            LineEnding::Lf => raw,
            LineEnding::Crlf => to_crlf(&raw),
            LineEnding::Cr => raw.replace("\r\n", "\n").replace('\n', "\r"),
        }
    }

    /// Saves to `path`, updating the saved revision and the document's path.
    pub fn save_to(&mut self, path: PathBuf) -> Result<()> {
        let bytes = self.encoded_bytes();
        std::fs::write(&path, &bytes).with_context(|| format!("writing {}", path.display()))?;
        self.path = Some(path);
        self.saved_revision = self.buffer.revision();
        // Committing a save closes the current coalescing group.
        self.flush_pending();
        Ok(())
    }
}

fn to_crlf(text: &str) -> String {
    // Normalize any existing CRLF to LF first, then expand.
    text.replace("\r\n", "\n").replace('\n', "\r\n")
}

/// A single caret with empty selection inserting a single non-newline char.
fn is_coalescable_insert(selections: &SelectionSet, text: &str) -> bool {
    selections.len() == 1
        && selections.primary().is_empty()
        && text.chars().count() == 1
        && !text.contains('\n')
        && !text.contains('\r')
}

/// Merges `txn` (a single-char insert) into `pending` if they are contiguous.
fn try_merge(pending: &mut Transaction, txn: &Transaction) -> bool {
    if pending.edits.len() != 1 || txn.edits.len() != 1 {
        return false;
    }
    let p = &pending.edits[0];
    let t = &txn.edits[0];
    if !p.old.is_empty() || !t.old.is_empty() {
        return false;
    }
    // The new char must be inserted exactly at the end of the pending insert.
    if t.start != p.start + p.new.chars().count() {
        return false;
    }
    pending.edits[0].new.push_str(&t.new);
    pending.selections_after = txn.selections_after.clone();
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(text: &str) -> Document {
        let mut d = Document::empty();
        d.replace_range(0..0, text);
        d.history.clear();
        d.pending = None;
        d.saved_revision = d.buffer.revision();
        d.set_caret(0);
        d
    }

    #[test]
    fn insert_at_caret() {
        let mut d = doc("");
        d.insert("h");
        d.insert("i");
        assert_eq!(d.buffer.to_string(), "hi");
        assert_eq!(d.selections.primary().head, 2);
    }

    #[test]
    fn typing_coalesces_into_one_undo() {
        let mut d = doc("");
        d.insert("a");
        d.insert("b");
        d.insert("c");
        assert_eq!(d.buffer.to_string(), "abc");
        assert!(d.undo());
        assert_eq!(d.buffer.to_string(), "");
    }

    #[test]
    fn backspace_deletes_char_and_selection() {
        let mut d = doc("hello");
        d.set_caret(5);
        d.delete_backward();
        assert_eq!(d.buffer.to_string(), "hell");

        d.set_selection(0, 4);
        d.delete_backward();
        assert_eq!(d.buffer.to_string(), "");
    }

    #[test]
    fn insert_over_selection_replaces() {
        let mut d = doc("hello");
        d.set_selection(0, 5);
        d.insert("bye");
        assert_eq!(d.buffer.to_string(), "bye");
        assert_eq!(d.selections.primary().head, 3);
    }

    #[test]
    fn undo_redo() {
        let mut d = doc("abc");
        d.set_selection(0, 3);
        d.insert("X");
        assert_eq!(d.buffer.to_string(), "X");
        assert!(d.undo());
        assert_eq!(d.buffer.to_string(), "abc");
        assert!(d.redo());
        assert_eq!(d.buffer.to_string(), "X");
    }

    #[test]
    fn dirty_tracking() {
        let mut d = doc("abc");
        assert!(!d.dirty());
        d.set_caret(3);
        d.insert("d");
        assert!(d.dirty());
    }

    #[test]
    fn crlf_normalization() {
        let mut d = doc("a\nb");
        d.set_line_ending(LineEnding::Crlf);
        assert_eq!(d.normalized_text(), "a\r\nb");
    }

    #[test]
    fn multi_cursor_insert() {
        let mut d = doc("a\na\na");
        // carets after each 'a': offsets 1, 3, 5
        let sels = SelectionSet::from_cursors(vec![
            Cursor::caret(1),
            Cursor::caret(3),
            Cursor::caret(5),
        ]);
        d.set_selections(sels);
        d.insert("!");
        assert_eq!(d.buffer.to_string(), "a!\na!\na!");
    }
}
