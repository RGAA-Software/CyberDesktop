//! Open the standalone settings window from the main page.

use gpui::Context;

use crate::settings_window::FilesSettingsWindowState;

use super::MainPage;

impl MainPage {
    pub(super) fn open_settings(&mut self, cx: &mut Context<Self>) {
        FilesSettingsWindowState::open_files(cx);
    }

    pub(super) fn flush_pending_settings_toggle(&mut self, cx: &mut Context<Self>) {
        if self.pending_settings_toggle {
            self.pending_settings_toggle = false;
            self.open_settings(cx);
        }
    }
}
