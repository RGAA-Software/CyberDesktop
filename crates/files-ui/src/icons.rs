//! App-wide Material icon render sizes (SVG assets are 24×24; on-screen size is unified).

use gpui::{div, prelude::*, px, AnyElement, App, Pixels};
use gpui_component::{ActiveTheme as _, Icon, IconName, Sizable as _, Size};

use crate::color_icon;
use crate::list_icon_cache;

const APP_ICON_PX: Size = Size::Size(px(18.));
const APP_ICON_IMAGE_PX: Pixels = px(18.);
const APP_LOGO_PX: Pixels = px(20.);

fn sized_icon(icon: IconName) -> Icon {
    Icon::new(icon).with_size(APP_ICON_PX)
}

fn named_icon(name: &str, fallback: IconName) -> Icon {
    let icon = sized_icon(fallback);
    if let Some(path) = list_icon_cache::named_icon_path(name) {
        icon.path(path)
    } else {
        icon
    }
}

fn named_svg_icon_element(name: &str) -> Option<AnyElement> {
    let path = list_icon_cache::named_icon_path(name)?;
    Some(color_icon::color_icon_box(path, APP_ICON_IMAGE_PX))
}

/// CyberFiles app icon for the title bar (left of the menu bar).
pub fn app_logo_element() -> AnyElement {
    color_icon::color_icon_box("icons/content_copy.svg", APP_LOGO_PX)
}

/// Toolbar, title bar, breadcrumbs, sidebar, settings, tab bar — all 18px.
pub fn toolbar_icon(icon: IconName) -> Icon {
    sized_icon(icon)
}

/// Icon tinted with the active theme primary text color (`currentColor` in SVG).
pub fn icon_foreground(icon: IconName, cx: &App) -> impl IntoElement {
    div()
        .flex_none()
        .text_color(cx.theme().foreground)
        .child(toolbar_icon(icon))
}

pub fn sidebar_icon(icon: IconName) -> Icon {
    sized_icon(icon)
}

pub fn inline_icon(icon: IconName) -> Icon {
    sized_icon(icon)
}

pub fn compact_icon(icon: IconName) -> Icon {
    sized_icon(icon)
}

pub fn folder_icon() -> Icon {
    named_icon("folder", IconName::Folder)
}

pub fn home_icon() -> Icon {
    named_icon("home", IconName::LayoutDashboard)
}

#[allow(dead_code)]
pub fn copy_icon() -> Icon {
    named_icon("copy", IconName::Copy)
}

#[allow(dead_code)]
pub fn cut_icon() -> Icon {
    named_icon("cut", IconName::Replace)
}

#[allow(dead_code)]
pub fn paste_icon() -> Icon {
    named_icon("paste", IconName::Replace)
}

pub fn folder_icon_element() -> AnyElement {
    named_svg_icon_element("folder").unwrap_or_else(|| folder_icon().into_any_element())
}

pub fn home_icon_element() -> AnyElement {
    named_svg_icon_element("home").unwrap_or_else(|| home_icon().into_any_element())
}

#[allow(dead_code)]
pub fn copy_icon_element() -> AnyElement {
    named_svg_icon_element("copy").unwrap_or_else(|| copy_icon().into_any_element())
}

#[allow(dead_code)]
pub fn cut_icon_element() -> AnyElement {
    named_svg_icon_element("cut").unwrap_or_else(|| cut_icon().into_any_element())
}

#[allow(dead_code)]
pub fn paste_icon_element() -> AnyElement {
    named_svg_icon_element("paste").unwrap_or_else(|| paste_icon().into_any_element())
}

pub fn inbox_icon_element() -> AnyElement {
    color_icon::color_icon_box("icons/inbox.svg", APP_ICON_IMAGE_PX)
}

pub fn delete_icon_element() -> AnyElement {
    color_icon::color_icon_box("icons/delete.svg", APP_ICON_IMAGE_PX)
}

/// Empty file-tag list (`folder_off`, Material Symbols).
pub fn file_tag_empty_icon_element() -> AnyElement {
    color_icon::color_icon_box("icons/folder_off.svg", px(48.))
}

/// Pinned folder / push pin (`icons/pin.svg`, Material `push_pin`).
pub fn pin_icon() -> Icon {
    Icon::new(IconName::Folder)
        .path("icons/pin.svg")
        .with_size(APP_ICON_PX)
}

pub fn tabs_icon() -> Icon {
    Icon::new(IconName::File)
        .path("icons/tab.svg")
        .with_size(APP_ICON_PX)
}
