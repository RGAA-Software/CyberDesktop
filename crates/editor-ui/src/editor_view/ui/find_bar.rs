//! UI fragment: `ui/find_bar.rs`.

use super::super::imports::*;
use super::icons::{paths, toolbar_icon, toolbar_icon_button};
use super::panel::floating_tool_panel;
use super::widgets::{panel_close_button, panel_input, panel_tool_lead, panel_tool_strip};
use gpui_component::IconName;

impl EngineEditor {
    pub(crate) fn render_find_bar(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let find = self.find.as_ref()?;
        let replace_mode = find.replace_mode;

        let opt_btn = |id: &'static str, path: &'static str, active: bool, tip: String| {
            toolbar_icon_button(id)
                .icon(toolbar_icon(IconName::Search).path(path))
                .selected(active)
                .tooltip(tip)
        };

        let find_tools = panel_tool_strip()
            .child(
                toolbar_icon_button("find-count")
                    .icon(toolbar_icon(IconName::Search).path(paths::COUNT))
                    .tooltip(t!("editor.find.count"))
                    .on_click(cx.listener(|this, _: &ClickEvent, _w, cx| this.do_count(cx))),
            )
            .child(
                opt_btn(
                    "find-prev",
                    paths::FIND_PREV,
                    false,
                    t!("editor.find.prev").to_string(),
                )
                .on_click(cx.listener(|this, _: &ClickEvent, _w, cx| this.do_find(false, cx))),
            )
            .child(
                opt_btn(
                    "find-next",
                    paths::FIND_NEXT,
                    false,
                    t!("editor.find.next").to_string(),
                )
                .on_click(cx.listener(|this, _: &ClickEvent, _w, cx| this.do_find(true, cx))),
            )
            .child(
                opt_btn(
                    "find-case",
                    paths::MATCH_CASE,
                    find.case_sensitive,
                    t!("editor.find.match_case").to_string(),
                )
                .on_click(cx.listener(|this, _: &ClickEvent, _w, cx| {
                    if let Some(f) = this.find.as_mut() {
                        f.case_sensitive = !f.case_sensitive;
                        f.cached_searcher = None;
                    }
                    cx.notify();
                })),
            )
            .child(
                opt_btn(
                    "find-word",
                    paths::MATCH_WORD,
                    find.whole_word,
                    t!("editor.find.whole_word").to_string(),
                )
                .on_click(cx.listener(|this, _: &ClickEvent, _w, cx| {
                    if let Some(f) = this.find.as_mut() {
                        f.whole_word = !f.whole_word;
                        f.cached_searcher = None;
                    }
                    cx.notify();
                })),
            )
            .child(
                opt_btn(
                    "find-regex",
                    paths::REGEX,
                    find.regex,
                    t!("editor.find.regex").to_string(),
                )
                .on_click(cx.listener(|this, _: &ClickEvent, _w, cx| {
                    if let Some(f) = this.find.as_mut() {
                        f.regex = !f.regex;
                        f.cached_searcher = None;
                    }
                    cx.notify();
                })),
            );

        let find_row = v_flex()
            .w_full()
            .gap_2()
            .child(panel_input(&find.query))
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .child(
                        Label::new(find.status.clone())
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .flex_1()
                            .min_w_0()
                            .truncate(),
                    )
                    .child(find_tools),
            );

        let close = panel_close_button("find-close")
            .icon(toolbar_icon(IconName::Close).path(paths::CLOSE))
            .tooltip(t!("editor.find.close"))
            .on_click(cx.listener(|this, _: &ClickEvent, _w, cx| this.close_find(cx)));

        let mut body = v_flex().w_full().gap_2().child(find_row);

        if replace_mode {
            let replace_tools = panel_tool_strip()
                .child(
                    toolbar_icon_button("replace-one")
                        .icon(toolbar_icon(IconName::Replace).path(paths::REPLACE))
                        .tooltip(t!("editor.find.replace"))
                        .on_click(cx.listener(|this, _: &ClickEvent, _w, cx| this.do_replace(cx))),
                )
                .child(
                    toolbar_icon_button("replace-all")
                        .icon(toolbar_icon(IconName::Replace).path(paths::REPLACE_ALL))
                        .tooltip(t!("editor.find.replace_all"))
                        .on_click(
                            cx.listener(|this, _: &ClickEvent, _w, cx| this.do_replace_all(cx)),
                        ),
                );

            let replace_row = v_flex()
                .w_full()
                .gap_2()
                .child(panel_input(&find.replace))
                .child(
                    h_flex()
                        .w_full()
                        .items_center()
                        .child(panel_tool_lead())
                        .child(replace_tools),
                );

            body = body.child(replace_row);
        }

        let title = if replace_mode {
            t!("editor.find.replace_title")
        } else {
            t!("editor.find.title")
        };

        let pos = self.resolved_panel_origin(FloatingPanel::Find);

        Some(
            div()
                .absolute()
                .left(pos.x)
                .top(pos.y)
                .w(FIND_PANEL_WIDTH)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(floating_tool_panel(
                    cx,
                    "find-bar",
                    title,
                    close,
                    cx.listener(|this, event: &MouseDownEvent, _window, cx| {
                        this.start_panel_drag(FloatingPanel::Find, event, cx);
                    }),
                    body,
                )),
        )
    }
}
