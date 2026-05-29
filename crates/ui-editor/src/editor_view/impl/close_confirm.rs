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
        match self.pending_close.take() {
            Some(CloseTarget::Tab(i)) => {
                if i != self.active {
                    self.switch_to_tab(i, cx);
                }
                if self.save_active_sync() {
                    self.force_close_tab(self.active, cx);
                } else {
                    cx.notify();
                }
            }
            Some(CloseTarget::Window) => {
                if self.save_all_sync(cx) {
                    self.allow_window_close = true;
                    window.remove_window();
                } else {
                    cx.notify();
                }
            }
            None => {}
        }
    }

    /// Saves the active document synchronously (prompting for a path if untitled).
    /// Returns false if the user cancelled the save dialog.
    pub(crate) fn save_active_sync(&mut self) -> bool {
        let path = match self.document.path().map(Path::to_path_buf) {
            Some(p) => p,
            None => match crate::pick_save_file_path(&PathBuf::from("untitled.txt")) {
                Some(p) => p,
                None => return false,
            },
        };
        if self.document.save_to(path.clone()).is_ok() {
            self.file_meta = read_file_meta(&path);
            self.disk_changed = false;
            true
        } else {
            true
        }
    }

    /// Saves every dirty tab synchronously. Returns false if the user cancelled.
    pub(crate) fn save_all_sync(&mut self, cx: &mut Context<Self>) -> bool {
        for i in self.dirty_tabs() {
            if self.active != i {
                self.switch_to_tab(i, cx);
            }
            if !self.save_active_sync() {
                return false;
            }
        }
        true
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
