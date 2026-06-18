//! `EngineEditor` — `movement`.

use super::super::imports::*;
use super::super::text_util::wrap_rows;

impl EngineEditor {
    /// Lines (or wrapped visual rows) visible in the editor viewport — used for Page Up/Down.
    pub(crate) fn visible_page_line_count(&self) -> usize {
        let Some(bounds) = self.last_bounds else {
            return 1;
        };
        let lh = f32::from(self.line_height);
        if lh <= 0.0 {
            return 1;
        }
        let viewport_h = f32::from(bounds.size.height - self.editor_bottom_inset());
        if self.soft_wrap {
            let content_bottom = f32::from(bounds.bottom() - self.editor_bottom_inset());
            let top = f32::from(bounds.top());
            let mut rows = 0usize;
            for wl in &self.wrapped_visible {
                let wl_top = f32::from(wl.top);
                let block = lh * wrap_rows(&wl.wrapped) as f32;
                if wl_top >= content_bottom {
                    break;
                }
                if wl_top + block > top {
                    rows += wrap_rows(&wl.wrapped);
                }
            }
            if rows > 0 {
                return rows;
            }
        }
        (viewport_h / lh).floor().max(1.0) as usize
    }

    pub(crate) fn page_vertical(&mut self, dir: isize, extend: bool) {
        let lines = self.visible_page_line_count();
        self.move_cursors(extend, |buf, c| {
            let pos = buf.char_to_position(c.head);
            let last_line = buf.line_count().saturating_sub(1) as isize;
            let target_line =
                (pos.line as isize + dir * lines as isize).clamp(0, last_line) as usize;
            buf.position_to_char(Position::new(target_line, pos.column))
        });
    }

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
