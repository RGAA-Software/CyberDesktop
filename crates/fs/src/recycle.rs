use crate::item::{DirectoryReadOptions, FileItem, FileItemKind};
use crate::sort::{sort_items, SortPreferences};

/// Virtual recycle-bin listing via the Windows Shell namespace.
#[cfg(windows)]
pub fn read_recycle_bin(
    options: DirectoryReadOptions,
    sort: SortPreferences,
) -> anyhow::Result<Vec<FileItem>> {
    let entries = cyber_desktop_platform_windows::list_recycle_bin_entries()?;
    let mut items: Vec<FileItem> = entries
        .into_iter()
        .map(|entry| file_item_from_recycle_entry(entry, options))
        .collect();
    sort_items(&mut items, sort);
    Ok(items)
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
    entry: cyber_desktop_platform_windows::RecycleBinEntry,
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
        kind: FileItemKind::File,
        size: entry.size,
        created: None,
        modified: entry.modified,
        accessed: None,
        is_hidden: false,
        is_system: false,
        is_readonly: false,
        is_symlink: false,
    }
}
