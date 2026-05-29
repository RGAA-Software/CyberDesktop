//! UI fragment: `ui/overlays.rs`.

use super::super::imports::*;
use super::widgets::bar_button;

impl EngineEditor {
    pub(crate) fn render_about(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        if !self.show_about {
            return None;
        }
        let panel = div()
            .w(px(360.0))
            .flex()
            .flex_col()
            .gap_2()
            .p_4()
            .rounded_lg()
            .bg(rgb(0x252526))
            .border_1()
            .border_color(rgb(0x454545))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(div().text_size(px(16.0)).child(SharedString::from("CyberEditor")))
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(rgb(0x9a9a9a))
                    .child(SharedString::from("High-performance text & code editor")),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(rgb(0x9a9a9a))
                    .child(SharedString::from("Rust · GPUI · rope text engine")),
            )
            .child(
                bar_button("about-close", "Close", false).on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _e: &MouseDownEvent, _w, cx| {
                        this.show_about = false;
                        cx.stop_propagation();
                        cx.notify();
                    }),
                ),
            );

        Some(
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(gpui::rgba(0x00000080))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _e: &MouseDownEvent, _w, cx| {
                        this.show_about = false;
                        cx.notify();
                    }),
                )
                .child(panel),
        )
    }

    /// Keyboard-shortcuts reference overlay (Help → Keyboard Shortcuts).
    pub(crate) fn render_shortcuts(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        if !self.show_shortcuts {
            return None;
        }
        const SHORTCUTS: &[(&str, &str)] = &[
            ("File", ""),
            ("New tab", "Ctrl+N / Ctrl+T"),
            ("Open file", "Ctrl+O"),
            ("Save / Save As", "Ctrl+S / Ctrl+Shift+S"),
            ("Close tab", "Ctrl+W"),
            ("Next / Prev tab", "Ctrl+Tab / Ctrl+Shift+Tab"),
            ("Recent files", "Ctrl+E"),
            ("Edit", ""),
            ("Undo / Redo", "Ctrl+Z / Ctrl+Y"),
            ("Cut / Copy / Paste", "Ctrl+X / Ctrl+C / Ctrl+V"),
            ("Select all / line", "Ctrl+A / Ctrl+L"),
            ("Indent / Outdent", "Alt+] / Alt+["),
            ("Toggle comment", "Ctrl+/"),
            ("Zoom in / out / reset", "Ctrl+= / Ctrl+- / Ctrl+0"),
            ("Search & navigate", ""),
            ("Find / Replace", "Ctrl+F / Ctrl+H"),
            ("Find next / prev", "F3 / Shift+F3"),
            ("Find in file", "Ctrl+Shift+F"),
            ("Go to line", "Ctrl+G"),
            ("Add next occurrence", "Ctrl+D"),
            ("Add caret (mouse)", "Alt+Click"),
            ("Select word / line", "Double / Triple click"),
            ("Page up / down", "PageUp / PageDown"),
            ("View", ""),
            ("Word wrap", "Menu: View → Word Wrap"),
            ("Line numbers", "Menu: View → Line Numbers"),
        ];

        let mut list = div().flex().flex_col().gap_0p5();
        for (label, keys) in SHORTCUTS {
            if keys.is_empty() {
                // Section header.
                list = list.child(
                    div()
                        .mt_2()
                        .text_size(px(12.0))
                        .text_color(rgb(0x6fb3d2))
                        .child(SharedString::from(*label)),
                );
            } else {
                list = list.child(
                    div()
                        .flex()
                        .flex_row()
                        .justify_between()
                        .gap_4()
                        .text_size(px(12.0))
                        .child(
                            div()
                                .text_color(rgb(0xcccccc))
                                .child(SharedString::from(*label)),
                        )
                        .child(
                            div()
                                .text_color(rgb(0x9a9a9a))
                                .child(SharedString::from(*keys)),
                        ),
                );
            }
        }

        let panel = div()
            .w(px(440.0))
            .max_h(px(560.0))
            .flex()
            .flex_col()
            .gap_2()
            .p_4()
            .rounded_lg()
            .bg(rgb(0x252526))
            .border_1()
            .border_color(rgb(0x454545))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .text_size(px(16.0))
                    .child(SharedString::from("Keyboard Shortcuts")),
            )
            .child(div().overflow_hidden().child(list))
            .child(
                bar_button("shortcuts-close", "Close", false).on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _e: &MouseDownEvent, _w, cx| {
                        this.show_shortcuts = false;
                        cx.stop_propagation();
                        cx.notify();
                    }),
                ),
            );

        Some(
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(gpui::rgba(0x00000080))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _e: &MouseDownEvent, _w, cx| {
                        this.show_shortcuts = false;
                        cx.notify();
                    }),
                )
                .child(panel),
        )
    }

    /// Unsaved-changes confirmation overlay (closing a tab or the window).
    pub(crate) fn render_close_confirm(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let target = self.pending_close?;
        let message = match target {
            CloseTarget::Tab(i) => format!("Save changes to \u{201c}{}\u{201d} before closing?", self.tab_name(i)),
            CloseTarget::Window => {
                let n = self.dirty_tabs().len();
                if n <= 1 {
                    "You have unsaved changes. Save before closing?".to_string()
                } else {
                    format!("{n} files have unsaved changes. Save them before closing?")
                }
            }
        };
        let save_label = if target == CloseTarget::Window && self.dirty_tabs().len() > 1 {
            "Save All"
        } else {
            "Save"
        };

        let panel = div()
            .w(px(400.0))
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .rounded_lg()
            .bg(rgb(0x252526))
            .border_1()
            .border_color(rgb(0x454545))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .text_size(px(15.0))
                    .child(SharedString::from("Unsaved Changes")),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(rgb(0xcccccc))
                    .child(SharedString::from(message)),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_end()
                    .gap_2()
                    .child(bar_button("close-cancel", "Cancel", false).on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _e: &MouseDownEvent, _w, cx| {
                            cx.stop_propagation();
                            this.close_confirm_cancel(cx);
                        }),
                    ))
                    .child(bar_button("close-discard", "Don't Save", false).on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _e: &MouseDownEvent, window, cx| {
                            cx.stop_propagation();
                            this.close_confirm_discard(window, cx);
                        }),
                    ))
                    .child(bar_button("close-save", save_label, true).on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _e: &MouseDownEvent, window, cx| {
                            cx.stop_propagation();
                            this.close_confirm_save(window, cx);
                        }),
                    )),
            );

        Some(
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(gpui::rgba(0x00000080))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(panel),
        )
    }

}
