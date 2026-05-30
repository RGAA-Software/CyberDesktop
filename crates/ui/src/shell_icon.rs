//! Windows Shell icons for paths (Files-style colorful folder / drive icons).

use std::path::Path;
use std::sync::Arc;

use gpui::{div, img, prelude::*, AnyElement, Image, ImageFormat, ObjectFit, Pixels, Window};

#[cfg(windows)]
use cyber_desktop_platform_windows::shell_icon_png_scaled;

#[cfg(windows)]
pub fn shell_icon_for_path(path: &Path, logical_size: Pixels, window: &Window) -> AnyElement {
    let scale = window.scale_factor();
    let png = shell_icon_png_scaled(path, logical_size.as_f32(), scale).unwrap_or_default();
    if png.is_empty() {
        return div().size(logical_size).into_any();
    }
    img(Arc::new(Image::from_bytes(ImageFormat::Png, png)))
        .size(logical_size)
        .object_fit(ObjectFit::Contain)
        .into_any()
}

#[cfg(not(windows))]
pub fn shell_icon_for_path(_path: &Path, logical_size: Pixels, _window: &Window) -> AnyElement {
    div().size(logical_size).into_any()
}
