use std::path::Path;

use crate::item::{should_include_item, DirectoryReadOptions, FileItem, FileItemKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct FolderEntryCounts {
    pub files: usize,
    pub folders: usize,
    pub other: usize,
}

impl FolderEntryCounts {
    pub fn total(&self) -> usize {
        self.files + self.folders + self.other
    }
}

/// Immediate children only (non-recursive).
pub fn count_directory_entries(
    path: &Path,
    options: DirectoryReadOptions,
) -> anyhow::Result<FolderEntryCounts> {
    let mut counts = FolderEntryCounts::default();
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let item = FileItem::from_path(entry.path(), options)?;
        if !should_include_item(&item, options) {
            continue;
        }
        match item.kind {
            FileItemKind::File => counts.files += 1,
            FileItemKind::Folder => counts.folders += 1,
            _ => counts.other += 1,
        }
    }
    Ok(counts)
}

/// Total byte size of all files under `path` (recursive; does not follow directory symlinks).
pub fn directory_tree_size(path: &Path) -> anyhow::Result<u64> {
    let mut total = 0u64;
    let mut stack = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir)? {
            let entry = entry?;
            let meta = entry.metadata()?;
            let entry_path = entry.path();
            if meta.is_dir() {
                if meta.file_type().is_symlink() {
                    continue;
                }
                stack.push(entry_path);
            } else if meta.is_file() {
                total = total.saturating_add(meta.len());
            }
        }
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn count_directory_entries_counts_immediate_children() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), b"x").unwrap();
        fs::create_dir(dir.path().join("sub")).unwrap();
        let counts = count_directory_entries(dir.path(), DirectoryReadOptions::default()).unwrap();
        assert_eq!(counts.files, 1);
        assert_eq!(counts.folders, 1);
        assert_eq!(counts.total(), 2);
    }

    #[test]
    fn directory_tree_size_sums_nested_files() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), [0u8; 10]).unwrap();
        let sub = dir.path().join("sub");
        fs::create_dir(&sub).unwrap();
        fs::write(sub.join("b.txt"), [0u8; 5]).unwrap();
        assert_eq!(directory_tree_size(dir.path()).unwrap(), 15);
    }
}
