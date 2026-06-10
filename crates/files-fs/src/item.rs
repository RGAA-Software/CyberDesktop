use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagRef {
    pub name: String,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileItemKind {
    File,
    Folder,
    Symlink,
    Other,
}

#[derive(Debug, Clone)]
pub struct FileItem {
    pub path: PathBuf,
    pub name_raw: String,
    pub display_name: String,
    pub extension: Option<String>,
    pub kind: FileItemKind,
    pub size: Option<u64>,
    pub created: Option<SystemTime>,
    pub modified: Option<SystemTime>,
    pub accessed: Option<SystemTime>,
    pub is_hidden: bool,
    pub is_system: bool,
    pub is_readonly: bool,
    pub is_symlink: bool,
    /// Tags assigned in config (not persisted on disk).
    pub tags: Vec<TagRef>,
    /// Network category (only set for items in the Shell Network virtual folder).
    pub network_category: Option<String>,
    /// Localized group title for the network category (System.ItemTypeText when available).
    pub network_category_label: Option<String>,
}

impl FileItem {
    pub fn from_path(path: PathBuf, options: DirectoryReadOptions) -> anyhow::Result<Self> {
        let metadata = std::fs::symlink_metadata(&path)?;
        let file_type = metadata.file_type();
        let is_symlink = file_type.is_symlink();
        let kind = if file_type.is_dir() {
            FileItemKind::Folder
        } else if file_type.is_file() {
            FileItemKind::File
        } else if is_symlink {
            FileItemKind::Symlink
        } else {
            FileItemKind::Other
        };

        let name_raw = path
            .file_name()
            .unwrap_or_else(|| OsStr::new(""))
            .to_string_lossy()
            .to_string();
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .filter(|value| !value.is_empty())
            .map(|value| value.to_string());
        let display_name = display_name_for(&path, kind, options.show_file_extensions);
        let is_hidden = is_hidden_path(&path, &metadata);
        let is_system = is_system_path(&metadata);

        Ok(Self {
            path,
            name_raw,
            display_name,
            extension,
            kind,
            size: (kind == FileItemKind::File).then_some(metadata.len()),
            created: metadata.created().ok(),
            modified: metadata.modified().ok(),
            accessed: metadata.accessed().ok(),
            is_hidden,
            is_system,
            is_readonly: metadata.permissions().readonly(),
            is_symlink,
            tags: Vec::new(),
            network_category: None,
            network_category_label: None,
        })
    }

    pub fn is_folder(&self) -> bool {
        self.kind == FileItemKind::Folder
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DirectoryReadOptions {
    pub show_hidden_items: bool,
    pub show_system_items: bool,
    pub show_dot_files: bool,
    pub show_file_extensions: bool,
}

impl Default for DirectoryReadOptions {
    fn default() -> Self {
        Self {
            show_hidden_items: false,
            show_system_items: false,
            show_dot_files: false,
            show_file_extensions: true,
        }
    }
}

fn display_name_for(path: &Path, kind: FileItemKind, show_file_extensions: bool) -> String {
    let name = path
        .file_name()
        .unwrap_or_else(|| OsStr::new(""))
        .to_string_lossy()
        .to_string();

    if show_file_extensions || kind != FileItemKind::File {
        return name;
    }

    path.file_stem()
        .and_then(|value| value.to_str())
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .unwrap_or(name)
}

pub(crate) fn should_include_item(item: &FileItem, options: DirectoryReadOptions) -> bool {
    if item.is_hidden && !options.show_hidden_items {
        return false;
    }

    if item.is_system && !options.show_system_items {
        return false;
    }

    if item.name_raw.starts_with('.') && !options.show_dot_files {
        return false;
    }

    true
}

#[cfg(windows)]
fn is_hidden_path(_: &Path, metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    metadata.file_attributes() & 0x2 != 0
}

#[cfg(not(windows))]
fn is_hidden_path(path: &Path, _: &std::fs::Metadata) -> bool {
    path.file_name()
        .and_then(|value| value.to_str())
        .is_some_and(|name| name.starts_with('.'))
}

#[cfg(windows)]
fn is_system_path(metadata: &std::fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;

    metadata.file_attributes() & 0x4 != 0
}

#[cfg(not(windows))]
fn is_system_path(_: &std::fs::Metadata) -> bool {
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_display_name_hides_extension_when_configured() {
        let path = PathBuf::from("report.final.txt");

        assert_eq!(
            display_name_for(&path, FileItemKind::File, false),
            "report.final"
        );
    }

    #[test]
    fn folder_display_name_keeps_extension_like_suffix() {
        let path = PathBuf::from("folder.name");

        assert_eq!(
            display_name_for(&path, FileItemKind::Folder, false),
            "folder.name"
        );
    }
}
