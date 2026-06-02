//! Shared UI helpers for editor overlays.

use super::super::imports::*;
use gpui::{ElementId, Styled};

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

