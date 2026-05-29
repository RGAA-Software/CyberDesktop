//! UI fragment: `ui/search_panel_ui.rs`.

use super::icons::{paths, toolbar_icon, toolbar_icon_button};
use super::super::imports::*;
use gpui::FontWeight;
use gpui_component::{progress::Progress, IconName, StyledExt as _};

fn search_progress_value(lines_scanned: usize, searching: bool) -> f32 {
    if !searching {
        return 0.0;
    }
    if lines_scanned == 0 {
        return 5.0;
    }
    (90.0 * (1.0 - (-0.002 * lines_scanned as f32).exp())).min(90.0)
}

impl EngineEditor {
    pub(crate) fn render_search_panel(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let panel = self.search_panel.as_ref()?;

        let opt_btn = |id: &'static str, path: &'static str, active: bool, tip: &'static str| {
            toolbar_icon_button(id)
                .icon(toolbar_icon(IconName::Search).path(path))
                .selected(active)
                .tooltip(tip)
        };

        let controls = h_flex()
            .w_full()
            .items_center()
            .gap_2()
            .child(div().flex_1().child(Input::new(&panel.query).small()))
            .child(
                toolbar_icon_button("file-search-go")
                    .icon(toolbar_icon(IconName::Search).path(paths::SEARCH))
                    .tooltip("Search in current file")
                    .on_click(
                        cx.listener(|this, _: &ClickEvent, _w, cx| this.run_find_in_file(cx)),
                    ),
            )
            .child(
                opt_btn(
                    "file-search-case",
                    paths::MATCH_CASE,
                    panel.case_sensitive,
                    "Match case",
                )
                .on_click(cx.listener(|this, _: &ClickEvent, _w, cx| {
                    if let Some(p) = this.search_panel.as_mut() {
                        p.case_sensitive = !p.case_sensitive;
                    }
                    cx.notify();
                })),
            )
            .child(
                opt_btn("file-search-word", paths::MATCH_WORD, panel.whole_word, "Whole word")
                    .on_click(cx.listener(|this, _: &ClickEvent, _w, cx| {
                        if let Some(p) = this.search_panel.as_mut() {
                            p.whole_word = !p.whole_word;
                        }
                        cx.notify();
                    })),
            )
            .child(
                opt_btn("file-search-regex", paths::REGEX, panel.regex, "Regular expression")
                    .on_click(cx.listener(|this, _: &ClickEvent, _w, cx| {
                        if let Some(p) = this.search_panel.as_mut() {
                            p.regex = !p.regex;
                        }
                        cx.notify();
                    })),
            )
            .child(
                toolbar_icon_button("file-search-close")
                    .icon(toolbar_icon(IconName::Close).path(paths::CLOSE))
                    .tooltip("Close (Esc)")
                    .on_click(
                        cx.listener(|this, _: &ClickEvent, _w, cx| this.close_search_panel(cx)),
                    ),
            );

        let scope_label = panel
            .scope_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("(Untitled)");
        let scope_hint = format!("Current tab: {scope_label}");

        let header = v_flex()
            .w_full()
            .gap_2()
            .child(controls)
            .child(
                Label::new(scope_hint)
                    .text_xs()
                    .text_color(cx.theme().muted_foreground),
            )
            .child(
                Label::new(panel.status.clone())
                    .text_sm()
                    .text_color(cx.theme().muted_foreground),
            )
            .children(if panel.searching {
                vec![div()
                    .w_full()
                    .child(
                        Progress::new("file-search-progress")
                            .w_full()
                            .h(px(4.0))
                            .value(search_progress_value(panel.lines_scanned, true)),
                    )
                    .into_any_element()]
            } else {
                Vec::new()
            });

        let row_count = panel.rows.len();
        let item_sizes: Rc<Vec<Size<Pixels>>> =
            Rc::new(vec![size(px(1.0), px(20.0)); row_count.max(1)]);
        let list = v_virtual_list(
            cx.entity().clone(),
            "file-search-virtual-list",
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
                    let SearchRow::Match { path, line, text } = row.clone();
                    out.push(
                        div()
                            .id(("file-search-match-row", index))
                            .h(px(20.0))
                            .px_2()
                            .flex()
                            .items_center()
                            .hover(|s| s.bg(cx.theme().list_hover))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _e: &MouseDownEvent, _w, cx| {
                                    this.open_search_result(path.clone(), line, cx);
                                    cx.stop_propagation();
                                }),
                            )
                            .child(
                                Label::new(format!("{line:>5}: {text}"))
                                    .text_sm()
                                    .truncate(),
                            ),
                    );
                }
                out
            },
        )
        .track_scroll(&panel.scroll);

        let results_list = div()
            .id("file-search-results")
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
                .w(px(400.0))
                .p_2()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
                .child(
                    v_flex()
                        .id("find-in-file-panel")
                        .h_full()
                        .popover_style(cx)
                        .shadow_xl()
                        .overflow_hidden()
                        .child(
                            h_flex()
                                .px_3()
                                .py_2()
                                .items_center()
                                .border_b_1()
                                .border_color(cx.theme().border)
                                .child(
                                    Label::new("Find in File")
                                        .text_sm()
                                        .font_weight(FontWeight::SEMIBOLD),
                                ),
                        )
                        .child(
                            v_flex()
                                .flex_1()
                                .min_h_0()
                                .p_3()
                                .gap_3()
                                .child(header)
                                .child(Separator::horizontal().w_full())
                                .child(results_list),
                        ),
                ),
        )
    }
}
