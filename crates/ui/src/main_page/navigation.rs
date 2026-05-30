use std::path::PathBuf;

use cyber_desktop_core::record_path_history;
use gpui::{prelude::*, *};

use super::MainPage;
use crate::file_ops::{spawn_file_transfer, FileTransferKind};
use crate::shell::navigation::NavigationTarget;
use crate::shell::{PaneShell, ShellPanes};

impl MainPage {
    pub fn open_path_in_new_tab(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        record_path_history(&path);
        self.add_tab(NavigationTarget::Path(path), cx);
    }

    pub fn open_path_in_secondary_pane(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        record_path_history(&path);
        let shell = self.active_shell();
        shell.update(cx, |shell, cx| {
            if !shell.dual_pane() {
                shell.toggle_dual_pane(cx);
            }
            shell.secondary().update(cx, |pane, cx| {
                pane.open_path(path, cx);
            });
            shell.set_active(crate::shell::PaneSide::Secondary, cx);
        });
        cx.notify();
    }

    pub fn drop_paths_on_directory(
        &mut self,
        dest: PathBuf,
        paths: Vec<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cancel_breadcrumb_drag_preview();
        if paths.is_empty() || !dest.is_dir() {
            return;
        }
        if paths.iter().all(|p| p.parent() == Some(dest.as_path())) {
            return;
        }
        let copy = window.modifiers().control;
        let kind = if copy {
            FileTransferKind::Copy
        } else {
            FileTransferKind::Move
        };
        let pane = self.active_pane(cx);
        let browser = pane.read(cx).file_browser().clone();
        browser.update(cx, |_, cx| {
            spawn_file_transfer(browser.clone(), window, cx, kind, paths, dest);
        });
        cx.notify();
    }

    pub(super) fn active_shell(&self) -> Entity<ShellPanes> {
        self.tabs[self.active_tab].shell.clone()
    }

    pub(super) fn active_pane(&self, cx: &App) -> Entity<PaneShell> {
        self.active_shell().read(cx).active_pane()
    }

    pub(super) fn active_file_browser(&self, cx: &App) -> Entity<crate::file_browser::FileBrowser> {
        self.active_pane(cx).read(cx).file_browser()
    }

    pub(super) fn file_navigation_active(&self, cx: &App) -> bool {
        matches!(
            self.active_pane(cx).read(cx).target(),
            NavigationTarget::Path(_) | NavigationTarget::RecycleBin | NavigationTarget::FileTag(_)
        )
    }

    pub fn navigate_to(&mut self, target: NavigationTarget, cx: &mut Context<Self>) {
        if self.active_navigation_target(cx) == target {
            return;
        }
        if let NavigationTarget::Path(ref path) = target {
            record_path_history(path);
        }
        let shell = self.active_shell();
        shell.update(cx, |shell, cx| {
            shell.navigate_active(target, cx);
        });
        self.omnibar_show_full_path = false;
        self.persist_session(cx);
        cx.notify();
    }

    pub(super) fn toggle_dual_pane(&mut self, cx: &mut Context<Self>) {
        let shell = self.active_shell();
        shell.update(cx, |shell, cx| shell.toggle_dual_pane(cx));
        self.persist_session(cx);
        cx.notify();
    }

    pub fn active_navigation_target(&self, cx: &App) -> NavigationTarget {
        self.active_pane(cx).read(cx).current_navigation_target(cx)
    }
}
