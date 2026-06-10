#[cfg(not(windows))]
use files_core::pinned_folder_paths;
use files_core::{path_history_list, record_search_history, search_history_list};
use std::path::PathBuf;
use std::rc::Rc;

use files_fs::{
    breadcrumb_root_menu_sections, list_drives, omnibar_path_suggestions,
    omnibar_search_suggestions, path_breadcrumbs, search_scope_path, OmnibarPathSuggestion,
    PathBreadcrumb,
};
#[cfg(windows)]
use app_platform_windows::list_shell_quick_access_folders;
use gpui::{prelude::*, *};
use gpui_component::{
    h_flex,
    input::{Input, InputEvent, InputState},
    ActiveTheme as _,
    ElementExt as _,
    IconName,
    Size,
    Sizable as _,
    StyledExt as _,
};
use rust_i18n::t;

use super::{MainPage, OMNIBAR_BAR_HEIGHT};
use crate::app_state::breadcrumb_navigation_target;
use crate::file_browser::BrowseLocation;
use crate::icons::toolbar_icon;
use crate::omnibar::{OmnibarBreadcrumbCallbacks, BREADCRUMB_DRAG_HOVER_OPEN_MS};
use crate::shell::navigation::NavigationTarget;
use crate::shell_icon::shell_icon_for_path;

const OMNIBAR_SUGGESTIONS_DEBOUNCE_MS: u64 = 150;
const OMNIBAR_PATH_BLUR_DISMISS_MS: u64 = 120;
const OMNIBAR_SUGGESTIONS_MIN_WIDTH: Pixels = px(220.);
const OMNIBAR_SUGGESTIONS_MAX_WIDTH: Pixels = px(350.);
const OMNIBAR_SUGGESTIONS_MAX_HEIGHT: Pixels = px(280.);

impl MainPage {
    pub fn cancel_breadcrumb_drag_preview(&mut self) {
        self.breadcrumb_drag_generation = self.breadcrumb_drag_generation.wrapping_add(1);
    }

    fn ensure_omnibar_breadcrumb_callbacks(&mut self, cx: &mut Context<Self>) {
        if self.omnibar_breadcrumb_callbacks.is_some() {
            return;
        }
        let page = cx.entity();
        let on_navigate = Rc::new(move |path: PathBuf, _: &mut Window, cx: &mut App| {
            let _ = page.update(cx, |page, cx| {
                page.navigate_to(breadcrumb_navigation_target(&path), cx);
            });
        });
        let page_tab = cx.entity();
        let on_navigate_new_tab = Rc::new(move |path: PathBuf, _: &mut Window, cx: &mut App| {
            let _ = page_tab.update(cx, |page, cx| page.open_path_in_new_tab(path, cx));
        });
        let page_home = cx.entity();
        let on_home = Rc::new(move |_: &mut Window, cx: &mut App| {
            let _ = page_home.update(cx, |page, cx| {
                page.navigate_to(NavigationTarget::Home, cx);
            });
        });
        let page_drop = cx.entity();
        let on_drop_paths = Rc::new(
            move |dest: PathBuf, paths: Vec<PathBuf>, window: &mut Window, cx: &mut App| {
                let _ = page_drop.update(cx, |page, cx| {
                    page.drop_paths_on_directory(dest, paths, window, cx);
                });
            },
        );
        let page_drop_external = cx.entity();
        let on_drop_external_paths = Rc::new(
            move |dest: PathBuf, paths: Vec<PathBuf>, window: &mut Window, cx: &mut App| {
                let _ = page_drop_external.update(cx, |page, cx| {
                    page.drop_external_paths_on_directory(dest, paths, window, cx);
                });
            },
        );
        let page_hover = cx.entity();
        let on_drag_hover = Rc::new(
            move |path: PathBuf, dragged_paths: Vec<PathBuf>, window: &mut Window, cx: &mut App| {
            let _ = page_hover.update(cx, |page, cx| {
                page.schedule_breadcrumb_drag_preview(path.clone(), cx);
                page.active_file_browser(cx).update(cx, |browser, cx| {
                    browser.set_breadcrumb_drag_hover_feedback(path, &dragged_paths, window, cx);
                });
            });
        });
        let page_hover_external = cx.entity();
        let on_drag_hover_external = Rc::new(
            move |path: PathBuf, dragged_paths: Vec<PathBuf>, window: &mut Window, cx: &mut App| {
                let _ = page_hover_external.update(cx, |page, cx| {
                    page.schedule_breadcrumb_drag_preview(path.clone(), cx);
                    page.active_file_browser(cx).update(cx, |browser, cx| {
                        browser.set_breadcrumb_drag_hover_feedback(
                            path,
                            &dragged_paths,
                            window,
                            cx,
                        );
                    });
                });
            },
        );
        let page_path_bar = cx.entity();
        let on_show_full_path = Rc::new(move |window: &mut Window, cx: &mut App| {
            let _ = page_path_bar.update(cx, |page, cx| {
                page.enter_omnibar_path_edit(window, cx);
            });
        });
        let root_menu = Rc::new(|| {
            let quick_access: Vec<(String, PathBuf)> = {
                #[cfg(windows)]
                {
                    list_shell_quick_access_folders()
                        .unwrap_or_default()
                        .into_iter()
                        .map(|e| (e.display_name, e.path))
                        .collect()
                }
                #[cfg(not(windows))]
                {
                    pinned_folder_paths()
                        .into_iter()
                        .map(|p| {
                            let label = p
                                .file_name()
                                .map(|n| n.to_string_lossy().to_string())
                                .filter(|n| !n.is_empty())
                                .unwrap_or_else(|| p.to_string_lossy().to_string());
                            (label, p)
                        })
                        .collect()
                }
            };
            let drive_entries: Vec<(String, PathBuf)> = list_drives()
                .into_iter()
                .map(|d| (d.label, d.path))
                .collect();
            breadcrumb_root_menu_sections(
                quick_access,
                drive_entries,
                Some(t!("omnibar.breadcrumb.quick_access").to_string()),
                Some(t!("omnibar.breadcrumb.drives").to_string()),
            )
        });
        self.omnibar_breadcrumb_callbacks = Some(OmnibarBreadcrumbCallbacks::new(
            true,
            root_menu,
            on_navigate,
            on_navigate_new_tab,
            on_home,
            on_drop_paths,
            on_drop_external_paths,
            on_drag_hover,
            on_drag_hover_external,
            on_show_full_path,
        ));
    }

    fn omnibar_working_directory(&self, cx: &App) -> Option<PathBuf> {
        let pane = self.active_pane(cx);
        if matches!(pane.read(cx).target(), NavigationTarget::Path(_)) {
            Some(
                pane.read(cx)
                    .file_browser()
                    .read(cx)
                    .current_directory()
                    .to_path_buf(),
            )
        } else {
            None
        }
    }

    pub fn schedule_breadcrumb_drag_preview(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.omnibar_working_directory(cx).as_ref() == Some(&path) {
            return;
        }
        self.breadcrumb_drag_generation = self.breadcrumb_drag_generation.wrapping_add(1);
        let generation = self.breadcrumb_drag_generation;
        let target = breadcrumb_navigation_target(&path);
        cx.spawn(async move |page, cx| {
            cx.background_spawn(async move {
                std::thread::sleep(std::time::Duration::from_millis(
                    BREADCRUMB_DRAG_HOVER_OPEN_MS,
                ));
            })
            .await;
            let _ = page.update(cx, |page, cx| {
                if page.breadcrumb_drag_generation != generation {
                    return;
                }
                page.navigate_to(target, cx);
            });
        })
        .detach();
    }

    pub(super) fn ensure_search_input(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InputState> {
        if let Some(input) = self.search_input.clone() {
            return input;
        }

        let input = cx.new(|cx| InputState::new(window, cx).placeholder(t!("search.placeholder")));
        self._search_subscription = Some(cx.subscribe(
            &input,
            move |page, _, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    page.apply_search_from_input(cx);
                }
            },
        ));
        self.search_input = Some(input.clone());
        input
    }

    pub fn focus_search_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input = self.ensure_search_input(window, cx);
        input.update(cx, |state, cx| state.focus(window, cx));
        cx.notify();
    }

    fn apply_search_from_input(&mut self, cx: &mut Context<Self>) {
        let query = self
            .search_input
            .as_ref()
            .map(|input| input.read(cx).value().to_string())
            .unwrap_or_default();
        let pane = self.active_pane(cx);
        pane.update(cx, |shell, cx| {
            if matches!(shell.target(), NavigationTarget::Path(_)) {
                shell.file_browser().update(cx, |browser, cx| {
                    browser.set_search_query(query, cx);
                });
            }
        });
    }

    pub fn omnibar_path_edit_active(&self) -> bool {
        self.omnibar_show_full_path
    }

    pub fn dismiss_omnibar_path_edit(&mut self, cx: &mut Context<Self>) {
        if !self.omnibar_show_full_path {
            return;
        }
        self.omnibar_show_full_path = false;
        self.clear_omnibar_suggestions();
        cx.notify();
    }

    pub fn omnibar_search_mode_active(&self) -> bool {
        self.omnibar_search_mode
    }

    pub fn dismiss_omnibar_search_mode(&mut self, cx: &mut Context<Self>) {
        if !self.omnibar_search_mode {
            return;
        }
        self.omnibar_search_mode = false;
        self.clear_omnibar_search_suggestions();
        cx.notify();
    }

    pub fn enter_omnibar_search_mode(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.dismiss_omnibar_path_edit(cx);
        self.omnibar_search_mode = true;
        let input = self.ensure_omnibar_search_input(window, cx);
        input.update(cx, |state, cx| {
            state.focus(window, cx);
        });
        self.apply_omnibar_search_suggestions_sync(cx);
        cx.notify();
    }

    pub fn submit_global_search(&mut self, cx: &mut Context<Self>) {
        let Some(input) = self.omnibar_search_input.clone() else {
            return;
        };
        let query = input.read(cx).value().trim().to_string();
        if query.is_empty() {
            return;
        }
        self.submit_global_search_query_text(query, cx);
    }

    fn submit_global_search_query(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(query) = self.omnibar_search_suggestions.get(index).cloned() else {
            return;
        };
        self.submit_global_search_query_text(query, cx);
    }

    fn submit_global_search_query_text(&mut self, query: String, cx: &mut Context<Self>) {
        record_search_history(&query);
        self.clear_omnibar_search_suggestions();
        self.dismiss_omnibar_search_mode(cx);
        let page = cx.entity();
        let target = NavigationTarget::SearchResults { query };
        cx.defer(move |cx| {
            page.update(cx, |page, cx| {
                page.navigate_to(target, cx);
            });
        });
    }

    pub(super) fn ensure_omnibar_search_input(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InputState> {
        if let Some(input) = self.omnibar_search_input.clone() {
            return input;
        }

        let input = cx.new(|cx| {
            InputState::new(window, cx).placeholder(t!("search.global.placeholder"))
        });
        self._omnibar_search_subscription = Some(cx.subscribe(
            &input,
            move |page, _, event: &InputEvent, cx| match event {
                InputEvent::PressEnter { .. } => {
                    if page.omnibar_search_suggestions_open {
                        if let Some(ix) = page.omnibar_search_suggestion_index {
                            page.submit_global_search_query(ix, cx);
                            return;
                        }
                    }
                    page.submit_global_search(cx);
                }
                InputEvent::Change => page.refresh_omnibar_search_suggestions(cx),
                _ => {}
            },
        ));
        self.omnibar_search_input = Some(input.clone());
        input
    }

    pub(super) fn on_omnibar_search_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event.keystroke.key.as_str() {
            "up" if self.omnibar_search_suggestions_open => {
                self.move_omnibar_search_suggestion_index(-1, cx);
                cx.stop_propagation();
            }
            "down" if self.omnibar_search_suggestions_open => {
                self.move_omnibar_search_suggestion_index(1, cx);
                cx.stop_propagation();
            }
            "escape" => {
                self.dismiss_omnibar_search_mode(cx);
                window.focus(&self.focus_handle, cx);
                cx.stop_propagation();
            }
            "tab" if self.omnibar_search_suggestions_open => {
                if let Some(ix) = self.omnibar_search_suggestion_index {
                    self.apply_omnibar_search_suggestion_to_input(ix, window, cx);
                }
                cx.stop_propagation();
            }
            _ => {}
        }
    }

    pub(super) fn clear_omnibar_search_suggestions(&mut self) {
        self.omnibar_search_suggestions.clear();
        self.omnibar_search_suggestion_index = None;
        self.omnibar_search_suggestions_open = false;
    }

    fn apply_omnibar_search_suggestions_sync(&mut self, cx: &mut Context<Self>) {
        let query = self
            .omnibar_search_input
            .as_ref()
            .map(|input| input.read(cx).value().to_string())
            .unwrap_or_default();
        let history = search_history_list();
        self.omnibar_search_suggestions = omnibar_search_suggestions(&query, &history);
        self.omnibar_search_suggestions_open = true;
        self.omnibar_search_suggestion_index = if self.omnibar_search_suggestions.is_empty() {
            None
        } else {
            Some(0)
        };
    }

    fn refresh_omnibar_search_suggestions(&mut self, cx: &mut Context<Self>) {
        let query = self
            .omnibar_search_input
            .as_ref()
            .map(|input| input.read(cx).value().to_string())
            .unwrap_or_default();
        self.omnibar_search_suggestions_generation =
            self.omnibar_search_suggestions_generation.wrapping_add(1);
        let generation = self.omnibar_search_suggestions_generation;
        cx.spawn(async move |page, cx| {
            cx.background_spawn(async move {
                std::thread::sleep(std::time::Duration::from_millis(
                    OMNIBAR_SUGGESTIONS_DEBOUNCE_MS,
                ));
            })
            .await;
            let history = search_history_list();
            let suggestions = cx
                .background_spawn(async move { omnibar_search_suggestions(&query, &history) })
                .await;
            let _ = page.update(cx, |page, cx| {
                if page.omnibar_search_suggestions_generation != generation {
                    return;
                }
                page.omnibar_search_suggestions = suggestions;
                page.omnibar_search_suggestions_open = true;
                page.omnibar_search_suggestion_index = if page.omnibar_search_suggestions.is_empty() {
                    None
                } else {
                    Some(0)
                };
                cx.notify();
            });
        })
        .detach();
    }

    fn move_omnibar_search_suggestion_index(&mut self, delta: isize, cx: &mut Context<Self>) {
        if !self.omnibar_search_suggestions_open || self.omnibar_search_suggestions.is_empty() {
            return;
        }
        let len = self.omnibar_search_suggestions.len();
        let next = match self.omnibar_search_suggestion_index {
            Some(ix) => {
                let next = ix as isize + delta;
                next.clamp(0, len as isize - 1) as usize
            }
            None => 0,
        };
        self.omnibar_search_suggestion_index = Some(next);
        cx.notify();
    }

    fn apply_omnibar_search_suggestion_to_input(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(label) = self.omnibar_search_suggestions.get(index).cloned() else {
            return;
        };
        let Some(input) = self.omnibar_search_input.clone() else {
            return;
        };
        input.update(cx, |state, cx| {
            state.set_value(label, window, cx);
        });
        self.refresh_omnibar_search_suggestions(cx);
    }

    pub(super) fn clear_omnibar_suggestions(&mut self) {
        self.omnibar_suggestions.clear();
        self.omnibar_suggestion_index = None;
        self.omnibar_suggestions_open = false;
    }

    fn refresh_omnibar_suggestions(&mut self, cx: &mut Context<Self>) {
        let query = self
            .omnibar_path_input
            .as_ref()
            .map(|input| input.read(cx).value().to_string())
            .unwrap_or_default();
        self.omnibar_suggestions_generation = self.omnibar_suggestions_generation.wrapping_add(1);
        let generation = self.omnibar_suggestions_generation;
        cx.spawn(async move |page, cx| {
            cx.background_spawn(async move {
                std::thread::sleep(std::time::Duration::from_millis(
                    OMNIBAR_SUGGESTIONS_DEBOUNCE_MS,
                ));
            })
            .await;
            let path_history = path_history_list();
            let suggestions = cx
                .background_spawn(async move { omnibar_path_suggestions(&query, &path_history) })
                .await;
            let _ = page.update(cx, |page, cx| {
                if page.omnibar_suggestions_generation != generation {
                    return;
                }
                page.omnibar_suggestions = suggestions;
                page.omnibar_suggestions_open = !page.omnibar_suggestions.is_empty();
                page.omnibar_suggestion_index = if page.omnibar_suggestions_open {
                    Some(0)
                } else {
                    None
                };
                cx.notify();
            });
        })
        .detach();
    }

    fn schedule_omnibar_path_blur_dismiss(&mut self, cx: &mut Context<Self>) {
        self.omnibar_path_blur_generation = self.omnibar_path_blur_generation.wrapping_add(1);
        let generation = self.omnibar_path_blur_generation;
        cx.spawn(async move |page, cx| {
            cx.background_spawn(async move {
                std::thread::sleep(std::time::Duration::from_millis(
                    OMNIBAR_PATH_BLUR_DISMISS_MS,
                ));
            })
            .await;
            let _ = page.update(cx, |page, cx| {
                if page.omnibar_path_blur_generation != generation {
                    return;
                }
                if page.omnibar_show_full_path {
                    page.dismiss_omnibar_path_edit(cx);
                }
            });
        })
        .detach();
    }

    fn move_omnibar_suggestion_index(&mut self, delta: isize, cx: &mut Context<Self>) {
        if !self.omnibar_suggestions_open || self.omnibar_suggestions.is_empty() {
            return;
        }
        let len = self.omnibar_suggestions.len();
        let next = match self.omnibar_suggestion_index {
            Some(ix) => {
                let next = ix as isize + delta;
                next.clamp(0, len as isize - 1) as usize
            }
            None => 0,
        };
        self.omnibar_suggestion_index = Some(next);
        cx.notify();
    }

    fn apply_omnibar_suggestion_to_input(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(label) = self
            .omnibar_suggestions
            .get(index)
            .map(|entry| entry.label.clone())
        else {
            return;
        };
        let Some(input) = self.omnibar_path_input.clone() else {
            return;
        };
        input.update(cx, |state, cx| {
            state.set_value(label, window, cx);
        });
        self.refresh_omnibar_suggestions(cx);
    }

    fn navigate_to_omnibar_suggestion(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(entry) = self.omnibar_suggestions.get(index).cloned() else {
            return;
        };
        if let Some(target) = Self::resolve_path_submit(&entry.label)
            .or_else(|| Self::resolve_path_submit(&entry.path.to_string_lossy()))
        {
            self.clear_omnibar_suggestions();
            self.omnibar_show_full_path = false;
            self.navigate_to(target, cx);
        }
    }

    pub fn on_omnibar_path_key_down(
        &mut self,
        event: &KeyDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match event.keystroke.key.as_str() {
            "up" if self.omnibar_suggestions_open => {
                self.move_omnibar_suggestion_index(-1, cx);
                cx.stop_propagation();
            }
            "down" if self.omnibar_suggestions_open => {
                self.move_omnibar_suggestion_index(1, cx);
                cx.stop_propagation();
            }
            "escape" => {
                if self.omnibar_suggestions_open {
                    self.clear_omnibar_suggestions();
                    cx.notify();
                } else {
                    self.dismiss_omnibar_path_edit(cx);
                }
                cx.stop_propagation();
            }
            "tab" if self.omnibar_suggestions_open => {
                if let Some(ix) = self.omnibar_suggestion_index {
                    self.apply_omnibar_suggestion_to_input(ix, window, cx);
                }
                cx.stop_propagation();
            }
            _ => {}
        }
    }

    fn ensure_omnibar_path_input(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Entity<InputState> {
        if let Some(input) = self.omnibar_path_input.clone() {
            return input;
        }

        let input =
            cx.new(|cx| InputState::new(window, cx).placeholder(t!("nav.path.placeholder")));
        self._omnibar_path_subscription = Some(cx.subscribe(
            &input,
            move |page, _, event: &InputEvent, cx| match event {
                InputEvent::Change => page.refresh_omnibar_suggestions(cx),
                InputEvent::PressEnter { .. } => {
                    if page.omnibar_suggestions_open {
                        if let Some(ix) = page.omnibar_suggestion_index {
                            page.navigate_to_omnibar_suggestion(ix, cx);
                            return;
                        }
                    }
                    page.submit_omnibar_path(cx);
                }
                InputEvent::Blur => page.schedule_omnibar_path_blur_dismiss(cx),
                _ => {}
            },
        ));
        self.omnibar_path_input = Some(input.clone());
        input
    }

    pub fn enter_omnibar_path_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.dismiss_omnibar_search_mode(cx);
        self.omnibar_show_full_path = true;
        let text = self.omnibar_full_path_text(cx);
        let input = self.ensure_omnibar_path_input(window, cx);
        input.update(cx, |state, cx| {
            state.set_value(text, window, cx);
            state.focus(window, cx);
        });
        self.refresh_omnibar_suggestions(cx);
        cx.notify();
    }

    fn submit_omnibar_path(&mut self, cx: &mut Context<Self>) {
        let Some(input) = self.omnibar_path_input.clone() else {
            return;
        };
        let text = input.read(cx).value().to_string();
        if let Some(target) = Self::resolve_path_submit(&text) {
            self.clear_omnibar_suggestions();
            self.omnibar_show_full_path = false;
            self.navigate_to(target, cx);
        }
    }

    fn resolve_path_submit(text: &str) -> Option<NavigationTarget> {
        let trimmed = text.trim();
        if trimmed.is_empty() {
            return None;
        }
        if trimmed.eq_ignore_ascii_case("home") {
            return Some(NavigationTarget::Home);
        }
        if trimmed.eq_ignore_ascii_case("settings") {
            return Some(NavigationTarget::Settings);
        }
        if trimmed.eq_ignore_ascii_case("recycle bin") || trimmed.eq_ignore_ascii_case("recycle") {
            return Some(NavigationTarget::RecycleBin);
        }

        let path = PathBuf::from(trimmed);
        if path.is_dir() {
            return Some(NavigationTarget::Path(path));
        }
        if path.is_file() {
            return path
                .parent()
                .map(|parent| NavigationTarget::Path(parent.to_path_buf()));
        }
        None
    }

    fn omnibar_full_path_text(&self, cx: &App) -> String {
        let pane = self.active_pane(cx);
        match pane.read(cx).current_navigation_target(cx) {
            NavigationTarget::Path(_) => pane
                .read(cx)
                .file_browser()
                .read(cx)
                .current_directory()
                .to_string_lossy()
                .to_string(),
            target => target.toolbar_path_label(),
        }
    }

    fn omnibar_breadcrumbs(&self, cx: &App) -> Vec<PathBreadcrumb> {
        let pane = self.active_pane(cx);
        let target = pane.read(cx).current_navigation_target(cx);
        match target {
            NavigationTarget::Path(_) => {
                let dir = pane
                    .read(cx)
                    .file_browser()
                    .read(cx)
                    .current_directory()
                    .clone();
                let dir_str = dir.to_string_lossy();
                if dir_str == r"::{F02C1A0D-BE21-4350-88B0-7367FC96EF3C}" {
                    vec![PathBreadcrumb {
                        label: t!("sidebar.network_places").to_string(),
                        path: dir,
                    }]
                } else {
                    path_breadcrumbs(&dir)
                }
            }
            NavigationTarget::Home => Vec::new(),
            NavigationTarget::Settings => vec![PathBreadcrumb {
                label: t!("nav.settings").to_string(),
                path: PathBuf::from("settings"),
            }],
            NavigationTarget::RecycleBin => vec![PathBreadcrumb {
                label: t!("nav.recycle_bin").to_string(),
                path: PathBuf::from("recycle"),
            }],
            NavigationTarget::FileTag(name) => vec![PathBreadcrumb {
                label: name.clone(),
                path: PathBuf::from(format!("tag:{name}")),
            }],
            NavigationTarget::SearchResults { query } => {
                let browser = pane.read(cx).file_browser().read(cx);
                let mut crumbs = if let BrowseLocation::SearchResults { scope, .. } =
                    browser.current_browse_location()
                {
                    path_breadcrumbs(&search_scope_path(scope))
                } else {
                    Vec::new()
                };
                crumbs.push(PathBreadcrumb {
                    label: format!("{}: {query}", t!("search.results")),
                    path: PathBuf::from(format!("search:{query}")),
                });
                crumbs
            }
        }
    }

    pub(super) fn render_omnibar(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let show_breadcrumbs = !self.omnibar_show_full_path && !self.omnibar_search_mode;
        let path_input = if self.omnibar_show_full_path {
            Some(self.ensure_omnibar_path_input(window, cx))
        } else {
            None
        };
        let search_input = if self.omnibar_search_mode {
            Some(self.ensure_omnibar_search_input(window, cx))
        } else {
            None
        };
        self.ensure_omnibar_breadcrumb_callbacks(cx);
        let breadcrumbs = self.omnibar_breadcrumbs(cx);
        let working_directory = self.omnibar_working_directory(cx);
        let read_options = *self
            .active_pane(cx)
            .read(cx)
            .file_browser()
            .read(cx)
            .read_options();
        let breadcrumb_width = self.omnibar_breadcrumb_width.max(1.);
        let breadcrumb_callbacks = self
            .omnibar_breadcrumb_callbacks
            .as_ref()
            .expect("breadcrumb callbacks");
        let breadcrumb_bar = breadcrumb_callbacks.breadcrumb_bar(
            breadcrumbs,
            breadcrumb_width,
            read_options,
            working_directory,
        );

        h_flex()
            .id("omnibar-bar")
            .w_full()
            .h(OMNIBAR_BAR_HEIGHT)
            .min_h(OMNIBAR_BAR_HEIGHT)
            .max_h(OMNIBAR_BAR_HEIGHT)
            .min_w_0()
            .items_center()
            .px(px(13.))
            .rounded(px(12.))
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().secondary)
            .relative()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .when(show_breadcrumbs, |bar| {
                bar.child({
                    let page = cx.entity();
                    h_flex()
                        .id("omnibar-breadcrumb-host")
                        .w_full()
                        .min_w_0()
                        .flex_1()
                        .overflow_x_hidden()
                        .items_center()
                        .on_prepaint(move |bounds, _, cx| {
                            let w = f32::from(bounds.size.width);
                            if w < 1.0 {
                                return;
                            }
                            let _ = page.update(cx, |page, cx| {
                                if (page.omnibar_breadcrumb_width - w).abs() > 1.5 {
                                    page.omnibar_breadcrumb_width = w;
                                    cx.notify();
                                }
                            });
                        })
                        .child(breadcrumb_bar)
                })
            })
            .when(!show_breadcrumbs && !self.omnibar_search_mode, |bar| {
                bar.child({
                    let page = cx.entity();
                    div()
                        .id("omnibar-path-input")
                        .w_full()
                        .min_w_0()
                        .flex_1()
                        .relative()
                        .on_prepaint({
                            let page = page.clone();
                            move |bounds, _, cx| {
                                let anchor = point(
                                    bounds.origin.x,
                                    bounds.origin.y + bounds.size.height,
                                );
                                let _ = page.update(cx, |page, cx| {
                                    let changed = page
                                        .omnibar_path_input_anchor
                                        .map(|prev| {
                                            (prev.x - anchor.x).abs() > px(0.5)
                                                || (prev.y - anchor.y).abs() > px(0.5)
                                        })
                                        .unwrap_or(true);
                                    if changed {
                                        page.omnibar_path_input_anchor = Some(anchor);
                                        if page.omnibar_suggestions_open {
                                            cx.notify();
                                        }
                                    }
                                });
                            }
                        })
                        .on_key_down(cx.listener(|page, event, window, cx| {
                            page.on_omnibar_path_key_down(event, window, cx);
                        }))
                        .when_some(path_input.as_ref(), |row, input| {
                            row.child(
                                Input::new(input)
                                    .w_full()
                                    .with_size(Size::Medium)
                                    .appearance(false),
                            )
                        })
                })
            })
            .when(self.omnibar_search_mode, |bar| {
                bar.child({
                    let page = cx.entity();
                    h_flex()
                        .id("omnibar-search-input")
                        .w_full()
                        .min_w_0()
                        .flex_1()
                        .gap_2()
                        .items_center()
                        .relative()
                        .on_prepaint({
                            let page = page.clone();
                            move |bounds, _, cx| {
                                let anchor = point(
                                    bounds.origin.x,
                                    bounds.origin.y + bounds.size.height,
                                );
                                let _ = page.update(cx, |page, cx| {
                                    let changed = page
                                        .omnibar_search_input_anchor
                                        .map(|prev| {
                                            (prev.x - anchor.x).abs() > px(0.5)
                                                || (prev.y - anchor.y).abs() > px(0.5)
                                        })
                                        .unwrap_or(true);
                                    if changed {
                                        page.omnibar_search_input_anchor = Some(anchor);
                                        cx.notify();
                                    }
                                });
                            }
                        })
                        .on_key_down(cx.listener(|page, event, window, cx| {
                            page.on_omnibar_search_key_down(event, window, cx);
                        }))
                        .child(
                            toolbar_icon(IconName::Search)
                                .text_color(cx.theme().muted_foreground),
                        )
                        .when_some(search_input.as_ref(), |row, input| {
                            row.child(
                                Input::new(input)
                                    .w_full()
                                    .with_size(Size::Medium)
                                    .appearance(false),
                            )
                        })
                })
            })
            .when(self.omnibar_search_mode, |bar| {
                bar.when_some(self.omnibar_search_input_anchor, |bar, anchor| {
                    bar.child(self.render_omnibar_search_panel(anchor, cx))
                })
            })
            .when(self.omnibar_show_full_path && self.omnibar_suggestions_open, |bar| {
                bar.when_some(self.omnibar_path_input_anchor, |bar, anchor| {
                    bar.child(self.render_omnibar_suggestions(anchor, window, cx))
                })
            })
    }

    fn render_omnibar_suggestion_row(
        entry: &OmnibarPathSuggestion,
        selected: bool,
        _window: &mut Window,
        cx: &App,
    ) -> impl IntoElement {
        h_flex()
            .w_full()
            .min_w_0()
            .px_2()
            .py_1()
            .gap_2()
            .items_center()
            .text_sm()
            .cursor_pointer()
            .when(entry.dimmed, |row| row.opacity(0.55))
            .when(selected, |row| {
                row.bg(cx.theme().accent)
                    .text_color(cx.theme().accent_foreground)
            })
            .when(!selected, |row| row.hover(|this| this.bg(cx.theme().accent.opacity(0.35))))
            .child(shell_icon_for_path(&entry.path, px(16.), cx))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .child(entry.label.clone()),
            )
    }

    fn render_omnibar_suggestions(
        &self,
        anchor: Point<Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let suggestions = self.omnibar_suggestions.clone();
        let selected = self.omnibar_suggestion_index;
        let page = cx.entity();
        deferred(
            anchored()
                .position(anchor)
                .anchor(Anchor::TopLeft)
                .snap_to_window_with_margin(px(8.))
                .child(
                    div()
                        .id("omnibar-suggestions")
                        .flex()
                        .flex_col()
                        .min_w(OMNIBAR_SUGGESTIONS_MIN_WIDTH)
                        .max_w(OMNIBAR_SUGGESTIONS_MAX_WIDTH)
                        .max_h(OMNIBAR_SUGGESTIONS_MAX_HEIGHT)
                        .overflow_y_scroll()
                        .rounded(cx.theme().radius)
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().background)
                        .shadow_lg()
                        .children(suggestions.iter().enumerate().map(|(ix, entry)| {
                            let page = page.clone();
                            let selected_row = selected == Some(ix);
                            let entry = entry.clone();
                            div()
                                .id(("omnibar-suggestion", ix))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    move |_, _, cx| {
                                        let _ = page.update(cx, |page, cx| {
                                            page.omnibar_path_blur_generation =
                                                page.omnibar_path_blur_generation.wrapping_add(1);
                                            page.navigate_to_omnibar_suggestion(ix, cx);
                                        });
                                        cx.stop_propagation();
                                    },
                                )
                                .child(Self::render_omnibar_suggestion_row(
                                    &entry,
                                    selected_row,
                                    window,
                                    cx,
                                ))
                        })),
                ),
        )
        .with_priority(1)
    }

    fn render_omnibar_search_suggestion_row(
        label: &str,
        selected: bool,
        cx: &App,
    ) -> impl IntoElement {
        h_flex()
            .w_full()
            .min_w_0()
            .px_2()
            .py_1()
            .gap_2()
            .items_center()
            .text_sm()
            .cursor_pointer()
            .when(selected, |row| {
                row.bg(cx.theme().accent)
                    .text_color(cx.theme().accent_foreground)
            })
            .when(!selected, |row| row.hover(|this| this.bg(cx.theme().accent.opacity(0.35))))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .child(label.to_string()),
            )
    }

    fn render_omnibar_search_help_row(label: &str, cx: &App) -> impl IntoElement {
        div()
            .w_full()
            .px_2()
            .py_0p5()
            .text_xs()
            .text_color(cx.theme().muted_foreground)
            .child(label.to_string())
    }

    fn render_omnibar_search_panel(
        &self,
        anchor: Point<Pixels>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let suggestions = self.omnibar_search_suggestions.clone();
        let selected = self.omnibar_search_suggestion_index;
        let query = self
            .omnibar_search_input
            .as_ref()
            .map(|input| input.read(cx).value().to_string())
            .unwrap_or_default();
        let history_empty = query.trim().is_empty() && suggestions.is_empty();
        let history_no_match = !query.trim().is_empty() && suggestions.is_empty();
        let page = cx.entity();
        deferred(
            anchored()
                .position(anchor)
                .anchor(Anchor::TopLeft)
                .snap_to_window_with_margin(px(8.))
                .child(
                    div()
                        .id("omnibar-search-panel")
                        .flex()
                        .flex_col()
                        .min_w(OMNIBAR_SUGGESTIONS_MIN_WIDTH)
                        .max_w(OMNIBAR_SUGGESTIONS_MAX_WIDTH)
                        .max_h(OMNIBAR_SUGGESTIONS_MAX_HEIGHT)
                        .overflow_y_scroll()
                        .rounded(cx.theme().radius)
                        .border_1()
                        .border_color(cx.theme().border)
                        .bg(cx.theme().background)
                        .shadow_lg()
                        .child(
                            div()
                                .px_2()
                                .pt_2()
                                .pb_1()
                                .text_xs()
                                .font_bold()
                                .text_color(cx.theme().muted_foreground)
                                .child(t!("search.help.title")),
                        )
                        .child(Self::render_omnibar_search_help_row(
                            &t!("search.help.plain"),
                            cx,
                        ))
                        .child(Self::render_omnibar_search_help_row(
                            &t!("search.help.tag"),
                            cx,
                        ))
                        .child(Self::render_omnibar_search_help_row(
                            &t!("search.help.aqs"),
                            cx,
                        ))
                        .child(
                            div()
                                .mt_1()
                                .mx_2()
                                .border_t_1()
                                .border_color(cx.theme().border),
                        )
                        .child(
                            div()
                                .px_2()
                                .pt_2()
                                .pb_1()
                                .text_xs()
                                .font_bold()
                                .text_color(cx.theme().muted_foreground)
                                .child(t!("search.history.title")),
                        )
                        .when(history_empty, |panel| {
                            panel.child(
                                div()
                                    .px_2()
                                    .pb_2()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(t!("search.history.empty")),
                            )
                        })
                        .when(history_no_match, |panel| {
                            panel.child(
                                div()
                                    .px_2()
                                    .pb_2()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(t!("search.history.no_match")),
                            )
                        })
                        .children(suggestions.iter().enumerate().map(|(ix, entry)| {
                            let page = page.clone();
                            let selected_row = selected == Some(ix);
                            let entry = entry.clone();
                            div()
                                .id(("omnibar-search-suggestion", ix))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    move |_, _, cx| {
                                        let _ = page.update(cx, |page, cx| {
                                            page.submit_global_search_query(ix, cx);
                                        });
                                        cx.stop_propagation();
                                    },
                                )
                                .child(Self::render_omnibar_search_suggestion_row(
                                    &entry,
                                    selected_row,
                                    cx,
                                ))
                        })),
                ),
        )
        .with_priority(1)
    }
}
