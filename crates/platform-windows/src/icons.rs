use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShellIconHint {
    Folder,
    File,
    Symlink,
    Executable,
    Image,
    Archive,
}

const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "gif", "bmp", "webp", "ico", "tif"];
const ARCHIVE_EXTENSIONS: &[&str] = &["zip", "rar", "7z", "tar", "gz", "bz2", "xz"];
const EXECUTABLE_EXTENSIONS: &[&str] = &["exe", "msi", "bat", "cmd", "com", "ps1"];

/// Maps a file extension to a shared icon category (no I/O).
pub fn icon_hint_from_extension(ext: Option<&str>) -> ShellIconHint {
    let ext = ext.and_then(|e| {
        let lower = e.to_ascii_lowercase();
        if lower.is_empty() {
            None
        } else {
            Some(lower)
        }
    });

    match ext.as_deref() {
        Some(e) if IMAGE_EXTENSIONS.contains(&e) => ShellIconHint::Image,
        Some(e) if ARCHIVE_EXTENSIONS.contains(&e) => ShellIconHint::Archive,
        Some(e) if EXECUTABLE_EXTENSIONS.contains(&e) => ShellIconHint::Executable,
        _ => ShellIconHint::File,
    }
}

/// Dummy path for extension-keyed list icons (Files `IconCacheService._dummyPath`).
pub fn shell_dummy_icon_path(cache_key: &str) -> PathBuf {
    let drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".to_string());
    let base = PathBuf::from(format!(r"{drive}\x46696c6573"));
    match cache_key {
        ":folder:" | ":symlink:" | ":noext:" => base,
        ext => PathBuf::from(format!("{}{ext}", base.display())),
    }
}

/// Maps a path to a UI icon category (full Shell icon bitmaps are future work).
pub fn icon_hint_for_path(path: &Path) -> ShellIconHint {
    if path.is_dir() {
        return ShellIconHint::Folder;
    }

    let metadata = std::fs::symlink_metadata(path).ok();
    if metadata
        .as_ref()
        .is_some_and(|m| m.file_type().is_symlink())
    {
        return ShellIconHint::Symlink;
    }

    icon_hint_from_extension(path.extension().and_then(|e| e.to_str()))
}
