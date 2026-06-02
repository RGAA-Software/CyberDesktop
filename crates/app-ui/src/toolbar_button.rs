//! Shared toolbar / icon button sizing (`Size::Medium`) and ghost styling (no border).

use gpui::{px, ElementId, Pixels};
use gpui_component::{
    button::{Button, ButtonVariants as _, DropdownButton},
    Icon, IconName, Sizable as _, Size,
};

const TOOLBAR_ICON_PX: Size = Size::Size(px(18.));

/// Material / bundled SVG at 18px — same sizing as the main CyberFiles app toolbar.
pub fn toolbar_icon(icon: IconName) -> Icon {
    Icon::new(icon).with_size(TOOLBAR_ICON_PX)
}

/// Icon-only `Button` / `DropdownButton` at gpui-component `Size::Medium` (32×32).
pub const TOOLBAR_ICON_BUTTON_SIZE: Size = Size::Medium;

/// Layout slot for a medium icon button (`size_8` = 32px).
pub const TOOLBAR_BUTTON_PX: Pixels = px(32.);

/// Medium icon-only control, ghost variant (no border).
pub fn toolbar_icon_button(id: impl Into<ElementId>) -> Button {
    Button::new(id).with_size(TOOLBAR_ICON_BUTTON_SIZE).ghost()
}

/// Medium toolbar row control with a text label (ghost, no border).
pub fn toolbar_labeled_button(id: impl Into<ElementId>) -> Button {
    Button::new(id).with_size(TOOLBAR_ICON_BUTTON_SIZE).ghost()
}

/// Medium toolbar dropdown (ghost, no border).
pub fn toolbar_dropdown_button(id: impl Into<ElementId>) -> DropdownButton {
    DropdownButton::new(id)
        .with_size(TOOLBAR_ICON_BUTTON_SIZE)
        .ghost()
}
