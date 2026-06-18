use std::path::PathBuf;

use files_core::record_path_history;
use files_fs::all_direct_children_of;
use gpui::{prelude::*, *};

use super::MainPage;
use crate::file_ops::{file_transfer_kind_for_drop, spawn_file_transfer};
use crate::shell::app_menus;
use crate::shell::navigation::NavigationTarget;
use crate::shell::{PaneShell, ShellPanes};

fn refresh_dual_pane_menus(cx: &mut App) {
    app_menus::reload(cx);
}

impl MainPage {
    /// Called when the set of available drives changes (e.g. USB inserted/removed).
    pub fn on_drives_changed(&mut self, cx: &mut Context<Self>) {
        use std::collections::HashSet;

        // Refresh sidebar so drive list stays current.
        self.refresh_sidebar_cache(cx);

        // Refresh every Home dashboard so drive cards update.
        for tab in &self.tabs {
            let shell = tab.shell.clone();
            shell.update(cx, |shell, cx| {
                shell.for_each_pane(|pane| {
                    pane.update(cx, |pane, cx| {
                        pane.reload_home(cx);
                    });
                });
            });
        }

        // If any pane is browsing a path whose drive root is gone, navigate it to Home.
        let available_roots: HashSet<String> = files_fs::list_drives()
            .into_iter()
            .filter_map(|d| files_fs::path_drive_root(&d.path))
            .collect();

        for tab in &self.tabs {
            let shell = tab.shell.clone();
            shell.update(cx, |shell, cx| {
                shell.for_each_pane(|pane| {
                    pane.update(cx, |pane, cx| {
                        if let NavigationTarget::Path(ref path) = pane.current_navigation_target(cx)
                        {
                            if let Some(root) = files_fs::path_drive_root(path) {
                                // Only react to local drive removals (e.g. USB eject).
                                // Network paths are not in list_drives() and should not trigger auto-navigation.
                                if root.len() == 2 && root.as_bytes().get(1) == Some(&b':') {
                                    if !available_roots.contains(&root) {
                                        pane.navigate(NavigationTarget::Home, cx);
                                    }
                                }
                            }
                        }
                    });
                });
            });
        }

        cx.notify();
    }
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
        refresh_dual_pane_menus(cx);
        cx.notify();
    }

    pub fn drop_paths_on_directory(
        &mut self,
        dest: PathBuf,
        paths: Vec<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.drop_paths_on_directory_impl(dest, paths, window, cx);
    }

    pub fn drop_external_paths_on_directory(
        &mut self,
        dest: PathBuf,
        paths: Vec<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.drop_paths_on_directory_impl(dest, paths, window, cx);
    }

    fn drop_paths_on_directory_impl(
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
        if all_direct_children_of(&paths, &dest) {
            return;
        }
        let kind = file_transfer_kind_for_drop(window.modifiers(), &paths, &dest);
        let pane = self.active_pane(cx);
        let browser = pane.read(cx).file_browser().clone();
        browser.update(cx, |_, cx| {
            spawn_file_transfer(browser.clone(), window, cx, kind, paths, dest);
        });
        cx.notify();
    }

    pub(crate) fn active_shell(&self) -> Entity<ShellPanes> {
        self.tabs[self.active_tab].shell.clone()
    }

    pub(super) fn active_pane(&self, cx: &App) -> Entity<PaneShell> {
        self.active_shell().read(cx).active_pane()
    }

    pub(super) fn active_file_browser(&self, cx: &App) -> Entity<crate::file_browser::FileBrowser> {
        self.active_pane(cx).read(cx).file_browser()
    }

    pub(crate) fn maybe_begin_native_drag_out(
        &self,
        position: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.active_file_browser(cx).update(cx, |browser, cx| {
            browser.maybe_begin_native_drag_out(position, window, cx);
        });
    }

    pub(super) fn file_navigation_active(&self, cx: &App) -> bool {
        matches!(
            self.active_pane(cx).read(cx).target(),
            NavigationTarget::Path(_)
                | NavigationTarget::RecycleBin
                | NavigationTarget::FileTag(_)
                | NavigationTarget::SearchResults { .. }
        )
    }

    pub fn navigate_to(&mut self, target: NavigationTarget, cx: &mut Context<Self>) {
        if target == NavigationTarget::Settings {
            self.pending_settings_toggle = true;
            cx.notify();
            return;
        }
        if self.active_navigation_target(cx) == target {
            self.refresh_active_view(cx);
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
        self.dismiss_omnibar_search_mode(cx);
        self.clear_omnibar_suggestions();
        self.persist_session(cx);
        cx.notify();
    }

    pub(crate) fn toggle_dual_pane(&mut self, cx: &mut Context<Self>) {
        let shell = self.active_shell();
        shell.update(cx, |shell, cx| shell.toggle_dual_pane(cx));
        self.persist_session(cx);
        refresh_dual_pane_menus(cx);
        cx.notify();
    }

    pub(super) fn focus_other_pane(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let shell = self.active_shell();
        shell.update(cx, |shell, cx| shell.focus_other_pane(window, cx));
        self.persist_session(cx);
        cx.notify();
    }

    pub(crate) fn close_active_pane(&mut self, cx: &mut Context<Self>) {
        let shell = self.active_shell();
        shell.update(cx, |shell, cx| shell.close_active_pane(cx));
        self.persist_session(cx);
        refresh_dual_pane_menus(cx);
        cx.notify();
    }

    pub(crate) fn dual_pane_active(&self, cx: &App) -> bool {
        self.active_shell().read(cx).dual_pane()
    }

    pub(super) fn adapt_active_shell_viewport(&mut self, width: Pixels, cx: &mut Context<Self>) {
        let shell = self.active_shell();
        shell.update(cx, |shell, cx| {
            shell.adapt_viewport_width(width, |pane, app| Self::encode_pane_session(pane, app), cx);
        });
    }

    pub(crate) fn split_pane_vertically(&mut self, cx: &mut Context<Self>) {
        if self.dual_pane_active(cx) {
            self.arrange_panes_vertically(cx);
            return;
        }
        let shell = self.active_shell();
        shell.update(cx, |shell, cx| {
            shell.split_pane(crate::shell::PaneArrangement::Vertical, cx);
        });
        self.persist_session(cx);
        refresh_dual_pane_menus(cx);
        cx.notify();
    }

    pub(crate) fn split_pane_horizontally(&mut self, cx: &mut Context<Self>) {
        if self.dual_pane_active(cx) {
            self.arrange_panes_horizontally(cx);
            return;
        }
        let shell = self.active_shell();
        shell.update(cx, |shell, cx| {
            shell.split_pane(crate::shell::PaneArrangement::Horizontal, cx);
        });
        self.persist_session(cx);
        refresh_dual_pane_menus(cx);
        cx.notify();
    }

    pub(super) fn arrange_panes_vertically(&mut self, cx: &mut Context<Self>) {
        let shell = self.active_shell();
        shell.update(cx, |shell, cx| {
            shell.arrange_panes(crate::shell::PaneArrangement::Vertical, cx);
        });
        self.persist_session(cx);
        refresh_dual_pane_menus(cx);
        cx.notify();
    }

    pub(super) fn arrange_panes_horizontally(&mut self, cx: &mut Context<Self>) {
        let shell = self.active_shell();
        shell.update(cx, |shell, cx| {
            shell.arrange_panes(crate::shell::PaneArrangement::Horizontal, cx);
        });
        self.persist_session(cx);
        refresh_dual_pane_menus(cx);
        cx.notify();
    }

    pub fn active_navigation_target(&self, cx: &App) -> NavigationTarget {
        self.active_pane(cx).read(cx).current_navigation_target(cx)
    }

    pub fn navigate_to_directory_and_select(
        &mut self,
        dir: PathBuf,
        select: PathBuf,
        cx: &mut Context<Self>,
    ) {
        self.navigate_to(NavigationTarget::Path(dir), cx);
        let browser = self.active_pane(cx).read(cx).file_browser().clone();
        cx.defer(move |cx| {
            browser.update(cx, |browser, cx| {
                browser.select_path_after_load(select, cx);
            });
        });
    }

    /// Reload the active pane (Home dashboard or file listing).
    pub(super) fn refresh_active_view(&mut self, cx: &mut Context<Self>) {
        let pane = self.active_pane(cx);
        pane.update(cx, |pane, cx| pane.reload_active(cx));
        cx.notify();
    }

    /// Repaint home dashboards after theme changes.
    pub(crate) fn notify_all_homes(&self, cx: &mut Context<Self>) {
        for tab in &self.tabs {
            let mut panes = Vec::new();
            tab.shell
                .read(cx)
                .for_each_pane(|pane| panes.push(pane.clone()));
            for pane in panes {
                if let Some(home) = pane.read(cx).home_page() {
                    let _ = home.update(cx, |_, cx| cx.notify());
                }
            }
        }
    }

    /// Refresh every file list (e.g. after in-app clipboard cut/copy/clear changes cut styling).
    pub(crate) fn notify_all_file_browsers(&self, cx: &mut Context<Self>) {
        for tab in &self.tabs {
            let mut panes = Vec::new();
            tab.shell
                .read(cx)
                .for_each_pane(|pane| panes.push(pane.clone()));
            for pane in panes {
                pane.read(cx).file_browser().update(cx, |_, cx| cx.notify());
            }
        }
    }
}
