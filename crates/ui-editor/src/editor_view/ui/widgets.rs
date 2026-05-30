//! Shared UI helpers for editor overlays.

use super::super::imports::*;
use gpui::{ElementId, Styled};
use gpui_component::Sizable as _;

pub(crate) fn panel_close_button(id: impl Into<ElementId>) -> Button {
    Button::new(id).xsmall().ghost()
}

pub(crate) const PANEL_INPUT_HEIGHT: Pixels = px(32.);

/// Single-line panel input at 32px height (find / replace / search / goto bars).
pub(crate) fn panel_input(state: &Entity<InputState>) -> Input {
    let mut input = Input::new(state).small().w_full();
    input.style().size.height = Some(PANEL_INPUT_HEIGHT.into());
    input
}

/// Left flexible region matching the status column so tool rows align vertically.
pub(crate) fn panel_tool_lead() -> gpui::Div {
    div().flex_1().min_w_0()
}

/// Right-aligned icon button strip (lines up with the panel header close button).
pub(crate) fn panel_tool_strip() -> gpui::Div {
    h_flex().gap_2().flex_shrink_0()
}

pub(crate) fn render_input_field(
    id: &'static str,
    value: &str,
    placeholder: &str,
    active: bool,
    caret: Option<usize>,
    on_down: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> Stateful<gpui::Div> {
    let (text, color) = if value.is_empty() && !active {
        (placeholder.to_string(), rgb(0x6a6a6a))
    } else if active {
        // Insert a caret bar at the caret char position (defaults to end).
        let pos = caret.unwrap_or_else(|| value.chars().count());
        let byte = char_to_byte(value, pos);
        (
            format!("{}\u{2502}{}", &value[..byte], &value[byte..]),
            rgb(0xd4d4d4),
        )
    } else {
        (value.to_string(), rgb(0xd4d4d4))
    };
    div()
        .id(id)
        .w(px(180.0))
        .h(px(22.0))
        .px_2()
        .flex()
        .items_center()
        .rounded_sm()
        .bg(rgb(0x3c3c3c))
        .border_1()
        .border_color(if active { rgb(0x007acc) } else { rgb(0x3c3c3c) })
        .text_color(color)
        .child(SharedString::from(text))
        .on_mouse_down(MouseButton::Left, on_down)
}

/// A small clickable chip/button for the find bar.
pub(crate) fn bar_button(id: &'static str, label: &str, active: bool) -> Stateful<gpui::Div> {
    let mut el = div()
        .id(id)
        .h(px(22.0))
        .px_2()
        .flex()
        .items_center()
        .justify_center()
        .rounded_sm()
        .text_color(rgb(0xcccccc))
        .hover(|s| s.bg(rgb(0x3e3e42)))
        .child(SharedString::from(label.to_string()));
    if active {
        el = el.bg(rgb(0x094771));
    }
    el
}
