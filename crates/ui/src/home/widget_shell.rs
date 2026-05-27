//! Shared Home widget chrome (card grid, icons, drive space bar).

use gpui::{prelude::*, *};
use gpui_component::progress::Progress;

pub const CARD_WIDTH: Pixels = px(240.);
pub const CARD_MIN_HEIGHT: Pixels = px(72.);
pub const FOLDER_CARD_WIDTH: Pixels = px(120.);
pub const FOLDER_CARD_HEIGHT: Pixels = px(88.);

/// Stop the Home page “show/hide widgets” menu from opening (bubble phase).
pub fn block_home_page_context_menu<T>(element: T) -> T
where
    T: InteractiveElement,
{
    element.on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
}

pub fn card_grid(children: impl IntoIterator<Item = AnyElement>) -> impl IntoElement {
    div()
        .id("home-card-grid")
        .w_full()
        .flex()
        .flex_wrap()
        .gap_2()
        .children(children)
}

pub fn space_progress_bar(id: impl Into<ElementId>, fraction: f32) -> impl IntoElement {
    Progress::new(id)
        .w_full()
        .h(px(4.))
        .value(fraction.clamp(0., 1.) * 100.)
}
