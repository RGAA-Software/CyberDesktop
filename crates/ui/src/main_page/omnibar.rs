#[cfg(not(windows))]
use cyberfiles_core::pinned_folder_paths;
use cyberfiles_core::record_path_history;
use std::path::PathBuf;
use std::rc::Rc;

use cyberfiles_fs::{
    breadcrumb_root_menu_sections, list_drives, path_breadcrumbs, PathBreadcrumb,
};
#[cfg(windows)]
use cyberfiles_platform_windows::list_shell_quick_access_folders;
use gpui::{prelude::*, *};
use gpui_component::{
    h_flex,
    input::{Input, InputEvent, InputState},
    ActiveTheme as _,
    ElementExt as _,
    Size,
    Sizable as _,
};
use rust_i18n::t;

use super::{MainPage, OMNIBAR_BAR_HEIGHT};
use crate::app_state::breadcrumb_navigation_target;
use crate::omnibar::{OmnibarBreadcrumbCallbacks, BREADCRUMB_DRAG_HOVER_OPEN_MS};
use crate::shell::navigation::NavigationTarget;

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
        let page_hover = cx.entity();
        let on_drag_hover = Rc::new(move |path: PathBuf, _: &mut Window, cx: &mut App| {
            let _ = page_hover.update(cx, |page, cx| {
                page.schedule_breadcrumb_drag_preview(path, cx);
            });
        });
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
            on_drag_hover,
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
        cx.notify();
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
                InputEvent::PressEnter { .. } => page.submit_omnibar_path(cx),
                InputEvent::Blur => page.dismiss_omnibar_path_edit(cx),
                _ => {}
            },
        ));
        self.omnibar_path_input = Some(input.clone());
        input
    }

    pub fn enter_omnibar_path_edit(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.omnibar_show_full_path = true;
        let text = self.omnibar_full_path_text(cx);
        let input = self.ensure_omnibar_path_input(window, cx);
        input.update(cx, |state, cx| {
            state.set_value(text, window, cx);
            state.focus(window, cx);
        });
        cx.notify();
    }

    fn submit_omnibar_path(&mut self, cx: &mut Context<Self>) {
        let Some(input) = self.omnibar_path_input.clone() else {
            return;
        };
        let text = input.read(cx).value().to_string();
        if let Some(target) = Self::resolve_path_submit(&text) {
            if let NavigationTarget::Path(ref path) = target {
                record_path_history(path);
            }
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
            NavigationTarget::Path(_) | NavigationTarget::RecycleBin => pane
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
                path_breadcrumbs(&dir)
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
        }
    }

    pub(super) fn render_omnibar(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let show_breadcrumbs = !self.omnibar_show_full_path;
        let path_input = if self.omnibar_show_full_path {
            Some(self.ensure_omnibar_path_input(window, cx))
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
            .px_2()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
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
            .when(!show_breadcrumbs, |bar| {
                bar.child(
                    div()
                        .id("omnibar-path-input")
                        .w_full()
                        .min_w_0()
                        .flex_1()
                        .when_some(path_input.as_ref(), |row, input| {
                            row.child(
                                Input::new(input)
                                    .w_full()
                                    .with_size(Size::Medium)
                                    .appearance(false),
                            )
                        }),
                )
            })
    }
}
