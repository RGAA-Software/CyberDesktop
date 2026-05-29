//! Undo/redo history built from edit transactions.
//!
//! A [`Transaction`] is a set of non-overlapping [`Edit`]s (one per cursor for
//! multi-cursor edits) plus the selection state before and after. Edits store
//! character offsets in the document state **before** the transaction is
//! applied. Applying right-to-left keeps earlier offsets valid; [`Transaction::inverted`]
//! recomputes offsets into the *after* state so undo is exact.

use crate::buffer::{EditSummary, TextBuffer};
use crate::selection::SelectionSet;

/// A single contiguous replacement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Edit {
    /// Character offset of the replacement start (pre-transaction coordinates).
    pub start: usize,
    /// Text that was there before.
    pub old: String,
    /// Text inserted in its place.
    pub new: String,
}

impl Edit {
    pub fn insert(start: usize, text: impl Into<String>) -> Self {
        Self {
            start,
            old: String::new(),
            new: text.into(),
        }
    }

    pub fn delete(start: usize, old: impl Into<String>) -> Self {
        Self {
            start,
            old: old.into(),
            new: String::new(),
        }
    }

    pub fn replace(start: usize, old: impl Into<String>, new: impl Into<String>) -> Self {
        Self {
            start,
            old: old.into(),
            new: new.into(),
        }
    }

    fn old_len(&self) -> usize {
        self.old.chars().count()
    }

    fn new_len(&self) -> usize {
        self.new.chars().count()
    }
}

/// A grouped, atomically-undoable change.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    /// Non-overlapping edits, sorted ascending by `start`.
    pub edits: Vec<Edit>,
    pub selections_before: SelectionSet,
    pub selections_after: SelectionSet,
}

impl Transaction {
    /// Builds a transaction, sorting edits ascending by start.
    pub fn new(
        mut edits: Vec<Edit>,
        selections_before: SelectionSet,
        selections_after: SelectionSet,
    ) -> Self {
        edits.sort_by_key(|e| e.start);
        Self {
            edits,
            selections_before,
            selections_after,
        }
    }

    /// Applies this transaction to `buffer` (right-to-left so offsets stay
    /// valid). Returns one [`EditSummary`] per edit in application order, ready
    /// to drive an incremental parser.
    pub fn apply(&self, buffer: &mut TextBuffer) -> Vec<EditSummary> {
        let mut summaries = Vec::with_capacity(self.edits.len());
        for edit in self.edits.iter().rev() {
            let end = edit.start + edit.old_len();
            summaries.push(buffer.replace(edit.start, end, &edit.new));
        }
        summaries
    }

    /// Returns the inverse transaction (offsets in the *after* state).
    pub fn inverted(&self) -> Transaction {
        let mut delta: isize = 0;
        let mut inv_edits = Vec::with_capacity(self.edits.len());
        for edit in &self.edits {
            let inv_start = (edit.start as isize + delta) as usize;
            inv_edits.push(Edit {
                start: inv_start,
                old: edit.new.clone(),
                new: edit.old.clone(),
            });
            delta += edit.new_len() as isize - edit.old_len() as isize;
        }
        Transaction {
            edits: inv_edits,
            selections_before: self.selections_after.clone(),
            selections_after: self.selections_before.clone(),
        }
    }
}

/// Undo/redo stacks.
#[derive(Debug)]
pub struct History {
    undo: Vec<Transaction>,
    redo: Vec<Transaction>,
    /// Undo-stack depth at which the buffer matches the on-disk file. The buffer
    /// is "clean" when `undo.len() == saved_len`; `None` means the saved state is
    /// no longer reachable (it lived in a redo branch that was discarded).
    saved_len: Option<usize>,
}

impl Default for History {
    fn default() -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            saved_len: Some(0),
        }
    }
}

impl History {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.saved_len = Some(0);
    }

    /// Marks the current undo depth as the saved (clean) state.
    pub fn mark_saved(&mut self) {
        self.saved_len = Some(self.undo.len());
    }

    /// True when the buffer content matches the last saved state.
    pub fn is_clean(&self) -> bool {
        self.saved_len == Some(self.undo.len())
    }

    /// Records a new transaction, invalidating the redo stack.
    pub fn record(&mut self, transaction: Transaction) {
        // If the saved state lived deeper in the (now-discarded) redo branch, it
        // is no longer reachable, so the buffer can never return to "clean".
        if let Some(saved) = self.saved_len {
            if saved > self.undo.len() {
                self.saved_len = None;
            }
        }
        self.undo.push(transaction);
        self.redo.clear();
    }

    /// Pops the last transaction for undo. The caller applies the returned
    /// transaction's inverse to the buffer and restores `selections_after`
    /// (which, on the returned inverse, equals the original `selections_before`).
    pub fn undo(&mut self) -> Option<Transaction> {
        let txn = self.undo.pop()?;
        let inverse = txn.inverted();
        self.redo.push(txn);
        Some(inverse)
    }

    /// Pops the last undone transaction for redo. The caller applies it and
    /// restores its `selections_after`.
    pub fn redo(&mut self) -> Option<Transaction> {
        let txn = self.redo.pop()?;
        self.undo.push(txn.clone());
        Some(txn)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::selection::{Cursor, SelectionSet};

    fn sel(o: usize) -> SelectionSet {
        SelectionSet::single(Cursor::caret(o))
    }

    #[test]
    fn apply_and_invert_single_insert() {
        let mut buf = TextBuffer::from_str("hello");
        let txn = Transaction::new(vec![Edit::insert(5, " world")], sel(5), sel(11));
        txn.apply(&mut buf);
        assert_eq!(buf.to_string(), "hello world");

        txn.inverted().apply(&mut buf);
        assert_eq!(buf.to_string(), "hello");
    }

    #[test]
    fn multi_edit_apply_and_invert() {
        // Insert "X" at 0 and "Y" at 3 (pre-state offsets) in "abc".
        let mut buf = TextBuffer::from_str("abc");
        let txn = Transaction::new(
            vec![Edit::insert(0, "X"), Edit::insert(3, "Y")],
            sel(0),
            sel(0),
        );
        txn.apply(&mut buf);
        assert_eq!(buf.to_string(), "XabcY");

        txn.inverted().apply(&mut buf);
        assert_eq!(buf.to_string(), "abc");
    }

    #[test]
    fn history_undo_redo_round_trip() {
        let mut buf = TextBuffer::from_str("abc");
        let mut hist = History::new();

        let txn = Transaction::new(vec![Edit::replace(0, "abc", "XYZ")], sel(0), sel(3));
        txn.apply(&mut buf);
        hist.record(txn);
        assert_eq!(buf.to_string(), "XYZ");

        let undo = hist.undo().unwrap();
        undo.apply(&mut buf);
        assert_eq!(buf.to_string(), "abc");
        assert!(hist.can_redo());

        let redo = hist.redo().unwrap();
        redo.apply(&mut buf);
        assert_eq!(buf.to_string(), "XYZ");
    }
}
