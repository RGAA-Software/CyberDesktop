//! Cursor and selection model.
//!
//! Offsets are **character** offsets into the [`crate::buffer::TextBuffer`].
//! A [`Cursor`] is an `anchor`/`head` pair; when they are equal it is a plain
//! caret with no selection. `head` is the moving end (where the caret blinks).

/// A single caret/selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    /// The fixed end of the selection (where it was started).
    pub anchor: usize,
    /// The moving end of the selection (where the caret is).
    pub head: usize,
}

impl Cursor {
    /// A caret with no selection at `offset`.
    pub fn caret(offset: usize) -> Self {
        Self {
            anchor: offset,
            head: offset,
        }
    }

    /// A selection spanning `anchor..head`.
    pub fn new(anchor: usize, head: usize) -> Self {
        Self { anchor, head }
    }

    /// Lower bound of the selection.
    pub fn start(&self) -> usize {
        self.anchor.min(self.head)
    }

    /// Upper bound of the selection.
    pub fn end(&self) -> usize {
        self.anchor.max(self.head)
    }

    /// Normalized `start..end` range.
    pub fn range(&self) -> std::ops::Range<usize> {
        self.start()..self.end()
    }

    /// True when there is no selected text.
    pub fn is_empty(&self) -> bool {
        self.anchor == self.head
    }

    /// Number of selected characters.
    pub fn len(&self) -> usize {
        self.end() - self.start()
    }

    /// Collapses the selection to its head (clears the selection).
    pub fn collapse_to_head(&mut self) {
        self.anchor = self.head;
    }
}

/// An ordered, non-overlapping set of cursors. Always non-empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectionSet {
    cursors: Vec<Cursor>,
}

impl Default for SelectionSet {
    fn default() -> Self {
        Self::single(Cursor::caret(0))
    }
}

impl SelectionSet {
    /// A set containing a single cursor.
    pub fn single(cursor: Cursor) -> Self {
        Self {
            cursors: vec![cursor],
        }
    }

    /// Builds a normalized set from arbitrary cursors (sorted + merged).
    pub fn from_cursors(mut cursors: Vec<Cursor>) -> Self {
        if cursors.is_empty() {
            cursors.push(Cursor::caret(0));
        }
        let mut set = Self { cursors };
        set.normalize();
        set
    }

    pub fn cursors(&self) -> &[Cursor] {
        &self.cursors
    }

    /// The primary cursor (last one added / lowest by convention here = first).
    pub fn primary(&self) -> Cursor {
        // Convention: primary is the last cursor in document order. For a single
        // selection this is just that cursor.
        *self.cursors.last().expect("selection set is never empty")
    }

    pub fn len(&self) -> usize {
        self.cursors.len()
    }

    pub fn is_empty(&self) -> bool {
        false
    }

    /// Replaces with a single caret at `offset`.
    pub fn set_caret(&mut self, offset: usize) {
        self.cursors.clear();
        self.cursors.push(Cursor::caret(offset));
    }

    /// Replaces with a single selection.
    pub fn set_single(&mut self, cursor: Cursor) {
        self.cursors.clear();
        self.cursors.push(cursor);
    }

    /// Adds a cursor and re-normalizes.
    pub fn add(&mut self, cursor: Cursor) {
        self.cursors.push(cursor);
        self.normalize();
    }

    /// Clamps all cursors into `0..=max` and re-normalizes. Call after the
    /// buffer length changes from outside an edit.
    pub fn clamp(&mut self, max: usize) {
        for c in &mut self.cursors {
            c.anchor = c.anchor.min(max);
            c.head = c.head.min(max);
        }
        self.normalize();
    }

    /// Sorts by start offset and merges overlapping/touching selections.
    fn normalize(&mut self) {
        self.cursors.sort_by_key(|c| (c.start(), c.end()));
        let mut merged: Vec<Cursor> = Vec::with_capacity(self.cursors.len());
        for cur in self.cursors.drain(..) {
            if let Some(last) = merged.last_mut() {
                if cur.start() <= last.end() {
                    // Overlap: extend, preserving the head direction of the later one.
                    let new_start = last.start().min(cur.start());
                    let new_end = last.end().max(cur.end());
                    // Keep caret at the later head.
                    *last = Cursor::new(new_start, new_end);
                    continue;
                }
            }
            merged.push(cur);
        }
        if merged.is_empty() {
            merged.push(Cursor::caret(0));
        }
        self.cursors = merged;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caret_has_no_selection() {
        let c = Cursor::caret(5);
        assert!(c.is_empty());
        assert_eq!(c.range(), 5..5);
        assert_eq!(c.len(), 0);
    }

    #[test]
    fn range_is_normalized() {
        let c = Cursor::new(10, 4);
        assert_eq!(c.start(), 4);
        assert_eq!(c.end(), 10);
        assert_eq!(c.range(), 4..10);
        assert_eq!(c.len(), 6);
    }

    #[test]
    fn overlapping_cursors_merge() {
        let set = SelectionSet::from_cursors(vec![
            Cursor::new(0, 5),
            Cursor::new(3, 8),
            Cursor::caret(20),
        ]);
        assert_eq!(set.len(), 2);
        assert_eq!(set.cursors()[0].range(), 0..8);
        assert_eq!(set.cursors()[1].range(), 20..20);
    }

    #[test]
    fn clamp_keeps_in_bounds() {
        let mut set = SelectionSet::single(Cursor::new(2, 50));
        set.clamp(10);
        assert_eq!(set.cursors()[0].range(), 2..10);
    }
}
