//! `EngineEditor` — `recent`.

use super::super::imports::*;

impl EngineEditor {
    pub(crate) fn push_recent(&mut self, path: PathBuf) {
        self.recent.retain(|p| p != &path);
        self.recent.insert(0, path);
        self.recent.truncate(12);
    }

    pub(crate) fn toggle_recent(&mut self, cx: &mut Context<Self>) {
        self.show_recent = !self.show_recent;
        cx.notify();
    }
}
