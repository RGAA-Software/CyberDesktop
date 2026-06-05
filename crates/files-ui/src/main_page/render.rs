use files_core::{APP_NAME, GITHUB_REPO_URL};
use files_fs::home_navigation_path;
use gpui::{prelude::*, *};
use gpui_component::{
    badge::Badge,
    button::{Button, ButtonVariants as _},
    h_flex,
    label::Label,
    ActiveTheme as _,
    Disableable as _,
    Icon, IconName,
    Sizable as _,
    ThemeMode,
    StyledExt as _,
    WindowExt as _,
};
use rust_i18n::t;

use super::{
    MainPage, NAV_TOOLBAR_HEIGHT, OMNIBAR_BAR_HEIGHT, TITLE_TAB_BAR_HEIGHT, TITLE_TAB_MIN_WIDTH,
    TITLE_TAB_WIDTH,
};
use crate::icons::{app_logo_element, compact_icon, pin_icon, toolbar_icon, toolbar_tabler};
use crate::tabler_icons;
use crate::shell::navigation::NavigationTarget;
use crate::shell::preferences::apply_theme_mode;
use app_ui::tab::{Tab, TabBar};
use app_ui::title_bar::TitleBar;
use app_ui::toolbar_button::toolbar_icon_button;

fn tab_icon_for_target(target: &NavigationTarget) -> Icon {
    let path = match target {
        NavigationTarget::Home | NavigationTarget::Settings => tabler_icons::HOME,
        NavigationTarget::RecycleBin => tabler_icons::TRASH,
        NavigationTarget::FileTag(_) => tabler_icons::TAG,
        NavigationTarget::SearchResults { .. } => tabler_icons::SEARCH,
        NavigationTarget::Path(_) => tabler_icons::FOLDER,
    };
    toolbar_tabler(path)
}

fn nav_icon_button(id: impl Into<ElementId>) -> Button {
    toolbar_icon_button(id)
        .h(px(32.))
        .w(px(32.))
        .rounded(px(10.))
}

fn path_tool_button(id: impl Into<ElementId>, cx: &App) -> Button {
    toolbar_icon_button(id)
        .h(px(30.))
        .w(px(30.))
        .rounded(px(9.))
        .border_1()
        .border_color(cx.theme().border)
        .bg(cx.theme().background)
}

impl MainPage {
    pub(super) fn render_tab_bar(&self, cx: &mut Context<Self>) -> TabBar {
        let active = self.active_tab;
        TabBar::new("main-tab-bar")
            .track_scroll(&self.tab_bar_scroll_handle)
            .medium_titlebar()
            .tab_height(TITLE_TAB_BAR_HEIGHT)
            .tab_gap(px(4.))
            .selected_index(active)
            .last_empty_space(
                h_flex()
                    .items_center()
                    .pl(px(4.))
                    .child(
                        toolbar_icon_button("main-new-tab")
                            .h(px(28.))
                            .w(px(28.))
                            .rounded_full()
                            .icon(compact_icon(IconName::Plus))
                            .tooltip(t!("nav.new_tab"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.add_tab(NavigationTarget::Path(home_navigation_path()), cx);
                            })),
                    ),
            )
            .children(self.tabs.iter().enumerate().map(|(index, tab)| {
                let title = self.tab_title(index, cx);
                let is_selected = index == active;
                let pane_target = self.tabs[index]
                    .shell
                    .read(cx)
                    .active_pane()
                    .read(cx)
                    .target();
                let is_home = matches!(pane_target, NavigationTarget::Home);
                let close_color = if is_selected {
                    cx.theme().tab_active_foreground
                } else {
                    cx.theme().muted_foreground
                };
                let tab_label = h_flex()
                    .w_full()
                    .min_w_0()
                    .gap(px(7.))
                    .items_center()
                    .child(tab_icon_for_target(&pane_target))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .overflow_hidden()
                            .child(Label::new(title).text_left().truncate()),
                    );
                let mut tab_item = Tab::new()
                    .w(TITLE_TAB_WIDTH)
                    .min_w(TITLE_TAB_MIN_WIDTH)
                    .max_w(TITLE_TAB_WIDTH)
                    .flex_shrink_0()
                    .child(tab_label);
                if !is_home {
                    tab_item = tab_item.suffix(
                        Button::new(format!("main-tab-close-{}", tab.id))
                            .ghost()
                            .h(px(18.))
                            .w(px(18.))
                            .rounded_full()
                            .text_color(close_color)
                            .icon(compact_icon(IconName::Close).small())
                            .tooltip(t!("nav.close_tab"))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                cx.stop_propagation();
                                this.close_tab(index, cx);
                            })),
                    );
                }
                tab_item
            }))
            .on_click(cx.listener(|this, ix: &usize, _, cx| {
                if this.active_tab != *ix {
                    this.active_tab = *ix;
                    this.pending_tab_scroll_to_ix = Some(*ix);
                    this.dismiss_omnibar_path_edit(cx);
                    this.dismiss_omnibar_search_mode(cx);
                    this.refresh_active_view(cx);
                    this.persist_session(cx);
                    cx.notify();
                }
            }))
    }

    /// Menu + tabs + window actions in one row (browser-style title bar).
    pub(super) fn render_title_bar(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let notifications_count = window.notifications(cx).len();
        let is_dark = cx.theme().mode.is_dark();
        let theme_icon = if is_dark {
            IconName::Moon
        } else {
            IconName::Sun
        };
        if let Some(ix) = self.pending_tab_scroll_to_ix.take() {
            self.tab_bar_scroll_handle.scroll_to_item(ix);
        }
        let tab_bar = self.render_tab_bar(cx);

        TitleBar::new().child(
            h_flex()
                .id("title-bar-inner")
                .h_full()
                .w_full()
                .min_w_0()
                .items_center()
                .child(
                    h_flex()
                        .id("app-logo")
                        .flex_none()
                        .items_center()
                        .gap(px(8.))
                        .pl(px(16.))
                        .pr(px(12.))
                        .child(app_logo_element(cx))
                        .child(
                            div()
                                .text_sm()
                                .font_semibold()
                                .text_color(cx.theme().foreground)
                                .child(APP_NAME),
                        ),
                )
                .child(
                    div()
                        .id("title-bar-tabs")
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .flex()
                        .overflow_hidden()
                        .items_center()
                        .px(px(10.))
                        .on_mouse_down(
                            MouseButton::Right,
                            cx.listener(|this, event: &MouseDownEvent, window, cx| {
                                this.open_tab_bar_context_menu(event.position, window, cx);
                                cx.stop_propagation();
                            }),
                        )
                        .child(
                            div()
                                .w_full()
                                .min_w_0()
                                .h(TITLE_TAB_BAR_HEIGHT)
                                .overflow_hidden()
                                .child(tab_bar.w_full().min_w_0().h(TITLE_TAB_BAR_HEIGHT)),
                        ),
                )
                .child(
                    h_flex()
                        .id("title-bar-actions")
                        .flex_none()
                        .items_center()
                        .gap(px(6.))
                        .px(px(10.))
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(
                            toolbar_icon_button("theme-toggle")
                                .icon(toolbar_icon(theme_icon))
                                .tooltip(t!("nav.theme_toggle"))
                                .on_click(move |_, _, cx| {
                                    let mode = if cx.theme().mode.is_dark() {
                                        ThemeMode::Light
                                    } else {
                                        ThemeMode::Dark
                                    };
                                    apply_theme_mode(mode, cx);
                                }),
                        )
                        .child(
                            toolbar_icon_button("settings")
                                .icon(toolbar_icon(IconName::Settings2))
                                .tooltip(t!("nav.settings"))
                                .on_mouse_down(
                                    MouseButton::Left,
                                    cx.listener(|this, _e: &MouseDownEvent, _, cx| {
                                        cx.stop_propagation();
                                        this.open_settings(cx);
                                    }),
                                ),
                        )
                        .child(
                            toolbar_icon_button("github")
                                .icon(toolbar_icon(IconName::Github))
                                .tooltip(t!("nav.github"))
                                .on_click(|_, _, cx| {
                                    cx.open_url(GITHUB_REPO_URL)
                                }),
                        )
                        .child(
                            div().relative().child(
                                Badge::new().count(notifications_count).max(99).child(
                                    toolbar_icon_button("bell")
                                        .icon(toolbar_icon(IconName::Bell))
                                        .tooltip(t!("nav.notifications")),
                                ),
                            ),
                        ),
                ),
        )
    }

    pub(super) fn render_navigation_toolbar(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let show_info_pane = self.show_info_pane;
        let pane = self.active_pane(cx);
        let target = pane.read(cx).current_navigation_target(cx);
        let browser = pane.read(cx).file_browser();
        let (can_back, can_forward, can_up) = if matches!(
            target,
            NavigationTarget::Path(_)
                | NavigationTarget::RecycleBin
                | NavigationTarget::SearchResults { .. }
        ) {
            let b = browser.read(cx);
            (b.can_go_back(), b.can_go_forward(), b.can_go_up())
        } else {
            (false, false, false)
        };
        let on_search_results = matches!(target, NavigationTarget::SearchResults { .. });
        let back_tooltip = if on_search_results {
            t!("search.back_to_folder").to_string()
        } else {
            t!("nav.back").to_string()
        };
        let show_file_search = matches!(
            target,
            NavigationTarget::Path(_)
                | NavigationTarget::RecycleBin
                | NavigationTarget::FileTag(_)
        );

        h_flex()
            .id("navigation-toolbar")
            .w_full()
            .flex_none()
            .min_w_0()
            .h(NAV_TOOLBAR_HEIGHT)
            .min_h(NAV_TOOLBAR_HEIGHT)
            .gap(px(8.))
            .px(px(16.))
            .items_center()
            .border_b_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .child(
                h_flex()
                    .id("nav-leading")
                    .flex_none()
                    .gap(px(4.))
                    .items_center()
                    .child(
                        nav_icon_button("nav-back")
                            .icon(toolbar_icon(IconName::ArrowLeft))
                            .tooltip(back_tooltip)
                            .disabled(!can_back)
                            .on_click(cx.listener(|this, _, _, cx| {
                                let browser = this.active_pane(cx).read(cx).file_browser().clone();
                                cx.defer(move |cx| {
                                    browser.update(cx, |b, cx| b.go_back(cx));
                                });
                            })),
                    )
                    .child(
                        nav_icon_button("nav-forward")
                            .icon(toolbar_icon(IconName::ArrowRight))
                            .tooltip(t!("nav.forward"))
                            .disabled(!can_forward)
                            .on_click(cx.listener(|this, _, _, cx| {
                                let browser = this.active_pane(cx).read(cx).file_browser().clone();
                                browser.update(cx, |b, cx| b.go_forward(cx));
                            })),
                    )
                    .child(
                        nav_icon_button("nav-up")
                            .icon(toolbar_icon(IconName::ArrowUp))
                            .tooltip(t!("nav.up"))
                            .disabled(!can_up)
                            .on_click(cx.listener(|this, _, _, cx| {
                                let browser = this.active_pane(cx).read(cx).file_browser().clone();
                                browser.update(cx, |b, cx| b.go_up(cx));
                            })),
                    )
                    .child(
                        nav_icon_button("nav-refresh")
                            .icon(toolbar_icon(IconName::Redo2))
                            .tooltip(t!("nav.refresh"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.refresh_active_view(cx);
                            })),
                    ),
            )
            .child(
                div()
                    .id("nav-omnibar-region")
                    .flex_1()
                    .min_w_0()
                    .h(OMNIBAR_BAR_HEIGHT)
                    .child(self.render_omnibar(window, cx)),
            )
            .child({
                let dual_pane = self.dual_pane_active(cx);
                let mut trailing = h_flex()
                    .id("path-actions")
                    .flex_none()
                    .gap(px(6.))
                    .ml(px(6.))
                    .items_center()
                    .child(
                        path_tool_button("nav-split-pane", cx)
                            .icon(toolbar_tabler(tabler_icons::LAYOUT_COLUMNS))
                            .tooltip(t!("nav.split_pane"))
                            .when(dual_pane, |btn| {
                                btn.bg(cx.theme().accent)
                                    .border_color(cx.theme().primary)
                                    .text_color(cx.theme().accent_foreground)
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.toggle_dual_pane(cx);
                            })),
                    )
                    .child(
                        nav_icon_button("nav-toggle-info")
                            .icon(toolbar_icon(if show_info_pane {
                                IconName::PanelRightClose
                            } else {
                                IconName::PanelRightOpen
                            }))
                            .tooltip(if show_info_pane {
                                t!("nav.hide_info_pane")
                            } else {
                                t!("nav.show_info_pane")
                            })
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.toggle_info_pane(cx);
                            })),
                    );
                if show_file_search {
                    trailing = trailing.child(
                        path_tool_button("nav-pin-folder", cx)
                            .icon(pin_icon())
                            .tooltip(t!("nav.pin_folder"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.pin_current_folder(cx);
                            })),
                    );
                }
                trailing
            })
    }
}
