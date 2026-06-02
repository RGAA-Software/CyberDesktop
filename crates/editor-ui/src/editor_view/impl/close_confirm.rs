//! `EngineEditor` — `close_confirm`.

use super::super::imports::*;

impl EngineEditor {
    /// the confirmation overlay and block the close.
    pub(crate) fn request_window_close(&mut self, cx: &mut Context<Self>) -> bool {
        if self.allow_window_close || self.dirty_tabs().is_empty() {
            return true;
        }
        self.pending_close = Some(CloseTarget::Window);
        cx.notify();
        false
    }

    pub(crate) fn close_confirm_cancel(&mut self, cx: &mut Context<Self>) {
        self.pending_close = None;
        cx.notify();
    }

    pub(crate) fn close_confirm_discard(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.pending_close.take() {
            Some(CloseTarget::Tab(i)) => self.force_close_tab(i, cx),
            Some(CloseTarget::Window) => {
                self.allow_window_close = true;
                window.remove_window();
            }
            None => {}
        }
    }

    pub(crate) fn close_confirm_save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.pending_close {
            Some(CloseTarget::Tab(i)) => {
                if i != self.active {
                    self.switch_to_tab(i, cx);
                }
                self.save_active_for_close(
                    move |this, ok, cx| {
                        if ok {
                            this.pending_close = None;
                            this.force_close_tab(i, cx);
                        } else {
                            this.pending_close = Some(CloseTarget::Tab(i));
                            cx.notify();
                        }
                    },
                    cx,
                );
            }
            Some(CloseTarget::Window) => self.save_all_dirty_for_close(window, cx),
            None => {}
        }
    }

    /// True when the current tab is a pristine, empty, untitled buffer (so we can
    /// open a file into it instead of spawning a new tab).
    pub(crate) fn active_is_pristine(&self) -> bool {
        self.document.path().is_none()
            && !self.document.dirty()
            && self.document.buffer().len_chars() == 0
    }

    // ---- Recent files ----------------------------------------------------

}
