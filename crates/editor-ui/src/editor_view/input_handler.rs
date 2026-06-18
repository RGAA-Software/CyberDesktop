//! IME / platform text input routing.

use super::imports::*;
use super::text_util::{expand_tabs, EDITOR_TAB_SIZE};

impl EntityInputHandler for EngineEditor {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let buf = self.document.buffer();
        let start = buf.utf16_to_char(range_utf16.start);
        let end = buf.utf16_to_char(range_utf16.end);
        actual_range.replace(buf.char_to_utf16(start)..buf.char_to_utf16(end));
        Some(buf.slice_text(start..end))
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let primary = self.document.selections().primary();
        let buf = self.document.buffer();
        Some(UTF16Selection {
            range: buf.char_to_utf16(primary.start())..buf.char_to_utf16(primary.end()),
            reversed: primary.head < primary.anchor,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        let r = self.marked_range.clone()?;
        let buf = self.document.buffer();
        Some(buf.char_to_utf16(r.start)..buf.char_to_utf16(r.end))
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.marked_range = None;
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Plain typing with multiple carets and no IME marking: insert at every
        // cursor (the engine handles the multi-span edit atomically).
        if range_utf16.is_none()
            && self.marked_range.is_none()
            && self.document.selections().len() > 1
        {
            self.document.insert(new_text);
            self.changed(cx);
            return;
        }
        let range_char = self.resolve_input_range(range_utf16);
        self.document
            .set_selection(range_char.start, range_char.end);
        self.document.insert(new_text);
        self.marked_range = None;
        self.changed(cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let range_char = self.resolve_input_range(range_utf16);
        self.document
            .set_selection(range_char.start, range_char.end);
        self.document.insert(new_text);
        if new_text.is_empty() {
            self.marked_range = None;
        } else {
            let inserted = new_text.chars().count();
            self.marked_range = Some(range_char.start..range_char.start + inserted);
        }
        self.changed(cx);
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let start_char = self.document.buffer().utf16_to_char(range_utf16.start);
        let pos = self.document.buffer().char_to_position(start_char);
        if self.soft_wrap {
            let wl = self.wrapped_visible.iter().find(|wl| wl.line == pos.line)?;
            let col = start_char.saturating_sub(wl.start_char);
            if !wl.fragment_text.is_empty() && col < wl.start_col {
                let x = element_bounds.left() + self.gutter_width;
                let top = wl.block_top;
                return Some(Bounds::from_corners(
                    point(x, top),
                    point(x, top + self.line_height),
                ));
            }
            let byte_col = if wl.fragment_text.is_empty() {
                col
            } else {
                col.saturating_sub(wl.start_col)
            };
            let byte = if wl.fragment_text.is_empty() {
                let line_text = self.document.buffer().line_text(wl.line);
                expand_tabs(&line_text, EDITOR_TAB_SIZE).original_char_to_expanded_byte(byte_col)
            } else {
                expand_tabs(&wl.fragment_text, EDITOR_TAB_SIZE)
                    .original_char_to_expanded_byte(byte_col)
            };
            let p = wl.wrapped.position_for_index(byte, self.line_height)?;
            let x = element_bounds.left() + self.gutter_width + p.x;
            let top = wl.top + p.y;
            return Some(Bounds::from_corners(
                point(x, top),
                point(x, top + self.line_height),
            ));
        }
        let vl = self.visible.iter().find(|vl| vl.line == pos.line)?;
        let col = start_char.saturating_sub(vl.start_char);
        if col < vl.start_col {
            let x = vl.fragment_left;
            return Some(Bounds::from_corners(
                point(x, vl.top),
                point(x, vl.top + self.line_height),
            ));
        }
        let frag_col = col.saturating_sub(vl.start_col);
        let byte = expand_tabs(&vl.fragment_text, EDITOR_TAB_SIZE)
            .original_char_to_expanded_byte(frag_col);
        let x = vl.fragment_left + vl.shaped.x_for_index(byte);
        Some(Bounds::from_corners(
            point(x, vl.top),
            point(x, vl.top + self.line_height),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let idx = self.index_for_position(point);
        Some(self.document.buffer().char_to_utf16(idx))
    }
}

impl EngineEditor {
    /// Resolves the target char range for a text-input edit: explicit range,
    /// else the marked range, else the current selection.
    pub(crate) fn resolve_input_range(&self, range_utf16: Option<Range<usize>>) -> Range<usize> {
        if let Some(r) = range_utf16 {
            let buf = self.document.buffer();
            return buf.utf16_to_char(r.start)..buf.utf16_to_char(r.end);
        }
        if let Some(m) = self.marked_range.clone() {
            return m;
        }
        self.document.selections().primary().range()
    }
}
