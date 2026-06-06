use std::collections::HashSet;
use std::path::{Path, PathBuf};

use files_core::{load_config, pinned_folder_paths, FileTagConfig};

#[cfg(windows)]
use app_platform_windows::{
    list_default_user_folders, DefaultUserFolderKind, shell_pin_to_quick_access,
    shell_unpin_from_quick_access,
};

/// Which Tabler icon / semantics apply to a Home quick-access row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuickAccessFolderKind {
    Desktop,
    Documents,
    Downloads,
    Music,
    Videos,
    Pictures,
    Custom,
}

#[cfg(windows)]
impl From<DefaultUserFolderKind> for QuickAccessFolderKind {
    fn from(kind: DefaultUserFolderKind) -> Self {
        match kind {
            DefaultUserFolderKind::Desktop => Self::Desktop,
            DefaultUserFolderKind::Documents => Self::Documents,
            DefaultUserFolderKind::Downloads => Self::Downloads,
            DefaultUserFolderKind::Music => Self::Music,
            DefaultUserFolderKind::Videos => Self::Videos,
            DefaultUserFolderKind::Pictures => Self::Pictures,
        }
    }
}

/// One quick-access folder on the Home page (default user folders + user pinned).
#[derive(Debug, Clone)]
pub struct QuickAccessEntry {
    pub label: String,
    pub path: PathBuf,
    pub kind: QuickAccessFolderKind,
    /// Present in `settings.json` pinned list (shown in sidebar Pinned section).
    pub is_pinned: bool,
    /// One of the six built-in user folders (Desktop, Documents, …).
    pub is_default: bool,
}

pub fn list_quick_access_entries() -> Vec<QuickAccessEntry> {
    let pinned_set: HashSet<String> = pinned_folder_paths()
        .into_iter()
        .map(|p| path_key(&p))
        .collect();
    let mut seen = HashSet::new();
    let mut entries = Vec::new();

    // 1. Default user folders — always first, never duplicated.
    #[cfg(windows)]
    for item in list_default_user_folders() {
        let key = path_key(&item.path);
        if seen.insert(key.clone()) {
            entries.push(QuickAccessEntry {
                label: item.display_name,
                path: item.path,
                kind: item.kind.into(),
                is_pinned: pinned_set.contains(&key),
                is_default: true,
            });
        }
    }

    // 2. Manually pinned folders — after defaults, skip duplicates.
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
            kind: QuickAccessFolderKind::Custom,
            is_pinned: true,
            is_default: false,
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
