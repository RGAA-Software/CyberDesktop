use std::path::{Path, PathBuf};

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

/// Permanently deletes paths (no recycle bin).
pub fn delete_paths(paths: &[PathBuf]) -> anyhow::Result<()> {
    for path in paths {
        crate::clipboard::remove_path_recursive(path)?;
    }
    Ok(())
}

/// Sends paths to the system recycle bin when supported.
pub fn recycle_paths(paths: &[PathBuf]) -> anyhow::Result<()> {
    #[cfg(windows)]
    {
        for path in paths {
            trash::delete(path)?;
        }
        Ok(())
    }

    #[cfg(not(windows))]
    {
        delete_paths(paths)
    }
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
