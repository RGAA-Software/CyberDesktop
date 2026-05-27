use std::path::PathBuf;

use crate::item::{DirectoryReadOptions, FileItem};
use crate::sort::{sort_items, SortPreferences};

/// Build a flat file list for a tag's associated paths (Files tag search result page subset).
pub fn file_items_for_tag_paths(
    paths: &[PathBuf],
    options: DirectoryReadOptions,
    sort: SortPreferences,
) -> Vec<FileItem> {
    let mut items = Vec::new();
    for path in paths {
        if !path.exists() {
            continue;
        }
        if let Ok(item) = FileItem::from_path(path.clone(), options) {
            items.push(item);
        }
    }
    sort_items(&mut items, sort);
    items
}
