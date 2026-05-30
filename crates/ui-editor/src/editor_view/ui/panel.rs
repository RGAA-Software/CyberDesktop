//! Floating tool panels — same chrome as CyberFiles (`popover_style` + shadow).

use super::super::imports::*;
use gpui::FontWeight;
use gpui_component::StyledExt as _;

pub(crate) const PANEL_HEADER_HEIGHT: Pixels = px(32.);

pub(crate) fn panel_title_bar(
    cx: &App,
    title: impl Into<SharedString>,
    close: impl IntoElement,
    on_title_drag: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    h_flex()
        .h(PANEL_HEADER_HEIGHT)
        .px_3()
        .flex_shrink_0()
        .items_center()
        .justify_between()
        .cursor_move()
        .border_b_1()
        .border_color(cx.theme().border)
        .on_mouse_down(MouseButton::Left, on_title_drag)
        .child(
            Label::new(title)
                .text_sm()
                .font_weight(FontWeight::SEMIBOLD)
                .truncate(),
        )
        .child(
            div()
                .flex_shrink_0()
                .cursor_default()
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(close),
        )
}

/// Popover-style floating panel (background, border, shadow) used across CyberFiles.
pub(crate) fn floating_tool_panel(
    cx: &App,
    id: impl Into<gpui::ElementId>,
    title: impl Into<SharedString>,
    close: impl IntoElement,
    on_title_drag: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
    body: impl IntoElement,
) -> impl IntoElement {
    v_flex()
        .id(id)
        .popover_style(cx)
        .shadow_xl()
        .overflow_hidden()
        .child(panel_title_bar(cx, title, close, on_title_drag))
        .child(div().p_3().w_full().child(body))
}
