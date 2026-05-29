//! `EngineEditor` — `goto`.

use super::super::imports::*;

impl EngineEditor {
    pub(crate) fn open_goto(&mut self, cx: &mut Context<Self>) {
        self.find = None;
        self.search_panel = None;
        self.goto = Some(String::new());
        self.input_target = InputTarget::GotoLine;
        cx.notify();
    }

    pub(crate) fn close_goto(&mut self, cx: &mut Context<Self>) {
        self.goto = None;
        self.input_target = InputTarget::Document;
        cx.notify();
    }

    pub(crate) fn goto_backspace(&mut self, cx: &mut Context<Self>) {
        if let Some(g) = self.goto.as_mut() {
            g.pop();
        }
        cx.notify();
    }

    pub(crate) fn do_goto(&mut self, cx: &mut Context<Self>) {
        if let Some(text) = self.goto.clone() {
            if let Ok(n) = text.trim().parse::<usize>() {
                let last = self.document.buffer().line_count().saturating_sub(1);
                let line = n.saturating_sub(1).min(last);
                let target = self
                    .document
                    .buffer()
                    .position_to_char(Position::new(line, 0));
                self.document.set_caret(target);
                self.ensure_caret_visible();
            }
        }
        self.close_goto(cx);
    }

    // ---- Find in Files (global search) -----------------------------------

}
