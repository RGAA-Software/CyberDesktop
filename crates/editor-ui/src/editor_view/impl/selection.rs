//! `EngineEditor` — `selection`.

use super::super::imports::*;

impl EngineEditor {
    pub(crate) fn move_home(&mut self, extend: bool) {
        self.move_cursors(extend, |buf, c| {
            let line = buf.char_to_position(c.head).line;
            buf.position_to_char(Position::new(line, 0))
        });
    }

    pub(crate) fn move_end(&mut self, extend: bool) {
        self.move_cursors(extend, |buf, c| {
            let line = buf.char_to_position(c.head).line;
            let col = buf.line_len_chars(line);
            buf.position_to_char(Position::new(line, col))
        });
    }

    // ---- Multi-cursor ----------------------------------------------------

    /// Adds a caret at `idx`, keeping existing cursors (Alt+Click).
    pub(crate) fn add_caret(&mut self, idx: usize, cx: &mut Context<Self>) {
        let mut set = self.document.selections().clone();
        set.add(Cursor::caret(idx));
        self.document.set_selections(set);
        cx.notify();
    }

    /// Selects the word around the primary caret (the seed for "add next match").
    pub(crate) fn select_word(&mut self, cx: &mut Context<Self>) {
        let primary = self.document.selections().primary();
        let buf = self.document.buffer();
        let pos = buf.char_to_position(primary.head);
        let line_start = buf.position_to_char(Position::new(pos.line, 0));
        let chars: Vec<char> = buf.line_text(pos.line).chars().collect();
        let is_word = |c: char| c.is_alphanumeric() || c == '_';
        let mut start = pos.column.min(chars.len());
        let mut end = start;
        while start > 0 && is_word(chars[start - 1]) {
            start -= 1;
        }
        while end < chars.len() && is_word(chars[end]) {
            end += 1;
        }
        if end > start {
            self.document
                .set_selection(line_start + start, line_start + end);
            self.changed(cx);
        }
    }

    /// Ctrl+D: select the word, or add the next occurrence of the current
    /// selection as an additional cursor (wrapping around the document).
    pub(crate) fn add_next_occurrence(&mut self, cx: &mut Context<Self>) {
        let primary = self.document.selections().primary();
        if primary.is_empty() {
            return self.select_word(cx);
        }
        let needle = self.document.buffer().slice_text(primary.range());
        if needle.is_empty() || needle.contains('\n') {
            return;
        }
        let options = SearchOptions {
            case_sensitive: true,
            whole_word: false,
            regex: false,
        };
        let Ok(searcher) = Searcher::new(&needle, options) else {
            return;
        };
        let found = searcher
            .find_next(self.document.buffer(), primary.end())
            .or_else(|| searcher.find_next(self.document.buffer(), 0));
        if let Some(m) = found {
            let exists = self
                .document
                .selections()
                .cursors()
                .iter()
                .any(|c| c.start() == m.start && c.end() == m.end);
            if !exists {
                let mut set = self.document.selections().clone();
                set.add(Cursor::new(m.start, m.end));
                self.document.set_selections(set);
                self.ensure_caret_visible();
                cx.notify();
            }
        }
    }

    /// Collapses any multi-selection back to a single primary caret.
    pub(crate) fn collapse_carets(&mut self, cx: &mut Context<Self>) {
        if self.document.selections().len() > 1 {
            let head = self.document.selections().primary().head;
            self.document.set_caret(head);
            cx.notify();
        }
    }
}
