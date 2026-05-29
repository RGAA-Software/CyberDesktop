//! `EngineEditor` — `core`.

use super::super::imports::*;

impl EngineEditor {
    pub(crate) fn refresh_syntax(&mut self) {
        let rev = self.document.revision();
        if self.parsed_revision == Some(rev) {
            return;
        }
        // Incremental on edits (feed byte-range edits to the old tree); full
        // parse only when we have no prior tree (load / language switch).
        if self.parsed_revision.is_some() {
            let edits = self.document.take_syntax_edits();
            for edit in &edits {
                self.syntax.edit(edit);
            }
        } else {
            self.document.take_syntax_edits();
        }
        self.syntax.reparse(self.document.buffer().rope());
        self.parsed_revision = Some(rev);
    }

    pub(crate) fn changed(&mut self, cx: &mut Context<Self>) {
        self.ensure_caret_visible();
        cx.notify();
    }

    pub(crate) fn max_scroll(&self) -> Pixels {
        let Some(b) = self.last_bounds else {
            return px(0.0);
        };
        let total = self.line_height * self.document.buffer().line_count() as f32;
        (total - b.size.height).max(px(0.0))
    }

    pub(crate) fn ensure_caret_visible(&mut self) {
        // Horizontal reveal needs shaped glyph metrics, so it is resolved in
        // `prepaint`; here we just flag it and handle the vertical axis.
        self.reveal_caret = true;
        let Some(b) = self.last_bounds else {
            return;
        };
        let line = self
            .document
            .buffer()
            .char_to_position(self.document.selections().primary().head)
            .line;
        if self.soft_wrap {
            // Anchor by document line: exact sub-row visibility is resolved when
            // `prepaint` lays the wrapped block out, so we only correct the line.
            if line < self.wrap_top_line {
                self.wrap_top_line = line;
                self.wrap_top_off = px(0.0);
            } else if line > self.wrap_bottom_line {
                self.wrap_top_line += line - self.wrap_bottom_line;
                self.wrap_top_off = px(0.0);
            }
            return;
        }
        let top = self.line_height * line as f32;
        let bottom = top + self.line_height;
        if top < self.scroll_y {
            self.scroll_y = top;
        } else if bottom > self.scroll_y + b.size.height {
            self.scroll_y = bottom - b.size.height;
        }
    }

    /// Width available for text (viewport minus gutter and scrollbar lane).
    pub(crate) fn view_width(&self) -> Pixels {
        let Some(b) = self.last_bounds else {
            return px(0.0);
        };
        (b.size.width - self.gutter_width - px(14.0)).max(px(0.0))
    }

    pub(crate) fn toggle_soft_wrap(&mut self, cx: &mut Context<Self>) {
        self.soft_wrap = !self.soft_wrap;
        if self.soft_wrap {
            // Seed the wrap anchor from the current pixel scroll position.
            self.wrap_top_line =
                (f32::from(self.scroll_y) / f32::from(self.line_height)).floor() as usize;
            self.wrap_top_off = px(0.0);
            self.scroll_x = px(0.0);
        } else {
            self.scroll_y = self.line_height * self.wrap_top_line as f32;
        }
        set_view_toggles(self.show_line_numbers, self.soft_wrap, cx);
        cx.notify();
    }

    /// Steps the wrap anchor so `wrap_top_off` lands inside `wrap_top_line`'s
    /// block. Only ever measures lines adjacent to the viewport (O(scroll step)).
    pub(crate) fn normalize_wrap_scroll(&mut self, window: &mut Window) {
        let line_count = self.document.buffer().line_count();
        if line_count == 0 {
            self.wrap_top_line = 0;
            self.wrap_top_off = px(0.0);
            return;
        }
        if self.wrap_top_line >= line_count {
            self.wrap_top_line = line_count - 1;
        }
        let view_w = self.view_width();
        let lh = self.line_height;
        loop {
            if self.wrap_top_off < px(0.0) {
                if self.wrap_top_line == 0 {
                    self.wrap_top_off = px(0.0);
                    break;
                }
                self.wrap_top_line -= 1;
                let rows = self.measure_wrap_rows(self.wrap_top_line, view_w, window);
                self.wrap_top_off += lh * rows as f32;
                continue;
            }
            let rows = self.measure_wrap_rows(self.wrap_top_line, view_w, window);
            let block = lh * rows as f32;
            if self.wrap_top_off >= block {
                if self.wrap_top_line + 1 >= line_count {
                    // Don't scroll the last line entirely off-screen.
                    self.wrap_top_off = (block - lh).max(px(0.0));
                    break;
                }
                self.wrap_top_off -= block;
                self.wrap_top_line += 1;
                continue;
            }
            break;
        }
    }

    /// Visual-row count of `line` at `width` (cheap single-run shaping; colours
    /// don't affect wrap boundaries, so this matches the painted layout).
    pub(crate) fn measure_wrap_rows(&self, line: usize, width: Pixels, window: &mut Window) -> usize {
        if width <= px(0.0) {
            return 1;
        }
        let text = self.document.buffer().line_text(line);
        if text.is_empty() {
            return 1;
        }
        let font = window.text_style().font();
        let run = TextRun {
            len: text.len(),
            font,
            color: rgb(0xffffff).into(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        match window.text_system().shape_text(
            SharedString::from(text),
            self.font_size,
            &[run],
            Some(width),
            None,
        ) {
            Ok(lines) => lines.first().map(wrap_rows).unwrap_or(1),
            Err(_) => 1,
        }
    }
}
