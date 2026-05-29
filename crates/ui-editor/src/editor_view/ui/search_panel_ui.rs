//! UI fragment: `ui/search_panel_ui.rs`.

use super::super::imports::*;

impl EngineEditor {
    pub(crate) fn render_search_panel(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let panel = self.search_panel.as_ref()?;

        let opt_btn = |id: &'static str, label: &str, active: bool, tip: &'static str| {
            Button::new(id)
                .ghost()
                .xsmall()
                .selected(active)
                .label(label.to_string())
                .tooltip(tip)
        };

        let controls = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_1()
            .child(div().flex_1().child(Input::new(&panel.query).small()))
            .child(
                Button::new("files-go")
                    .primary()
                    .xsmall()
                    .label("Search")
                    .on_click(
                        cx.listener(|this, _: &ClickEvent, _w, cx| this.run_global_search(cx)),
                    ),
            )
            .child(
                opt_btn("files-case", "Aa", panel.case_sensitive, "Match case").on_click(
                    cx.listener(|this, _: &ClickEvent, _w, cx| {
                        if let Some(p) = this.search_panel.as_mut() {
                            p.case_sensitive = !p.case_sensitive;
                        }
                        cx.notify();
                    }),
                ),
            )
            .child(
                opt_btn("files-word", "W", panel.whole_word, "Whole word").on_click(cx.listener(
                    |this, _: &ClickEvent, _w, cx| {
                        if let Some(p) = this.search_panel.as_mut() {
                            p.whole_word = !p.whole_word;
                        }
                        cx.notify();
                    },
                )),
            )
            .child(
                opt_btn("files-regex", ".*", panel.regex, "Regular expression").on_click(
                    cx.listener(|this, _: &ClickEvent, _w, cx| {
                        if let Some(p) = this.search_panel.as_mut() {
                            p.regex = !p.regex;
                        }
                        cx.notify();
                    }),
                ),
            )
            .child(
                Button::new("files-close")
                    .ghost()
                    .xsmall()
                    .label("\u{2715}")
                    .tooltip("Close (Esc)")
                    .on_click(
                        cx.listener(|this, _: &ClickEvent, _w, cx| this.close_search_panel(cx)),
                    ),
            );

        let root_label = panel.root.display().to_string();
        let header = div()
            .flex()
            .flex_col()
            .gap_1()
            .p_2()
            .border_b_1()
            .border_color(rgb(0x3a3a3a))
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(rgb(0xcccccc))
                    .child(SharedString::from("Find in Files")),
            )
            .child(controls)
            .child(
                div()
                    .text_size(px(10.0))
                    .text_color(rgb(0x6a6a6a))
                    .child(SharedString::from(format!("in {root_label}"))),
            )
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(rgb(0x9a9a9a))
                    .child(SharedString::from(panel.status.clone())),
            );

        // High-performance virtualized results list: only visible rows render.
        let row_count = panel.rows.len();
        let item_sizes: Rc<Vec<Size<Pixels>>> =
            Rc::new(vec![size(px(1.0), px(20.0)); row_count.max(1)]);
        let list = v_virtual_list(
            cx.entity().clone(),
            "files-virtual-list",
            item_sizes,
            move |this, range, _window, cx| {
                let Some(panel) = this.search_panel.as_ref() else {
                    return Vec::new();
                };
                let mut out = Vec::new();
                for index in range {
                    let Some(row) = panel.rows.get(index) else {
                        continue;
                    };
                    out.push(match row.clone() {
                        SearchRow::File { label, count } => div()
                            .id(("files-file-row", index))
                            .h(px(20.0))
                            .px_2()
                            .flex()
                            .items_center()
                            .text_size(px(11.0))
                            .text_color(rgb(0x7fb0e0))
                            .child(SharedString::from(format!("{label}  ({count})"))),
                        SearchRow::Match { path, line, text } => div()
                            .id(("files-match-row", index))
                            .h(px(20.0))
                            .px_2()
                            .pl_4()
                            .flex()
                            .items_center()
                            .text_size(px(12.0))
                            .text_color(rgb(0xd4d4d4))
                            .hover(|s| s.bg(rgb(0x094771)))
                            .child(SharedString::from(format!(
                                "{line:>5}: {text}"
                            )))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _e: &MouseDownEvent, _w, cx| {
                                    this.open_search_result(path.clone(), line, cx);
                                    cx.stop_propagation();
                                }),
                            ),
                    });
                }
                out
            },
        )
        .track_scroll(&panel.scroll);

        let results_list = div()
            .id("files-results")
            .flex_1()
            .min_h_0()
            .child(list)
            .scrollbar(&panel.scroll, ScrollbarAxis::Vertical);

        Some(
            div()
                .absolute()
                .top_0()
                .right_0()
                .bottom_0()
                .w(px(380.0))
                .flex()
                .flex_col()
                .bg(rgb(0x252526))
                .border_l_1()
                .border_color(rgb(0x3a3a3a))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
                .child(header)
                .child(results_list),
        )
    }

}
