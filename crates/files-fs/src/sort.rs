use std::cmp::Ordering;

use crate::item::FileItem;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOption {
    Name,
    DateModified,
    DateCreated,
    Size,
    FileType,
    Path,
    Tag,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Ascending,
    Descending,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SortPreferences {
    pub option: SortOption,
    pub direction: SortDirection,
    pub directories_first: bool,
}

impl Default for SortPreferences {
    fn default() -> Self {
        Self {
            option: SortOption::Name,
            direction: SortDirection::Ascending,
            directories_first: true,
        }
    }
}

pub fn sort_items(items: &mut [FileItem], preferences: SortPreferences) {
    items.sort_by(|left, right| compare_items(left, right, preferences));
}

fn compare_items(left: &FileItem, right: &FileItem, preferences: SortPreferences) -> Ordering {
    if preferences.directories_first {
        match (left.is_folder(), right.is_folder()) {
            (true, false) => return Ordering::Less,
            (false, true) => return Ordering::Greater,
            _ => {}
        }
    }

    let ordering = match preferences.option {
        SortOption::Name => compare_text(&left.display_name, &right.display_name),
        SortOption::DateModified => left.modified.cmp(&right.modified),
        SortOption::DateCreated => left.created.cmp(&right.created),
        SortOption::Size => left.size.unwrap_or(0).cmp(&right.size.unwrap_or(0)),
        SortOption::FileType => compare_text(
            left.extension.as_deref().unwrap_or(""),
            right.extension.as_deref().unwrap_or(""),
        )
        .then_with(|| compare_text(&left.display_name, &right.display_name)),
        SortOption::Path => {
            compare_text(&left.path.to_string_lossy(), &right.path.to_string_lossy())
        }
        SortOption::Tag => compare_tag_names(left, right),
    };

    match preferences.direction {
        SortDirection::Ascending => ordering,
        SortDirection::Descending => ordering.reverse(),
    }
}

/// Lexicographic, case-sensitive string order (Files-style name / type / path sort).
fn compare_text(left: &str, right: &str) -> Ordering {
    left.cmp(right)
}

fn compare_tag_names(left: &FileItem, right: &FileItem) -> Ordering {
    match (left.tags.first(), right.tags.first()) {
        (None, None) => Ordering::Equal,
        (None, Some(_)) => Ordering::Greater,
        (Some(_), None) => Ordering::Less,
        (Some(left_tag), Some(right_tag)) => compare_text(&left_tag.name, &right_tag.name),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::item::FileItemKind;

    use super::*;

    fn item(name: &str, kind: FileItemKind, size: Option<u64>) -> FileItem {
        FileItem {
            path: PathBuf::from(name),
            name_raw: name.to_string(),
            display_name: name.to_string(),
            extension: PathBuf::from(name)
                .extension()
                .and_then(|value| value.to_str())
                .map(|value| value.to_string()),
            kind,
            size,
            created: None,
            modified: None,
            accessed: None,
            is_hidden: false,
            is_system: false,
            is_readonly: false,
            is_symlink: false,
            tags: Vec::new(),
            network_category: None,
            network_category_label: None,
        }
    }

    #[test]
    fn sorts_directories_before_files_by_default() {
        let mut items = vec![
            item("b.txt", FileItemKind::File, Some(1)),
            item("a-folder", FileItemKind::Folder, None),
            item("a.txt", FileItemKind::File, Some(1)),
        ];

        sort_items(&mut items, SortPreferences::default());

        let names: Vec<_> = items.iter().map(|item| item.name_raw.as_str()).collect();
        assert_eq!(names, ["a-folder", "a.txt", "b.txt"]);
    }

    #[test]
    fn sorts_names_case_sensitively() {
        let mut items = vec![
            item("banana", FileItemKind::File, None),
            item("Apple", FileItemKind::File, None),
            item("cherry", FileItemKind::File, None),
        ];

        sort_items(&mut items, SortPreferences::default());

        let names: Vec<_> = items.iter().map(|item| item.name_raw.as_str()).collect();
        assert_eq!(names, ["Apple", "banana", "cherry"]);
    }

    #[test]
    fn sorts_names_distinguishes_letter_case() {
        let mut items = vec![
            item("readme", FileItemKind::File, None),
            item("Readme", FileItemKind::File, None),
            item("README", FileItemKind::File, None),
        ];

        sort_items(&mut items, SortPreferences::default());

        let names: Vec<_> = items.iter().map(|item| item.name_raw.as_str()).collect();
        assert_eq!(names, ["README", "Readme", "readme"]);
    }

    #[test]
    fn sorts_files_by_size_descending() {
        let mut items = vec![
            item("small.txt", FileItemKind::File, Some(1)),
            item("large.txt", FileItemKind::File, Some(100)),
        ];

        sort_items(
            &mut items,
            SortPreferences {
                option: SortOption::Size,
                direction: SortDirection::Descending,
                directories_first: false,
            },
        );

        let names: Vec<_> = items.iter().map(|item| item.name_raw.as_str()).collect();
        assert_eq!(names, ["large.txt", "small.txt"]);
    }
}
