//! `EngineEditor` — `movement`.

use super::super::imports::*;

impl EngineEditor {
    // ---- Movement (operates on every cursor) -----------------------------

    /// Remaps every cursor's head via `f`, extending the selection or collapsing
    /// to a caret, then re-normalizes (merging any cursors that collide).
    pub(crate) fn move_cursors(&mut self, extend: bool, f: impl Fn(&TextBuffer, Cursor) -> usize) {
        let buf = self.document.buffer();
        let new_cursors: Vec<Cursor> = self
            .document
            .selections()
            .cursors()
            .iter()
            .map(|c| {
                let head = f(buf, *c);
                if extend {
                    Cursor::new(c.anchor, head)
                } else {
                    Cursor::caret(head)
                }
            })
            .collect();
        self.document
            .set_selections(SelectionSet::from_cursors(new_cursors));
    }

    pub(crate) fn move_horizontal(&mut self, dir: isize, extend: bool) {
        let len = self.document.buffer().len_chars();
        let new_cursors: Vec<Cursor> = self
            .document
            .selections()
            .cursors()
            .iter()
            .map(|c| {
                if !extend && !c.is_empty() {
                    // Collapse a selection to the edge in the move direction.
                    Cursor::caret(if dir < 0 { c.start() } else { c.end() })
                } else {
                    let head = if dir < 0 {
                        c.head.saturating_sub(1)
                    } else {
                        (c.head + 1).min(len)
                    };
                    if extend {
                        Cursor::new(c.anchor, head)
                    } else {
                        Cursor::caret(head)
                    }
                }
            })
            .collect();
        self.document
            .set_selections(SelectionSet::from_cursors(new_cursors));
    }

    pub(crate) fn move_vertical(&mut self, dir: isize, extend: bool) {
        self.move_cursors(extend, |buf, c| {
            let pos = buf.char_to_position(c.head);
            let last_line = buf.line_count().saturating_sub(1) as isize;
            let target_line = (pos.line as isize + dir).clamp(0, last_line) as usize;
            buf.position_to_char(Position::new(target_line, pos.column))
        });
    }

}
