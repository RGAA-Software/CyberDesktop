//! Async Shell thumbnails for Home folder/drive cards (H5).

use std::path::Path;
use std::sync::Arc;

use gpui::{img, prelude::*, AnyElement, App, Context, Image, ImageFormat, ObjectFit, Pixels, Window};

use super::page::HomePage;

#[cfg(windows)]
use app_platform_windows::shell_thumbnail_png_scaled;

use crate::shell_icon::shell_icon_for_path;

#[allow(dead_code)]
pub fn thumbnail_cache_key(path: &Path) -> String {
    std::fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_ascii_lowercase()
}

impl HomePage {
#[allow(dead_code)]
    pub(super) fn ensure_home_thumbnail(
        &mut self,
        path: &Path,
        logical_px: f32,
        window: &Window,
        cx: &mut Context<Self>,
    ) {
        #[cfg(not(windows))]
        {
            let _ = (path, logical_px, window, cx);
            return;
        }
        #[cfg(windows)]
        {
            if !path.exists() {
                return;
            }
            let key = thumbnail_cache_key(path);
            if self.thumbnail_bytes.contains_key(&key) || self.thumbnail_pending.contains(&key) {
                return;
            }
            self.thumbnail_pending.insert(key.clone());
            let path = path.to_path_buf();
            let scale = window.scale_factor();
            cx.spawn(async move |page, cx| {
                let png = cx
                    .background_spawn(async move {
                        shell_thumbnail_png_scaled(&path, logical_px, scale)
                    })
                    .await
                    .ok()
                    .flatten();
                let _ = page.update(cx, |page, cx| {
                    page.thumbnail_pending.remove(&key);
                    if let Some(bytes) = png {
                        page.thumbnail_bytes.insert(key, Arc::new(bytes));
                        cx.notify();
                    }
                });
            })
            .detach();
        }
    }

#[allow(dead_code)]
    pub(super) fn home_card_image(
        &self,
        path: &Path,
        logical_size: Pixels,
        cx: &App,
    ) -> AnyElement {
        if let Some(bytes) = self.thumbnail_bytes.get(&thumbnail_cache_key(path)) {
            return img(Arc::new(Image::from_bytes(
                ImageFormat::Png,
                bytes.as_ref().clone(),
            )))
            .size(logical_size)
            .object_fit(ObjectFit::Contain)
            .into_any();
        }
        shell_icon_for_path(path, logical_size, cx)
    }
}
