use cyber_desktop_core::load_config;
use cyber_desktop_fs::home_navigation_path;
use gpui::{prelude::*, *};
use gpui_component::{
    badge::Badge,
    button::{Button, ButtonVariants as _},
    h_flex,
    input::Input,
    label::Label,
    ActiveTheme as _,
    Disableable as _,
    IconName,
    Sizable as _,
    Size,
    ThemeMode,
    WindowExt as _,
};
use rust_i18n::t;

use super::{
    MainPage, NAV_TOOLBAR_HEIGHT, OMNIBAR_BAR_HEIGHT, TITLE_TAB_BAR_HEIGHT,
    TITLE_TAB_CLOSE_RIGHT_INSET, TITLE_TAB_WIDTH,
};
use crate::icons::{compact_icon, pin_icon, toolbar_icon};
use crate::shell::app_menus;
use crate::shell::navigation::NavigationTarget;
use crate::shell::preferences::apply_theme_mode;
use crate::tab::{Tab, TabBar};
use crate::title_bar::TitleBar;
use crate::toolbar_button::toolbar_icon_button;

impl MainPage {
    pub(super) fn render_tab_bar(&self, cx: &mut Context<Self>) -> TabBar {
        let active = self.active_tab;
        TabBar::new("main-tab-bar")
            .track_scroll(&self.tab_bar_scroll_handle)
            .with_size(Size::Medium)
            .selected_index(active)
            .last_empty_space(
                h_flex().gap_1().pr_1().child(
                    toolbar_icon_button("main-new-tab")
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
                let close_color = if is_selected {
                    cx.theme().tab_active_foreground
                } else {
                    cx.theme().muted_foreground
                };
                Tab::new()
                    .w(TITLE_TAB_WIDTH)
                    .min_w(TITLE_TAB_WIDTH)
                    .max_w(TITLE_TAB_WIDTH)
                    .flex_shrink_0()
                    .child(
                        div()
                            .w_full()
                            .min_w_0()
                            .overflow_hidden()
                            .flex()
                            .items_center()
                            .child(Label::new(title).text_left().truncate()),
                    )
                    .suffix(
                        Button::new(format!("main-tab-close-{}", tab.id))
                            .small()
                            .ghost()
                            .mr(TITLE_TAB_CLOSE_RIGHT_INSET)
                            .text_color(close_color)
                            .icon(compact_icon(IconName::Close).small())
                            .tooltip(t!("nav.close_tab"))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                cx.stop_propagation();
                                this.close_tab(index, cx);
                            })),
                    )
            }))
            .on_click(cx.listener(|this, ix: &usize, _, cx| {
                this.active_tab = *ix;
                this.pending_tab_scroll_to_ix = Some(*ix);
                this.persist_session(cx);
                cx.notify();
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
        let app_menu_bar = app_menus::menu_bar(cx);
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
                .gap_1()
                .child(div().flex_none().child(app_menu_bar))
                .child(
                    div()
                        .id("title-bar-tabs")
                        .flex_1()
                        .min_w_0()
                        .h_full()
                        .flex()
                        .overflow_hidden()
                        .items_center()
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
                        .gap_2()
                        .px_2()
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
                            toolbar_icon_button("github")
                                .icon(toolbar_icon(IconName::Github))
                                .tooltip(t!("nav.github"))
                                .on_click(|_, _, cx| {
                                    cx.open_url("https://github.com/RGAA-Software/CyberFiles")
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
        let sidebar_collapsed = load_config().map(|c| c.sidebar_collapsed).unwrap_or(false);
        let pane = self.active_pane(cx);
        let target = pane.read(cx).current_navigation_target(cx);
        let browser = pane.read(cx).file_browser();
        let (can_back, can_forward, can_up) = if matches!(
            target,
            NavigationTarget::Path(_) | NavigationTarget::RecycleBin
        ) {
            let b = browser.read(cx);
            (b.can_go_back(), b.can_go_forward(), b.can_go_up())
        } else {
            (false, false, false)
        };
        let show_file_search = matches!(
            target,
            NavigationTarget::Path(_) | NavigationTarget::RecycleBin | NavigationTarget::FileTag(_)
        );

        h_flex()
            .id("navigation-toolbar")
            .w_full()
            .flex_none()
            .min_w_0()
            .h(NAV_TOOLBAR_HEIGHT)
            .min_h(NAV_TOOLBAR_HEIGHT)
            .gap_1()
            .px_1()
            .items_center()
            .border_b_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .child(
                h_flex()
                    .id("nav-leading")
                    .flex_none()
                    .gap_1()
                    .items_center()
                    .child(
                        toolbar_icon_button("nav-sidebar-toggle")
                            .icon(toolbar_icon(if sidebar_collapsed {
                                IconName::PanelLeftOpen
                            } else {
                                IconName::PanelLeftClose
                            }))
                            .tooltip(t!("sidebar.toggle"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.toggle_sidebar_collapsed(cx);
                            })),
                    )
                    .child(
                        toolbar_icon_button("nav-back")
                            .icon(toolbar_icon(IconName::ArrowLeft))
                            .tooltip(t!("nav.back"))
                            .disabled(!can_back)
                            .on_click(cx.listener(|this, _, _, cx| {
                                let browser = this.active_pane(cx).read(cx).file_browser().clone();
                                browser.update(cx, |b, cx| b.go_back(cx));
                            })),
                    )
                    .child(
                        toolbar_icon_button("nav-forward")
                            .icon(toolbar_icon(IconName::ArrowRight))
                            .tooltip(t!("nav.forward"))
                            .disabled(!can_forward)
                            .on_click(cx.listener(|this, _, _, cx| {
                                let browser = this.active_pane(cx).read(cx).file_browser().clone();
                                browser.update(cx, |b, cx| b.go_forward(cx));
                            })),
                    )
                    .child(
                        toolbar_icon_button("nav-up")
                            .icon(toolbar_icon(IconName::ArrowUp))
                            .tooltip(t!("nav.up"))
                            .disabled(!can_up)
                            .on_click(cx.listener(|this, _, _, cx| {
                                let browser = this.active_pane(cx).read(cx).file_browser().clone();
                                browser.update(cx, |b, cx| b.go_up(cx));
                            })),
                    )
                    .child(
                        toolbar_icon_button("nav-refresh")
                            .icon(toolbar_icon(IconName::Redo2))
                            .tooltip(t!("nav.refresh"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                let pane = this.active_pane(cx);
                                pane.update(cx, |shell, cx| {
                                    shell.file_browser().update(cx, |b, cx| {
                                        b.reload();
                                        cx.notify();
                                    });
                                });
                                cx.notify();
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
                let mut trailing = h_flex()
                    .id("nav-trailing")
                    .flex_none()
                    .gap_1()
                    .items_center()
                    .child(
                        toolbar_icon_button("nav-split-pane")
                            .icon(toolbar_icon(IconName::LayoutDashboard).path("icons/splitscreen.svg"))
                            .tooltip(t!("nav.split_pane"))
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.toggle_dual_pane(cx);
                            })),
                    )
                    .child(
                        toolbar_icon_button("nav-toggle-info")
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
                    let search_input = self.ensure_search_input(window, cx);
                    trailing = trailing
                        .child(
                            toolbar_icon_button("nav-pin-folder")
                                .icon(pin_icon())
                                .tooltip(t!("nav.pin_folder"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.pin_current_folder(cx);
                                })),
                        )
                        .child(
                            div()
                                .id("nav-search-wrap")
                                .w(px(200.))
                                .min_w(px(140.))
                                .flex_none()
                                .h(OMNIBAR_BAR_HEIGHT)
                                .flex()
                                .items_center()
                                .child(Input::new(&search_input).w_full().with_size(Size::Medium)),
                        );
                }
                trailing
            })
    }
}
