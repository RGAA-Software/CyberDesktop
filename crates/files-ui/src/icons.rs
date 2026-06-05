//! App-wide Tabler icon helpers (24×24 outline, 18px on screen).

use gpui::{div, prelude::*, px, AnyElement, App, Pixels};
use gpui_component::{ActiveTheme as _, Icon, IconName};

use crate::color_icon;
use crate::list_icon_cache;
use crate::tabler_icons;

const APP_ICON_IMAGE_PX: Pixels = px(18.);

fn named_icon(name: &str) -> Icon {
    let path = list_icon_cache::named_icon_path(name).unwrap_or(tabler_icons::FILE);
    tabler_icons::icon(path)
}

fn named_svg_icon_element(name: &str) -> Option<AnyElement> {
    let path = list_icon_cache::named_icon_path(name)?;
    Some(color_icon::color_icon_box(path, APP_ICON_IMAGE_PX))
}

/// CyberFiles app icon for the title bar (left of the menu bar).
pub fn app_logo_element() -> AnyElement {
    color_icon::color_icon_box(tabler_icons::FILES, tabler_icons::logo_px())
}

/// Toolbar, title bar, breadcrumbs, sidebar, settings, tab bar — all 18px.
pub fn toolbar_icon(icon: IconName) -> Icon {
    tabler_icons::from_icon_name(icon)
}

/// Explicit Tabler asset path at toolbar size.
pub fn toolbar_tabler(path: &'static str) -> Icon {
    tabler_icons::icon(path)
}

/// Icon tinted with the active theme primary text color (`currentColor` in SVG).
pub fn icon_foreground(icon: IconName, cx: &App) -> impl IntoElement {
    div()
        .flex_none()
        .text_color(cx.theme().foreground)
        .child(toolbar_icon(icon))
}

pub fn sidebar_icon(icon: IconName) -> Icon {
    tabler_icons::from_icon_name(icon)
}

pub fn inline_icon(icon: IconName) -> Icon {
    tabler_icons::from_icon_name(icon)
}

pub fn compact_icon(icon: IconName) -> Icon {
    tabler_icons::from_icon_name(icon)
}

pub fn folder_icon() -> Icon {
    named_icon("folder")
}

pub fn home_icon() -> Icon {
    named_icon("home")
}

pub fn folder_icon_element() -> AnyElement {
    named_svg_icon_element("folder").unwrap_or_else(|| folder_icon().into_any_element())
}

pub fn home_icon_element() -> AnyElement {
    named_svg_icon_element("home").unwrap_or_else(|| home_icon().into_any_element())
}

pub fn inbox_icon_element() -> AnyElement {
    color_icon::color_icon_box(tabler_icons::INBOX, APP_ICON_IMAGE_PX)
}

pub fn delete_icon_element() -> AnyElement {
    color_icon::color_icon_box(tabler_icons::TRASH, APP_ICON_IMAGE_PX)
}

/// Empty file-tag list placeholder.
pub fn file_tag_empty_icon_element() -> AnyElement {
    color_icon::color_icon_box(tabler_icons::FOLDER_OFF, px(48.))
}

pub fn pin_icon() -> Icon {
    tabler_icons::icon(tabler_icons::PIN)
}

pub fn tabs_icon() -> Icon {
    tabler_icons::icon(tabler_icons::PLUS)
}
