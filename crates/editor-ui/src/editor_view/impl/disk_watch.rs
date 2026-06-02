//! `EngineEditor` — `disk_watch`.

use super::super::imports::*;

impl EngineEditor {
    pub(crate) fn start_disk_watch(&mut self, cx: &mut Context<Self>) {
        if self.watch_started {
            return;
        }
        self.watch_started = true;
        cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(Duration::from_millis(1500)).await;
            let keep = this
                .update(cx, |this, cx| this.check_disk_change(cx))
                .unwrap_or(false);
            if !keep {
                break;
            }
        })
        .detach();
    }

    /// Returns `false` if the entity is gone (stop polling).
    pub(crate) fn check_disk_change(&mut self, cx: &mut Context<Self>) -> bool {
        if self.disk_changed {
            return true;
        }
        let Some(path) = self.document.path().map(Path::to_path_buf) else {
            return true;
        };
        if let (Some(now), Some(prev)) = (read_file_meta(&path), self.file_meta) {
            if now != prev {
                self.disk_changed = true;
                cx.notify();
            }
        }
        true
    }

    pub(crate) fn reload_from_disk(&mut self, cx: &mut Context<Self>) {
        self.disk_changed = false;
        if let Some(path) = self.document.path().map(Path::to_path_buf) {
            let caret = self.document.selections().primary().head;
            let scroll_y = self.scroll_y;
            let target = self.active;
            self.spawn_load(path, target, Some((caret, scroll_y)), cx);
        }
        cx.notify();
    }
}
