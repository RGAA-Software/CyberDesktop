use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone)]
pub struct RecentItem {
    pub path: PathBuf,
    pub label: String,
    pub modified: Option<SystemTime>,
}

const RECENT_LIMIT: usize = 20;

/// Whether Windows is tracking recent documents (Explorer privacy).
pub fn recent_documents_enabled() -> bool {
    #[cfg(windows)]
    {
        cyberfiles_platform_windows::recent_documents_tracking_enabled()
    }
    #[cfg(not(windows))]
    {
        true
    }
}

/// Recent document shortcuts from the Windows Recent folder.
pub fn list_recent_files() -> Vec<RecentItem> {
    #[cfg(windows)]
    {
        list_windows_recent_files()
    }

    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

#[cfg(windows)]
fn recent_folder() -> Option<PathBuf> {
    std::env::var_os("APPDATA").map(|appdata| {
        PathBuf::from(appdata)
            .join("Microsoft")
            .join("Windows")
            .join("Recent")
    })
}

#[cfg(windows)]
fn list_windows_recent_files() -> Vec<RecentItem> {
    let dir = match recent_folder() {
        Some(dir) if dir.is_dir() => dir,
        _ => return Vec::new(),
    };

    let mut items = Vec::new();
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };

    for entry in entries.flatten() {
        let link_path = entry.path();
        if link_path.extension().and_then(|e| e.to_str()) != Some("lnk") {
            continue;
        }
        let metadata = match entry.metadata() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let target = match resolve_lnk_target(&link_path) {
            Some(path) if path.exists() => path,
            _ => continue,
        };
        let label = target
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| {
                link_path
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| target.to_string_lossy().to_string())
            });
        items.push(RecentItem {
            path: target,
            label,
            modified: metadata.modified().ok(),
        });
    }

    items.sort_by(|a, b| b.modified.cmp(&a.modified));
    items.dedup_by(|a, b| a.path == b.path);
    items.truncate(RECENT_LIMIT);
    items
}

#[cfg(windows)]
fn resolve_lnk_target(link_path: &Path) -> Option<PathBuf> {
    use lnk::encoding::WINDOWS_1252;

    lnk::ShellLink::open(link_path, WINDOWS_1252)
        .ok()
        .and_then(|shell_link| shell_link.link_target().map(PathBuf::from))
}
