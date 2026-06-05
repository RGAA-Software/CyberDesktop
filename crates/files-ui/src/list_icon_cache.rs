//! Per-extension Shell list icons (Files `IconCacheService` — dummy path + extension key).

use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, OnceLock, RwLock};

use files_fs::{FileItem, FileItemKind};
use app_platform_windows::{self as platform};

/// Cache key (`:folder:`, `.zip`, `:noext:`) — matches Files `IconCacheService`.
pub type ListIconKey = String;

fn named_icon_paths() -> &'static HashMap<&'static str, &'static str> {
    static NAMED_ICON_PATHS: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
    NAMED_ICON_PATHS.get_or_init(|| {
        HashMap::from([
            ("folder", "icons/ic_folder.svg"),
            ("new_folder", "icons/ic_new_folder.svg"),
            ("new_file", "icons/ic_new_file.svg"),
            ("home", "icons/ic_home.svg"),
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

/// App-bundled colored SVG path for a file extension (e.g. `"pdf"` → `"icons/ic_pdf.svg"`).
pub fn extension_svg_path(ext: &str) -> Option<&'static str> {
    fn extension_icon_paths() -> &'static HashMap<&'static str, &'static str> {
        static MAP: OnceLock<HashMap<&'static str, &'static str>> = OnceLock::new();
        MAP.get_or_init(|| {
            HashMap::from([
                ("cpp", "icons/ic_cpp.svg"),
                ("cc", "icons/ic_cpp.svg"),
                ("cxx", "icons/ic_cpp.svg"),
                ("hpp", "icons/ic_cpp.svg"),
                ("go", "icons/ic_go.svg"),
                ("h", "icons/ic_h.svg"),
                ("html", "icons/ic_html.svg"),
                ("ico", "icons/ic_image.svg"),
                ("png", "icons/ic_image.svg"),
                ("jpg", "icons/ic_image.svg"),
                ("jpeg", "icons/ic_image.svg"),
                ("gif", "icons/ic_gif.svg"),
                ("bmp", "icons/ic_image.svg"),
                ("webp", "icons/ic_image.svg"),
                ("java", "icons/ic_java.svg"),
                ("gradle", "icons/ic_java.svg"),
                ("js", "icons/ic_js.svg"),
                ("json", "icons/ic_json.svg"),
                ("kts", "icons/ic_kts.svg"),
                ("pdf", "icons/ic_pdf.svg"),
                ("rs", "icons/ic_rust.svg"),
                ("svg", "icons/ic_svg.svg"),
                ("toml", "icons/ic_toml.svg"),
                ("ts", "icons/ic_ts.svg"),
                ("tsx", "icons/ic_ts.svg"),
                ("txt", "icons/ic_txt.svg"),
                ("yml", "icons/ic_yml.svg"),
                ("yaml", "icons/ic_yml.svg"),
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
