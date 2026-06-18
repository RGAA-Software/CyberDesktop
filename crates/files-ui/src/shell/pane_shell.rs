use std::path::PathBuf;

use files_fs::{home_navigation_path, SearchScope};
use gpui::{prelude::*, *};

use crate::file_browser::{BrowseLocation, FileBrowser};
use crate::home::HomePage;
use crate::shell::navigation::NavigationTarget;

pub struct PaneShell {
    target: NavigationTarget,
    file_browser: Entity<FileBrowser>,
    /// Created lazily — only Home/Settings panes need dashboard data.
    home: Option<Entity<HomePage>>,
}

impl PaneShell {
    pub fn new(cx: &mut Context<Self>, target: NavigationTarget) -> Self {
        files_core::log_startup_step("pane_shell_new_begin");
        let initial_path = match &target {
            NavigationTarget::Path(path) => path.clone(),
            _ => files_fs::home_navigation_path(),
        };
        files_core::log_startup_step("pane_shell_file_browser_begin");
        let home = if matches!(target, NavigationTarget::Home | NavigationTarget::Settings) {
            Some(cx.new(HomePage::new))
        } else {
            None
        };
        let mut this = Self {
            target,
            file_browser: cx.new(|cx| FileBrowser::for_shell(cx, initial_path)),
            home,
        };
        files_core::log_startup_step("pane_shell_bootstrap_target_begin");
        this.bootstrap_target(cx);
        files_core::log_startup_step("pane_shell_new_done");
        this
    }

    fn ensure_home(&mut self, cx: &mut Context<Self>) -> Entity<HomePage> {
        if let Some(home) = self.home.clone() {
            return home;
        }
        let home = cx.new(HomePage::new);
        self.home = Some(home.clone());
        home
    }

    /// Align `FileBrowser` state with `target` after construction (session restore).
    fn bootstrap_target(&mut self, cx: &mut Context<Self>) {
        match &self.target {
            NavigationTarget::RecycleBin => {
                self.file_browser.update(cx, |browser, cx| {
                    browser.open_recycle_bin(cx);
                });
            }
            NavigationTarget::FileTag(name) => {
                self.file_browser.update(cx, |browser, cx| {
                    browser.open_file_tag(name.clone(), cx);
                });
            }
            NavigationTarget::SearchResults { query } => {
                let scope = SearchScope::Home(home_navigation_path());
                self.file_browser.update(cx, |browser, cx| {
                    browser.open_global_search(query.clone(), scope, cx);
                });
            }
            NavigationTarget::Home | NavigationTarget::Settings => {
                // HomePage::new already scheduled the initial snapshot load.
            }
            NavigationTarget::Path(_) => {}
        }
    }

    pub fn target(&self) -> &NavigationTarget {
        &self.target
    }

    pub fn current_navigation_target(&self, cx: &App) -> NavigationTarget {
        match self.target {
            NavigationTarget::Home => self.target.clone(),
            NavigationTarget::Settings => NavigationTarget::Home,
            NavigationTarget::Path(_)
            | NavigationTarget::RecycleBin
            | NavigationTarget::FileTag(_)
            | NavigationTarget::SearchResults { .. } => {
                self.file_browser.read(cx).navigation_target()
            }
        }
    }

    pub fn file_browser(&self) -> Entity<FileBrowser> {
        self.file_browser.clone()
    }

    pub fn home_page(&self) -> Option<Entity<HomePage>> {
        self.home.clone()
    }

    pub fn navigate(&mut self, target: NavigationTarget, cx: &mut Context<Self>) {
        if self.current_navigation_target(cx) == target {
            return;
        }
        let prior_target = self.target.clone();
        let search_scope = if matches!(&target, NavigationTarget::SearchResults { .. }) {
            Some(search_scope_for_browser(
                self.file_browser.read(cx),
                &prior_target,
            ))
        } else {
            None
        };
        self.target = match target {
            NavigationTarget::Settings => NavigationTarget::Home,
            other => other,
        };
        let reload_home = matches!(self.target, NavigationTarget::Home);
        self.file_browser.update(cx, |browser, cx| {
            match &self.target {
                NavigationTarget::Path(path) => {
                    browser.open_directory_reset_history(path.clone(), cx);
                }
                NavigationTarget::RecycleBin => browser.open_recycle_bin(cx),
                NavigationTarget::FileTag(name) => {
                    browser.open_file_tag(name.clone(), cx);
                }
                NavigationTarget::SearchResults { query } => {
                    let scope =
                        search_scope.unwrap_or_else(|| SearchScope::Home(home_navigation_path()));
                    browser.open_global_search(query.clone(), scope, cx);
                }
                NavigationTarget::Home | _ => {}
            }
            cx.notify();
        });
        if reload_home {
            self.ensure_home(cx).update(cx, |home, cx| home.reload(cx));
        }
        cx.notify();
    }

    pub fn reload_home(&mut self, cx: &mut Context<Self>) {
        if matches!(self.target, NavigationTarget::Home) {
            if let Some(home) = self.home.clone() {
                home.update(cx, |home, cx| home.reload(cx));
            }
        }
    }

    pub fn reload_active(&mut self, cx: &mut Context<Self>) {
        match &self.target {
            NavigationTarget::Home => {
                if let Some(home) = self.home.clone() {
                    home.update(cx, |home, cx| home.reload(cx));
                }
            }
            NavigationTarget::Path(_)
            | NavigationTarget::RecycleBin
            | NavigationTarget::FileTag(_)
            | NavigationTarget::SearchResults { .. } => {
                self.file_browser.update(cx, |browser, cx| {
                    browser.reload(cx);
                    cx.notify();
                });
            }
            _ => {}
        }
        cx.notify();
    }
}

impl Render for PaneShell {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match &self.target {
            NavigationTarget::Home | NavigationTarget::Settings => {
                self.ensure_home(cx).into_any_element()
            }
            NavigationTarget::Path(_) => div()
                .id("pane-file-browser")
                .size_full()
                .min_h_0()
                .child(self.file_browser.clone())
                .into_any_element(),
            NavigationTarget::RecycleBin | NavigationTarget::FileTag(_) => div()
                .id("pane-file-browser-special")
                .size_full()
                .min_h_0()
                .child(self.file_browser.clone())
                .into_any_element(),
            NavigationTarget::SearchResults { .. } => div()
                .id("pane-file-browser-search")
                .size_full()
                .min_h_0()
                .child(self.file_browser.clone())
                .into_any_element(),
        }
    }
}

impl PaneShell {
    pub fn open_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.navigate(NavigationTarget::Path(path), cx);
    }
}

fn search_scope_for_browser(browser: &FileBrowser, pane_target: &NavigationTarget) -> SearchScope {
    if let BrowseLocation::SearchResults { scope, .. } = browser.current_browse_location() {
        return scope.clone();
    }
    match pane_target {
        NavigationTarget::Home | NavigationTarget::Settings => {
            SearchScope::Home(home_navigation_path())
        }
        _ => SearchScope::CurrentFolder(browser.current_directory().clone()),
    }
}
