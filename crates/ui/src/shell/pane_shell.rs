use std::path::PathBuf;

use gpui::{prelude::*, *};

use crate::file_browser::FileBrowser;
use crate::home::HomePage;
use crate::settings_view::build_settings;
use crate::shell::navigation::NavigationTarget;

pub struct PaneShell {
    target: NavigationTarget,
    file_browser: Entity<FileBrowser>,
    home: Entity<HomePage>,
}

impl PaneShell {
    pub fn new(cx: &mut Context<Self>, target: NavigationTarget) -> Self {
        let initial_path = match &target {
            NavigationTarget::Path(path) => path.clone(),
            _ => cyberfiles_fs::home_navigation_path(),
        };
        Self {
            target,
            file_browser: cx.new(|cx| FileBrowser::for_shell(cx, initial_path)),
            home: cx.new(HomePage::new),
        }
    }

    pub fn target(&self) -> &NavigationTarget {
        &self.target
    }

    pub fn current_navigation_target(&self, cx: &App) -> NavigationTarget {
        match self.target {
            NavigationTarget::Home | NavigationTarget::Settings => self.target.clone(),
            NavigationTarget::Path(_)
            | NavigationTarget::RecycleBin
            | NavigationTarget::FileTag(_) => self.file_browser.read(cx).navigation_target(),
        }
    }

    pub fn file_browser(&self) -> Entity<FileBrowser> {
        self.file_browser.clone()
    }

    pub fn navigate(&mut self, target: NavigationTarget, cx: &mut Context<Self>) {
        if self.current_navigation_target(cx) == target {
            return;
        }
        self.target = target.clone();
        self.file_browser.update(cx, |browser, cx| {
            match &target {
                NavigationTarget::Path(path) => {
                    browser.open_directory_reset_history(path.clone(), cx);
                }
                NavigationTarget::RecycleBin => browser.open_recycle_bin(cx),
                NavigationTarget::FileTag(name) => {
                    browser.open_file_tag(name.clone(), cx);
                }
                NavigationTarget::Home => {
                    self.home.update(cx, |home, cx| home.reload(cx));
                }
                _ => {}
            }
            cx.notify();
        });
        cx.notify();
    }

    pub fn reload_home(&mut self, cx: &mut Context<Self>) {
        if matches!(self.target, NavigationTarget::Home) {
            self.home.update(cx, |home, cx| home.reload(cx));
        }
    }
}

impl Render for PaneShell {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        match &self.target {
            NavigationTarget::Home => self.home.clone().into_any_element(),
            NavigationTarget::Settings => div()
                .id("settings-page")
                .size_full()
                .min_h_0()
                .overflow_hidden()
                .child(build_settings(cx))
                .into_any_element(),
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
        }
    }
}

impl PaneShell {
    pub fn open_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.navigate(NavigationTarget::Path(path), cx);
    }
}
