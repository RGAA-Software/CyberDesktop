//! `EngineEditor` — `editing`.

use super::super::imports::*;

impl EngineEditor {
    pub(crate) fn selected_line_range(&self) -> (usize, usize) {
        let primary = self.document.selections().primary();
        let buf = self.document.buffer();
        let first = buf.char_to_position(primary.start()).line;
        let end_pos = buf.char_to_position(primary.end());
        let last = if end_pos.column == 0 && primary.end() > primary.start() {
            end_pos.line.saturating_sub(1)
        } else {
            end_pos.line
        };
        (first, last)
    }

    pub(crate) fn select_line(&mut self, cx: &mut Context<Self>) {
        let buf = self.document.buffer();
        let line = buf.char_to_position(self.document.selections().primary().head).line;
        let start = buf.position_to_char(Position::new(line, 0));
        let line_count = buf.line_count();
        let end = if line + 1 < line_count {
            buf.position_to_char(Position::new(line + 1, 0))
        } else {
            buf.position_to_char(Position::new(line, buf.line_len_chars(line)))
        };
        self.document.set_selection(start, end);
        cx.notify();
    }

    pub(crate) fn indent(&mut self, cx: &mut Context<Self>) {
        let (first, last) = self.selected_line_range();
        for line in (first..=last).rev() {
            let start = self.document.buffer().position_to_char(Position::new(line, 0));
            self.document.replace_range(start..start, "    ");
        }
        self.changed(cx);
    }

    pub(crate) fn outdent(&mut self, cx: &mut Context<Self>) {
        let (first, last) = self.selected_line_range();
        for line in (first..=last).rev() {
            let text = self.document.buffer().line_text(line);
            let spaces = text.chars().take(4).take_while(|c| *c == ' ').count();
            if spaces > 0 {
                let start = self.document.buffer().position_to_char(Position::new(line, 0));
                self.document.replace_range(start..start + spaces, "");
            }
        }
        self.changed(cx);
    }

    pub(crate) fn toggle_comment(&mut self, cx: &mut Context<Self>) {
        let Some(prefix) = comment_prefix(self.document.language()) else {
            return;
        };
        let prefix = prefix.to_string();
        let (first, last) = self.selected_line_range();
        let all_commented = (first..=last).all(|line| {
            let t = self.document.buffer().line_text(line);
            let trimmed = t.trim_start();
            trimmed.is_empty() || trimmed.starts_with(&prefix)
        });
        let prefix_len = prefix.chars().count();
        for line in (first..=last).rev() {
            let text = self.document.buffer().line_text(line);
            if text.trim_start().is_empty() {
                continue;
            }
            let indent = text.chars().take_while(|c| c.is_whitespace()).count();
            let start = self.document.buffer().position_to_char(Position::new(line, 0));
            let at = start + indent;
            if all_commented {
                let rest: Vec<char> = text.chars().skip(indent).collect();
                let mut remove = prefix_len;
                if rest.get(prefix_len) == Some(&' ') {
                    remove += 1;
                }
                self.document.replace_range(at..at + remove, "");
            } else {
                self.document.replace_range(at..at, &format!("{prefix} "));
            }
        }
        self.changed(cx);
    }

    // ---- View ------------------------------------------------------------

    pub(crate) fn toggle_line_numbers(&mut self, cx: &mut Context<Self>) {
        self.show_line_numbers = !self.show_line_numbers;
        set_view_toggles(self.show_line_numbers, self.soft_wrap, cx);
        cx.notify();
    }

    pub(crate) fn zoom(&mut self, delta: f32, cx: &mut Context<Self>) {
        let size = (f32::from(self.font_size) + delta).clamp(8.0, 40.0);
        self.font_size = px(size);
        self.line_height = px((size * 1.45).round());
        cx.notify();
    }

    pub(crate) fn zoom_reset(&mut self, cx: &mut Context<Self>) {
        self.font_size = px(14.0);
        self.line_height = px(20.0);
        cx.notify();
    }

    pub(crate) fn toggle_shortcuts(&mut self, cx: &mut Context<Self>) {
        self.show_shortcuts = !self.show_shortcuts;
        cx.notify();
    }

    pub(crate) fn toggle_about(&mut self, cx: &mut Context<Self>) {
        self.show_about = !self.show_about;
        cx.notify();
    }

    pub(crate) fn toggle_settings(&mut self, cx: &mut Context<Self>) {
        self.show_settings = !self.show_settings;
        cx.notify();
    }

    // ---- Go to Line ------------------------------------------------------

}
