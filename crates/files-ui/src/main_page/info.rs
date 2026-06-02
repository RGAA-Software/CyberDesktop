use files_core::{load_config, save_config};
use files_fs::DirectoryReadOptions;
use gpui::{prelude::*, *};

use super::MainPage;
use crate::info_pane::InfoPaneSelection;
use crate::shell::navigation::NavigationTarget;

impl MainPage {
    pub(super) fn toggle_info_pane(&mut self, cx: &mut Context<Self>) {
        self.show_info_pane = !self.show_info_pane;
        self.info_pane.update(cx, |pane, cx| {
            pane.set_visible(self.show_info_pane, cx);
        });
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

    pub(super) fn info_pane_update(
        &self,
        cx: &App,
    ) -> (InfoPaneSelection, DirectoryReadOptions) {
        let pane = self.active_pane(cx);
        if !matches!(
            pane.read(cx).target(),
            NavigationTarget::Path(_) | NavigationTarget::RecycleBin
        ) {
            return (InfoPaneSelection::None, DirectoryReadOptions::default());
        }
        let browser = pane.read(cx).file_browser().read(cx);
        let items = browser.selected_file_items();
        let read_options = *browser.read_options();
        let selection = match items.len() {
            0 => browser
                .primary_selected_item()
                .map(|item| InfoPaneSelection::Single(item.clone()))
                .unwrap_or(InfoPaneSelection::None),
            1 => InfoPaneSelection::Single(items.into_iter().next().expect("one item")),
            _ => InfoPaneSelection::Multiple(items),
        };
        (selection, read_options)
    }
}
