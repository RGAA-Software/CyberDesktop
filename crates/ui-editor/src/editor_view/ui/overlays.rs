//! UI fragment: `ui/overlays.rs`.

use super::super::imports::*;

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

fn shortcut_list(cx: &App) -> impl IntoElement {
    let mut list = v_flex().w_full().gap_1();
    for (label, keys) in SHORTCUTS {
        if keys.is_empty() {
            list = list.child(
                Label::new(*label)
                    .mt_2()
                    .text_sm()
                    .text_color(cx.theme().accent_foreground),
            );
        } else {
            list = list.child(
                h_flex()
                    .w_full()
                    .items_center()
                    .justify_between()
                    .gap_4()
                    .child(Label::new(*label).text_sm())
                    .child(
                        Label::new(*keys)
                            .text_xs()
                            .text_color(cx.theme().muted_foreground),
                    ),
            );
        }
    }
    list
}

impl EngineEditor {
    pub(crate) fn render_about(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        if !self.show_about {
            return None;
        }

        let weak = cx.weak_entity();
        Some(
            Dialog::new(cx)
                .width(px(400.0))
                .title("About CyberEditor")
                .overlay(true)
                .overlay_closable(true)
                .keyboard(true)
                .on_cancel(move |_: &ClickEvent, _w, cx| {
                    let _ = weak.update(cx, |this, cx| {
                        this.show_about = false;
                        cx.notify();
                    });
                    true
                })
                .child(
                    v_flex()
                        .gap_2()
                        .child(
                            Label::new("High-performance text & code editor")
                                .text_sm()
                                .text_color(cx.theme().muted_foreground),
                        )
                        .child(
                            Label::new("Rust · GPUI · rope text engine")
                                .text_sm()
                                .text_color(cx.theme().muted_foreground),
                        ),
                )
                .footer(
                    DialogFooter::new().child(
                        Button::new("about-close")
                            .label("Close")
                            .on_click(cx.listener(|this, _: &ClickEvent, _w, cx| {
                                this.show_about = false;
                                cx.notify();
                            })),
                    ),
                ),
        )
    }

    pub(crate) fn render_shortcuts(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        if !self.show_shortcuts {
            return None;
        }

        let weak = cx.weak_entity();
        Some(
            Dialog::new(cx)
                .width(px(680.0))
                .title("Keyboard Shortcuts")
                .overlay(true)
                .overlay_closable(true)
                .keyboard(true)
                .on_cancel(move |_: &ClickEvent, _w, cx| {
                    let _ = weak.update(cx, |this, cx| {
                        this.show_shortcuts = false;
                        cx.notify();
                    });
                    true
                })
                .child(
                    div()
                        .max_h(px(560.0))
                        .overflow_y_scrollbar()
                        .child(shortcut_list(cx)),
                )
                .footer(
                    DialogFooter::new().child(
                        Button::new("shortcuts-close")
                            .label("Close")
                            .on_click(cx.listener(|this, _: &ClickEvent, _w, cx| {
                                this.show_shortcuts = false;
                                cx.notify();
                            })),
                    ),
                ),
        )
    }

    pub(crate) fn render_close_confirm(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let target = self.pending_close?;
        let message = match target {
            CloseTarget::Tab(i) => {
                format!("Save changes to \u{201c}{}\u{201d} before closing?", self.tab_name(i))
            }
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

        let weak = cx.weak_entity();
        Some(
            Dialog::new(cx)
                .width(px(480.0))
                .title("Unsaved Changes")
                .overlay(true)
                .overlay_closable(false)
                .keyboard(true)
                .on_cancel(move |_: &ClickEvent, _w, cx| {
                    let _ = weak.update(cx, |this, cx| {
                        this.close_confirm_cancel(cx);
                    });
                    true
                })
                .child(
                    Label::new(message)
                        .text_sm()
                        .text_color(cx.theme().foreground),
                )
                .footer(
                    DialogFooter::new()
                        .child(
                            Button::new("close-cancel")
                                .ghost()
                                .label("Cancel")
                                .on_click(cx.listener(|this, _: &ClickEvent, _w, cx| {
                                    this.close_confirm_cancel(cx);
                                })),
                        )
                        .child(
                            Button::new("close-discard")
                                .ghost()
                                .label("Don't Save")
                                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                    this.close_confirm_discard(window, cx);
                                })),
                        )
                        .child(
                            Button::new("close-save")
                                .primary()
                                .label(save_label)
                                .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                    this.close_confirm_save(window, cx);
                                })),
                        ),
                ),
        )
    }
}
