//! UI fragment: `ui/goto_bar.rs`.

use super::super::imports::*;

impl EngineEditor {
    pub(crate) fn render_goto(&self, cx: &mut Context<Self>) -> Option<impl IntoElement> {
        let goto = self.goto.as_ref()?;
        let last = self.document.buffer().line_count();

        Some(
            div()
                .absolute()
                .top_2()
                .left_4()
                .w(px(320.0))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    GroupBox::new()
                        .id("goto-bar")
                        .outline()
                        .title("Go to Line")
                        .child(
                            h_flex()
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
                                    Button::new("goto-go")
                                        .xsmall()
                                        .label("Go")
                                        .on_click(
                                            cx.listener(|this, _: &ClickEvent, _w, cx| {
                                                this.do_goto(cx);
                                            }),
                                        ),
                                )
                                .child(
                                    Button::new("goto-close")
                                        .ghost()
                                        .xsmall()
                                        .label("\u{2715}")
                                        .tooltip("Close (Esc)")
                                        .on_click(cx.listener(|this, _: &ClickEvent, _w, cx| {
                                            this.close_goto(cx);
                                        })),
                                ),
                        ),
                ),
        )
    }
}
