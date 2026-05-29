//! UI fragment: `ui/chrome.rs`.

use super::super::imports::*;
use super::widgets::bar_button;

impl EngineEditor {
    pub(crate) fn render_header(&self) -> gpui::Div {
        let name = self
            .document
            .path()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("Untitled")
            .to_string();
        let dirty = if self.document.dirty() { " ●" } else { "" };
        let caret = self.document.selections().primary().head;
        let pos = self.document.buffer().char_to_position(caret);
        let info = format!(
            "{}   {}   {}   Ln {}, Col {}",
            self.document.language(),
            self.document.encoding().label(),
            self.document.line_ending().label(),
            pos.line + 1,
            pos.column + 1,
        );

        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .h(px(28.0))
            .px_3()
            .bg(rgb(0x252526))
            .text_color(rgb(0xcccccc))
            .text_size(px(12.0))
            .child(div().child(SharedString::from(format!("{name}{dirty}"))))
            .child(div().text_color(rgb(0x8a8a8a)).child(SharedString::from(info)))
    }

    /// The tab strip (one chip per open document + a "new tab" button).
    pub(crate) fn render_tab_bar(&self, cx: &mut Context<Self>) -> gpui::Div {
        let active = self.active;
        let mut strip = div()
            .flex()
            .flex_row()
            .items_center()
            .h(px(30.0))
            .bg(rgb(0x202020))
            .text_size(px(12.0))
            .border_b_1()
            .border_color(rgb(0x161616));

        for index in 0..self.tabs.len() {
            let is_active = index == active;
            let title = self.tab_title(index);
            let close = div()
                .id(("tab-close", index))
                .flex()
                .items_center()
                .justify_center()
                .w(px(16.0))
                .h(px(16.0))
                .rounded_sm()
                .text_color(rgb(0x9a9a9a))
                .hover(|s| s.bg(rgb(0x4a4a4a)).text_color(rgb(0xffffff)))
                .child(SharedString::from("\u{00d7}"))
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
                .border_color(rgb(0x161616))
                .bg(if is_active {
                    rgb(0x1e1e1e)
                } else {
                    rgb(0x2a2a2a)
                })
                .text_color(if is_active {
                    rgb(0xffffff)
                } else {
                    rgb(0xb5b5b5)
                })
                .hover(|s| s.bg(rgb(0x333333)))
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
            div()
                .id("tab-new")
                .flex()
                .items_center()
                .justify_center()
                .w(px(28.0))
                .h_full()
                .text_color(rgb(0x9a9a9a))
                .hover(|s| s.bg(rgb(0x333333)).text_color(rgb(0xffffff)))
                .child(SharedString::from("+"))
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
        let reload = bar_button("disk-reload", "Reload", false).on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _e: &MouseDownEvent, _w, cx| this.reload_from_disk(cx)),
        );
        let ignore = bar_button("disk-ignore", "Ignore", false).on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _e: &MouseDownEvent, _w, cx| {
                this.disk_changed = false;
                cx.notify();
            }),
        );
        Some(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .h(px(28.0))
                .px_3()
                .bg(rgb(0x5a4a1a))
                .text_color(rgb(0xf0e0b0))
                .text_size(px(12.0))
                .child(SharedString::from(
                    "This file changed on disk.".to_string(),
                ))
                .child(reload)
                .child(ignore),
        )
    }

    /// The Recent Files dropdown (toggled with Ctrl+E).
    pub(crate) fn render_recent(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        if !self.show_recent {
            return None;
        }
        let mut list = div()
            .absolute()
            .top(px(4.0))
            .left(px(4.0))
            .w(px(420.0))
            .max_h(px(360.0))
            .overflow_hidden()
            .rounded_md()
            .bg(rgb(0x252526))
            .border_1()
            .border_color(rgb(0x3c3c3c))
            .text_size(px(12.0))
            .text_color(rgb(0xd4d4d4))
            .child(
                div()
                    .px_3()
                    .py_1()
                    .bg(rgb(0x2d2d2d))
                    .text_color(rgb(0x9a9a9a))
                    .child(SharedString::from("Recent Files")),
            );

        if self.recent.is_empty() {
            list = list.child(
                div()
                    .px_3()
                    .py_2()
                    .text_color(rgb(0x8a8a8a))
                    .child(SharedString::from("No recent files")),
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
                    .hover(|s| s.bg(rgb(0x094771)))
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
        TitleBar::new().child(
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
                        .child(SharedString::from("CyberEditor")),
                )
                .child(div().flex_none().child(menu_bar)),
        )
    }

}
