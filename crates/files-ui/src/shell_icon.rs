//! Bundled SVG path icons (Tabler + Zed file_icons).

use std::path::Path;

use gpui::{AnyElement, App, Pixels};

use crate::file_type_icons;
use crate::icons::{chrome_icon_color, palette_icon_fg, tabler_icon_element};

pub fn path_icon_for_path(path: &Path, logical_size: Pixels, cx: &App) -> AnyElement {
    let svg_path = file_type_icons::svg_path_for_path(path);
    let color = if path.is_dir() {
        chrome_icon_color(cx)
    } else {
        palette_icon_fg(svg_path, cx)
    };
    tabler_icon_element(svg_path, logical_size, color)
}

/// Legacy name kept for call sites migrating off Windows Shell icons.
pub fn shell_icon_for_path(path: &Path, logical_size: Pixels, cx: &App) -> AnyElement {
    path_icon_for_path(path, logical_size, cx)
}
