//! Create and extract zip/7z/tar/rar archives.
//!
//! On Windows, bundled `7z.dll` is loaded in-process (7-Zip COM API, UTF-16 paths, multi-thread).
//! RAR uses UnRAR via `unrar-ng` (`7z.dll` cannot open `.rar`).

use std::fs::File;
use std::io::{copy, Read, Seek, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use sevenz_rust::{decompress_file_with_extract_fn, Password, SevenZReader};

use zip::read::ZipArchive;
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

/// Returned when the user cancels during extraction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExtractCancelled;

impl std::fmt::Display for ExtractCancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "extract cancelled")
    }
}

impl std::error::Error for ExtractCancelled {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArchiveFormat {
    Zip,
    SevenZip,
    Rar,
    Tar,
    TarGz,
    TarBz2,
    TarXz,
}

/// Detect archive type from the file name extension.
pub fn detect_archive_format(path: &Path) -> Option<ArchiveFormat> {
    let name = path.file_name()?.to_string_lossy();
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".tar.gz") || lower.ends_with(".tgz") {
        return Some(ArchiveFormat::TarGz);
    }
    if lower.ends_with(".tar.bz2") || lower.ends_with(".tbz2") {
        return Some(ArchiveFormat::TarBz2);
    }
    if lower.ends_with(".tar.xz") || lower.ends_with(".txz") {
        return Some(ArchiveFormat::TarXz);
    }
    if lower.ends_with(".cbz") {
        return Some(ArchiveFormat::Zip);
    }
    if lower.ends_with(".cbr") {
        return Some(ArchiveFormat::Rar);
    }
    if lower.ends_with(".rar") {
        return Some(ArchiveFormat::Rar);
    }
    match path
        .extension()
        .map(|ext| ext.to_string_lossy().to_ascii_lowercase())
        .as_deref()
    {
        Some("zip") => Some(ArchiveFormat::Zip),
        Some("7z") => Some(ArchiveFormat::SevenZip),
        Some("tar") => Some(ArchiveFormat::Tar),
        ext if ext.is_some_and(is_rar_volume_extension) => Some(ArchiveFormat::Rar),
        _ => None,
    }
}

/// RAR multipart volumes use `.r00` … `.r99` (and `.rar` for the first part).
fn is_rar_volume_extension(ext: &str) -> bool {
    ext.len() == 3 && ext.starts_with('r') && ext[1..].chars().all(|c| c.is_ascii_digit())
}

/// True when `path` looks like an extractable archive (name-based; safe for context menus).
pub fn is_archive_path(path: &Path) -> bool {
    !path.is_dir() && detect_archive_format(path).is_some()
}

/// Bundled in-process `7z.dll` handles zip/7z/tar/etc. RAR uses UnRAR (`unrar-ng`).
fn uses_7zip_engine(format: ArchiveFormat) -> bool {
    !matches!(format, ArchiveFormat::Rar)
}

/// Progress denominator for status UI (ZIP/7z entry count; tar/rar report 1).
pub fn archive_progress_total(path: &Path) -> u32 {
    let started = Instant::now();
    let format = detect_archive_format(path);
    let total = match format {
        Some(ArchiveFormat::Zip) => File::open(path)
            .ok()
            .and_then(|file| ZipArchive::new(file).ok())
            .map(|archive| archive.len() as u32)
            .unwrap_or(1)
            .max(1),
        Some(ArchiveFormat::SevenZip) => count_7z_entries(path).unwrap_or(1).max(1),
        Some(ArchiveFormat::Rar) => 1,
        Some(ArchiveFormat::Tar)
        | Some(ArchiveFormat::TarGz)
        | Some(ArchiveFormat::TarBz2)
        | Some(ArchiveFormat::TarXz) => 1,
        None => 1,
    };
    extract_log(format!(
        "archive_progress_total path={} format={format:?} entries={total} elapsed={:?}",
        path.display(),
        started.elapsed()
    ));
    total
}

fn extract_log(message: impl AsRef<str>) {
    tracing::debug!(target: "extract", "{}", message.as_ref());
}

/// Log bundled 7-Zip layout at startup (Windows). Call from `main`.
#[cfg(windows)]
pub fn log_extract_environment() {
    extract_log("=== extract environment ===");
    match std::env::current_exe() {
        Ok(exe) => {
            extract_log(format!("current_exe={}", exe.display()));
            if let Some(dir) = exe.parent() {
                let dll = dir.join("7z.dll");
                let status = if dll.is_file() {
                    format!(
                        "present size={}",
                        dll.metadata().map(|m| m.len()).unwrap_or(0)
                    )
                } else {
                    "missing".into()
                };
                extract_log(format!("  7z.dll: {status}"));
            }
        }
        Err(error) => extract_log(format!("current_exe unavailable: {error}")),
    }
    if let Some(dll) = app_platform_windows::bundled_dll_path() {
        extract_log(format!("bundled 7z.dll: {}", dll.display()));
    } else {
        extract_log("bundled 7z.dll not found next to executable");
    }
    extract_log(format!(
        "logical_cpus={}",
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
    ));
}

#[cfg(not(windows))]
pub fn log_extract_environment() {}

/// Subfolder named like the archive (handles `.tar.gz` / `.tgz`), uniqued on conflict.
pub fn extract_to_child_dir(archive: &Path, parent_dir: &Path) -> PathBuf {
    unique_extract_dir(parent_dir.join(archive_stem(archive)))
}

/// Extract `archive` into `dest_dir` (must already exist).
pub fn extract_archive_cancellable(
    archive: &Path,
    dest_dir: &Path,
    cancel: &AtomicBool,
    mut on_progress: impl FnMut(u32, u32),
) -> anyhow::Result<()> {
    if cancel.load(Ordering::Relaxed) {
        return Err(ExtractCancelled.into());
    }
    if !archive.is_file() {
        anyhow::bail!("archive not found: {}", archive.display());
    }
    if !dest_dir.is_dir() {
        anyhow::bail!("destination is not a directory: {}", dest_dir.display());
    }
    let format = detect_archive_format(archive)
        .ok_or_else(|| anyhow::anyhow!("unsupported archive: {}", archive.display()))?;
    let archive_size = std::fs::metadata(archive).map(|m| m.len()).unwrap_or(0);
    let progress_total = archive_progress_total(archive);
    extract_log(format!(
        "begin archive={} size={archive_size} dest={} format={format:?} progress_total={progress_total}",
        archive.display(),
        dest_dir.display()
    ));
    #[cfg(windows)]
    if uses_7zip_engine(format) {
        if let Some(dll) = app_platform_windows::bundled_dll_path() {
            extract_log(format!(
                "7-Zip in-process dll={} threads={}",
                dll.display(),
                std::thread::available_parallelism()
                    .map(|n| n.get())
                    .unwrap_or(1)
            ));
            match extract_via_7zip_dll(
                &dll,
                archive,
                dest_dir,
                cancel,
                progress_total,
                &mut on_progress,
            ) {
                Ok(()) => {
                    extract_log("7-Zip in-process extract success");
                    return Ok(());
                }
                Err(error) if error.is::<ExtractCancelled>() => return Err(error),
                Err(error) => {
                    extract_log(format!(
                        "7-Zip in-process failed archive={}: {error:#}",
                        archive.display()
                    ));
                }
            }
        } else {
            extract_log("7z.dll missing; skipping in-process 7-Zip");
        }
        extract_log(format!(
            "falling back to native ({format:?}) for archive={}",
            archive.display()
        ));
    }
    let native_started = Instant::now();
    extract_log(format!("native extract backend={format:?}"));
    let result = match format {
        ArchiveFormat::Zip => extract_zip(archive, dest_dir, cancel, &mut on_progress),
        ArchiveFormat::SevenZip => extract_sevenz_rust(archive, dest_dir, cancel, &mut on_progress),
        ArchiveFormat::Rar => extract_rar(archive, dest_dir, cancel, &mut on_progress),
        ArchiveFormat::Tar => extract_tar(archive, dest_dir, cancel, &mut on_progress),
        ArchiveFormat::TarGz => extract_tar_gz(archive, dest_dir, cancel, &mut on_progress),
        ArchiveFormat::TarBz2 => extract_tar_bz2(archive, dest_dir, cancel, &mut on_progress),
        ArchiveFormat::TarXz => extract_tar_xz(archive, dest_dir, cancel, &mut on_progress),
    };
    if let Err(error) = &result {
        extract_log(format!(
            "native extract failed archive={} elapsed={:?}: {error:#}",
            archive.display(),
            native_started.elapsed()
        ));
    } else {
        extract_log(format!(
            "native extract done archive={} elapsed={:?}",
            archive.display(),
            native_started.elapsed()
        ));
    }
    result
}

fn extract_zip(
    archive: &Path,
    dest_dir: &Path,
    cancel: &AtomicBool,
    on_progress: &mut dyn FnMut(u32, u32),
) -> anyhow::Result<()> {
    let open_started = Instant::now();
    let file = File::open(archive)?;
    let mut zip = ZipArchive::new(file)?;
    let total = zip.len() as u32;
    extract_log(format!(
        "rust zip opened entries={total} open_elapsed={:?}",
        open_started.elapsed()
    ));
    on_progress(0, total.max(1));
    let extract_started = Instant::now();
    let mut last_log = Instant::now();
    for index in 0..zip.len() {
        if cancel.load(Ordering::Relaxed) {
            return Err(ExtractCancelled.into());
        }
        let mut entry = zip.by_index(index)?;
        let entry_name = entry.name().to_string();
        let Some(relative) = safe_zip_entry_path(&entry) else {
            anyhow::bail!("unsafe zip entry path: {entry_name}");
        };
        let out_path = dest_dir.join(&relative);
        if entry.is_dir() {
            std::fs::create_dir_all(&out_path)?;
        } else {
            if let Some(parent) = out_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            if out_path.exists() && out_path.is_dir() {
                anyhow::bail!(
                    "cannot extract file over existing directory: {}",
                    out_path.display()
                );
            }
            let mut out_file = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&out_path)?;
            copy(&mut entry, &mut out_file)?;
        }
        let step = (index + 1) as u32;
        on_progress(step, total.max(1));
        if step == 1 || step == total || last_log.elapsed() >= Duration::from_secs(2) {
            extract_log(format!(
                "rust zip progress {step}/{total} elapsed={:?}",
                extract_started.elapsed()
            ));
            last_log = Instant::now();
        }
    }
    extract_log(format!(
        "rust zip complete entries={total} elapsed={:?}",
        extract_started.elapsed()
    ));
    Ok(())
}

/// Normalize zip entry names from Windows / Unix archivers.
fn safe_zip_entry_path(entry: &zip::read::ZipFile<'_>) -> Option<PathBuf> {
    if let Some(path) = entry.enclosed_name() {
        return Some(path);
    }
    let normalized = entry.name().replace('\\', "/");
    let trimmed = normalized.trim_start_matches('/');
    if trimmed.is_empty() || trimmed.split('/').any(|part| part == "..") {
        return None;
    }
    Some(PathBuf::from(trimmed))
}

fn count_7z_entries(archive: &Path) -> anyhow::Result<u32> {
    let reader = SevenZReader::open(archive, Password::empty())
        .map_err(|error| anyhow::anyhow!("open 7z archive: {error}"))?;
    Ok(reader.archive().files.len() as u32)
}

#[cfg(windows)]
fn extract_via_7zip_dll(
    dll: &Path,
    archive: &Path,
    dest_dir: &Path,
    cancel: &AtomicBool,
    progress_total: u32,
    on_progress: &mut dyn FnMut(u32, u32),
) -> anyhow::Result<()> {
    let started = Instant::now();
    on_progress(0, progress_total);
    extract_log(format!(
        "7-Zip in-process archive={} dest={}",
        archive.display(),
        dest_dir.display()
    ));
    app_platform_windows::extract_in_process(dll, archive, dest_dir, cancel, on_progress).map_err(
        |error| match error {
            app_platform_windows::SevenZipExtractError::Cancelled => ExtractCancelled.into(),
            other => anyhow::anyhow!("{other}"),
        },
    )?;
    extract_log(format!(
        "7-Zip in-process finished elapsed={:?}",
        started.elapsed()
    ));
    Ok(())
}

fn extract_sevenz_rust(
    archive: &Path,
    dest_dir: &Path,
    cancel: &AtomicBool,
    on_progress: &mut dyn FnMut(u32, u32),
) -> anyhow::Result<()> {
    let entry_count = count_7z_entries(archive)?.max(1);
    on_progress(0, entry_count);
    let mut completed = 0u32;
    decompress_file_with_extract_fn(archive, dest_dir, |entry, reader, dest_path| {
        if cancel.load(Ordering::Relaxed) {
            return Ok(false);
        }
        if !is_safe_archive_entry_path(entry.name()) {
            return Err(sevenz_rust::Error::other(format!(
                "unsafe 7z entry path: {}",
                entry.name()
            )));
        }
        if entry.is_directory() {
            std::fs::create_dir_all(dest_path).map_err(sevenz_rust::Error::io)?;
        } else {
            if let Some(parent) = dest_path.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            let mut file = std::fs::File::create(dest_path).map_err(sevenz_rust::Error::io)?;
            if entry.size() > 0 {
                std::io::copy(reader, &mut file).map_err(sevenz_rust::Error::io)?;
            }
        }
        completed += 1;
        on_progress(completed, entry_count);
        Ok(true)
    })
    .map_err(|error| anyhow::anyhow!("7z extract failed: {error}"))?;
    Ok(())
}

fn is_safe_archive_entry_path(name: &str) -> bool {
    if name.contains('\0') {
        return false;
    }
    let normalized = name.replace('\\', "/");
    let path = Path::new(&normalized);
    !path.components().any(|component| {
        matches!(
            component,
            Component::ParentDir | Component::RootDir | Component::Prefix(_)
        )
    })
}

fn extract_tar(
    archive: &Path,
    dest_dir: &Path,
    cancel: &AtomicBool,
    on_progress: &mut dyn FnMut(u32, u32),
) -> anyhow::Result<()> {
    let file = File::open(archive)?;
    extract_tar_reader(file, dest_dir, cancel, on_progress)
}

fn extract_tar_gz(
    archive: &Path,
    dest_dir: &Path,
    cancel: &AtomicBool,
    on_progress: &mut dyn FnMut(u32, u32),
) -> anyhow::Result<()> {
    let file = File::open(archive)?;
    let decoder = flate2::read::GzDecoder::new(file);
    extract_tar_reader(decoder, dest_dir, cancel, on_progress)
}

fn extract_tar_bz2(
    archive: &Path,
    dest_dir: &Path,
    cancel: &AtomicBool,
    on_progress: &mut dyn FnMut(u32, u32),
) -> anyhow::Result<()> {
    let file = File::open(archive)?;
    let decoder = bzip2::read::BzDecoder::new(file);
    extract_tar_reader(decoder, dest_dir, cancel, on_progress)
}

fn extract_tar_xz(
    archive: &Path,
    dest_dir: &Path,
    cancel: &AtomicBool,
    on_progress: &mut dyn FnMut(u32, u32),
) -> anyhow::Result<()> {
    let file = File::open(archive)?;
    let decoder = xz2::read::XzDecoder::new(file);
    extract_tar_reader(decoder, dest_dir, cancel, on_progress)
}

fn extract_rar(
    archive: &Path,
    dest_dir: &Path,
    cancel: &AtomicBool,
    on_progress: &mut dyn FnMut(u32, u32),
) -> anyhow::Result<()> {
    use unrar_ng::{Archive, ExtractEvent, ExtractStatus};

    if cancel.load(Ordering::Relaxed) {
        return Err(ExtractCancelled.into());
    }
    on_progress(0, 1);
    let open = Archive::new(archive)
        .open_for_processing()
        .map_err(|error| anyhow::anyhow!("open rar archive: {error}"))?;
    let status = open
        .extract_all_with_callback(dest_dir, |event| {
            if cancel.load(Ordering::Relaxed) {
                return false;
            }
            !matches!(event, ExtractEvent::Err { .. })
        })
        .map_err(|error| anyhow::anyhow!("rar extract failed: {error}"))?;
    match status {
        ExtractStatus::Completed => {
            on_progress(1, 1);
            Ok(())
        }
        ExtractStatus::Cancelled => Err(ExtractCancelled.into()),
        other => anyhow::bail!("rar extract incomplete: {other:?}"),
    }
}

fn extract_tar_reader<R: Read>(
    reader: R,
    dest_dir: &Path,
    cancel: &AtomicBool,
    on_progress: &mut dyn FnMut(u32, u32),
) -> anyhow::Result<()> {
    if cancel.load(Ordering::Relaxed) {
        return Err(ExtractCancelled.into());
    }
    let mut archive = tar::Archive::new(reader);
    on_progress(0, 1);
    archive.unpack(dest_dir)?;
    on_progress(1, 1);
    Ok(())
}

fn archive_stem(path: &Path) -> String {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| "Archive".into());
    let lower = name.to_ascii_lowercase();
    if lower.ends_with(".tar.gz") {
        return name[..name.len().saturating_sub(7)].to_string();
    }
    if lower.ends_with(".tgz") {
        return name[..name.len().saturating_sub(4)].to_string();
    }
    if lower.ends_with(".tar.bz2") {
        return name[..name.len().saturating_sub(8)].to_string();
    }
    if lower.ends_with(".tbz2") {
        return name[..name.len().saturating_sub(5)].to_string();
    }
    if lower.ends_with(".tar.xz") {
        return name[..name.len().saturating_sub(7)].to_string();
    }
    if lower.ends_with(".txz") {
        return name[..name.len().saturating_sub(4)].to_string();
    }
    if lower.ends_with(".cbz") {
        return name[..name.len().saturating_sub(4)].to_string();
    }
    if lower.ends_with(".cbr") {
        return name[..name.len().saturating_sub(4)].to_string();
    }
    path.file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .filter(|stem| !stem.is_empty())
        .unwrap_or(name)
}

fn unique_extract_dir(base_path: PathBuf) -> PathBuf {
    if !base_path.exists() {
        return base_path;
    }
    let parent = base_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default();
    let stem = base_path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "Archive".into());
    for index in 2.. {
        let candidate = parent.join(format!("{stem} ({index})"));
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!()
}

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
    Ok(destination_dir.join(zip_file_name(sources, destination_dir)))
}

/// Resolves a non-conflicting final zip path for `sources`.
pub fn unique_zip_output_path(
    sources: &[PathBuf],
    destination_dir: &Path,
) -> anyhow::Result<PathBuf> {
    let base_path = zip_output_path(sources, destination_dir)?;
    Ok(unique_zip_path(base_path))
}

/// File name for the compress menu label (e.g. `report.zip`, `Archive (2).zip`).
pub fn compress_zip_file_display_name(sources: &[PathBuf], destination_dir: &Path) -> String {
    match unique_zip_output_path(sources, destination_dir) {
        Ok(path) => path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| zip_file_name(sources, destination_dir)),
        Err(_) => zip_file_name(sources, destination_dir),
    }
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

fn zip_file_name(sources: &[PathBuf], destination_dir: &Path) -> String {
    if sources.len() == 1 {
        let stem = sources[0]
            .file_stem()
            .map(|s| s.to_string_lossy().into_owned())
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "Archive".into());
        return format!("{stem}.zip");
    }
    let stem = destination_dir
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "Archive".into());
    format!("{stem}.zip")
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

#[cfg(test)]
mod extract_tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn compress_zip_file_display_name_uses_single_stem() {
        let dir = std::env::temp_dir();
        let sources = vec![PathBuf::from(r"C:\fake\report.docx")];
        assert_eq!(
            compress_zip_file_display_name(&sources, &dir),
            "report.docx.zip"
        );
    }

    #[test]
    fn compress_zip_file_display_name_multi_selection() {
        let dir = std::env::temp_dir().join("ProjectFiles");
        let _ = std::fs::create_dir_all(&dir);
        let sources = vec![
            PathBuf::from(r"C:\fake\a.txt"),
            PathBuf::from(r"C:\fake\b.txt"),
        ];
        assert_eq!(
            compress_zip_file_display_name(&sources, &dir),
            "ProjectFiles.zip"
        );
    }

    #[test]
    fn is_archive_path_detects_rar_by_name_without_disk_check() {
        let path = Path::new(r"C:\Downloads\missing-volume.rar");
        assert!(is_archive_path(path));
        assert_eq!(detect_archive_format(path), Some(ArchiveFormat::Rar));
    }

    #[test]
    fn detect_rar_multipart_volume_extensions() {
        assert_eq!(
            detect_archive_format(Path::new("data.r00")),
            Some(ArchiveFormat::Rar)
        );
        assert_eq!(
            detect_archive_format(Path::new("data.part01.rar")),
            Some(ArchiveFormat::Rar)
        );
    }

    #[test]
    fn detect_rar_and_comic_extensions() {
        assert_eq!(
            detect_archive_format(Path::new("book.rar")),
            Some(ArchiveFormat::Rar)
        );
        assert_eq!(
            detect_archive_format(Path::new("comic.cbz")),
            Some(ArchiveFormat::Zip)
        );
        assert_eq!(
            detect_archive_format(Path::new("comic.cbr")),
            Some(ArchiveFormat::Rar)
        );
    }

    #[test]
    fn extract_to_child_dir_strips_tar_bz2() {
        let archive = Path::new(r"C:\Downloads\backup.tar.bz2");
        assert_eq!(
            extract_to_child_dir(archive, Path::new(r"C:\Downloads")),
            PathBuf::from(r"C:\Downloads\backup")
        );
    }

    #[test]
    fn extract_to_child_dir_strips_tar_gz() {
        let archive = Path::new(r"C:\Downloads\project.tar.gz");
        assert_eq!(
            extract_to_child_dir(archive, Path::new(r"C:\Downloads")),
            PathBuf::from(r"C:\Downloads\project")
        );
    }

    #[test]
    fn extract_to_child_dir_uniques_conflicts() {
        let parent = std::env::temp_dir().join("cyberfiles_extract_dir_test");
        let _ = std::fs::remove_dir_all(&parent);
        std::fs::create_dir_all(&parent).unwrap();
        std::fs::create_dir_all(parent.join("bundle")).unwrap();
        let archive = parent.join("bundle.zip");
        std::fs::write(&archive, b"").unwrap();
        assert_eq!(
            extract_to_child_dir(&archive, &parent),
            parent.join("bundle (2)")
        );
        let _ = std::fs::remove_dir_all(&parent);
    }

    #[test]
    fn round_trip_zip_extract() {
        let root = std::env::temp_dir().join("cyberfiles_extract_zip_test");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let source = root.join("source");
        std::fs::create_dir_all(&source).unwrap();
        std::fs::write(source.join("hello.txt"), b"hello").unwrap();

        let zip_path = compress_paths_to_zip(&[source.clone()], &root).unwrap();

        let dest = root.join("out");
        std::fs::create_dir_all(&dest).unwrap();
        extract_archive_cancellable(&zip_path, &dest, &AtomicBool::new(false), |_, _| {}).unwrap();

        assert!(dest.join("source").join("hello.txt").is_file());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn zip_slip_entry_is_rejected() {
        let root = std::env::temp_dir().join("cyberfiles_extract_slip_test");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let zip_path = root.join("slip.zip");
        {
            let file = File::create(&zip_path).unwrap();
            let mut zip = ZipWriter::new(file);
            let options = SimpleFileOptions::default();
            zip.start_file("../evil.txt", options).unwrap();
            zip.write_all(b"bad").unwrap();
            zip.finish().unwrap();
        }
        let dest = root.join("out");
        std::fs::create_dir_all(&dest).unwrap();
        let result =
            extract_archive_cancellable(&zip_path, &dest, &AtomicBool::new(false), |_, _| {});
        assert!(result.is_err(), "zip slip paths must fail extraction");
        assert!(!dest.parent().unwrap().join("evil.txt").exists());
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn extract_zip_with_backslash_paths() {
        let root = std::env::temp_dir().join("cyberfiles_extract_backslash_test");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let zip_path = root.join("win.zip");
        {
            let file = File::create(&zip_path).unwrap();
            let mut zip = ZipWriter::new(file);
            let options = SimpleFileOptions::default();
            zip.start_file("folder\\hello.txt", options).unwrap();
            zip.write_all(b"hello").unwrap();
            zip.finish().unwrap();
        }
        let dest = root.join("out");
        std::fs::create_dir_all(&dest).unwrap();
        extract_archive_cancellable(&zip_path, &dest, &AtomicBool::new(false), |_, _| {}).unwrap();
        assert!(dest.join("folder").join("hello.txt").is_file());
        assert_eq!(
            std::fs::read_to_string(dest.join("folder").join("hello.txt")).unwrap(),
            "hello"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    /// Manual: `cargo test -p files-fs extract_bench_log -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn extract_bench_log() {
        let primary = Path::new(r"D:\aDownload\New folder\usbip修改.zip");
        let alt = Path::new(r"D:\aDownload\usbip_test.zip");
        let archive = if primary.is_file() {
            primary
        } else if alt.is_file() {
            alt
        } else {
            return;
        };
        extract_bench_log_at(archive);
    }

    fn extract_bench_log_at(archive: &Path) {
        log_extract_environment();
        let dest = std::env::temp_dir().join("cyberfiles_extract_bench_log");
        let _ = std::fs::remove_dir_all(&dest);
        std::fs::create_dir_all(&dest).unwrap();
        let started = std::time::Instant::now();
        extract_archive_cancellable(archive, &dest, &AtomicBool::new(false), |done, total| {
            if done == 1 || done == total || done % 2000 == 0 {
                tracing::debug!(target: "extract", done, total, "UI progress callback");
            }
        })
        .expect("extract should succeed");
        tracing::debug!(
            target: "extract",
            elapsed = ?started.elapsed(),
            "extract_bench_log complete"
        );
        let _ = std::fs::remove_dir_all(&dest);
    }

    /// Manual: `cargo test -p files-fs rar_smoke_local -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn rar_smoke_local() {
        let archive = Path::new(r"D:\aDownload\New folder\UE532_FirstShooter.rar");
        if !archive.is_file() {
            return;
        }
        let dest = std::env::temp_dir().join("cyberfiles_rar_smoke");
        let _ = std::fs::remove_dir_all(&dest);
        std::fs::create_dir_all(&dest).unwrap();
        extract_archive_cancellable(archive, &dest, &AtomicBool::new(false), |_, _| {})
            .expect("rar extract should succeed");
        assert!(dest.read_dir().unwrap().next().is_some());
        let _ = std::fs::remove_dir_all(&dest);
    }
}
