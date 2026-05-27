//! Create zip archives (Rust `zip` crate; per-item progress for the status bar).

use std::fs::File;
use std::io::{Seek, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

/// Returned when the user cancels during compression.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompressCancelled;

impl std::fmt::Display for CompressCancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "compress cancelled")
    }
}

impl std::error::Error for CompressCancelled {}

/// Builds `destination_dir / {name}.zip` containing all `sources`.
pub fn compress_paths_to_zip(
    sources: &[PathBuf],
    destination_dir: &Path,
) -> anyhow::Result<PathBuf> {
    let zip_path = unique_zip_output_path(sources, destination_dir)?;
    compress_paths_to_zip_at_path_cancellable(
        sources,
        &zip_path,
        &AtomicBool::new(false),
        |_, _| {},
    )
}

/// Resolves the default final zip path for `sources` before any conflict handling.
pub fn zip_output_path(sources: &[PathBuf], destination_dir: &Path) -> anyhow::Result<PathBuf> {
    if sources.is_empty() {
        anyhow::bail!("no paths to compress");
    }
    if !destination_dir.is_dir() {
        anyhow::bail!("destination is not a directory");
    }
    Ok(destination_dir.join(zip_file_name(sources)))
}

/// Resolves a non-conflicting final zip path for `sources`.
pub fn unique_zip_output_path(sources: &[PathBuf], destination_dir: &Path) -> anyhow::Result<PathBuf> {
    let base_path = zip_output_path(sources, destination_dir)?;
    Ok(unique_zip_path(base_path))
}

/// Resolves the temporary partial path used while compressing before the final rename.
pub fn temp_zip_output_path(zip_path: &Path) -> PathBuf {
    temp_zip_path(zip_path)
}

/// Like [`compress_paths_to_zip`], but checks `cancel` and reports `on_progress(completed, total)`.
pub fn compress_paths_to_zip_cancellable(
    sources: &[PathBuf],
    destination_dir: &Path,
    cancel: &AtomicBool,
    on_progress: impl FnMut(u32, u32),
) -> anyhow::Result<PathBuf> {
    if cancel.load(Ordering::Relaxed) {
        return Err(CompressCancelled.into());
    }
    let zip_path = unique_zip_output_path(sources, destination_dir)?;
    compress_paths_to_zip_at_path_cancellable(sources, &zip_path, cancel, on_progress)
}

/// Like [`compress_paths_to_zip_cancellable`], but writes to a caller-selected final zip path.
pub fn compress_paths_to_zip_at_path_cancellable(
    sources: &[PathBuf],
    zip_path: &Path,
    cancel: &AtomicBool,
    mut on_progress: impl FnMut(u32, u32),
) -> anyhow::Result<PathBuf> {
    if cancel.load(Ordering::Relaxed) {
        return Err(CompressCancelled.into());
    }
    compress_paths_to_zip_impl(sources, zip_path, cancel, &mut on_progress)
}

fn compress_paths_to_zip_impl(
    sources: &[PathBuf],
    zip_path: &Path,
    cancel: &AtomicBool,
    on_progress: &mut dyn FnMut(u32, u32),
) -> anyhow::Result<PathBuf> {
    if sources.is_empty() {
        anyhow::bail!("no paths to compress");
    }
    let destination_dir = zip_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("zip path has no parent directory"))?;
    if !destination_dir.is_dir() {
        anyhow::bail!("destination is not a directory");
    }

    for source in sources {
        if !source.exists() {
            anyhow::bail!("path not found: {}", source.display());
        }
    }

    let total = sources.len() as u32;
    on_progress(0, total);

    let temp_zip_path = temp_zip_path(zip_path);
    if temp_zip_path.exists() {
        std::fs::remove_file(&temp_zip_path)?;
    }

    let file = File::create(&temp_zip_path)?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);

    for (index, source) in sources.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            let _ = std::fs::remove_file(&temp_zip_path);
            return Err(CompressCancelled.into());
        }
        let entry_name = source
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("invalid source path {}", source.display()))?
            .to_string_lossy()
            .into_owned();
        write_zip_tree(&mut zip, source, &entry_name, &options, cancel)?;
        on_progress((index + 1) as u32, total);
    }

    zip.finish()?;
    if zip_path.exists() {
        anyhow::bail!("target already exists: {}", zip_path.display());
    }
    std::fs::rename(&temp_zip_path, zip_path)?;
    Ok(zip_path.to_path_buf())
}

fn write_zip_tree<W: Write + Seek>(
    zip: &mut ZipWriter<W>,
    path: &Path,
    name_in_archive: &str,
    options: &SimpleFileOptions,
    cancel: &AtomicBool,
) -> anyhow::Result<()> {
    if cancel.load(Ordering::Relaxed) {
        return Err(CompressCancelled.into());
    }

    if path.is_file() {
        zip.start_file(name_in_archive, *options)?;
        let mut file = File::open(path)?;
        std::io::copy(&mut file, zip)?;
        return Ok(());
    }

    if path.is_dir() {
        let mut entries = std::fs::read_dir(path)?.collect::<Result<Vec<_>, _>>()?;
        entries.sort_by_key(|e| e.file_name());
        if entries.is_empty() {
            zip.add_directory(format!("{name_in_archive}/"), *options)?;
            return Ok(());
        }
        for entry in entries {
            if cancel.load(Ordering::Relaxed) {
                return Err(CompressCancelled.into());
            }
            let child_path = entry.path();
            let child_name = entry.file_name().to_string_lossy().into_owned();
            let relative = format!("{name_in_archive}/{child_name}");
            write_zip_tree(zip, &child_path, &relative, options, cancel)?;
        }
        return Ok(());
    }

    anyhow::bail!("unsupported path type: {}", path.display())
}

fn zip_file_name(sources: &[PathBuf]) -> String {
    if sources.len() == 1 {
        let stem = sources[0]
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Archive".into());
        return format!("{stem}.zip");
    }
    "Archive.zip".into()
}

fn temp_zip_path(zip_path: &Path) -> PathBuf {
    let file_name = zip_path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Archive.zip".to_string());
    zip_path.with_file_name(format!("{file_name}.partial"))
}

fn unique_zip_path(base_path: PathBuf) -> PathBuf {
    if !base_path.exists() && !temp_zip_path(&base_path).exists() {
        return base_path;
    }

    let parent = base_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default();
    let stem = base_path
        .file_stem()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "Archive".into());
    let ext = base_path
        .extension()
        .map(|ext| ext.to_string_lossy().into_owned())
        .filter(|ext| !ext.is_empty())
        .unwrap_or_else(|| "zip".into());

    for index in 2.. {
        let candidate = parent.join(format!("{stem} ({index}).{ext}"));
        if !candidate.exists() && !temp_zip_path(&candidate).exists() {
            return candidate;
        }
    }

    unreachable!()
}
