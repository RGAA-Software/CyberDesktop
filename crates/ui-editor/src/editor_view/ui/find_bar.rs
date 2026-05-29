//! UI fragment: `ui/find_bar.rs`.

use super::icons::{paths, toolbar_icon, toolbar_icon_button};
use super::panel::floating_tool_panel;
use super::super::imports::*;
use gpui_component::IconName;

impl EngineEditor {
    pub(crate) fn render_find_bar(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let find = self.find.as_ref()?;
        let replace_mode = find.replace_mode;

        let opt_btn = |id: &'static str, path: &'static str, active: bool, tip: &'static str| {
            toolbar_icon_button(id)
                .icon(toolbar_icon(IconName::Search).path(path))
                .selected(active)
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
                toolbar_icon_button("find-count")
                    .icon(toolbar_icon(IconName::Search).path(paths::COUNT))
                    .tooltip("Count all matches in document")
                    .on_click(cx.listener(|this, _: &ClickEvent, _w, cx| this.do_count(cx))),
            )
            .child(
                opt_btn("find-prev", paths::FIND_PREV, false, "Find previous (Shift+F3)").on_click(
                    cx.listener(|this, _: &ClickEvent, _w, cx| this.do_find(false, cx)),
                ),
            )
            .child(
                opt_btn("find-next", paths::FIND_NEXT, false, "Find next (F3)").on_click(
                    cx.listener(|this, _: &ClickEvent, _w, cx| this.do_find(true, cx)),
                ),
            )
            .child(
                opt_btn("find-case", paths::MATCH_CASE, find.case_sensitive, "Match case")
                    .on_click(cx.listener(|this, _: &ClickEvent, _w, cx| {
                        if let Some(f) = this.find.as_mut() {
                            f.case_sensitive = !f.case_sensitive;
                            f.cached_searcher = None;
                        }
                        cx.notify();
                    })),
            )
            .child(
                opt_btn("find-word", paths::MATCH_WORD, find.whole_word, "Whole word").on_click(
                    cx.listener(|this, _: &ClickEvent, _w, cx| {
                        if let Some(f) = this.find.as_mut() {
                            f.whole_word = !f.whole_word;
                            f.cached_searcher = None;
                        }
                        cx.notify();
                    }),
                ),
            )
            .child(
                opt_btn("find-regex", paths::REGEX, find.regex, "Regular expression").on_click(
                    cx.listener(|this, _: &ClickEvent, _w, cx| {
                        if let Some(f) = this.find.as_mut() {
                            f.regex = !f.regex;
                            f.cached_searcher = None;
                        }
                        cx.notify();
                    }),
                ),
            )
            .child(
                toolbar_icon_button("find-close")
                    .icon(toolbar_icon(IconName::Close).path(paths::CLOSE))
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
                    toolbar_icon_button("replace-all")
                        .icon(toolbar_icon(IconName::Replace).path(paths::REPLACE_ALL))
                        .tooltip("Replace all matches")
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
                .child(floating_tool_panel(cx, "find-bar", title, body)),
        )
    }
}
