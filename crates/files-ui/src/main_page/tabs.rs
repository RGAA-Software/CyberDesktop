use files_core::load_config;
use gpui::{prelude::*, *};

use super::MainPage;
use crate::shell::app_menus;
use crate::shell::navigation::NavigationTarget;
use crate::shell::preferences::persist_window_bounds;
use crate::shell::ShellPanes;

impl MainPage {
    pub(super) fn add_tab(&mut self, target: NavigationTarget, cx: &mut Context<Self>) {
        let id = self.next_tab_id;
        self.next_tab_id += 1;
        let open_dual = load_config()
            .map(|c| c.always_open_dual_pane_in_new_tab)
            .unwrap_or(false);
        let shell = cx.new(|cx| {
            let mut shell = ShellPanes::new(cx, target);
            if open_dual {
                shell.open_secondary_at_active(cx);
            }
            shell
        });
        self.tabs.push(super::TabEntry { id, shell });
        self.active_tab = self.tabs.len() - 1;
        self.pending_tab_scroll_to_ix = Some(self.active_tab);
        self.refresh_active_view(cx);
        self.persist_session(cx);
        cx.notify();
    }

    pub(super) fn close_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if self.tabs.len() <= 1 {
            persist_window_bounds(cx);
            files_core::flush_config();
            cx.quit();
            return;
        }
        let closed = self.capture_tab_session(index, cx);
        self.record_closed_tab(closed);
        app_menus::reload(cx);
        self.tabs.remove(index);
        if self.active_tab >= self.tabs.len() {
            self.active_tab = self.tabs.len() - 1;
        } else if index < self.active_tab {
            self.active_tab -= 1;
        }
        self.pending_tab_scroll_to_ix = Some(self.active_tab);
        self.persist_session(cx);
        cx.notify();
    }

    pub(super) fn tab_title(&self, index: usize, cx: &App) -> SharedString {
        let pane = self.tabs[index].shell.read(cx).active_pane().read(cx);
        match pane.target() {
            NavigationTarget::Path(_) => {
                let path = pane.file_browser().read(cx).current_directory();
                SharedString::from(
                    path.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| path.to_string_lossy().to_string()),
                )
            }
            target => target.tab_title(),
        }
    }
}
