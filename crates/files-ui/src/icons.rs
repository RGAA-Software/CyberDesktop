//! App-wide Tabler icon helpers (24×24 outline, 18px on screen).

use files_fs::{parse_tag_color_hex, DriveInfo, QuickAccessFolderKind};
use gpui::{div, prelude::*, px, AnyElement, App, Hsla, Pixels};
use gpui_component::{ActiveTheme as _, Icon, IconName, Sizable as _, Size};

use crate::color_icon::color_icon_box;
use crate::list_icon_cache;
use crate::tabler_icons;

/// Full-color title bar logo (`app-assets/assets/app/logo/ic_cyber_files.svg`).
pub const APP_LOGO_PATH: &str = "app/logo/ic_cyber_files.svg";

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
pub fn app_logo_element(_cx: &App) -> AnyElement {
    color_icon_box(APP_LOGO_PATH, tabler_icons::logo_px())
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

/// Tabler asset for a Home quick-access row (`design/cyber_files.html` `.qa-icon`).
pub fn home_quick_access_tabler_icon(kind: QuickAccessFolderKind) -> &'static str {
    match kind {
        QuickAccessFolderKind::Desktop => tabler_icons::DEVICE_DESKTOP,
        QuickAccessFolderKind::Documents => tabler_icons::FILE_TEXT,
        QuickAccessFolderKind::Downloads => tabler_icons::DOWNLOAD,
        QuickAccessFolderKind::Music => tabler_icons::MUSIC,
        QuickAccessFolderKind::Videos => tabler_icons::MOVIE,
        QuickAccessFolderKind::Pictures => tabler_icons::PHOTO,
        QuickAccessFolderKind::Custom => tabler_icons::FOLDER_FILLED,
    }
}

/// Icon + tile background for a built-in quick-access folder.
///
/// Colors come from [`files_fs::TAG_COLOR_PRESETS`] (same palette as file tags).
/// Returns `None` for manually pinned folders — use theme accent there.
pub fn home_quick_access_palette(kind: QuickAccessFolderKind) -> Option<(Hsla, Hsla)> {
    let hex = match kind {
        QuickAccessFolderKind::Desktop => "#1E88E5",
        QuickAccessFolderKind::Documents => "#3949AB",
        QuickAccessFolderKind::Downloads => "#00897B",
        QuickAccessFolderKind::Music => "#D81B60",
        QuickAccessFolderKind::Videos => "#F4511E",
        QuickAccessFolderKind::Pictures => "#FFB300",
        QuickAccessFolderKind::Custom => return None,
    };
    let rgb = parse_tag_color_hex(hex)?;
    let icon: Hsla = gpui::rgb(rgb).into();
    let tile = icon.opacity(0.14);
    Some((icon, tile))
}

/// Tabler asset for a Home drive card (`design/cyber_files.html` `.drive-icon`).
pub fn home_drive_tabler_icon(drive: &DriveInfo) -> &'static str {
    let root = drive.path.to_string_lossy().to_ascii_uppercase();
    if root.starts_with("C:\\") {
        tabler_icons::BRAND_WINDOWS
    } else {
        tabler_icons::DATABASE
    }
}

/// Render a Tabler SVG at an explicit pixel size and tint.
pub fn tabler_icon_element(path: &'static str, size: Pixels, color: Hsla) -> AnyElement {
    div()
        .flex_none()
        .text_color(color)
        .child(tabler_icons::icon(path).with_size(Size::Size(size)))
        .into_any_element()
}
