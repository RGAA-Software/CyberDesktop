use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

use gpui::{
    div, img, AnyElement, App, AssetSource, ImageCacheError, IntoElement, ObjectFit, ParentElement,
    Pixels, RenderImage, Styled, StyledImage, Window,
};

use app_assets::Assets;

fn render_cache() -> &'static RwLock<HashMap<(String, u32), Arc<RenderImage>>> {
    static CACHE: OnceLock<RwLock<HashMap<(String, u32), Arc<RenderImage>>>> = OnceLock::new();
    CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

fn render_svg(
    path: &str,
    logical_px: Pixels,
    cx: &App,
) -> Result<Arc<RenderImage>, ImageCacheError> {
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
        Ok(None) => {
            tracing::warn!(target: "color_icon", path, "asset not found");
            return Err(ImageCacheError::Asset(path.into()));
        }
        Err(e) => {
            tracing::error!(target: "color_icon", path, error = %e, "asset load error");
            return Err(ImageCacheError::Asset(path.into()));
        }
    };

    match cx.svg_renderer().render_single_frame(&bytes, 1.0) {
        Ok(image) => {
            if let Ok(mut cache) = render_cache().write() {
                cache.insert((path.to_string(), px), image.clone());
            }
            Ok(image)
        }
        Err(e) => {
            tracing::error!(target: "color_icon", path, error = %e, "svg render error");
            Err(e.into())
        }
    }
}

pub fn color_icon(path: &'static str, logical_px: Pixels) -> AnyElement {
    let size = logical_px;
    img(move |_window: &mut Window, cx: &mut App| Some(render_svg(path, size, cx)))
        .size(size)
        .object_fit(ObjectFit::Contain)
        .with_fallback(move || {
            div()
                .size(size)
                .rounded_md()
                .bg(gpui::rgb(0xff0000))
                .into_any_element()
        })
        .into_any_element()
}

pub fn color_icon_box(path: &'static str, logical_px: Pixels) -> AnyElement {
    div()
        .size(logical_px)
        .flex_none()
        .child(color_icon(path, logical_px))
        .into_any_element()
}
