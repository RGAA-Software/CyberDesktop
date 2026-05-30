//! UI fragment: `ui/chrome.rs`.

use super::icons::{paths, toolbar_icon, toolbar_icon_button};
use cyber_desktop_ui::{apply_theme_mode, Tab, TabBar};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex,
    label::Label,
    ActiveTheme as _, IconName, ThemeMode,
};
use super::super::imports::*;

const EDITOR_TAB_BAR_HEIGHT: Pixels = px(30.);
/// Fixed width per document tab (label truncates inside).
const EDITOR_TAB_WIDTH: Pixels = px(200.);
const EDITOR_TAB_CLOSE_RIGHT_INSET: Pixels = px(5.);

impl EngineEditor {
    pub(crate) fn render_header(&self, cx: &mut Context<Self>) -> gpui::Div {
        let theme = cx.theme();
        let name = self
            .document
            .path()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| t!("editor.untitled").to_string());
        let dirty = if self.document.dirty() { " ●" } else { "" };
        let caret = self.document.selections().primary().head;
        let pos = self.document.buffer().char_to_position(caret);
        let info = t!(
            "editor.status.ln_col",
            line = pos.line + 1,
            col = pos.column + 1
        );
        let mut info = format!(
            "{}   {}   {}   {}",
            self.document.language(),
            self.document.encoding().label(),
            self.document.line_ending().label(),
            info,
        );
        if self.has_long_line() {
            info.push_str("   ");
            info.push_str(
                &t!(
                    "editor.status.long_line",
                    cols = self.status_line_len_chars()
                )
                .to_string(),
            );
        }

        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .h(px(28.0))
            .px_3()
            .bg(theme.tab_bar)
            .text_color(theme.foreground)
            .text_size(px(12.0))
            .child(div().child(SharedString::from(format!("{name}{dirty}"))))
            .child(
                div()
                    .text_color(theme.muted_foreground)
                    .child(SharedString::from(info)),
            )
    }

    /// The tab strip (one chip per open document + a "new tab" button).
    pub(crate) fn render_tab_bar(&self, cx: &mut Context<Self>) -> TabBar {
        let active = self.active;
        TabBar::new("editor-tab-bar")
            .h(EDITOR_TAB_BAR_HEIGHT)
            .tab_height(EDITOR_TAB_BAR_HEIGHT)
            .w_full()
            .track_scroll(&self.tab_bar_scroll_handle)
            .bottom_border(true)
            .inactive_separators(true)
            .menu(true)
            .selected_index(active)
            .last_empty_space(
                h_flex().gap_1().pr_1().child(
                    toolbar_icon_button("tab-new")
                        .icon(toolbar_icon(IconName::Plus).path("icons/plus.svg"))
                        .tooltip(t!("editor.tooltip.new_tab"))
                        .on_click(cx.listener(|this, _, _, cx| this.new_tab(cx))),
                ),
            )
            .children((0..self.tabs.len()).map(|index| {
                let title = SharedString::from(self.tab_title(index));
                let is_selected = index == active;
                let close_color = if is_selected {
                    cx.theme().tab_active_foreground
                } else {
                    cx.theme().muted_foreground
                };
                Tab::new()
                    .w(EDITOR_TAB_WIDTH)
                    .min_w(EDITOR_TAB_WIDTH)
                    .max_w(EDITOR_TAB_WIDTH)
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
                        Button::new(format!("editor-tab-close-{index}"))
                            .xsmall()
                            .ghost()
                            .mr(EDITOR_TAB_CLOSE_RIGHT_INSET)
                            .text_color(close_color)
                            .icon(toolbar_icon(IconName::Close).path(paths::CLOSE))
                            .tooltip(t!("nav.close_tab"))
                            .on_click(cx.listener(move |this, _, _, cx| {
                                cx.stop_propagation();
                                this.close_tab(index, cx);
                            })),
                    )
            }))
            .on_click(cx.listener(|this, ix: &usize, _, cx| {
                this.switch_to_tab(*ix, cx);
            }))
    }

    /// A banner shown when the active file was modified on disk by another app.
    pub(crate) fn render_disk_banner(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        if !self.disk_changed {
            return None;
        }
        let theme = cx.theme();
        Some(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .h(px(28.0))
                .px_3()
                .bg(theme.warning)
                .text_color(theme.warning_foreground)
                .text_size(px(12.0))
                .child(SharedString::from(t!("editor.disk.changed")))
                .child(
                    Button::new("disk-reload")
                        .xsmall()
                        .label(t!("editor.disk.reload"))
                        .on_click(cx.listener(|this, _: &ClickEvent, _w, cx| {
                            this.reload_from_disk(cx);
                        })),
                )
                .child(
                    Button::new("disk-ignore")
                        .xsmall()
                        .ghost()
                        .label(t!("editor.disk.ignore"))
                        .on_click(cx.listener(|this, _: &ClickEvent, _w, cx| {
                            this.disk_changed = false;
                            cx.notify();
                        })),
                ),
        )
    }

    /// The Recent Files dropdown (toggled with Ctrl+E).
    pub(crate) fn render_recent(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        if !self.show_recent {
            return None;
        }
        let theme = cx.theme();
        let mut list = div()
            .absolute()
            .top(px(4.0))
            .left(px(4.0))
            .w(px(420.0))
            .max_h(px(360.0))
            .overflow_hidden()
            .rounded_md()
            .bg(theme.popover)
            .border_1()
            .border_color(theme.border)
            .text_size(px(12.0))
            .text_color(theme.popover_foreground)
            .child(
                div()
                    .px_3()
                    .py_1()
                    .bg(theme.list_head)
                    .text_color(theme.muted_foreground)
                    .child(SharedString::from(t!("editor.recent.title"))),
            );

        if self.recent.is_empty() {
            list = list.child(
                div()
                    .px_3()
                    .py_2()
                    .text_color(theme.muted_foreground)
                    .child(SharedString::from(t!("editor.recent.empty"))),
            );
        }
        for (i, path) in self.recent.iter().enumerate() {
            let display = path.to_string_lossy().to_string();
            let target = path.clone();
            list = list.child(
                div()
                    .id(("recent", i))
                    .px_3()
                    .py_1()
                    .overflow_hidden()
                    .hover(|s| s.bg(theme.list_hover))
                    .child(SharedString::from(display))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _e: &MouseDownEvent, _w, cx| {
                            this.show_recent = false;
                            this.open_path_in_tab(target.clone(), cx);
                        }),
                    ),
            );
        }
        Some(list)
    }

    pub(crate) fn render_title_bar(&self, cx: &mut Context<Self>) -> TitleBar {
        let menu_bar = editor_menu_bar(cx);
        let theme_icon = if cx.theme().mode.is_dark() {
            IconName::Moon
        } else {
            IconName::Sun
        };
        let theme_toggle = toolbar_icon_button("editor-theme-toggle")
            .icon(toolbar_icon(theme_icon))
            .tooltip(t!("nav.theme_toggle"))
            .on_click(|_, _, cx| {
                let mode = if cx.theme().mode.is_dark() {
                    ThemeMode::Light
                } else {
                    ThemeMode::Dark
                };
                apply_theme_mode(mode, cx);
            });
        let settings_btn = toolbar_icon_button("editor-settings")
            .icon(toolbar_icon(IconName::Settings2).path(paths::SETTINGS))
            .tooltip(t!("editor.tooltip.settings"))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _e: &MouseDownEvent, _w, cx| {
                    cx.stop_propagation();
                    this.toggle_settings(cx);
                }),
            );

        TitleBar::new()
            .trailing_before_controls(theme_toggle)
            .trailing_before_controls(settings_btn)
            .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .px_2()
                .size_full()
                .child(
                    div()
                        .flex_none()
                        .text_size(px(13.0))
                        .child(SharedString::from(t!("editor.app_name"))),
                )
                .child(div().flex_none().child(menu_bar)),
        )
    }
}
