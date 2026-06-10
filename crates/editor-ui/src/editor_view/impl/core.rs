//! `EngineEditor` — `core`.

use editor_text_engine::parse_rope;

use super::super::canvas::{cols_per_row, estimated_wrap_rows, measure_avg_char_width};
use super::super::state::LONG_LINE_COL_THRESHOLD;
use super::super::imports::*;

impl EngineEditor {
    pub(crate) fn refresh_syntax(&mut self, cx: &mut Context<Self>) {
        let rev = self.document.revision();
        if self.parsed_revision == Some(rev) {
            return;
        }
        if self.parsed_revision.is_some() {
            let edits = self.document.take_syntax_edits();
            self.line_width_cache.invalidate_from_edits(&edits);
            for edit in &edits {
                self.syntax.edit(edit);
            }
        } else {
            self.document.take_syntax_edits();
        }

        if self.should_defer_syntax() {
            if self.syntax_parse_inflight && self.syntax_parse_target_rev == Some(rev) {
                return;
            }
            self.schedule_syntax_reparse(cx);
            return;
        }

        self.syntax.reparse(self.document.buffer().rope());
        self.parsed_revision = Some(rev);
    }

    fn should_defer_syntax(&self) -> bool {
        if !self.syntax.is_supported() {
            return false;
        }
        let buf = self.document.buffer();
        if buf.line_count() == 1 {
            return buf.line_len_chars(0) > LONG_LINE_COL_THRESHOLD;
        }
        let caret_line = buf
            .char_to_position(self.document.selections().primary().head)
            .line;
        if buf.line_len_chars(caret_line) > LONG_LINE_COL_THRESHOLD {
            return true;
        }
        for &line in &self.display_lines {
            if buf.line_len_chars(line) > LONG_LINE_COL_THRESHOLD {
                return true;
            }
        }
        false
    }

    fn schedule_syntax_reparse(&mut self, cx: &mut Context<Self>) {
        let target_rev = self.document.revision();
        self.syntax_parse_inflight = true;
        self.syntax_parse_target_rev = Some(target_rev);
        let gen = self.syntax_parse_gen.wrapping_add(1);
        self.syntax_parse_gen = gen;
        let rope = self.document.buffer().rope().clone();
        let language = self.syntax.language_id().to_string();
        let old_tree = self.syntax.clone_tree();

        let task = cx
            .background_executor()
            .spawn(async move { parse_rope(&language, rope, old_tree) });

        cx.spawn(async move |this, cx| {
            let tree = task.await;
            let _ = this.update(cx, |this, cx| {
                this.syntax_parse_inflight = false;
                this.syntax_parse_target_rev = None;
                if this.syntax_parse_gen != gen {
                    return;
                }
                if this.document.revision() != target_rev {
                    return;
                }
                this.syntax.replace_tree(tree);
                this.parsed_revision = Some(target_rev);
                cx.notify();
            });
        })
        .detach();
    }

    pub(crate) fn sync_markdown_preview(&mut self, cx: &mut Context<Self>) {
        if let Some(preview) = self.markdown_preview.as_ref() {
            let text = self.document.buffer().to_string();
            preview.update(cx, |preview, cx| {
                preview.set_text(&text, cx);
            });
        }
    }

    pub(crate) fn changed(&mut self, cx: &mut Context<Self>) {
        self.rebuild_display_lines();
        self.caret_blink_visible = true;
        self.ensure_caret_visible();
        self.sync_markdown_preview(cx);
        cx.notify();
    }

    /// Longest relevant line length for status hints (caret line or sole line).
    pub(crate) fn status_line_len_chars(&self) -> usize {
        let buf = self.document.buffer();
        if buf.line_count() == 1 {
            return buf.line_len_chars(0);
        }
        let line = buf
            .char_to_position(self.document.selections().primary().head)
            .line;
        buf.line_len_chars(line)
    }

    pub(crate) fn has_long_line(&self) -> bool {
        self.status_line_len_chars() > LONG_LINE_COL_THRESHOLD
    }

    /// Starts the caret blink timer (530 ms half-period, standard editor rate).
    pub(crate) fn start_caret_blink(&mut self, cx: &mut Context<Self>) {
        if self.blink_started {
            return;
        }
        self.blink_started = true;
        cx.spawn(async move |this, cx| loop {
            cx.background_executor()
                .timer(Duration::from_millis(530))
                .await;
            let keep = this
                .update(cx, |this, cx| {
                    this.caret_blink_visible = !this.caret_blink_visible;
                    cx.notify();
                    true
                })
                .unwrap_or(false);
            if !keep {
                break;
            }
        })
        .detach();
    }

    pub(crate) fn max_scroll(&self) -> Pixels {
        let Some(b) = self.last_bounds else {
            return px(0.0);
        };
        let lines = if self.soft_wrap {
            self.document.buffer().line_count()
        } else {
            self.display_line_count()
        };
        let total = self.line_height * lines as f32;
        (total - b.size.height).max(px(0.0))
    }

    pub(crate) fn ensure_caret_visible(&mut self) {
        // Horizontal reveal needs shaped glyph metrics, so it is resolved in
        // `prepaint`; here we just flag it and handle the vertical axis.
        self.reveal_caret = true;
        let Some(b) = self.last_bounds else {
            return;
        };
        let viewport_h = b.size.height - self.editor_bottom_inset();
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
        } else if bottom > self.scroll_y + viewport_h {
            self.scroll_y = bottom - viewport_h;
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
        set_view_toggles(
            self.show_line_numbers,
            self.soft_wrap,
            self.show_preview,
            self.show_full_preview,
            cx,
        );
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
        let buf = self.document.buffer();
        let len = buf.line_len_chars(line);
        if len == 0 {
            return 1;
        }
        if len > LONG_LINE_COL_THRESHOLD {
            let font = window.text_style().font();
            let char_width =
                measure_avg_char_width(window, &font, self.font_size);
            return estimated_wrap_rows(len, cols_per_row(char_width, width));
        }
        let text = buf.line_text(line);
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
