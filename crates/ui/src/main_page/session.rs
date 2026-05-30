use std::path::PathBuf;

use cyber_desktop_core::{
    load_config, save_config, ClosedTabSession, SessionPaneLayout,
};
use gpui::{prelude::*, *};

use super::MainPage;
use crate::shell::app_menus;
use crate::shell::navigation::NavigationTarget;
use crate::shell::ShellPanes;

impl MainPage {
    pub(super) fn encode_session_target(
        target: &NavigationTarget,
        current_path: Option<&PathBuf>,
    ) -> String {
        match target {
            NavigationTarget::Home => "home".into(),
            NavigationTarget::RecycleBin => "recycle".into(),
            NavigationTarget::Settings => "settings".into(),
            NavigationTarget::FileTag(name) => format!("tag:{name}"),
            NavigationTarget::Path(_) => current_path
                .cloned()
                .unwrap_or_else(cyber_desktop_fs::home_navigation_path)
                .to_string_lossy()
                .into_owned(),
        }
    }

    pub(super) fn decode_session_target(value: &str) -> NavigationTarget {
        NavigationTarget::decode_session_tab(value)
    }

    pub(super) fn capture_tab_session(&self, index: usize, cx: &App) -> ClosedTabSession {
        let shell = self.tabs[index].shell.read(cx);
        let pane = shell.active_pane().read(cx);
        let current_path = match pane.target() {
            NavigationTarget::Path(_) => {
                Some(pane.file_browser().read(cx).current_directory().clone())
            }
            _ => None,
        };
        ClosedTabSession {
            tab: Self::encode_session_target(pane.target(), current_path.as_ref()),
            pane_layout: Self::capture_shell_layout(shell, cx),
        }
    }

    pub(super) fn record_closed_tab(&self, session: ClosedTabSession) {
        let mut config = load_config().unwrap_or_default();
        config
            .session_closed_tabs
            .retain(|closed| closed.tab != session.tab);
        config.session_closed_tabs.insert(0, session);
        config.session_closed_tabs.truncate(super::MAX_CLOSED_TABS);
        let _ = save_config(&config);
    }

    pub fn reopen_closed_tab(&mut self, cx: &mut Context<Self>) {
        self.reopen_closed_tab_at(0, cx);
    }

    pub fn reopen_closed_tab_at(&mut self, index: usize, cx: &mut Context<Self>) {
        let mut config = load_config().unwrap_or_default();
        if index >= config.session_closed_tabs.len() {
            return;
        }
        let closed = config.session_closed_tabs.remove(index);
        let _ = save_config(&config);

        let target = Self::decode_session_target(&closed.tab);
        let id = self.next_tab_id;
        self.next_tab_id += 1;
        let layout = closed.pane_layout;
        let shell = cx.new(|cx| {
            let mut shell = ShellPanes::new(cx, target);
            shell.restore_layout(&layout, Self::decode_session_target, cx);
            shell
        });
        self.tabs.push(super::TabEntry { id, shell });
        self.active_tab = self.tabs.len() - 1;
        self.pending_tab_scroll_to_ix = Some(self.active_tab);
        self.persist_session(cx);
        app_menus::reload(cx);
        cx.notify();
    }

    pub(super) fn capture_shell_layout(shell: &ShellPanes, cx: &App) -> SessionPaneLayout {
        let secondary_tab = if shell.dual_pane() {
            let pane = shell.secondary().read(cx);
            let current_path = match pane.target() {
                NavigationTarget::Path(_) => {
                    Some(pane.file_browser().read(cx).current_directory().clone())
                }
                _ => None,
            };
            Self::encode_session_target(pane.target(), current_path.as_ref())
        } else {
            String::new()
        };
        let active_side = match shell.active_side() {
            crate::shell::PaneSide::Secondary => "secondary",
            crate::shell::PaneSide::Primary => "primary",
        };
        SessionPaneLayout {
            dual_pane: shell.dual_pane(),
            active_side: active_side.into(),
            secondary_tab,
        }
    }

    pub fn persist_session(&mut self, cx: &mut Context<Self>) {
        let tabs: Vec<String> = self
            .tabs
            .iter()
            .enumerate()
            .map(|(index, _)| {
                let pane = self.tabs[index].shell.read(cx).active_pane().read(cx);
                let current_path = match pane.target() {
                    NavigationTarget::Path(_) => {
                        Some(pane.file_browser().read(cx).current_directory().clone())
                    }
                    _ => None,
                };
                Self::encode_session_target(pane.target(), current_path.as_ref())
            })
            .collect();
        let layouts: Vec<SessionPaneLayout> = self
            .tabs
            .iter()
            .map(|entry| Self::capture_shell_layout(&entry.shell.read(cx), cx))
            .collect();
        let mut config = load_config().unwrap_or_default();
        config.session_tabs = tabs;
        config.session_active_tab = self.active_tab;
        config.session_pane_layouts = layouts;
        let _ = save_config(&config);
    }
}
