//! Material chevron icons for code folding (Google Fonts / Material Symbols paths).

use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

use app_assets::Assets;
use gpui::{
    point, px, App, AssetSource, Bounds, Corners, Hsla, ImageCacheError, Pixels, RenderImage,
    Window,
};

use super::super::r#impl::FOLD_GUTTER_WIDTH;

/// Material `keyboard_arrow_down` — expanded (open) fold.
const FOLD_EXPANDED: &str = "icons/chevron-down.svg";
/// Material `keyboard_arrow_right` — collapsed fold.
const FOLD_COLLAPSED: &str = "icons/chevron-right.svg";

/// Square hover / hit target and icon size, centered in the fold gutter cell.
pub const FOLD_HIT_PX: Pixels = px(14.);

pub(crate) fn fold_hit_bounds(
    fold_left: Pixels,
    top: Pixels,
    line_height: Pixels,
) -> Bounds<Pixels> {
    let x = fold_left + (FOLD_GUTTER_WIDTH - FOLD_HIT_PX) * 0.5;
    let y = top + (line_height - FOLD_HIT_PX) * 0.5;
    Bounds::from_corners(point(x, y), point(x + FOLD_HIT_PX, y + FOLD_HIT_PX))
}

fn render_cache() -> &'static RwLock<HashMap<(String, u32, u32), Arc<RenderImage>>> {
    static CACHE: OnceLock<RwLock<HashMap<(String, u32, u32), Arc<RenderImage>>>> = OnceLock::new();
    CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

fn svg_fill_color(color: Hsla) -> String {
    let rgba = color.to_rgb();
    let r = (rgba.r * 255.0).round() as u8;
    let g = (rgba.g * 255.0).round() as u8;
    let b = (rgba.b * 255.0).round() as u8;
    let a = (rgba.a * 255.0).round() as u8;
    if a == 255 {
        format!("#{:02x}{:02x}{:02x}", r, g, b)
    } else {
        format!("#{:02x}{:02x}{:02x}{:02x}", r, g, b, a)
    }
}

fn tint_svg_bytes(bytes: &[u8], color: Hsla) -> Vec<u8> {
    let fill = svg_fill_color(color);
    String::from_utf8_lossy(bytes)
        .replace("currentColor", &fill)
        .into_bytes()
}

fn render_svg(
    path: &str,
    logical_px: Pixels,
    color: Hsla,
    cx: &App,
) -> Result<Arc<RenderImage>, ImageCacheError> {
    let px = logical_px.as_f32().max(1.0).ceil() as u32;
    let color_key = u32::from(color.to_rgb());
    let cache_key = (path.to_string(), px, color_key);
    if let Some(image) = render_cache()
        .read()
        .ok()
        .and_then(|cache| cache.get(&cache_key).cloned())
    {
        return Ok(image);
    }

    let bytes = match Assets.load(path) {
        Ok(Some(data)) => tint_svg_bytes(&data, color),
        Ok(None) => return Err(ImageCacheError::Asset(path.into())),
        Err(_) => return Err(ImageCacheError::Asset(path.into())),
    };

    match cx.svg_renderer().render_single_frame(&bytes, 1.0) {
        Ok(image) => {
            if let Ok(mut cache) = render_cache().write() {
                cache.insert(cache_key, image.clone());
            }
            Ok(image)
        }
        Err(e) => Err(e.into()),
    }
}

pub(crate) fn paint_fold_chevron(
    window: &mut Window,
    cx: &App,
    fold_left: Pixels,
    top: Pixels,
    line_height: Pixels,
    collapsed: bool,
    color: Hsla,
) {
    let path = if collapsed {
        FOLD_COLLAPSED
    } else {
        FOLD_EXPANDED
    };
    let Ok(image) = render_svg(path, FOLD_HIT_PX, color, cx) else {
        return;
    };
    let cell = fold_hit_bounds(fold_left, top, line_height);
    let _ = window.paint_image(cell, Corners::default(), image, 0, false);
}
