use std::collections::HashSet;
use std::path::{Path, PathBuf};

use files_core::{load_config, pinned_folder_paths, FileTagConfig};

#[cfg(windows)]
use app_platform_windows::{
    list_shell_quick_access_folders, shell_pin_to_quick_access, shell_unpin_from_quick_access,
};

/// One quick-access folder on the Home page (Shell QA + user pinned).
#[derive(Debug, Clone)]
pub struct QuickAccessEntry {
    pub label: String,
    pub path: PathBuf,
    pub is_pinned: bool,
}

pub fn list_quick_access_entries() -> Vec<QuickAccessEntry> {
    let pinned_set: HashSet<String> = pinned_folder_paths()
        .into_iter()
        .map(|p| path_key(&p))
        .collect();
    let mut seen = HashSet::new();
    let mut entries = Vec::new();

    #[cfg(windows)]
    if let Ok(shell) = list_shell_quick_access_folders() {
        for item in shell {
            if item.path.exists() && seen.insert(path_key(&item.path)) {
                entries.push(QuickAccessEntry {
                    label: item.display_name,
                    path: item.path.clone(),
                    is_pinned: pinned_set.contains(&path_key(&item.path)),
                });
            }
        }
    }

    for path in pinned_folder_paths() {
        if !path.exists() || !seen.insert(path_key(&path)) {
            continue;
        }
        let label = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .filter(|n| !n.is_empty())
            .unwrap_or_else(|| path.to_string_lossy().to_string());
        entries.push(QuickAccessEntry {
            label,
            path,
            is_pinned: true,
        });
    }

    entries
}

#[derive(Debug, Clone)]
pub struct FileTagPreview {
    pub tag: FileTagConfig,
    pub preview_items: Vec<(String, PathBuf)>,
}

const TAG_PREVIEW_LIMIT: usize = 8;

pub fn file_tag_previews(tags: &[FileTagConfig]) -> Vec<FileTagPreview> {
    tags.iter()
        .map(|tag| {
            let preview_items: Vec<(String, PathBuf)> = tag
                .paths
                .iter()
                .map(PathBuf::from)
                .filter(|p| p.exists())
                .take(TAG_PREVIEW_LIMIT)
                .map(|path| {
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| path.to_string_lossy().to_string());
                    (name, path)
                })
                .collect();
            FileTagPreview {
                tag: tag.clone(),
                preview_items,
            }
        })
        .collect()
}

pub fn load_home_file_tags() -> Vec<FileTagConfig> {
    load_config().map(|c| c.file_tags).unwrap_or_default()
}

/// `%AppData%\Microsoft\Windows\Recent\AutomaticDestinations` (Quick Access jumps).
#[cfg(windows)]
pub fn quick_access_automatic_destinations_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("APPDATA").map(|appdata| {
        std::path::PathBuf::from(appdata)
            .join("Microsoft")
            .join("Windows")
            .join("Recent")
            .join("AutomaticDestinations")
    })
}

#[cfg(not(windows))]
pub fn quick_access_automatic_destinations_dir() -> Option<std::path::PathBuf> {
    None
}

#[cfg(windows)]
pub fn eject_drive(drive: &crate::drives::DriveInfo) -> anyhow::Result<()> {
    if !drive.is_removable && !drive.is_network {
        anyhow::bail!("drive does not support eject");
    }
    app_platform_windows::eject_volume(&drive.path, drive.is_network)
}

#[cfg(not(windows))]
pub fn eject_drive(_drive: &crate::drives::DriveInfo) -> anyhow::Result<()> {
    anyhow::bail!("eject is only supported on Windows")
}

/// Pin a folder in Explorer Quick Access (in addition to `settings.json` pins).
#[cfg(windows)]
pub fn sync_pin_to_shell_quick_access(path: &Path) -> anyhow::Result<()> {
    shell_pin_to_quick_access(path)
}

#[cfg(not(windows))]
pub fn sync_pin_to_shell_quick_access(_path: &Path) -> anyhow::Result<()> {
    Ok(())
}

/// Unpin from Explorer Quick Access.
#[cfg(windows)]
pub fn sync_unpin_from_shell_quick_access(path: &Path) -> anyhow::Result<()> {
    shell_unpin_from_quick_access(path)
}

#[cfg(not(windows))]
pub fn sync_unpin_from_shell_quick_access(_path: &Path) -> anyhow::Result<()> {
    Ok(())
}

/// Open Windows Storage Sense settings (Home drive cards).
#[cfg(windows)]
pub fn open_storage_sense_settings() -> anyhow::Result<()> {
    app_platform_windows::open_storage_sense_settings()
}

#[cfg(not(windows))]
pub fn open_storage_sense_settings() -> anyhow::Result<()> {
    anyhow::bail!("storage settings are only supported on Windows")
}

fn path_key(path: &Path) -> String {
    std::fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_ascii_lowercase()
}
