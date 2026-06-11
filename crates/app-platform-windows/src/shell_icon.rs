//! Windows Shell icons (colorful folder / drive / file icons like Files.app).

use std::collections::HashMap;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use windows::core::{Interface, PCWSTR};
use windows::Win32::Foundation::SIZE;
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits, GetObjectW, ReleaseDC,
    SelectObject, BITMAP, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HBITMAP,
};
use windows::Win32::Storage::FileSystem::FILE_FLAGS_AND_ATTRIBUTES;
use windows::Win32::UI::Controls::{IImageList, ILD_TRANSPARENT};
use windows::Win32::UI::Shell::{
    ExtractIconExW, ILFree, IShellItem, IShellItemImageFactory, SHCreateItemFromIDList,
    SHCreateItemFromParsingName, SHParseDisplayName, SHGetFileInfoW, SHGetImageList, SHFILEINFOW,
    SHGFI_SYSICONINDEX, SHGFI_USEFILEATTRIBUTES, SHIL_EXTRALARGE, SHIL_JUMBO, SHIL_LARGE,
    SHIL_SMALL, SIIGBF_BIGGERSIZEOK, SIIGBF_ICONONLY, SIIGBF_SCALEUP, SIIGBF_THUMBNAILONLY,
};
use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, HICON, ICONINFO};

use crate::com::run_sta_task;
use crate::icons::shell_dummy_icon_path;
use crate::paths::{is_recycle_bin_path, SHELL_RECYCLE_BIN_PATH};

/// Maximum Shell icon dimension we request (matches Files `ShellIconSizes.Jumbo`).
pub const MAX_ICON_SIZE: u32 = 256;

/// Logical menu-row icon size (matches gpui-component popup menu icons).
pub const MENU_ICON_LOGICAL_PX: f32 = 16.;

/// Display scale for the primary display (`GetDpiForSystem` / 96).
pub fn system_scale_factor() -> f32 {
    #[cfg(windows)]
    unsafe {
        use windows::Win32::UI::HiDpi::GetDpiForSystem;
        (GetDpiForSystem() as f32 / 96.0).max(1.0)
    }
    #[cfg(not(windows))]
    {
        1.0
    }
}

/// Physical pixels to extract for a 16×16 logical Shell menu icon at `scale_factor`.
pub fn menu_icon_pixel_size(scale_factor: f32) -> u32 {
    shell_icon_pixel_size(MENU_ICON_LOGICAL_PX, scale_factor)
}

static ICON_CACHE: Mutex<Option<HashMap<(PathBuf, u32), Vec<u8>>>> = Mutex::new(None);
static THUMBNAIL_CACHE: Mutex<Option<HashMap<(PathBuf, u32), Vec<u8>>>> = Mutex::new(None);
static LIST_KEY_CACHE: Mutex<Option<HashMap<(String, u32), Vec<u8>>>> = Mutex::new(None);

/// Physical pixel size for a logical UI size at the given display scale (Files: `size * DPI`).
pub fn shell_icon_pixel_size(logical_px: f32, scale_factor: f32) -> u32 {
    let scaled = (logical_px * scale_factor).ceil();
    (scaled as u32).clamp(16, MAX_ICON_SIZE)
}

fn path_to_wide(path: &Path) -> Vec<u16> {
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// Returns cached PNG bytes without loading from Shell (safe on the UI thread).
pub fn shell_icon_png_from_cache(path: &Path, size: u32) -> Option<Vec<u8>> {
    let size = size.max(16);
    let key = (path.to_path_buf(), size);
    let guard = ICON_CACHE.lock().ok()?;
    let cache = guard.as_ref()?;
    cache.get(&key).cloned()
}

/// PNG bytes for the Shell icon of `path` (same source as Files `FileThumbnailHelper`).
pub fn shell_icon_png(path: &Path, size: u32) -> anyhow::Result<Vec<u8>> {
    let size = size.max(16);
    let key = (path.to_path_buf(), size);
    {
        let guard = ICON_CACHE.lock().map_err(|e| anyhow::anyhow!("{e}"))?;
        if let Some(bytes) = guard.as_ref().and_then(|c| c.get(&key)) {
            return Ok(bytes.clone());
        }
    }
    let png = shell_icon_png_uncached(path, size, false)?;
    if let Ok(mut guard) = ICON_CACHE.lock() {
        guard.get_or_insert_with(HashMap::new).insert(key, png.clone());
    }
    Ok(png)
}

/// List row icon by Files `IconCacheService` key (`:folder:`, `.zip`, …).
pub fn shell_icon_png_for_list_key(cache_key: &str, size: u32) -> anyhow::Result<Vec<u8>> {
    let size = size.max(16);
    let key = (cache_key.to_string(), size);
    {
        let guard = LIST_KEY_CACHE.lock().map_err(|e| anyhow::anyhow!("{e}"))?;
        if let Some(bytes) = guard.as_ref().and_then(|c| c.get(&key)) {
            return Ok(bytes.clone());
        }
    }
    let path = shell_dummy_icon_path(cache_key);
    let is_folder = cache_key == ":folder:";
    let png = shell_icon_png_uncached(&path, size, is_folder)?;
    if let Ok(mut guard) = LIST_KEY_CACHE.lock() {
        guard.get_or_insert_with(HashMap::new).insert(key, png.clone());
    }
    Ok(png)
}

/// PNG bytes sized for how the icon is drawn in the UI (logical px × scale factor).
pub fn shell_icon_png_scaled(
    path: &Path,
    logical_px: f32,
    scale_factor: f32,
) -> anyhow::Result<Vec<u8>> {
    shell_icon_png(path, shell_icon_pixel_size(logical_px, scale_factor))
}

/// Extract an icon from a registry-style location string such as
/// `"C:\Windows\System32\shell32.dll,-16769"` or `"%SystemRoot%\system32\mspaint.exe,0"`.
pub(crate) fn shell_icon_png_from_location(location: &str, size: u32) -> anyhow::Result<Vec<u8>> {
    let size = size.max(16);
    let location = location.trim();
    let (path, index) = parse_icon_location(location);
    let path = expand_percent_vars(path.trim_matches('"'));
    if path.is_empty() {
        anyhow::bail!("empty icon location");
    }
    let path = PathBuf::from(path);
    run_sta_task(move || unsafe { extract_icon_png_inner(&path, index, size) })
}

/// Shell **thumbnail** for Home cards (`SIIGBF_THUMBNAILONLY`). Returns `None` when unavailable.
pub fn shell_thumbnail_png_scaled(
    path: &Path,
    logical_px: f32,
    scale_factor: f32,
) -> anyhow::Result<Option<Vec<u8>>> {
    let size = shell_icon_pixel_size(logical_px, scale_factor);
    let key = (path.to_path_buf(), size);
    {
        let guard = THUMBNAIL_CACHE.lock().map_err(|e| anyhow::anyhow!("{e}"))?;
        if let Some(bytes) = guard.as_ref().and_then(|c| c.get(&key)) {
            return Ok(Some(bytes.clone()));
        }
    }
    let Some(png) = shell_thumbnail_png_uncached(path, size)? else {
        return Ok(None);
    };
    if let Ok(mut guard) = THUMBNAIL_CACHE.lock() {
        guard.get_or_insert_with(HashMap::new).insert(key, png.clone());
    }
    Ok(Some(png))
}

fn shell_thumbnail_png_uncached(path: &Path, size: u32) -> anyhow::Result<Option<Vec<u8>>> {
    let path = path.to_path_buf();
    run_sta_task(move || unsafe {
        match shell_thumbnail_png_inner(&path, size) {
            Ok(png) => Ok(Some(png)),
            Err(_) => Ok(None),
        }
    })
}

unsafe fn shell_thumbnail_png_inner(path: &Path, size: u32) -> anyhow::Result<Vec<u8>> {
    let parsing = shell_icon_parsing_path(path);
    let wide = path_to_wide(&parsing);
    let item: IShellItem = SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()), None)?;
    let factory: IShellItemImageFactory = item.cast()?;
    let hbitmap = factory.GetImage(
        SIZE {
            cx: size as i32,
            cy: size as i32,
        },
        SIIGBF_THUMBNAILONLY | SIIGBF_BIGGERSIZEOK | SIIGBF_SCALEUP,
    )?;
    hbitmap_to_png(hbitmap)
}

fn shell_icon_png_uncached(path: &Path, size: u32, is_folder: bool) -> anyhow::Result<Vec<u8>> {
    let path = path.to_path_buf();
    let folder = is_folder;
    run_sta_task(move || unsafe {
        shell_icon_png_inner(&path, size, folder)
            .or_else(|_| shell_icon_via_shgetfileinfo(&path, folder, size))
    })
}

/// Batch-extract Shell icons for multiple paths in a **single** STA thread.
/// Avoids the per-icon thread-spawn overhead that makes `shell_icon_png` slow
/// when called in a loop (e.g. warming 10+ network device icons).
pub fn shell_icon_png_batch(
    entries: &[(PathBuf, u32)],
) -> Vec<(PathBuf, u32, Option<Vec<u8>>)> {
    let entries = entries.to_vec();
    let count = entries.len();
    let t0 = std::time::Instant::now();
    let results = run_sta_task(move || unsafe {
        let mut results = Vec::with_capacity(entries.len());
        for (path, size) in entries {
            let png = shell_icon_png_inner(&path, size, false)
                .or_else(|_| shell_icon_via_shgetfileinfo(&path, false, size))
                .ok()
                .filter(|b| !b.is_empty());
            results.push((path, size, png));
        }
        results
    });
    tracing::info!(
        target: "shell_icon",
        count,
        total_ms = %t0.elapsed().as_secs_f64() * 1000.0,
        "shell_icon_png_batch complete"
    );
    results
}

/// Path string passed to `SHCreateItemFromParsingName` (Files uses `Shell:RecycleBinFolder` for the bin icon).
fn shell_icon_parsing_path(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if s.eq_ignore_ascii_case(SHELL_RECYCLE_BIN_PATH)
        || s.eq_ignore_ascii_case("recycle")
        || is_recycle_bin_path(path)
    {
        PathBuf::from(SHELL_RECYCLE_BIN_PATH)
    } else {
        path.to_path_buf()
    }
}

unsafe fn shell_icon_png_inner(
    path: &Path,
    size: u32,
    _is_folder: bool,
) -> anyhow::Result<Vec<u8>> {
    let t0 = std::time::Instant::now();
    let parsing = shell_icon_parsing_path(path);
    let wide = path_to_wide(&parsing);

    // Try SHCreateItemFromParsingName first (fastest for filesystem paths).
    let item: IShellItem = SHCreateItemFromParsingName(PCWSTR(wide.as_ptr()), None)
        .or_else(|_| {
            // Fallback for virtual-folder parsing paths (e.g. `{GUID}\{GUID}`).
            let mut pidl: *mut windows::Win32::UI::Shell::Common::ITEMIDLIST = std::ptr::null_mut();
            SHParseDisplayName(PCWSTR(wide.as_ptr()), None, &mut pidl, 0, None)?;
            if pidl.is_null() {
                anyhow::bail!("SHParseDisplayName returned null PIDL");
            }
            let item: IShellItem = SHCreateItemFromIDList(pidl)?;
            ILFree(Some(pidl));
            Ok(item)
        })?;
    let t1 = t0.elapsed();

    let factory: IShellItemImageFactory = item.cast()?;
    let hbitmap = factory.GetImage(
        SIZE {
            cx: size as i32,
            cy: size as i32,
        },
        SIIGBF_ICONONLY | SIIGBF_SCALEUP,
    )?;
    let t2 = t0.elapsed();

    let png = hbitmap_to_png(hbitmap)?;
    let t3 = t0.elapsed();

    tracing::info!(
        target: "shell_icon",
        path = %path.display(),
        size,
        create_item_ms = %t1.as_secs_f64() * 1000.0,
        get_image_ms = %(t2 - t1).as_secs_f64() * 1000.0,
        encode_png_ms = %(t3 - t2).as_secs_f64() * 1000.0,
        total_ms = %t3.as_secs_f64() * 1000.0,
        "shell_icon_png_inner timing"
    );
    Ok(png)
}

/// Fallback when the dummy path does not exist (Files `SHGetFileInfo` + `USEFILEATTRIBUTES`).
unsafe fn shell_icon_via_shgetfileinfo(
    path: &Path,
    is_folder: bool,
    size: u32,
) -> anyhow::Result<Vec<u8>> {
    let wide = path_to_wide(path);
    let mut shfi = SHFILEINFOW::default();
    let attrs = if is_folder {
        FILE_FLAGS_AND_ATTRIBUTES(0x10) // FILE_ATTRIBUTE_DIRECTORY
    } else {
        FILE_FLAGS_AND_ATTRIBUTES(0x80) // FILE_ATTRIBUTE_NORMAL
    };
    let flags = SHGFI_SYSICONINDEX | SHGFI_USEFILEATTRIBUTES;
    let ret = SHGetFileInfoW(
        PCWSTR(wide.as_ptr()),
        attrs,
        Some(&mut shfi),
        std::mem::size_of::<SHFILEINFOW>() as u32,
        flags,
    );
    if ret == 0 {
        anyhow::bail!("SHGetFileInfoW failed for {}", path.display());
    }

    let icon_idx = (shfi.iIcon & 0x00FF_FFFF) as i32;
    let shil = match size {
        0..=16 => SHIL_SMALL,
        17..=32 => SHIL_LARGE,
        33..=48 => SHIL_EXTRALARGE,
        _ => SHIL_JUMBO,
    };
    let image_list: IImageList = SHGetImageList(shil as i32)?;
    let hicon = image_list.GetIcon(icon_idx, ILD_TRANSPARENT.0)?;
    let png = hicon_to_png(hicon)?;
    let _ = DestroyIcon(hicon);
    Ok(png)
}

unsafe fn hicon_to_png(hicon: HICON) -> anyhow::Result<Vec<u8>> {
    let mut info = ICONINFO::default();
    GetIconInfo(hicon, &mut info)?;
    let png = hbitmap_to_png(info.hbmColor);
    if !info.hbmColor.is_invalid() {
        let _ = DeleteObject(info.hbmColor);
    }
    if !info.hbmMask.is_invalid() {
        let _ = DeleteObject(info.hbmMask);
    }
    png
}

fn parse_icon_location(location: &str) -> (&str, i32) {
    let Some((path, suffix)) = location.rsplit_once(',') else {
        return (location, 0);
    };
    let suffix = suffix.trim();
    match suffix.parse::<i32>() {
        Ok(index) => (path.trim(), index),
        Err(_) => (location, 0),
    }
}

fn expand_percent_vars(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '%' {
            out.push(ch);
            continue;
        }
        let mut name = String::new();
        while let Some(&next) = chars.peek() {
            chars.next();
            if next == '%' {
                break;
            }
            name.push(next);
        }
        if name.is_empty() {
            out.push('%');
            continue;
        }
        match std::env::var(&name) {
            Ok(value) => out.push_str(&value),
            Err(_) => {
                out.push('%');
                out.push_str(&name);
                out.push('%');
            }
        }
    }
    out
}

unsafe fn extract_icon_png_inner(path: &Path, index: i32, _size: u32) -> anyhow::Result<Vec<u8>> {
    let wide: Vec<u16> = OsStr::new(path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut small = [HICON::default(); 1];
    let extracted = ExtractIconExW(
        PCWSTR(wide.as_ptr()),
        index,
        None,
        Some(small.as_mut_ptr()),
        1,
    );
    if extracted == 0 || small[0].is_invalid() {
        anyhow::bail!("ExtractIconExW failed for {}", path.display());
    }
    let png = hicon_to_png(small[0])?;
    let _ = DestroyIcon(small[0]);
    Ok(png)
}

unsafe fn hbitmap_to_png(hbitmap: HBITMAP) -> anyhow::Result<Vec<u8>> {
    let mut bm = BITMAP::default();
    if GetObjectW(
        hbitmap,
        std::mem::size_of::<BITMAP>() as i32,
        Some(&mut bm as *mut _ as *mut _),
    ) == 0
    {
        anyhow::bail!("GetObjectW failed for shell icon bitmap");
    }

    let width = bm.bmWidth.unsigned_abs() as u32;
    let height = bm.bmHeight.unsigned_abs() as u32;
    if width == 0 || height == 0 {
        anyhow::bail!("shell icon bitmap has zero size");
    }

    let hdc_screen = GetDC(None);
    let hdc_mem = CreateCompatibleDC(hdc_screen);
    let _selected = SelectObject(hdc_mem, hbitmap);

    let mut bmi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: width as i32,
            biHeight: -(height as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };

    let stride = (width * 4) as usize;
    let mut pixels = vec![0u8; stride * height as usize];
    let lines = GetDIBits(
        hdc_mem,
        hbitmap,
        0,
        height,
        Some(pixels.as_mut_ptr() as *mut _),
        &mut bmi,
        DIB_RGB_COLORS,
    );
    SelectObject(hdc_mem, _selected);
    let _ = DeleteDC(hdc_mem);
    let _ = ReleaseDC(None, hdc_screen);
    let _ = DeleteObject(hbitmap);

    if lines == 0 {
        anyhow::bail!("GetDIBits failed for shell icon");
    }

    let mut img = image::RgbaImage::new(width, height);
    for y in 0..height {
        for x in 0..width {
            let i = (y as usize * stride) + (x as usize * 4);
            let b = pixels[i];
            let g = pixels[i + 1];
            let r = pixels[i + 2];
            let a = pixels[i + 3];
            img.put_pixel(x, y, image::Rgba([r, g, b, a]));
        }
    }

    let mut png = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut png);
    img.write_to(&mut cursor, image::ImageFormat::Png)?;
    Ok(png)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_key_icons_load_via_shgetfileinfo() {
        for key in [":folder:", ".txt", ".exe", ":noext:"] {
            let png = shell_icon_png_for_list_key(key, 32).unwrap_or_else(|e| {
                panic!("list key {key}: {e:#}");
            });
            assert!(!png.is_empty(), "list key {key}");
        }
    }
}
