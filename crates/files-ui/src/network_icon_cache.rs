//! Network device icon cache with asynchronous extraction and on-disk persistence.
//!
//! Icons are looked up in this order:
//! 1. In-memory cache (fast, non-blocking).
//! 2. On-disk cache (loaded into memory on first hit).
//! 3. Windows Shell extraction (performed on a background thread).

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

#[cfg(windows)]
use app_platform_windows::{self as platform};

#[derive(Clone)]
enum CacheEntry {
    /// Extracted PNG bytes (non-empty).
    Hit(Arc<Vec<u8>>),
    /// Extraction attempted; use the bundled SVG fallback.
    Miss,
}

struct Cache {
    memory: Mutex<HashMap<(PathBuf, u32), CacheEntry>>,
    disk_dir: Option<PathBuf>,
}

fn cache() -> &'static Cache {
    static CACHE: OnceLock<Cache> = OnceLock::new();
    CACHE.get_or_init(|| {
        let disk_dir = files_core::cache_dir().map(|d| d.join("network_icons"));
        if let Some(ref dir) = disk_dir {
            let _ = std::fs::create_dir_all(dir);
        }
        Cache {
            memory: Mutex::new(HashMap::new()),
            disk_dir,
        }
    })
}

fn cache_file_name(path: &Path, size_px: u32) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    path.to_string_lossy().hash(&mut hasher);
    size_px.hash(&mut hasher);
    format!("{:016x}.png", hasher.finish())
}

fn read_disk_cache(path: &Path, size_px: u32) -> Option<Vec<u8>> {
    let dir = cache().disk_dir.as_ref()?;
    let file = dir.join(cache_file_name(path, size_px));
    if !file.exists() {
        return None;
    }
    let data = std::fs::read(&file).ok()?;
    if data.is_empty() {
        let _ = std::fs::remove_file(&file);
        return None;
    }
    Some(data)
}

fn write_disk_cache(path: &Path, size_px: u32, png: &[u8]) {
    let Some(dir) = cache().disk_dir.as_ref() else {
        return;
    };
    let file = dir.join(cache_file_name(path, size_px));
    let _ = std::fs::create_dir_all(dir);
    let _ = std::fs::write(file, png);
}

/// Cached PNG for a network device path, if previously extracted or present on disk.
/// Safe to call on the UI thread — never blocks.
pub fn cached_png(path: &Path, size_px: u32) -> Option<Arc<Vec<u8>>> {
    let cache = cache();
    let key = (path.to_path_buf(), size_px);

    // 1. Memory cache
    if let Ok(guard) = cache.memory.lock() {
        if let Some(entry) = guard.get(&key) {
            return match entry {
                CacheEntry::Hit(png) => Some(Arc::clone(png)),
                CacheEntry::Miss => None,
            };
        }
    }

    // 2. Disk cache
    if let Some(png) = read_disk_cache(path, size_px) {
        let arc = Arc::new(png);
        if let Ok(mut guard) = cache.memory.lock() {
            guard.insert(key, CacheEntry::Hit(Arc::clone(&arc)));
        }
        return Some(arc);
    }

    None
}

fn store_hit(path: &Path, size_px: u32, png: Vec<u8>) {
    if png.is_empty() {
        store_miss(path, size_px);
        return;
    }
    write_disk_cache(path, size_px, &png);
    let arc = Arc::new(png);
    let cache = cache();
    if let Ok(mut guard) = cache.memory.lock() {
        guard.insert((path.to_path_buf(), size_px), CacheEntry::Hit(arc));
    }
}

fn store_miss(path: &Path, size_px: u32) {
    let cache = cache();
    if let Ok(mut guard) = cache.memory.lock() {
        guard.insert((path.to_path_buf(), size_px), CacheEntry::Miss);
    }
}

fn needs_warm(path: &Path, size_px: u32) -> bool {
    let cache = cache();
    let Ok(guard) = cache.memory.lock() else {
        return false;
    };
    !guard.contains_key(&(path.to_path_buf(), size_px))
}

/// Load missing network device icons on a background thread.
/// Icons are extracted in a single STA thread batch to avoid the
/// per-icon thread-spawn overhead (each `run_sta_task` creates a
/// new thread + OleInitialize, which dominates the actual icon read).
#[cfg(windows)]
pub fn warm_network_icons(paths: Vec<PathBuf>, size_px: u32) {
    let entries: Vec<(PathBuf, u32)> = paths
        .into_iter()
        .filter(|p| needs_warm(p, size_px))
        .map(|p| (p, size_px))
        .collect();
    if entries.is_empty() {
        return;
    }
    let results = platform::shell_icon_png_batch(&entries);
    for (path, size, png) in results {
        match png {
            Some(png) if !png.is_empty() => store_hit(&path, size, png),
            _ => store_miss(&path, size),
        }
    }
}

#[cfg(not(windows))]
pub fn warm_network_icons(_paths: Vec<PathBuf>, _size_px: u32) {}
