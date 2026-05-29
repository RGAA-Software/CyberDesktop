//! UI fragment: `ui/goto_bar.rs`.

use super::super::imports::*;
use super::widgets::{bar_button, render_input_field};

impl EngineEditor {
    pub(crate) fn render_goto(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let value = self.goto.as_ref()?;
        let last = self.document.buffer().line_count();
        let field = render_input_field(
            "goto-field",
            value,
            "Line",
            true,
            None,
            cx.listener(|_this, _e: &MouseDownEvent, _w, cx| cx.stop_propagation()),
        );
        Some(
            div()
                .absolute()
                .top_2()
                .left_4()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .p_2()
                .rounded_md()
                .bg(rgb(0x2d2d30))
                .border_1()
                .border_color(rgb(0x454545))
                .text_color(rgb(0xcccccc))
                .text_size(px(12.0))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .text_color(rgb(0x9a9a9a))
                        .child(SharedString::from(format!("Go to line (1-{last}):"))),
                )
                .child(field)
                .child(bar_button("goto-go", "Go", false).on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _e: &MouseDownEvent, _w, cx| {
                        this.do_goto(cx);
                        cx.stop_propagation();
                    }),
                )),
        )
    }

}
