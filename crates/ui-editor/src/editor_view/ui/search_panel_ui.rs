//! UI fragment: `ui/search_panel_ui.rs`.

use super::icons::{paths, toolbar_icon, toolbar_icon_button};
use super::panel::panel_title_bar;
use super::widgets::{panel_close_button, panel_input, panel_tool_lead, panel_tool_strip};
use super::super::imports::*;
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

        let opt_btn = |id: &'static str, path: &'static str, active: bool, tip: String| {
            toolbar_icon_button(id)
                .icon(toolbar_icon(IconName::Search).path(path))
                .selected(active)
                .tooltip(tip)
        };

        let search_tools = panel_tool_strip()
            .child(
                toolbar_icon_button("file-search-go")
                    .icon(toolbar_icon(IconName::Search).path(paths::SEARCH))
                    .tooltip(t!("editor.search_in_file.action"))
                    .on_click(
                        cx.listener(|this, _: &ClickEvent, _w, cx| this.run_find_in_file(cx)),
                    ),
            )
            .child(
                opt_btn(
                    "file-search-case",
                    paths::MATCH_CASE,
                    panel.case_sensitive,
                    t!("editor.find.match_case").to_string(),
                )
                .on_click(cx.listener(|this, _: &ClickEvent, _w, cx| {
                    if let Some(p) = this.search_panel.as_mut() {
                        p.case_sensitive = !p.case_sensitive;
                    }
                    cx.notify();
                })),
            )
            .child(
                opt_btn(
                    "file-search-word",
                    paths::MATCH_WORD,
                    panel.whole_word,
                    t!("editor.find.whole_word").to_string(),
                )
                .on_click(cx.listener(|this, _: &ClickEvent, _w, cx| {
                    if let Some(p) = this.search_panel.as_mut() {
                        p.whole_word = !p.whole_word;
                    }
                    cx.notify();
                })),
            )
            .child(
                opt_btn(
                    "file-search-regex",
                    paths::REGEX,
                    panel.regex,
                    t!("editor.find.regex").to_string(),
                )
                .on_click(cx.listener(|this, _: &ClickEvent, _w, cx| {
                    if let Some(p) = this.search_panel.as_mut() {
                        p.regex = !p.regex;
                    }
                    cx.notify();
                })),
            );

        let close = panel_close_button("file-search-close")
            .icon(toolbar_icon(IconName::Close).path(paths::CLOSE))
            .tooltip(t!("editor.find.close"))
            .on_click(cx.listener(|this, _: &ClickEvent, _w, cx| this.close_search_panel(cx)));

        let scope_label = panel
            .scope_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| t!("editor.untitled").to_string());
        let scope_hint = t!("editor.search_in_file.scope", name = scope_label);

        let header = v_flex()
            .w_full()
            .gap_2()
            .child(panel_input(&panel.query))
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .child(panel_tool_lead())
                    .child(search_tools),
            )
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

        let pos = self.resolved_panel_origin(FloatingPanel::SearchInFile);
        let panel_size = self.resolved_search_panel_size();
        let resize_hit = PANEL_RESIZE_HANDLE;

        let resize_start = |edge: PanelResizeEdge| {
            cx.listener(move |this, event: &MouseDownEvent, _window, cx| {
                this.start_search_panel_resize(edge, event, cx);
                cx.stop_propagation();
            })
        };

        Some(
            div()
                .absolute()
                .left(pos.x)
                .top(pos.y)
                .w(panel_size.width)
                .h(panel_size.height)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
                .child(
                    v_flex()
                        .id("find-in-file-panel")
                        .size_full()
                        .popover_style(cx)
                        .shadow_xl()
                        .overflow_hidden()
                        .child(panel_title_bar(
                            cx,
                            t!("editor.search_in_file.title"),
                            close,
                            cx.listener(|this, event: &MouseDownEvent, _window, cx| {
                                this.start_panel_drag(FloatingPanel::SearchInFile, event, cx);
                            }),
                        ))
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
                )
                .child(
                    div()
                        .id("file-search-resize-right")
                        .absolute()
                        .top_0()
                        .right_0()
                        .bottom(resize_hit)
                        .w(resize_hit)
                        .cursor_col_resize()
                        .on_mouse_down(MouseButton::Left, resize_start(PanelResizeEdge::Right)),
                )
                .child(
                    div()
                        .id("file-search-resize-bottom")
                        .absolute()
                        .left_0()
                        .right_0()
                        .bottom_0()
                        .h(resize_hit)
                        .cursor_row_resize()
                        .on_mouse_down(MouseButton::Left, resize_start(PanelResizeEdge::Bottom)),
                )
                .child(
                    div()
                        .id("file-search-resize-corner")
                        .absolute()
                        .right_0()
                        .bottom_0()
                        .size(resize_hit)
                        .cursor_nwse_resize()
                        .on_mouse_down(MouseButton::Left, resize_start(PanelResizeEdge::BottomRight)),
                ),
        )
    }
}
