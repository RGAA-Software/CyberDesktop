//! `EngineEditor` — `goto`.

use super::super::imports::*;

impl EngineEditor {
    pub(crate) fn open_goto(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.find = None;
        self.search_panel = None;
        let input =
            cx.new(|cx| InputState::new(window, cx).placeholder(t!("editor.goto.placeholder")));
        let mut subs = Vec::new();
        subs.push(cx.subscribe(&input, |this, _, ev: &InputEvent, cx| {
            if let InputEvent::PressEnter { .. } = ev {
                this.do_goto(cx);
            }
        }));
        input.update(cx, |state, cx| state.focus(window, cx));
        self.goto = Some(GotoState { input, _subs: subs });
        cx.notify();
    }

    pub(crate) fn close_goto(&mut self, cx: &mut Context<Self>) {
        self.goto = None;
        self.end_panel_drag();
        self.clear_panel_origin(FloatingPanel::Goto);
        cx.notify();
    }

    pub(crate) fn do_goto(&mut self, cx: &mut Context<Self>) {
        let Some(goto) = self.goto.as_ref() else {
            return;
        };
        let text = goto.input.read(cx).value().to_string();
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
        self.close_goto(cx);
    }

    // ---- Find in Files (global search) -----------------------------------
}
