use std::path::PathBuf;

use crate::item::{DirectoryReadOptions, FileItem, FileItemKind};
use crate::sort::{sort_items, SortPreferences};

/// Recycle-bin listing via Windows Shell (`IEnumShellItems`).
#[cfg(windows)]
pub fn read_recycle_bin(
    options: DirectoryReadOptions,
    sort: SortPreferences,
) -> anyhow::Result<Vec<FileItem>> {
    let entries = app_platform_windows::list_recycle_bin_entries()?;
    let mut items: Vec<FileItem> = entries
        .into_iter()
        .map(|entry| file_item_from_recycle_entry(entry, options))
        .collect();
    sort_items(&mut items, sort);
    Ok(items)
}

#[cfg(windows)]
pub fn empty_recycle_bin() -> anyhow::Result<()> {
    app_platform_windows::empty_recycle_bin()
}

#[cfg(not(windows))]
pub fn empty_recycle_bin() -> anyhow::Result<()> {
    Ok(())
}

#[cfg(windows)]
pub fn restore_recycle_items(paths: &[PathBuf]) -> anyhow::Result<()> {
    app_platform_windows::restore_recycle_bin_items(paths)
}

#[cfg(not(windows))]
pub fn restore_recycle_items(_paths: &[PathBuf]) -> anyhow::Result<()> {
    Ok(())
}

#[cfg(not(windows))]
pub fn restore_recycled_originals(_original_paths: &[PathBuf]) -> anyhow::Result<()> {
    Ok(())
}

#[cfg(windows)]
pub fn restore_recycled_originals(original_paths: &[PathBuf]) -> anyhow::Result<()> {
    let shell_paths =
        app_platform_windows::recycle_shell_paths_for_originals(original_paths)?;
    if shell_paths.is_empty() && !original_paths.is_empty() {
        anyhow::bail!("recycle bin items not found for restore");
    }
    app_platform_windows::restore_recycle_bin_items(&shell_paths)
}

#[cfg(not(windows))]
pub fn read_recycle_bin(
    _options: DirectoryReadOptions,
    _sort: SortPreferences,
) -> anyhow::Result<Vec<FileItem>> {
    Ok(Vec::new())
}

#[cfg(windows)]
fn file_item_from_recycle_entry(
    entry: app_platform_windows::RecycleBinEntry,
    options: DirectoryReadOptions,
) -> FileItem {
    let extension = entry
        .shell_path
        .extension()
        .and_then(|e| e.to_str())
        .filter(|e| !e.is_empty())
        .map(|e| e.to_string());

    let display_name = if options.show_file_extensions {
        entry.display_name.clone()
    } else if let Some(ext) = &extension {
        entry
            .display_name
            .strip_suffix(&format!(".{ext}"))
            .unwrap_or(&entry.display_name)
            .to_string()
    } else {
        entry.display_name.clone()
    };

    FileItem {
        path: entry.shell_path,
        name_raw: entry.display_name,
        display_name,
        extension,
        kind: match entry.kind {
            app_platform_windows::RecycleBinItemKind::Folder => FileItemKind::Folder,
            app_platform_windows::RecycleBinItemKind::File => FileItemKind::File,
        },
        size: entry.size,
        created: None,
        modified: entry.modified,
        accessed: None,
        is_hidden: false,
        is_system: false,
        is_readonly: false,
        is_symlink: false,
        tags: Vec::new(),
    }
}
