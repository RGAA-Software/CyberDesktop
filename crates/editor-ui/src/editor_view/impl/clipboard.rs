//! `EngineEditor` — `clipboard`.

use super::super::imports::*;

impl EngineEditor {
    pub(crate) fn copy(&mut self, cx: &mut Context<Self>) {
        let primary = self.document.selections().primary();
        if primary.is_empty() {
            return;
        }
        let text = self.document.buffer().slice_text(primary.range());
        cx.write_to_clipboard(ClipboardItem::new_string(text));
    }

    pub(crate) fn cut(&mut self, cx: &mut Context<Self>) {
        let range = self.document.selections().primary().range();
        if range.is_empty() {
            return;
        }
        let text = self.document.buffer().slice_text(range.clone());
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.document.replace_range(range, "");
        self.changed(cx);
    }

    pub(crate) fn paste(&mut self, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.document.insert(&text);
            self.changed(cx);
        }
    }

    // ---- Selection / editing helpers -------------------------------------

}
