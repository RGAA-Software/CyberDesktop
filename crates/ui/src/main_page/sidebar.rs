use std::path::PathBuf;

use cyberfiles_core::{load_config, save_config};
use gpui::prelude::*;

use super::MainPage;
use crate::app_state::AppNavigation;
use crate::shell::navigation::NavigationTarget;
use crate::sidebar::sidebar_cache_key;

impl MainPage {
    /// Rebuild sidebar section lists when settings or pins change (async when cache exists).
    pub fn refresh_sidebar_cache(&mut self, cx: &mut Context<Self>) {
        self.sidebar_cache_key = 0;
        self.ensure_sidebar_cache(cx);
    }

    /// Reload file browsers that are browsing `tag_name` (all tabs / dual panes).
    pub fn reload_file_tag_browsers(&mut self, tag_name: &str, cx: &mut Context<Self>) {
        for tab in &self.tabs {
            let shell = tab.shell.clone();
            shell.update(cx, |shell, cx| {
                shell.for_each_pane(|pane| {
                    pane.update(cx, |pane, cx| {
                        if matches!(
                            pane.target(),
                            NavigationTarget::FileTag(name) if name == tag_name
                        ) {
                            pane.file_browser().update(cx, |browser, cx| {
                                browser.reload();
                                cx.notify();
                            });
                        }
                    });
                });
            });
        }
    }

    pub(super) fn ensure_sidebar_cache(&mut self, cx: &mut Context<Self>) {
        let config = load_config().unwrap_or_default();
        let key = sidebar_cache_key(&config);
        if self.sidebar_cache_key == key && !self.sidebar_sections.is_empty() {
            return;
        }
        if self.sidebar_cache_loading {
            return;
        }
        if self.sidebar_sections.is_empty() {
            self.sidebar_sections = crate::sidebar::build_sidebar_sections_cached(&config);
            self.sidebar_cache_key = key;
            return;
        }
        self.sidebar_cache_loading = true;
        self.sidebar_cache_generation = self.sidebar_cache_generation.wrapping_add(1);
        let generation = self.sidebar_cache_generation;
        cx.spawn(async move |page, cx| {
            let sections = cx
                .background_spawn(async move {
                    let config = load_config().unwrap_or_default();
                    crate::sidebar::build_sidebar_sections_cached(&config)
                })
                .await;
            let key = load_config().map(|c| sidebar_cache_key(&c)).unwrap_or(0);
            let _ = page.update(cx, |page, cx| {
                page.sidebar_cache_loading = false;
                if page.sidebar_cache_generation != generation {
                    return;
                }
                page.sidebar_sections = sections;
                page.sidebar_cache_key = key;
                cx.notify();
            });
        })
        .detach();
    }

    pub fn toggle_sidebar_collapsed(&mut self, cx: &mut Context<Self>) {
        let mut config = load_config().unwrap_or_default();
        config.sidebar_collapsed = !config.sidebar_collapsed;
        let _ = save_config(&config);
        cx.notify();
    }

    pub fn refresh_home_widgets(&mut self, cx: &mut Context<Self>) {
        self.active_pane(cx)
            .update(cx, |pane, cx| pane.reload_home(cx));
        cx.notify();
    }

    pub fn pin_folder_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let path_string = path.to_string_lossy().to_string();
        let mut config = load_config().unwrap_or_default();
        if !config.pinned_folders.iter().any(|p| p == &path_string) {
            config.pinned_folders.push(path_string);
            let _ = save_config(&config);
            if let Err(error) = cyberfiles_fs::sync_pin_to_shell_quick_access(&path) {
                eprintln!("[home] pintohome: {error:#}");
            }
            self.refresh_sidebar_cache(cx);
            AppNavigation::refresh_quick_access(cx);
            cx.notify();
        }
    }

    pub fn unpin_folder_path(&mut self, path_string: &str, cx: &mut Context<Self>) {
        let mut config = load_config().unwrap_or_default();
        if let Some(index) = config.pinned_folders.iter().position(|p| p == path_string) {
            config.pinned_folders.remove(index);
            let _ = save_config(&config);
            let path = PathBuf::from(path_string);
            if path.exists() {
                if let Err(error) = cyberfiles_fs::sync_unpin_from_shell_quick_access(&path) {
                    eprintln!("[home] unpinfromhome: {error:#}");
                }
            }
            self.refresh_sidebar_cache(cx);
            AppNavigation::refresh_quick_access(cx);
            cx.notify();
        }
    }

    pub fn move_pinned_folder(&mut self, path_string: &str, delta: i32, cx: &mut Context<Self>) {
        let mut config = load_config().unwrap_or_default();
        let Some(index) = config.pinned_folders.iter().position(|p| p == path_string) else {
            return;
        };
        let new_index =
            (index as i32 + delta).clamp(0, config.pinned_folders.len() as i32 - 1) as usize;
        if new_index == index {
            return;
        }
        let entry = config.pinned_folders.remove(index);
        config.pinned_folders.insert(new_index, entry);
        let _ = save_config(&config);
        self.refresh_sidebar_cache(cx);
        cx.notify();
    }

    pub(super) fn pin_current_folder(&mut self, cx: &mut Context<Self>) {
        let pane = self.active_pane(cx);
        let path = pane
            .read(cx)
            .file_browser()
            .read(cx)
            .current_directory()
            .clone();
        let path_string = path.to_string_lossy().to_string();
        let mut config = load_config().unwrap_or_default();
        if let Some(index) = config.pinned_folders.iter().position(|p| p == &path_string) {
            config.pinned_folders.remove(index);
        } else {
            config.pinned_folders.push(path_string);
        }
        let _ = save_config(&config);
        cx.notify();
    }
}
