//! Embedded `.exe` icon extraction with in-memory cache (Windows Shell).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use files_fs::{FileItem, FileItemKind};

#[cfg(windows)]
use app_platform_windows::{self as platform};

#[derive(Clone)]
enum CacheEntry {
    /// Extracted PNG bytes (non-empty).
    Hit(Arc<Vec<u8>>),
    /// Extraction attempted; use the bundled SVG fallback.
    Miss,
}

fn cache() -> &'static Mutex<HashMap<(PathBuf, u32), CacheEntry>> {
    static CACHE: OnceLock<Mutex<HashMap<(PathBuf, u32), CacheEntry>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub fn is_exe_item(item: &FileItem) -> bool {
    item.kind == FileItemKind::File
        && item
            .extension
            .as_deref()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
}

fn cache_key(path: &Path, size_px: u32) -> (PathBuf, u32) {
    (path.to_path_buf(), size_px)
}

/// Cached embedded icon PNG, if previously extracted successfully.
pub fn cached_png(path: &Path, size_px: u32) -> Option<Arc<Vec<u8>>> {
    let key = cache_key(path, size_px);
    if let Ok(guard) = cache().lock() {
        if let Some(CacheEntry::Hit(png)) = guard.get(&key) {
            return Some(Arc::clone(png));
        }
        if matches!(guard.get(&key), Some(CacheEntry::Miss)) {
            return None;
        }
    }
    #[cfg(windows)]
    if let Some(png) = platform::shell_icon_png_from_cache(path, size_px).filter(|p| !p.is_empty()) {
        store_hit(path, size_px, png);
        return cached_png(path, size_px);
    }
    None
}

fn store_hit(path: &Path, size_px: u32, png: Vec<u8>) {
    if png.is_empty() {
        store_miss(path, size_px);
        return;
    }
    if let Ok(mut guard) = cache().lock() {
        guard.insert(cache_key(path, size_px), CacheEntry::Hit(Arc::new(png)));
    }
}

fn store_miss(path: &Path, size_px: u32) {
    if let Ok(mut guard) = cache().lock() {
        guard.insert(cache_key(path, size_px), CacheEntry::Miss);
    }
}

fn needs_warm(path: &Path, size_px: u32) -> bool {
    let Ok(guard) = cache().lock() else {
        return false;
    };
    !guard.contains_key(&cache_key(path, size_px))
}

/// Load each missing `.exe` icon once on a background thread (STA Shell APIs).
#[cfg(windows)]
pub fn warm_exe_icons(paths: Vec<PathBuf>, size_px: u32) {
    for path in paths {
        if !needs_warm(&path, size_px) {
            continue;
        }
        let result = platform::shell_icon_png(&path, size_px);
        match result {
            Ok(png) if !png.is_empty() => store_hit(&path, size_px, png),
            _ => store_miss(&path, size_px),
        }
    }
}

#[cfg(not(windows))]
pub fn warm_exe_icons(_paths: Vec<PathBuf>, _size_px: u32) {}
