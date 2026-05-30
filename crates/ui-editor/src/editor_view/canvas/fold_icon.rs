//! Material chevron icons for code folding (Google Fonts / Material Symbols paths).

use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

use cyber_desktop_assets::Assets;
use gpui::{
    point, px, App, AssetSource, Bounds, Corners, ImageCacheError, Pixels, RenderImage, Window,
};

/// Material `keyboard_arrow_down` — expanded (open) fold.
const FOLD_EXPANDED: &str = "icons/chevron-down.svg";
/// Material `keyboard_arrow_right` — collapsed fold.
const FOLD_COLLAPSED: &str = "icons/chevron-right.svg";

pub const FOLD_ICON_PX: Pixels = px(12.);

fn render_cache() -> &'static RwLock<HashMap<(String, u32), Arc<RenderImage>>> {
    static CACHE: OnceLock<RwLock<HashMap<(String, u32), Arc<RenderImage>>>> = OnceLock::new();
    CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

fn render_svg(path: &str, logical_px: Pixels, cx: &App) -> Result<Arc<RenderImage>, ImageCacheError> {
    let px = logical_px.as_f32().max(1.0).ceil() as u32;
    if let Some(image) = render_cache()
        .read()
        .ok()
        .and_then(|cache| cache.get(&(path.to_string(), px)).cloned())
    {
        return Ok(image);
    }

    let bytes = match Assets.load(path) {
        Ok(Some(data)) => data.into_owned(),
        Ok(None) => return Err(ImageCacheError::Asset(path.into())),
        Err(_) => return Err(ImageCacheError::Asset(path.into())),
    };

    match cx.svg_renderer().render_single_frame(&bytes, 1.0) {
        Ok(image) => {
            if let Ok(mut cache) = render_cache().write() {
                cache.insert((path.to_string(), px), image.clone());
            }
            Ok(image)
        }
        Err(e) => Err(e.into()),
    }
}

pub(crate) fn paint_fold_chevron(
    window: &mut Window,
    cx: &App,
    origin_x: Pixels,
    top: Pixels,
    line_height: Pixels,
    collapsed: bool,
) {
    let path = if collapsed {
        FOLD_COLLAPSED
    } else {
        FOLD_EXPANDED
    };
    let Ok(image) = render_svg(path, FOLD_ICON_PX, cx) else {
        return;
    };
    let y = top + (line_height - FOLD_ICON_PX) * 0.5;
    let bounds = Bounds::from_corners(
        point(origin_x, y),
        point(origin_x + FOLD_ICON_PX, y + FOLD_ICON_PX),
    );
    let _ = window.paint_image(bounds, Corners::default(), image, 0, false);
}
