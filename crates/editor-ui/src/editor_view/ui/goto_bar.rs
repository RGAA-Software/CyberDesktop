//! UI fragment: `ui/goto_bar.rs`.

use super::super::imports::*;
use super::icons::{paths, toolbar_icon, toolbar_icon_button};
use super::panel::floating_tool_panel;
use super::widgets::{panel_close_button, panel_input, panel_tool_strip};
use gpui_component::IconName;

impl EngineEditor {
    pub(crate) fn render_goto(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let goto = self.goto.as_ref()?;
        let last = self.document.buffer().line_count();

        let body = v_flex()
            .w_full()
            .gap_2()
            .child(panel_input(&goto.input))
            .child(
                h_flex()
                    .w_full()
                    .items_center()
                    .child(
                        Label::new(t!("editor.goto.prompt", last = last))
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .flex_1()
                            .min_w_0()
                            .truncate(),
                    )
                    .child(
                        panel_tool_strip().child(
                            toolbar_icon_button("goto-go")
                                .icon(toolbar_icon(IconName::Search).path(paths::GOTO))
                                .tooltip(t!("editor.goto.action"))
                                .on_click(
                                    cx.listener(|this, _: &ClickEvent, _w, cx| this.do_goto(cx)),
                                ),
                        ),
                    ),
            );

        let close = panel_close_button("goto-close")
            .icon(toolbar_icon(IconName::Close).path(paths::CLOSE))
            .tooltip(t!("editor.find.close"))
            .on_click(cx.listener(|this, _: &ClickEvent, _w, cx| this.close_goto(cx)));

        let pos = self.resolved_panel_origin(FloatingPanel::Goto);

        Some(
            div()
                .absolute()
                .left(pos.x)
                .top(pos.y)
                .w(GOTO_PANEL_WIDTH)
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(floating_tool_panel(
                    cx,
                    "goto-bar",
                    t!("editor.goto.title"),
                    close,
                    cx.listener(|this, event: &MouseDownEvent, _window, cx| {
                        this.start_panel_drag(FloatingPanel::Goto, event, cx);
                    }),
                    body,
                )),
        )
    }
}
