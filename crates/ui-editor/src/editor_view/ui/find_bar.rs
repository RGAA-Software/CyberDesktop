//! UI fragment: `ui/find_bar.rs`.

use super::super::imports::*;

impl EngineEditor {
    pub(crate) fn render_find_bar(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let find = self.find.as_ref()?;
        let replace_mode = find.replace_mode;

        let opt_btn = |id: &'static str, label: &str, active: bool, tip: &'static str| {
            Button::new(id)
                .ghost()
                .xsmall()
                .selected(active)
                .label(label.to_string())
                .tooltip(tip)
        };

        let find_row = h_flex()
            .w_full()
            .items_center()
            .gap_2()
            .child(div().w(px(220.0)).child(Input::new(&find.query).small()))
            .child(
                Label::new(find.status.clone())
                    .text_xs()
                    .text_color(cx.theme().muted_foreground),
            )
            .child(
                Button::new("find-count")
                    .xsmall()
                    .label("Count")
                    .tooltip("Count all matches in document")
                    .on_click(cx.listener(|this, _: &ClickEvent, _w, cx| this.do_count(cx))),
            )
            .child(opt_btn("find-prev", "\u{2191}", false, "Find previous (Shift+F3)").on_click(
                cx.listener(|this, _: &ClickEvent, _w, cx| this.do_find(false, cx)),
            ))
            .child(opt_btn("find-next", "\u{2193}", false, "Find next (F3)").on_click(
                cx.listener(|this, _: &ClickEvent, _w, cx| this.do_find(true, cx)),
            ))
            .child(
                opt_btn("find-case", "Aa", find.case_sensitive, "Match case").on_click(
                    cx.listener(|this, _: &ClickEvent, _w, cx| {
                        if let Some(f) = this.find.as_mut() {
                            f.case_sensitive = !f.case_sensitive;
                            f.cached_searcher = None;
                        }
                        cx.notify();
                    }),
                ),
            )
            .child(
                opt_btn("find-word", "W", find.whole_word, "Whole word").on_click(cx.listener(
                    |this, _: &ClickEvent, _w, cx| {
                        if let Some(f) = this.find.as_mut() {
                            f.whole_word = !f.whole_word;
                            f.cached_searcher = None;
                        }
                        cx.notify();
                    },
                )),
            )
            .child(
                opt_btn("find-regex", ".*", find.regex, "Regular expression").on_click(cx.listener(
                    |this, _: &ClickEvent, _w, cx| {
                        if let Some(f) = this.find.as_mut() {
                            f.regex = !f.regex;
                            f.cached_searcher = None;
                        }
                        cx.notify();
                    },
                )),
            )
            .child(
                Button::new("find-close")
                    .ghost()
                    .xsmall()
                    .label("\u{2715}")
                    .tooltip("Close (Esc)")
                    .on_click(cx.listener(|this, _: &ClickEvent, _w, cx| this.close_find(cx))),
            );

        let mut body = v_flex().w_full().gap_2().child(find_row);

        if replace_mode {
            let replace_row = h_flex()
                .w_full()
                .items_center()
                .gap_2()
                .child(div().w(px(220.0)).child(Input::new(&find.replace).small()))
                .child(
                    Button::new("replace-one")
                        .xsmall()
                        .label("Replace")
                        .on_click(
                            cx.listener(|this, _: &ClickEvent, _w, cx| this.do_replace(cx)),
                        ),
                )
                .child(
                    Button::new("replace-all")
                        .xsmall()
                        .label("Replace All")
                        .on_click(
                            cx.listener(|this, _: &ClickEvent, _w, cx| this.do_replace_all(cx)),
                        ),
                );
            body = body
                .child(Separator::horizontal().w_full())
                .child(replace_row);
        }

        let title = if replace_mode {
            "Find and Replace"
        } else {
            "Find"
        };

        Some(
            div()
                .absolute()
                .top_2()
                .right_4()
                .w(px(520.0))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    GroupBox::new()
                        .id("find-bar")
                        .outline()
                        .title(title)
                        .child(body),
                ),
        )
    }
}
