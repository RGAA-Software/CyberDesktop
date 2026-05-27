use cyberfiles_core::{load_config, save_config};
use gpui::{prelude::*, *};

use super::MainPage;
use crate::shell::navigation::NavigationTarget;

impl MainPage {
    pub(super) fn toggle_info_pane(&mut self, cx: &mut Context<Self>) {
        self.show_info_pane = !self.show_info_pane;
        let mut config = load_config().unwrap_or_default();
        config.show_info_pane = self.show_info_pane;
        let _ = save_config(&config);
        for tab in &self.tabs {
            let shell = tab.shell.clone();
            let panes = {
                let shell_ref = shell.read(cx);
                let mut panes = Vec::new();
                shell_ref.for_each_pane(|pane| {
                    panes.push(pane.clone());
                });
                panes
            };
            for pane in panes {
                let file_browser = pane.read(cx).file_browser();
                file_browser.update(cx, |browser, cx| {
                    browser.set_show_info_pane(self.show_info_pane, cx);
                });
            }
        }
        cx.notify();
    }

    pub(super) fn info_selection(&self, cx: &App) -> Option<cyberfiles_fs::FileItem> {
        let pane = self.active_pane(cx);
        if !matches!(
            pane.read(cx).target(),
            NavigationTarget::Path(_) | NavigationTarget::RecycleBin
        ) {
            return None;
        }
        pane.read(cx)
            .file_browser()
            .read(cx)
            .primary_selected_item()
            .cloned()
    }
}
