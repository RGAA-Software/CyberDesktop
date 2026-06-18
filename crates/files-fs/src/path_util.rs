use std::path::{Path, PathBuf};

/// Normalize a path for comparison (slashes, verbatim prefix on Windows).
pub fn normalize_path(path: &Path) -> PathBuf {
    let mut text = path.to_string_lossy().to_string();
    #[cfg(windows)]
    {
        if let Some(stripped) = text.strip_prefix(r"\\?\") {
            text = stripped.to_string();
        }
        text = text.replace('/', "\\");
    }
    PathBuf::from(text)
}

/// Compare two paths, treating Windows drive roots like `C:` and `C:\` as equal.
pub fn paths_equal_directory(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    normalize_directory_path(a) == normalize_directory_path(b)
}

/// Compare two paths after normalization (handles mixed slashes and `\\?\` prefixes).
pub fn paths_equal(a: &Path, b: &Path) -> bool {
    if a == b {
        return true;
    }
    normalize_path(a) == normalize_path(b)
}

/// True when `child` is a direct child of `dir` (not nested deeper).
pub fn is_direct_child_of(child: &Path, dir: &Path) -> bool {
    child
        .parent()
        .is_some_and(|parent| paths_equal_directory(parent, dir))
}

/// True when every path is already a direct child of `dir`.
pub fn all_direct_children_of(paths: &[PathBuf], dir: &Path) -> bool {
    !paths.is_empty() && paths.iter().all(|path| is_direct_child_of(path, dir))
}

/// Drive or UNC share root for Files-style `AreItemsInSameDrive` checks.
pub fn path_drive_root(path: &Path) -> Option<String> {
    let normalized = normalize_path(path);
    let text = normalized.to_string_lossy().into_owned();
    #[cfg(windows)]
    {
        let bytes = text.as_bytes();
        if bytes.len() >= 2 && bytes[1] == b':' {
            return Some(text[..2].to_ascii_uppercase());
        }
        if text.starts_with(r"\\") {
            let trimmed = text.trim_start_matches('\\');
            let parts: Vec<&str> = trimmed
                .split('\\')
                .filter(|part| !part.is_empty())
                .collect();
            if parts.len() >= 2 {
                return Some(format!(r"\\{}\{}", parts[0], parts[1]));
            }
        }
        return None;
    }
    #[cfg(not(windows))]
    {
        normalized
            .components()
            .next()
            .map(|component| component.as_os_str().to_string_lossy().into_owned())
    }
}

/// True when `path` is a WSL UNC path (`\\wsl$\` or `\\wsl.localhost\`).
pub fn is_wsl_path(path: &Path) -> bool {
    let text = path.to_string_lossy();
    // Strip the verbatim prefix if present (`\\?\UNC\wsl$\...` -> `\\wsl$\...`).
    let text = text.strip_prefix(r"\\?\UNC\").unwrap_or(&text);
    let lower = text.to_ascii_lowercase();
    lower.starts_with(r"\\wsl$\") || lower.starts_with(r"\\wsl.localhost\")
}

/// True when `path` points at a network computer root such as `\\COMPUTER`
/// or `\\?\UNC\COMPUTER` (as opposed to a share like `\\COMPUTER\SHARE`).
#[cfg(windows)]
pub fn is_network_computer_root(path: &Path) -> bool {
    let s = path.to_string_lossy();
    let stripped = s.strip_prefix(r"\\?\UNC\").unwrap_or(&s);
    if !stripped.starts_with(r"\\") {
        return false;
    }
    let after_server = &stripped[2..];
    !after_server.contains('\\') && !after_server.is_empty()
}

#[cfg(not(windows))]
pub fn is_network_computer_root(_path: &Path) -> bool {
    false
}

/// True when any source path shares a drive/share root with `destination` (Files `AreItemsInSameDrive`).
pub fn are_paths_on_same_drive(source_paths: &[PathBuf], destination: &Path) -> bool {
    let Some(dest_root) = path_drive_root(destination) else {
        return false;
    };
    source_paths
        .iter()
        .any(|path| path_drive_root(path).as_deref() == Some(dest_root.as_str()))
}

pub fn normalize_directory_path(path: &Path) -> PathBuf {
    let mut text = normalize_path(path).to_string_lossy().to_string();
    while text.len() > 1 && text.ends_with('\\') {
        text.pop();
    }
    #[cfg(windows)]
    {
        let bytes = text.as_bytes();
        if bytes.len() == 2 && bytes[1] == b':' {
            text.push('\\');
        }
    }
    PathBuf::from(text)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    #[cfg(windows)]
    fn drive_root_directory_equality() {
        assert!(paths_equal_directory(Path::new(r"C:\"), Path::new(r"C:")));
        assert!(paths_equal_directory(Path::new(r"C:"), Path::new(r"C:\")));
        assert!(paths_equal_directory(Path::new(r"C:/"), Path::new(r"C:\")));
    }

    #[test]
    #[cfg(windows)]
    fn direct_child_of_drive_root() {
        let root = Path::new(r"C:\");
        let child = Path::new(r"C:\Windows");
        assert!(is_direct_child_of(child, root));
        assert!(is_direct_child_of(child, Path::new(r"C:")));
        assert!(is_direct_child_of(Path::new(r"C:/Windows"), root));
        assert!(all_direct_children_of(&[child.to_path_buf()], root));
    }

    #[test]
    #[cfg(windows)]
    fn nested_path_is_not_direct_child_of_root() {
        let root = Path::new(r"C:\");
        let nested = Path::new(r"C:\Users\Public");
        assert!(!is_direct_child_of(nested, root));
    }

    #[test]
    #[cfg(windows)]
    fn paths_equal_mixed_slashes() {
        assert!(paths_equal(
            Path::new(r"C:\Users\test"),
            Path::new(r"C:/Users/test"),
        ));
    }

    #[test]
    #[cfg(windows)]
    fn same_drive_detection() {
        let sources = vec![PathBuf::from(r"D:\a\file.txt")];
        assert!(are_paths_on_same_drive(&sources, Path::new(r"D:\b\folder"),));
        assert!(!are_paths_on_same_drive(
            &sources,
            Path::new(r"E:\b\folder"),
        ));
    }
}
