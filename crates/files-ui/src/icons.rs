//! App-wide Tabler icon helpers (24×24 outline, 18px on screen).

use gpui::{div, prelude::*, px, AnyElement, App, Hsla, Pixels};
use gpui_component::{ActiveTheme as _, Icon, IconName, Sizable as _, Size};

use crate::list_icon_cache;
use crate::tabler_icons;

const APP_ICON_IMAGE_PX: Pixels = px(18.);
/// Sidebar row icons (design V11 ~15–16px).
pub const SIDEBAR_ICON_PX: Pixels = px(16.);

fn named_icon(name: &str) -> Icon {
    let path = list_icon_cache::named_icon_path(name).unwrap_or(tabler_icons::FILE);
    tabler_icons::icon(path)
}

/// Default tint for chrome icons (nav, title bar, action bar, breadcrumbs).
pub fn chrome_icon_color(cx: &App) -> Hsla {
    cx.theme().muted_foreground
}

fn chrome_icon_box(path: &'static str, color: Hsla, size: Pixels) -> AnyElement {
    div()
        .size(size)
        .flex_none()
        .text_color(color)
        .child(toolbar_tabler(path))
        .into_any_element()
}

/// CyberFiles app icon for the title bar (left of the menu bar).
pub fn app_logo_element(cx: &App) -> AnyElement {
    div()
        .flex_none()
        .text_color(cx.theme().primary)
        .child(
            tabler_icons::icon(tabler_icons::FILES).with_size(Size::Size(tabler_icons::logo_px())),
        )
        .into_any_element()
}

/// Toolbar, title bar, breadcrumbs, sidebar, settings, tab bar — all 18px.
pub fn toolbar_icon(icon: IconName) -> Icon {
    tabler_icons::from_icon_name(icon)
}

/// Explicit Tabler asset path at toolbar size.
pub fn toolbar_tabler(path: &'static str) -> Icon {
    tabler_icons::icon(path)
}

/// Tabler icon at sidebar row size with an explicit tint.
pub fn sidebar_tabler_icon(path: &'static str, color: Hsla) -> AnyElement {
    div()
        .flex_none()
        .text_color(color)
        .child(
            tabler_icons::icon(path).with_size(Size::Size(SIDEBAR_ICON_PX)),
        )
        .into_any_element()
}

/// Icon tinted with the active theme primary text color (`currentColor` in SVG).
pub fn icon_foreground(icon: IconName, cx: &App) -> impl IntoElement {
    div()
        .flex_none()
        .text_color(chrome_icon_color(cx))
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

pub fn folder_icon_element(cx: &App) -> AnyElement {
    chrome_icon_box(tabler_icons::FOLDER, chrome_icon_color(cx), APP_ICON_IMAGE_PX)
}

pub fn home_icon_element(cx: &App) -> AnyElement {
    chrome_icon_box(tabler_icons::HOME, chrome_icon_color(cx), APP_ICON_IMAGE_PX)
}

pub fn inbox_icon_element(cx: &App) -> AnyElement {
    chrome_icon_box(tabler_icons::INBOX, chrome_icon_color(cx), APP_ICON_IMAGE_PX)
}

pub fn delete_icon_element(cx: &App) -> AnyElement {
    chrome_icon_box(tabler_icons::TRASH, chrome_icon_color(cx), APP_ICON_IMAGE_PX)
}

/// Empty file-tag list placeholder.
pub fn file_tag_empty_icon_element(cx: &App) -> AnyElement {
    chrome_icon_box(tabler_icons::FOLDER_OFF, chrome_icon_color(cx), px(48.))
}

pub fn pin_icon() -> Icon {
    tabler_icons::icon(tabler_icons::PIN)
}

pub fn tabs_icon() -> Icon {
    tabler_icons::icon(tabler_icons::PLUS)
}
