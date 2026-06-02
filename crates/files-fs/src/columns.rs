use std::path::{Path, PathBuf};

/// Builds the ancestor path chain for column view (e.g. `C:\` → `C:\Users` → … → current).
pub fn column_trail_for(path: &Path) -> Vec<PathBuf> {
    let mut trail = Vec::new();
    let mut acc = PathBuf::new();
    for component in path.components() {
        acc.push(component);
        if acc.as_os_str().is_empty() {
            continue;
        }
        // Skip Windows drive-relative paths like "C:" (without a root dir).
        // They represent the current directory on that drive, not a real directory.
        #[cfg(windows)]
        if acc.components().count() == 1
            && acc
                .components()
                .next()
                .map(|c| matches!(c, std::path::Component::Prefix(_)))
                .unwrap_or(false)
        {
            continue;
        }
        trail.push(acc.clone());
    }
    if trail.is_empty() {
        trail.push(path.to_path_buf());
    } else if trail.last().map(|p| p.as_path()) != Some(path) {
        trail.push(path.to_path_buf());
    }
    trail
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn column_trail_includes_each_prefix() {
        let path = PathBuf::from(r"C:\Users\test");
        let trail = column_trail_for(&path);
        assert!(!trail.is_empty());
        assert_eq!(trail.last().map(|p| p.as_path()), Some(path.as_path()));
    }

    #[test]
    #[cfg(windows)]
    fn column_trail_skips_drive_relative_prefix() {
        let path = PathBuf::from(r"C:\Users\test");
        let trail = column_trail_for(&path);
        // "C:" (drive-relative) must not appear; first entry should be "C:\"
        assert_eq!(trail.first().map(|p| p.as_path()), Some(Path::new(r"C:\")));
        assert!(!trail.iter().any(|p| p.as_path() == Path::new(r"C:")));
    }
}
