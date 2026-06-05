//! Per-extension list icons and named UI icon paths (Tabler SVG assets).

use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, OnceLock, RwLock};

use files_fs::{FileItem, FileItemKind};
use app_platform_windows::{self as platform};

use crate::tabler_icons;

/// Cache key (`:folder:`, `.zip`, `:noext:`) — matches Files `IconCacheService`.
pub type ListIconKey = String;

fn named_icon_paths() -> &'static HashMap<&'static str, &'static str> {
    static NAMED_ICON_PATHS: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    NAMED_ICON_PATHS.get_or_init(|| {
        HashMap::from([
            ("folder", tabler_icons::FOLDER),
            ("new_folder", tabler_icons::FOLDER_PLUS),
            ("new_file", tabler_icons::FILE_PLUS),
            ("home", tabler_icons::HOME),
        ])
    })
}

fn cache() -> &'static RwLock<HashMap<(ListIconKey, u32), Arc<Vec<u8>>>> {
    static CACHE: OnceLock<RwLock<HashMap<(ListIconKey, u32), Arc<Vec<u8>>>>> = OnceLock::new();
    CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

/// Extension / kind key for a row (no Shell I/O).
pub fn list_icon_key(item: &FileItem) -> ListIconKey {
    match item.kind {
        FileItemKind::Folder => ":folder:".into(),
        FileItemKind::Symlink => ":symlink:".into(),
        _ => item
            .extension
            .as_ref()
            .filter(|e| !e.is_empty())
            .map(|e| format!(".{}", e.to_ascii_lowercase()))
            .unwrap_or_else(|| ":noext:".into()),
    }
}

/// Unique keys for all rows in a directory listing.
pub fn list_icon_keys_for_items(items: &[FileItem]) -> Vec<ListIconKey> {
    items
        .iter()
        .map(list_icon_key)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

/// Cached PNG for a list icon key, if already loaded.
pub fn list_icon_png_cached(key: &ListIconKey, size_px: u32) -> Option<Arc<Vec<u8>>> {
    cache().read().ok()?.get(&(key.clone(), size_px)).cloned()
}

/// App-bundled SVG path for a named UI icon.
pub fn named_icon_path(name: &str) -> Option<&'static str> {
    named_icon_paths().get(name).copied()
}

/// App-bundled Tabler SVG path for a file extension (e.g. `"pdf"`).
pub fn extension_svg_path(ext: &str) -> Option<&'static str> {
    fn extension_icon_paths() -> &'static HashMap<&'static str, &'static str> {
        static MAP: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
        MAP.get_or_init(|| {
            HashMap::from([
                ("cpp", tabler_icons::FILE_TYPE_CPP),
                ("cc", tabler_icons::FILE_TYPE_CPP),
                ("cxx", tabler_icons::FILE_TYPE_CPP),
                ("hpp", tabler_icons::FILE_TYPE_CPP),
                ("go", tabler_icons::FILE_CODE),
                ("h", tabler_icons::FILE_CODE),
                ("html", tabler_icons::FILE_TYPE_HTML),
                ("ico", tabler_icons::PHOTO),
                ("png", tabler_icons::PHOTO),
                ("jpg", tabler_icons::PHOTO),
                ("jpeg", tabler_icons::PHOTO),
                ("gif", tabler_icons::PHOTO),
                ("bmp", tabler_icons::PHOTO),
                ("webp", tabler_icons::PHOTO),
                ("java", tabler_icons::FILE_CODE),
                ("gradle", tabler_icons::FILE_CODE),
                ("js", tabler_icons::FILE_TYPE_JS),
                ("json", tabler_icons::FILE_CODE),
                ("kts", tabler_icons::FILE_CODE),
                ("pdf", tabler_icons::FILE_TYPE_PDF),
                ("rs", tabler_icons::FILE_CODE),
                ("svg", tabler_icons::PHOTO),
                ("toml", tabler_icons::FILE_CODE),
                ("ts", tabler_icons::FILE_TYPE_TS),
                ("tsx", tabler_icons::FILE_TYPE_TS),
                ("txt", tabler_icons::FILE_TEXT),
                ("yml", tabler_icons::FILE_CODE),
                ("yaml", tabler_icons::FILE_CODE),
                ("mp4", tabler_icons::MOVIE),
                ("mkv", tabler_icons::MOVIE),
                ("epub", tabler_icons::BOOK),
                ("zip", tabler_icons::FILE_ZIP),
            ])
        })
    }
    extension_icon_paths().get(ext.to_ascii_lowercase().as_str()).copied()
}

fn store_list_icon(key: ListIconKey, size_px: u32, png: Vec<u8>) {
    if png.is_empty() {
        return;
    }
    if let Ok(mut guard) = cache().write() {
        guard.insert((key, size_px), Arc::new(png));
    }
}

fn load_one(key: ListIconKey, size_px: u32) {
    if list_icon_png_cached(&key, size_px).is_some() {
        return;
    }
    match platform::shell_icon_png_for_list_key(&key, size_px) {
        Ok(png) if !png.is_empty() => store_list_icon(key, size_px, png),
        Ok(_) | Err(_) => {
            tracing::debug!(target: "list_icon", key = ?key, size_px, "failed to load icon");
        }
    }
}

/// Load each missing extension icon once (background thread; Files `STATask` per icon).
pub fn warm_list_icons(keys: Vec<ListIconKey>, size_px: u32) {
    for key in keys {
        load_one(key, size_px);
    }
}
