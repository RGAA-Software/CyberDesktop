//! UI fragment: `ui/find_bar.rs`.

use super::super::imports::*;

impl EngineEditor {
    pub(crate) fn render_find_bar(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let find = self.find.as_ref()?;
        let replace_mode = find.replace_mode;

        let query_field = div().w(px(200.0)).child(Input::new(&find.query).small());

        let status = div()
            .min_w(px(64.0))
            .text_size(px(11.0))
            .text_color(rgb(0x9a9a9a))
            .child(SharedString::from(find.status.clone()));

        let opt_btn = |id: &'static str, label: &str, active: bool, tip: &'static str| {
            Button::new(id)
                .ghost()
                .xsmall()
                .selected(active)
                .label(label.to_string())
                .tooltip(tip)
        };

        let find_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_1()
            .child(query_field)
            .child(status)
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

        let mut bar = div()
            .absolute()
            .top_2()
            .right_4()
            .flex()
            .flex_col()
            .gap_1()
            .p_2()
            .rounded_md()
            .bg(rgb(0x2d2d30))
            .border_1()
            .border_color(rgb(0x454545))
            .text_color(rgb(0xcccccc))
            .text_size(px(12.0))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(find_row);

        if replace_mode {
            let replace_field = div().w(px(200.0)).child(Input::new(&find.replace).small());
            let replace_row = div()
                .flex()
                .flex_row()
                .items_center()
                .gap_1()
                .child(replace_field)
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
            bar = bar.child(replace_row);
        }

        Some(bar)
    }
}
