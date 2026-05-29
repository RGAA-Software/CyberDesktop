//! Floating tool panels — same chrome as CyberFiles (`popover_style` + shadow).

use super::super::imports::*;
use gpui::FontWeight;
use gpui_component::StyledExt as _;

/// Popover-style floating panel (background, border, shadow) used across CyberFiles.
pub(crate) fn floating_tool_panel(
    cx: &App,
    id: impl Into<gpui::ElementId>,
    title: impl Into<SharedString>,
    body: impl IntoElement,
) -> impl IntoElement {
    v_flex()
        .id(id)
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
                    Label::new(title)
                        .text_sm()
                        .font_weight(FontWeight::SEMIBOLD),
                ),
        )
        .child(div().p_3().w_full().child(body))
}
