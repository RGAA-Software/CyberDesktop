//! UI fragment: `ui/chrome.rs`.

use super::icons::{paths, toolbar_icon, toolbar_icon_button};
use gpui_component::IconName;
use super::super::imports::*;

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
        let info = format!(
            "{}   {}   {}   {}",
            self.document.language(),
            self.document.encoding().label(),
            self.document.line_ending().label(),
            info,
        );

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
    pub(crate) fn render_tab_bar(&self, cx: &mut Context<Self>) -> gpui::Div {
        let theme = cx.theme();
        let active = self.active;
        let mut strip = div()
            .flex()
            .flex_row()
            .items_center()
            .h(px(30.0))
            .bg(theme.tab_bar)
            .text_size(px(12.0))
            .border_b_1()
            .border_color(theme.border);

        for index in 0..self.tabs.len() {
            let is_active = index == active;
            let title = self.tab_title(index);
            let close = toolbar_icon_button(("tab-close", index))
                .icon(toolbar_icon(IconName::Close).path(paths::CLOSE))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _e: &MouseDownEvent, _w, cx| {
                        cx.stop_propagation();
                        this.close_tab(index, cx);
                    }),
                );

            let chip = div()
                .id(("tab", index))
                .flex()
                .flex_row()
                .items_center()
                .gap_1()
                .px_2()
                .h_full()
                .max_w(px(220.0))
                .border_r_1()
                .border_color(theme.border)
                .bg(if is_active {
                    theme.tab_active
                } else {
                    theme.tab
                })
                .text_color(if is_active {
                    theme.foreground
                } else {
                    theme.muted_foreground
                })
                .hover(|s| s.bg(theme.list_hover))
                .child(
                    div()
                        .overflow_hidden()
                        .child(SharedString::from(title)),
                )
                .child(close)
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _e: &MouseDownEvent, _w, cx| {
                        this.switch_to_tab(index, cx);
                    }),
                );
            strip = strip.child(chip);
        }

        strip.child(
            toolbar_icon_button("tab-new")
                .icon(toolbar_icon(IconName::Plus).path("icons/plus.svg"))
                .tooltip(t!("editor.tooltip.new_tab"))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _e: &MouseDownEvent, _w, cx| this.new_tab(cx)),
                ),
        )
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
