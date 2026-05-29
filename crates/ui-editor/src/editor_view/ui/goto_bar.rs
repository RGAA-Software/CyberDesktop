//! UI fragment: `ui/goto_bar.rs`.

use super::icons::{paths, toolbar_icon, toolbar_icon_button};
use super::panel::floating_tool_panel;
use super::super::imports::*;
use gpui_component::IconName;

impl EngineEditor {
    pub(crate) fn render_goto(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let goto = self.goto.as_ref()?;
        let last = self.document.buffer().line_count();

        let body = h_flex()
            .w_full()
            .items_center()
            .gap_2()
            .child(
                Label::new(format!("Line (1–{last}):"))
                    .text_sm()
                    .text_color(cx.theme().muted_foreground),
            )
            .child(div().flex_1().child(Input::new(&goto.input).small()))
            .child(
                toolbar_icon_button("goto-go")
                    .icon(toolbar_icon(IconName::Search).path(paths::GOTO))
                    .tooltip("Go to line")
                    .on_click(cx.listener(|this, _: &ClickEvent, _w, cx| this.do_goto(cx))),
            )
            .child(
                toolbar_icon_button("goto-close")
                    .icon(toolbar_icon(IconName::Close).path(paths::CLOSE))
                    .tooltip("Close (Esc)")
                    .on_click(cx.listener(|this, _: &ClickEvent, _w, cx| this.close_goto(cx))),
            );

        Some(
            div()
                .absolute()
                .top_2()
                .left_4()
                .w(px(320.0))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(floating_tool_panel(cx, "goto-bar", "Go to Line", body)),
        )
    }
}
