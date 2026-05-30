//! UI fragment: `ui/overlays.rs`.

use super::super::imports::*;

fn shortcut_entries() -> Vec<(String, Option<String>)> {
    vec![
        (t!("editor.shortcuts.section.file").to_string(), None),
        (
            t!("editor.shortcuts.new_tab").to_string(),
            Some("Ctrl+N / Ctrl+T".into()),
        ),
        (
            t!("editor.shortcuts.open_file").to_string(),
            Some("Ctrl+O".into()),
        ),
        (
            t!("editor.shortcuts.save").to_string(),
            Some("Ctrl+S / Ctrl+Shift+S".into()),
        ),
        (
            t!("editor.shortcuts.close_tab").to_string(),
            Some("Ctrl+W".into()),
        ),
        (
            t!("editor.shortcuts.next_tab").to_string(),
            Some("Ctrl+Tab / Ctrl+Shift+Tab".into()),
        ),
        (
            t!("editor.shortcuts.recent").to_string(),
            Some("Ctrl+E".into()),
        ),
        (t!("editor.shortcuts.section.edit").to_string(), None),
        (
            t!("editor.shortcuts.undo_redo").to_string(),
            Some("Ctrl+Z / Ctrl+Y".into()),
        ),
        (
            t!("editor.shortcuts.clipboard").to_string(),
            Some("Ctrl+X / Ctrl+C / Ctrl+V".into()),
        ),
        (
            t!("editor.shortcuts.select").to_string(),
            Some("Ctrl+A / Ctrl+L".into()),
        ),
        (
            t!("editor.shortcuts.indent").to_string(),
            Some("Alt+] / Alt+[".into()),
        ),
        (
            t!("editor.shortcuts.comment").to_string(),
            Some("Ctrl+/".into()),
        ),
        (
            t!("editor.shortcuts.zoom").to_string(),
            Some("Ctrl+= / Ctrl+- / Ctrl+0".into()),
        ),
        (t!("editor.shortcuts.section.search").to_string(), None),
        (
            t!("editor.shortcuts.find_replace").to_string(),
            Some("Ctrl+F / Ctrl+H".into()),
        ),
        (
            t!("editor.shortcuts.find_next").to_string(),
            Some("F3 / Shift+F3".into()),
        ),
        (
            t!("editor.shortcuts.find_in_file").to_string(),
            Some("Ctrl+Shift+F".into()),
        ),
        (
            t!("editor.shortcuts.go_to_line").to_string(),
            Some("Ctrl+G".into()),
        ),
        (
            t!("editor.shortcuts.add_occurrence").to_string(),
            Some("Ctrl+D".into()),
        ),
        (
            t!("editor.shortcuts.add_caret").to_string(),
            Some("Alt+Click".into()),
        ),
        (
            t!("editor.shortcuts.select_word").to_string(),
            Some("Double / Triple click".into()),
        ),
        (
            t!("editor.shortcuts.page").to_string(),
            Some("PageUp / PageDown".into()),
        ),
        (t!("editor.shortcuts.section.view").to_string(), None),
        (
            t!("editor.shortcuts.line_numbers").to_string(),
            Some("Alt+L".into()),
        ),
        (
            t!("editor.shortcuts.word_wrap").to_string(),
            Some("Alt+Z".into()),
        ),
    ]
}

/// Split before the "Search & navigate" section header.
const SHORTCUTS_SPLIT: usize = 15;

fn shortcut_list(cx: &App, entries: &[(String, Option<String>)]) -> impl IntoElement {
    let mut list = v_flex().w_full().gap_1();
    for (label, keys) in entries {
        if keys.is_none() {
            list = list.child(
                Label::new(label.clone())
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
                    .child(Label::new(label.clone()).text_sm())
                    .child(
                        Label::new(keys.clone().unwrap_or_default())
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
                .title(t!("editor.about.title"))
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
                            Label::new(t!("editor.about.tagline"))
                                .text_sm()
                                .text_color(cx.theme().muted_foreground),
                        )
                        .child(
                            Label::new(t!("editor.about.stack"))
                                .text_sm()
                                .text_color(cx.theme().muted_foreground),
                        ),
                )
                .footer(
                    DialogFooter::new().child(
                        Button::new("about-close")
                            .label(t!("editor.close"))
                            .on_click(cx.listener(|this, _: &ClickEvent, _w, cx| {
                                this.show_about = false;
                                cx.notify();
                            })),
                    ),
                ),
        )
    }

    pub(crate) fn render_settings(
        &self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        if !self.show_settings {
            return None;
        }

        let view = window.viewport_size();
        let dialog_w = (view.width * 0.88).clamp(px(780.0), px(1024.0));
        let content_h = (view.height * 0.72).clamp(px(520.0), px(760.0));

        let weak = cx.weak_entity();
        Some(
            Dialog::new(cx)
                .width(dialog_w)
                .h(content_h + px(96.0))
                .title(t!("nav.settings"))
                .overlay(true)
                .overlay_closable(true)
                .keyboard(true)
                .on_cancel(move |_: &ClickEvent, _w, cx| {
                    let _ = weak.update(cx, |this, cx| {
                        this.show_settings = false;
                        cx.notify();
                    });
                    true
                })
                .child(
                    div()
                        .w_full()
                        .h(content_h)
                        .min_h_0()
                        .child(build_editor_settings(cx)),
                )
                .footer(
                    DialogFooter::new().child(
                        Button::new("settings-close")
                            .label(t!("editor.close"))
                            .on_click(cx.listener(|this, _: &ClickEvent, _w, cx| {
                                this.show_settings = false;
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
        let entries = shortcut_entries();
        let (left, right) = entries.split_at(SHORTCUTS_SPLIT);
        Some(
            Dialog::new(cx)
                .width(px(820.0))
                .title(t!("editor.shortcuts.title"))
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
                    h_flex()
                        .w_full()
                        .gap_8()
                        .items_start()
                        .child(div().flex_1().child(shortcut_list(cx, left)))
                        .child(div().flex_1().child(shortcut_list(cx, right))),
                )
                .footer(
                    DialogFooter::new().child(
                        Button::new("shortcuts-close")
                            .label(t!("editor.close"))
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
            CloseTarget::Tab(i) => t!(
                "editor.unsaved.tab",
                name = self.tab_name(i)
            )
            .to_string(),
            CloseTarget::Window => {
                let n = self.dirty_tabs().len();
                if n <= 1 {
                    t!("editor.unsaved.single").to_string()
                } else {
                    t!("editor.unsaved.multiple", count = n).to_string()
                }
            }
        };
        let save_label = if target == CloseTarget::Window && self.dirty_tabs().len() > 1 {
            t!("editor.save_all")
        } else {
            t!("editor.save")
        };

        let weak = cx.weak_entity();
        Some(
            Dialog::new(cx)
                .width(px(480.0))
                .title(t!("editor.unsaved.title"))
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
                                .label(t!("files.cancel"))
                                .on_click(cx.listener(|this, _: &ClickEvent, _w, cx| {
                                    this.close_confirm_cancel(cx);
                                })),
                        )
                        .child(
                            Button::new("close-discard")
                                .ghost()
                                .label(t!("editor.dont_save"))
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
