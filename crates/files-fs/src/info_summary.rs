use std::collections::HashMap;

use crate::{FileItem, FileItemKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MultiSelectSummary {
    pub count: usize,
    pub files: usize,
    pub folders: usize,
    pub symlinks: usize,
    pub other: usize,
    pub total_bytes: u64,
}

pub fn multi_select_summary(items: &[FileItem]) -> MultiSelectSummary {
    let mut summary = MultiSelectSummary {
        count: items.len(),
        files: 0,
        folders: 0,
        symlinks: 0,
        other: 0,
        total_bytes: 0,
    };

    for item in items {
        match item.kind {
            FileItemKind::File => {
                summary.files += 1;
                if let Some(size) = item.size {
                    summary.total_bytes = summary.total_bytes.saturating_add(size);
                }
            }
            FileItemKind::Folder => summary.folders += 1,
            FileItemKind::Symlink => summary.symlinks += 1,
            FileItemKind::Other => summary.other += 1,
        }
    }

    summary
}

/// Extension name (lowercase) → count, sorted by count descending then name.
pub fn extension_type_counts(items: &[FileItem]) -> Vec<(String, usize)> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for item in items {
        if item.kind != FileItemKind::File {
            continue;
        }
        let key = item
            .extension
            .as_deref()
            .map(str::to_ascii_lowercase)
            .filter(|ext| !ext.is_empty())
            .unwrap_or_else(|| String::from(""));
        *counts.entry(key).or_default() += 1;
    }
    let mut rows: Vec<(String, usize)> = counts.into_iter().collect();
    rows.sort_by(|left, right| {
        right
            .1
            .cmp(&left.1)
            .then_with(|| left.0.cmp(&right.0))
    });
    rows
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::FileItem;

    fn sample_file(path: &str, size: u64, ext: Option<&str>) -> FileItem {
        FileItem {
            path: PathBuf::from(path),
            name_raw: path.rsplit('\\').next().unwrap_or(path).to_string(),
            display_name: path.rsplit('\\').next().unwrap_or(path).to_string(),
            extension: ext.map(str::to_string),
            kind: FileItemKind::File,
            size: Some(size),
            created: None,
            modified: None,
            accessed: None,
            is_hidden: false,
            is_system: false,
            is_readonly: false,
            is_symlink: false,
            tags: Vec::new(),
        }
    }

    fn sample_folder(path: &str) -> FileItem {
        FileItem {
            path: PathBuf::from(path),
            name_raw: "folder".into(),
            display_name: "folder".into(),
            extension: None,
            kind: FileItemKind::Folder,
            size: None,
            created: None,
            modified: None,
            accessed: None,
            is_hidden: false,
            is_system: false,
            is_readonly: false,
            is_symlink: false,
            tags: Vec::new(),
        }
    }

    #[test]
    fn multi_select_summary_sums_file_sizes_and_counts_kinds() {
        let items = vec![
            sample_file(r"C:\a.txt", 100, Some("txt")),
            sample_file(r"C:\b.pdf", 250, Some("pdf")),
            sample_folder(r"C:\dir"),
        ];
        let summary = multi_select_summary(&items);
        assert_eq!(summary.count, 3);
        assert_eq!(summary.files, 2);
        assert_eq!(summary.folders, 1);
        assert_eq!(summary.total_bytes, 350);
    }

    #[test]
    fn extension_type_counts_groups_by_extension() {
        let items = vec![
            sample_file(r"C:\a.txt", 1, Some("txt")),
            sample_file(r"C:\b.txt", 1, Some("txt")),
            sample_file(r"C:\c.pdf", 1, Some("pdf")),
            sample_folder(r"C:\d"),
        ];
        let counts = extension_type_counts(&items);
        assert_eq!(counts, vec![("txt".into(), 2), ("pdf".into(), 1)]);
    }
}
