use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// Returned when the user cancels during deletion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DeleteCancelled;

impl std::fmt::Display for DeleteCancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "delete cancelled")
    }
}

impl std::error::Error for DeleteCancelled {}

pub fn create_directory(parent: &Path, name: &str) -> anyhow::Result<PathBuf> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        anyhow::bail!("folder name cannot be empty");
    }

    let path = parent.join(trimmed);
    if path.exists() {
        anyhow::bail!("{} already exists", path.display());
    }

    std::fs::create_dir(&path)?;
    Ok(path)
}

pub fn rename_path(path: &Path, new_name: &str) -> anyhow::Result<PathBuf> {
    let trimmed = new_name.trim();
    if trimmed.is_empty() {
        anyhow::bail!("name cannot be empty");
    }

    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("cannot rename {}", path.display()))?;
    let target = parent.join(trimmed);

    if target == path {
        return Ok(target);
    }

    if target.exists() {
        anyhow::bail!("{} already exists", target.display());
    }

    std::fs::rename(path, &target)?;
    Ok(target)
}

/// Counts files and directories that would be removed (each path counts as one item).
pub fn count_delete_items(paths: &[PathBuf]) -> u32 {
    paths
        .iter()
        .map(|path| count_delete_tree(path))
        .sum::<u32>()
        .max(1)
}

fn count_delete_tree(path: &Path) -> u32 {
    let Ok(meta) = std::fs::symlink_metadata(path) else {
        return 1;
    };
    if !meta.is_dir() || meta.file_type().is_symlink() {
        return 1;
    }
    let mut count = 1u32;
    if let Ok(entries) = std::fs::read_dir(path) {
        for entry in entries.flatten() {
            count += count_delete_tree(&entry.path());
        }
    }
    count
}

/// Permanently deletes paths (no recycle bin).
pub fn delete_paths(paths: &[PathBuf]) -> anyhow::Result<()> {
    delete_paths_cancellable(paths, &AtomicBool::new(false), |_, _| {})
}

/// Like [`delete_paths`], but checks `cancel` between items and reports progress.
pub fn delete_paths_cancellable(
    paths: &[PathBuf],
    cancel: &AtomicBool,
    mut on_progress: impl FnMut(u32, u32),
) -> anyhow::Result<()> {
    let total = count_delete_items(paths);
    let mut completed = 0u32;
    on_progress(completed, total);
    for path in paths {
        delete_tree_cancellable(path, cancel, &mut completed, total, &mut on_progress)?;
    }
    Ok(())
}

/// Sends paths to the system recycle bin when supported.
pub fn recycle_paths(paths: &[PathBuf]) -> anyhow::Result<()> {
    recycle_paths_cancellable(paths, &AtomicBool::new(false), |_, _| {})
}

/// Like [`recycle_paths`], but checks `cancel` between items and reports progress.
pub fn recycle_paths_cancellable(
    paths: &[PathBuf],
    cancel: &AtomicBool,
    mut on_progress: impl FnMut(u32, u32),
) -> anyhow::Result<()> {
    #[cfg(windows)]
    {
        let total = count_delete_items(paths);
        let mut completed = 0u32;
        on_progress(completed, total);
        for path in paths {
            recycle_tree_cancellable(path, cancel, &mut completed, total, &mut on_progress)?;
        }
        Ok(())
    }

    #[cfg(not(windows))]
    {
        delete_paths_cancellable(paths, cancel, on_progress)
    }
}

fn delete_tree_cancellable(
    path: &Path,
    cancel: &AtomicBool,
    completed: &mut u32,
    total: u32,
    on_progress: &mut dyn FnMut(u32, u32),
) -> anyhow::Result<()> {
    if cancel.load(Ordering::Relaxed) {
        return Err(DeleteCancelled.into());
    }

    let meta = std::fs::symlink_metadata(path)
        .map_err(|error| anyhow::anyhow!("stat {}: {error}", path.display()))?;
    if meta.is_dir() && !meta.file_type().is_symlink() {
        let entries = std::fs::read_dir(path)?.collect::<Result<Vec<_>, _>>()?;
        for entry in entries {
            delete_tree_cancellable(&entry.path(), cancel, completed, total, on_progress)?;
        }
        std::fs::remove_dir(path)?;
    } else {
        std::fs::remove_file(path)?;
    }

    *completed += 1;
    on_progress(*completed, total);
    Ok(())
}

#[cfg(windows)]
fn recycle_tree_cancellable(
    path: &Path,
    cancel: &AtomicBool,
    completed: &mut u32,
    total: u32,
    on_progress: &mut dyn FnMut(u32, u32),
) -> anyhow::Result<()> {
    if cancel.load(Ordering::Relaxed) {
        return Err(DeleteCancelled.into());
    }

    let meta = std::fs::symlink_metadata(path)
        .map_err(|error| anyhow::anyhow!("stat {}: {error}", path.display()))?;
    if meta.is_dir() && !meta.file_type().is_symlink() {
        let entries = std::fs::read_dir(path)?.collect::<Result<Vec<_>, _>>()?;
        for entry in entries {
            recycle_tree_cancellable(&entry.path(), cancel, completed, total, on_progress)?;
        }
        trash::delete(path)?;
    } else {
        trash::delete(path)?;
    }

    *completed += 1;
    on_progress(*completed, total);
    Ok(())
}

pub fn create_file(parent: &Path, name: &str) -> anyhow::Result<PathBuf> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        anyhow::bail!("file name cannot be empty");
    }

    let path = parent.join(trimmed);
    if path.exists() {
        anyhow::bail!("{} already exists", path.display());
    }

    std::fs::write(&path, [])?;
    Ok(path)
}

pub fn unique_new_file_name(parent: &Path) -> String {
    let base = "New Text Document.txt";
    let mut candidate = base.to_string();
    let mut counter = 2;

    while parent.join(&candidate).exists() {
        candidate = format!("New Text Document ({counter}).txt");
        counter += 1;
    }

    candidate
}

pub fn unique_new_folder_name(parent: &Path) -> String {
    let base = "New folder";
    let mut candidate = base.to_string();
    let mut counter = 2;

    while parent.join(&candidate).exists() {
        candidate = format!("{base} ({counter})");
        counter += 1;
    }

    candidate
}

#[cfg(test)]
mod delete_tests {
    use super::*;
    use std::fs;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn count_delete_items_includes_nested_files() {
        let root = std::env::temp_dir().join("cyberfiles_delete_count_test");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("a/b")).unwrap();
        fs::write(root.join("a/b/file.txt"), b"x").unwrap();
        fs::write(root.join("top.txt"), b"y").unwrap();

        assert_eq!(count_delete_items(&[root.clone()]), 5);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn delete_paths_cancellable_reports_progress() {
        let root = std::env::temp_dir().join("cyberfiles_delete_progress_test");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("nested")).unwrap();
        fs::write(root.join("nested/a.txt"), b"a").unwrap();
        fs::write(root.join("nested/b.txt"), b"b").unwrap();

        let mut last = (0u32, 0u32);
        delete_paths_cancellable(&[root.clone()], &AtomicBool::new(false), |done, total| {
            last = (done, total);
        })
        .unwrap();
        assert_eq!(last, (4, 4));
        assert!(!root.exists());
    }
}
