//! In-process 7-Zip extraction on Windows via bundled `7z.dll`.

#![cfg(windows)]

use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use app_sevenzip_ffi as ffi;
use ffi::sys;

#[derive(Debug)]
pub enum SevenZipExtractError {
    InvalidArguments,
    LoadLibrary,
    OpenArchive,
    ExtractFailed,
    PasswordRequired,
    Cancelled,
    Other(String),
}

impl std::fmt::Display for SevenZipExtractError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidArguments => write!(f, "invalid arguments"),
            Self::LoadLibrary => write!(f, "failed to load 7z.dll"),
            Self::OpenArchive => write!(f, "cannot open archive"),
            Self::ExtractFailed => write!(f, "7-Zip extract failed"),
            Self::PasswordRequired => write!(f, "password required"),
            Self::Cancelled => write!(f, "extract cancelled"),
            Self::Other(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for SevenZipExtractError {}

pub fn wide_path(path: &Path) -> Vec<u16> {
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

pub fn bundled_dll_path() -> Option<std::path::PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let dll = exe.with_file_name("7z.dll");
    dll.is_file().then_some(dll)
}

fn format_hint_from_extension(path: &Path) -> i32 {
    let name = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        return sys::SEVENZIP_FORMAT_GZIP;
    }
    if name.ends_with(".tar.bz2") || name.ends_with(".tbz2") {
        return sys::SEVENZIP_FORMAT_BZIP2;
    }
    if name.ends_with(".tar.xz") || name.ends_with(".txz") {
        return sys::SEVENZIP_FORMAT_XZ;
    }
    match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .as_deref()
    {
        Some("zip") | Some("cbz") => sys::SEVENZIP_FORMAT_ZIP,
        Some("7z") => sys::SEVENZIP_FORMAT_7Z,
        Some("tar") => sys::SEVENZIP_FORMAT_TAR,
        Some("gz") => sys::SEVENZIP_FORMAT_GZIP,
        Some("bz2") => sys::SEVENZIP_FORMAT_BZIP2,
        Some("xz") => sys::SEVENZIP_FORMAT_XZ,
        _ => sys::SEVENZIP_FORMAT_AUTO,
    }
}

struct CallbackContext<'a> {
    cancel: &'a AtomicBool,
    on_progress: &'a mut dyn FnMut(u32, u32),
}

extern "C" fn progress_trampoline(ctx: *mut std::ffi::c_void, completed: u32, total: u32) {
    if ctx.is_null() {
        return;
    }
    // SAFETY: ctx points to CallbackContext for the duration of sevenzip_extract.
    let context = unsafe { &mut *(ctx as *mut CallbackContext<'_>) };
    (context.on_progress)(completed, total.max(1));
}

extern "C" fn cancel_trampoline(ctx: *mut std::ffi::c_void) -> i32 {
    if ctx.is_null() {
        return 0;
    }
    let context = unsafe { &*(ctx as *const CallbackContext<'_>) };
    i32::from(context.cancel.load(Ordering::Relaxed))
}

pub fn extract_in_process(
    dll_path: &Path,
    archive: &Path,
    dest_dir: &Path,
    cancel: &AtomicBool,
    on_progress: &mut dyn FnMut(u32, u32),
) -> Result<(), SevenZipExtractError> {
    let dll = wide_path(dll_path);
    let archive_w = wide_path(archive);
    let dest_w = wide_path(dest_dir);
    let format_hint = format_hint_from_extension(archive);
    let thread_count = std::thread::available_parallelism()
        .map(|n| n.get() as u32)
        .unwrap_or(1)
        .max(1);

    let mut context = CallbackContext { cancel, on_progress };
    let mut error_buf = [0u16; 512];

    let code = unsafe {
        sys::sevenzip_extract(
            dll.as_ptr(),
            archive_w.as_ptr(),
            dest_w.as_ptr(),
            format_hint,
            thread_count,
            Some(progress_trampoline),
            &mut context as *mut _ as *mut std::ffi::c_void,
            Some(cancel_trampoline),
            &mut context as *mut _ as *mut std::ffi::c_void,
            error_buf.as_mut_ptr(),
            error_buf.len(),
        )
    };

    if code == 0 {
        return Ok(());
    }

    let message = String::from_utf16_lossy(
        &error_buf[..error_buf.iter().position(|&c| c == 0).unwrap_or(error_buf.len())],
    );
    Err(match code {
        1 => SevenZipExtractError::Cancelled,
        -1 => SevenZipExtractError::InvalidArguments,
        -2 => SevenZipExtractError::LoadLibrary,
        -3 => SevenZipExtractError::OpenArchive,
        -4 => SevenZipExtractError::ExtractFailed,
        -5 => SevenZipExtractError::PasswordRequired,
        _ => SevenZipExtractError::Other(if message.is_empty() {
            format!("7-Zip error code {code}")
        } else {
            message
        }),
    })
}
