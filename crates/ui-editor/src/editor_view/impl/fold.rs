//! Code folding (indentation-based creases, Zed-style).

use cyber_desktop_text_engine::{build_display_lines, crease_at_line, fold_header, FoldRange};

use super::super::imports::*;

pub(crate) const FOLD_GUTTER_WIDTH: Pixels = px(14.);

impl EngineEditor {
    pub(crate) fn rebuild_display_lines(&mut self) {
        self.display_lines = build_display_lines(self.document.buffer(), &self.active_folds);
    }

    pub(crate) fn display_line_count(&self) -> usize {
        if self.display_lines.is_empty() {
            self.document.buffer().line_count()
        } else {
            self.display_lines.len()
        }
    }

    pub(crate) fn buffer_line_for_display(&self, display_ix: usize) -> Option<usize> {
        self.display_lines.get(display_ix).copied()
    }

    pub(crate) fn display_index_for_buffer_line(&self, line: usize) -> Option<usize> {
        self.display_lines.iter().position(|&l| l == line)
    }

    pub(crate) fn toggle_fold_at_line(&mut self, line: usize, cx: &mut Context<Self>) {
        if let Some(existing) = self.active_folds.iter().position(|f| f.header_line == line) {
            self.active_folds.remove(existing);
        } else if let Some(crease) = crease_at_line(self.document.buffer(), line) {
            self.active_folds.push(crease);
        } else {
            return;
        }
        self.rebuild_display_lines();
        cx.notify();
    }

    pub(crate) fn fold_all(&mut self, cx: &mut Context<Self>) {
        let buf = self.document.buffer();
        let count = buf.line_count();
        self.active_folds.clear();
        for line in 0..count {
            if let Some(crease) = crease_at_line(buf, line) {
                if !self
                    .active_folds
                    .iter()
                    .any(|f| f.contains_hidden_line(crease.header_line))
                {
                    self.active_folds.push(crease);
                }
            }
        }
        self.rebuild_display_lines();
        cx.notify();
    }

    pub(crate) fn unfold_all(&mut self, cx: &mut Context<Self>) {
        if self.active_folds.is_empty() {
            return;
        }
        self.active_folds.clear();
        self.rebuild_display_lines();
        cx.notify();
    }

    pub(crate) fn toggle_fold_at_caret(&mut self, cx: &mut Context<Self>) {
        let line = self
            .document
            .buffer()
            .char_to_position(self.document.selections().primary().head)
            .line;
        self.toggle_fold_at_line(line, cx);
    }

    pub(crate) fn crease_at(&self, line: usize) -> Option<FoldRange> {
        crease_at_line(self.document.buffer(), line)
    }

    pub(crate) fn is_folded_header(&self, line: usize) -> bool {
        fold_header(line, &self.active_folds).is_some()
    }

    /// Hit-test the fold gutter (after line numbers); returns buffer line index.
    pub(crate) fn fold_gutter_hit(&self, pos: Point<Pixels>) -> Option<usize> {
        let bounds = self.last_bounds?;
        let fold_left = bounds.left() + self.gutter_width - FOLD_GUTTER_WIDTH;
        if pos.x < fold_left || pos.x >= fold_left + FOLD_GUTTER_WIDTH {
            return None;
        }
        let vl = self.visible.iter().find(|vl| {
            pos.y >= vl.top && pos.y < vl.top + self.line_height
        })?;
        Some(vl.line)
    }
}
