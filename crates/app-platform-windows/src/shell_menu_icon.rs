//! Extract icons from Shell `IContextMenu` HMENU items (bitmap, callback draw, verb fallback).
//!
//! Icons are cached in a process-global in-memory store keyed by icon location, verb/extension,
//! or menu row (verb + label + extension). A cache miss for a row is also cached so we do not
//! repeatedly run owner-draw / registry lookups for entries that genuinely have no icon.

use std::cell::Cell;
use std::collections::HashMap;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::sync::{Arc, Mutex, OnceLock};

use windows::core::{Interface, PCWSTR, PWSTR};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleBitmap, CreateCompatibleDC, CreateSolidBrush, DeleteDC, DeleteObject, FillRect,
    GetDC, GetDIBits, GetObjectW, ReleaseDC, SelectObject, BITMAP, BITMAPINFO, BITMAPINFOHEADER,
    BI_RGB, DIB_RGB_COLORS, HBITMAP, HDC, HGDIOBJ,
};
use windows::Win32::System::Registry::{
    RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY_CLASSES_ROOT, KEY_READ, REG_VALUE_TYPE,
};
use windows::Win32::UI::Controls::{
    DRAWITEMSTRUCT, MEASUREITEMSTRUCT, ODA_DRAWENTIRE, ODS_DEFAULT, ODT_MENU,
};
use windows::Win32::UI::Shell::{
    AssocQueryStringW, IContextMenu, IContextMenu2, IContextMenu3, ASSOCF_INIT_BYEXENAME,
    ASSOCSTR_EXECUTABLE, ASSOCSTR_PROGID,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetMenuItemInfoW, HBMMENU_CALLBACK, HMENU, MENUITEMINFOW, MIIM_BITMAP, WM_DRAWITEM,
    WM_INITMENUPOPUP, WM_MEASUREITEM,
};

use crate::shell_icon::{shell_icon_png, shell_icon_png_from_location};

thread_local! {
    static MENU_ICON_EXTRACT_PX: Cell<u32> = const { Cell::new(16) };
}

/// In-memory PNG cache for Shell context-menu row icons (stable across menu opens).
/// `Some(png)` means an icon was resolved; `None` means we already tried and found none.
static MENU_ICON_PNG_CACHE: OnceLock<Mutex<HashMap<MenuIconCacheKey, Option<Arc<Vec<u8>>>>>> =
    OnceLock::new();

#[derive(Clone, Hash, Eq, PartialEq)]
enum MenuIconCacheKey {
    /// `%SystemRoot%\\...`, `shell32.dll,-123`, etc.
    Location { location: String, size: u32 },
    /// Executable resolved for a file-type verb (`open`, `print`, …).
    VerbExe {
        extension: String,
        verb: String,
        size: u32,
    },
    /// Bitmap / owner-draw row keyed by verb + visible label + file extension.
    MenuRow {
        verb: String,
        label: String,
        extension: String,
        size: u32,
    },
}

fn menu_icon_cache() -> &'static Mutex<HashMap<MenuIconCacheKey, Option<Arc<Vec<u8>>>>> {
    MENU_ICON_PNG_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Returns `Some(...)` when the key has been looked up before:
/// - `Some(Some(png))` -> cached icon
/// - `Some(None)`      -> cached "no icon"
/// Returns `None` when the key has not been seen yet.
fn menu_icon_cache_get(key: &MenuIconCacheKey) -> Option<Option<Arc<Vec<u8>>>> {
    menu_icon_cache()
        .lock()
        .ok()
        .and_then(|cache| cache.get(key).cloned())
}

fn menu_icon_cache_put(key: MenuIconCacheKey, png: Option<Vec<u8>>) -> Option<Arc<Vec<u8>>> {
    let arc = png.map(Arc::new);
    if let Ok(mut cache) = menu_icon_cache().lock() {
        cache.insert(key, arc.clone());
    }
    arc
}

fn normalize_menu_icon_label(label: &str) -> String {
    label.trim().to_ascii_lowercase()
}

fn normalize_menu_icon_verb(verb: Option<&str>) -> String {
    verb.map(|v| v.trim().to_ascii_lowercase())
        .filter(|v| !v.is_empty())
        .unwrap_or_default()
}

fn menu_row_cache_key(verb: Option<&str>, label: &str, primary_path: &Path) -> MenuIconCacheKey {
    let extension = primary_path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    MenuIconCacheKey::MenuRow {
        verb: normalize_menu_icon_verb(verb),
        label: normalize_menu_icon_label(label),
        extension,
        size: menu_icon_extract_px(),
    }
}

/// BGR in memory; matches GDI `RGB(255, 0, 255)` chroma for owner-draw (Files `MakeTransparent`).
const CHROMA_B: u8 = 255;
const CHROMA_G: u8 = 0;
const CHROMA_R: u8 = 255;

pub(crate) fn set_menu_icon_extract_px(px: u32) {
    MENU_ICON_EXTRACT_PX.with(|c| {
        c.set(px.clamp(16, crate::shell_icon::MAX_ICON_SIZE));
    });
}

fn menu_icon_extract_px() -> u32 {
    MENU_ICON_EXTRACT_PX.with(|c| c.get())
}

/// Notify shell extensions that the popup is about to display (populates `hbmpItem` / callbacks).
pub(crate) unsafe fn init_popup_menu(popup: HMENU, menu: &IContextMenu) {
    let Ok(cmenu2) = menu.cast::<IContextMenu2>() else {
        return;
    };
    let _ = cmenu2.HandleMenuMsg(WM_INITMENUPOPUP, WPARAM(popup.0 as usize), LPARAM(0));
}

/// Shell `hbmpItem` sentinel values (Files `HBITMAP_HMENU`); not real bitmap handles.
fn is_special_menu_bitmap(hbmp: HBITMAP) -> bool {
    if hbmp.is_invalid() {
        return true;
    }
    let v = hbmp.0 as i64;
    matches!(v, -1 | 1 | 2 | 3 | 5 | 6 | 7 | 8 | 9 | 10 | 11)
}

fn is_callback_bitmap(hbmp: HBITMAP) -> bool {
    hbmp == HBMMENU_CALLBACK
}

fn is_chroma_bgr(b: u8, g: u8, r: u8) -> bool {
    b.abs_diff(CHROMA_B) <= 12 && g.abs_diff(CHROMA_G) <= 12 && r.abs_diff(CHROMA_R) <= 12
}

fn unpremultiply_rgba(r: u8, g: u8, b: u8, a: u8) -> image::Rgba<u8> {
    if a == 0 {
        return image::Rgba([0, 0, 0, 0]);
    }
    if a == 255 {
        return image::Rgba([r, g, b, 255]);
    }
    let a_u = a as u32;
    let scale = |c: u8| -> u8 { ((c as u32 * 255 + a_u / 2) / a_u).min(255) as u8 };
    image::Rgba([scale(r), scale(g), scale(b), a])
}

fn pixel_to_rgba(
    b: u8,
    g: u8,
    r: u8,
    a: u8,
    chroma_key: bool,
    unpremultiply: bool,
) -> image::Rgba<u8> {
    if chroma_key && is_chroma_bgr(b, g, r) {
        return image::Rgba([0, 0, 0, 0]);
    }
    if unpremultiply {
        unpremultiply_rgba(r, g, b, a)
    } else {
        image::Rgba([r, g, b, a])
    }
}

unsafe fn fill_dc_chroma(dc: HDC, width: i32, height: i32) {
    let brush = CreateSolidBrush(COLORREF(0x00FF_00FF));
    if brush.is_invalid() {
        return;
    }
    let rect = RECT {
        left: 0,
        top: 0,
        right: width,
        bottom: height,
    };
    let _ = FillRect(dc, &rect, brush);
    let _ = DeleteObject(HGDIOBJ::from(brush));
}

unsafe fn rgba_pixels_to_png(
    pixels: &[u8],
    width: u32,
    height: u32,
    stride: usize,
    chroma_key: bool,
    unpremultiply: bool,
    // When true, row 0 of the PNG comes from the last scanline in `pixels`
    // (GDI bottom-up DIB layout).
    flip_y: bool,
) -> Option<Vec<u8>> {
    if width == 0 || height == 0 {
        return None;
    }
    let mut img = image::RgbaImage::new(width, height);
    let mut visible_pixels = 0u32;
    for y in 0..height {
        let src_y = if flip_y { height - 1 - y } else { y };
        for x in 0..width {
            let i = src_y as usize * stride + x as usize * 4;
            if i + 3 >= pixels.len() {
                return None;
            }
            let rgba = pixel_to_rgba(
                pixels[i],
                pixels[i + 1],
                pixels[i + 2],
                pixels[i + 3],
                chroma_key,
                unpremultiply,
            );
            if rgba[3] != 0 {
                visible_pixels += 1;
            }
            img.put_pixel(x, y, rgba);
        }
    }
    if visible_pixels == 0 {
        return None;
    }
    let mut png = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut png);
    img.write_to(&mut cursor, image::ImageFormat::Png).ok()?;
    Some(png)
}

/// Files `GetBitmapFromHBitmap`: read DIB-section pixels (PARGB) before `CopyImage`.
unsafe fn hbitmap_dibsection_png(hbmp: HBITMAP, chroma_key: bool) -> Option<Vec<u8>> {
    let mut bm = BITMAP::default();
    if GetObjectW(
        hbmp,
        std::mem::size_of::<BITMAP>() as i32,
        Some(&mut bm as *mut _ as *mut _),
    ) == 0
    {
        return None;
    }
    // Positive `bmHeight` → bottom-up DIB (origin lower-left); negative → top-down.
    let top_down = bm.bmHeight < 0;
    let width = bm.bmWidth.unsigned_abs();
    let height = bm.bmHeight.unsigned_abs();
    if width == 0 || height == 0 {
        return None;
    }
    if bm.bmBits.is_null() || bm.bmBitsPixel < 32 {
        return None;
    }

    let stride = bm.bmWidthBytes.max(width as i32 * 4) as usize;
    let size = stride.saturating_mul(height as usize);
    let bits = std::slice::from_raw_parts(bm.bmBits.cast::<u8>(), size);
    let mut has_alpha = false;
    for y in 0..height as usize {
        for x in 0..width as usize {
            let a = bits[y * stride + x * 4 + 3];
            if a != 0 && a != 255 {
                has_alpha = true;
                break;
            }
        }
        if has_alpha {
            break;
        }
    }
    rgba_pixels_to_png(
        bits, width, height, stride, chroma_key, has_alpha, !top_down,
    )
}

unsafe fn hbitmap_via_copy_image(hbmp: HBITMAP, chroma_key: bool) -> Option<Vec<u8>> {
    use windows::Win32::Foundation::HANDLE;
    use windows::Win32::UI::WindowsAndMessaging::{CopyImage, IMAGE_BITMAP, LR_COPYRETURNORG};

    let extract_px = menu_icon_extract_px() as i32;
    let copy = CopyImage(
        HANDLE(hbmp.0),
        IMAGE_BITMAP,
        extract_px,
        extract_px,
        LR_COPYRETURNORG,
    )
    .ok()?;
    let copy = HBITMAP(copy.0);
    let png = hbitmap_compatible_dc_png(copy, chroma_key);
    let _ = DeleteObject(HGDIOBJ::from(copy));
    png
}

unsafe fn hbitmap_compatible_dc_png(hbmp: HBITMAP, chroma_key: bool) -> Option<Vec<u8>> {
    let mut bm = BITMAP::default();
    if GetObjectW(
        hbmp,
        std::mem::size_of::<BITMAP>() as i32,
        Some(&mut bm as *mut _ as *mut _),
    ) == 0
    {
        return None;
    }
    let width = bm.bmWidth.unsigned_abs();
    let height = bm.bmHeight.unsigned_abs();
    if width == 0 || height == 0 {
        return None;
    }

    let hdc_screen = GetDC(None);
    if hdc_screen.is_invalid() {
        return None;
    }
    let hdc_mem = CreateCompatibleDC(hdc_screen);
    if hdc_mem.is_invalid() {
        let _ = ReleaseDC(None, hdc_screen);
        return None;
    }
    let _selected = SelectObject(hdc_mem, HBITMAP(hbmp.0));

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
        hbmp,
        0,
        height,
        Some(pixels.as_mut_ptr() as *mut _),
        &mut bmi,
        DIB_RGB_COLORS,
    );
    let _ = SelectObject(hdc_mem, _selected);
    let _ = DeleteDC(hdc_mem);
    let _ = ReleaseDC(None, hdc_screen);

    if lines == 0 {
        return None;
    }
    // `biHeight` is negative above, so `GetDIBits` returns a top-down buffer.
    rgba_pixels_to_png(&pixels, width, height, stride, chroma_key, true, false)
}

pub(crate) unsafe fn menu_item_bitmap_png(hbmp: HBITMAP) -> Option<Vec<u8>> {
    menu_item_bitmap_png_inner(hbmp, false)
}

unsafe fn menu_item_bitmap_png_inner(hbmp: HBITMAP, chroma_key: bool) -> Option<Vec<u8>> {
    if is_special_menu_bitmap(hbmp) {
        return None;
    }
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        hbitmap_dibsection_png(hbmp, chroma_key)
            .or_else(|| hbitmap_via_copy_image(hbmp, chroma_key))
            .or_else(|| hbitmap_compatible_dc_png(hbmp, chroma_key))
    }))
    .ok()
    .flatten()
}

unsafe fn dispatch_menu_msg(menu: &IContextMenu, msg: u32, wparam: WPARAM, lparam: LPARAM) {
    if let Ok(cmenu3) = menu.cast::<IContextMenu3>() {
        let mut result = LRESULT::default();
        if cmenu3
            .HandleMenuMsg2(msg, wparam, lparam, Some(&mut result as *mut _))
            .is_ok()
        {
            return;
        }
    }
    if let Ok(cmenu2) = menu.cast::<IContextMenu2>() {
        let _ = cmenu2.HandleMenuMsg(msg, wparam, lparam);
    }
}

/// Simulate owner-draw for `HBMMENU_CALLBACK` (MSDN: `DRAWITEMSTRUCT.hwndItem` is the `HMENU`).
unsafe fn menu_item_icon_via_draw(
    menu: &IContextMenu,
    popup: HMENU,
    item_id: u32,
) -> Option<Vec<u8>> {
    let px = menu_icon_extract_px().max(16) as i32;
    let screen_dc = GetDC(None);
    if screen_dc.is_invalid() {
        return None;
    }
    let mem_dc = CreateCompatibleDC(screen_dc);
    if mem_dc.is_invalid() {
        let _ = ReleaseDC(None, screen_dc);
        return None;
    }
    let bmp = CreateCompatibleBitmap(screen_dc, px, px);
    if bmp.is_invalid() {
        let _ = DeleteDC(mem_dc);
        let _ = ReleaseDC(None, screen_dc);
        return None;
    }
    let old = SelectObject(mem_dc, HGDIOBJ::from(bmp));
    fill_dc_chroma(mem_dc, px, px);

    let mut measure = MEASUREITEMSTRUCT {
        CtlType: ODT_MENU,
        CtlID: 0,
        itemID: item_id,
        itemWidth: 0,
        itemHeight: 0,
        itemData: 0,
    };
    dispatch_menu_msg(
        menu,
        WM_MEASUREITEM,
        WPARAM(0),
        LPARAM(&mut measure as *mut _ as isize),
    );
    let item_height = if measure.itemHeight == 0 {
        px as u32
    } else {
        measure.itemHeight
    };

    let mut draw = DRAWITEMSTRUCT {
        CtlType: ODT_MENU,
        CtlID: 0,
        itemID: item_id,
        itemAction: ODA_DRAWENTIRE,
        itemState: ODS_DEFAULT,
        hwndItem: HWND(popup.0),
        hDC: mem_dc,
        rcItem: RECT {
            left: 0,
            top: 0,
            right: px,
            bottom: item_height as i32,
        },
        itemData: 0,
    };
    dispatch_menu_msg(
        menu,
        WM_DRAWITEM,
        WPARAM(0),
        LPARAM(&mut draw as *mut _ as isize),
    );

    let png = menu_item_bitmap_png_inner(bmp, true);
    let _ = SelectObject(mem_dc, old);
    let _ = DeleteObject(HGDIOBJ::from(bmp));
    let _ = DeleteDC(mem_dc);
    let _ = ReleaseDC(None, screen_dc);
    png
}

pub(crate) unsafe fn refresh_item_bitmap(popup: HMENU, index: u32) -> HBITMAP {
    let mut info = MENUITEMINFOW {
        cbSize: std::mem::size_of::<MENUITEMINFOW>() as u32,
        fMask: MIIM_BITMAP,
        ..Default::default()
    };
    if GetMenuItemInfoW(popup, index, true, &mut info).is_ok() {
        info.hbmpItem
    } else {
        HBITMAP::default()
    }
}

fn assoc_query_string(
    flags: windows::Win32::UI::Shell::ASSOCF,
    str_type: windows::Win32::UI::Shell::ASSOCSTR,
    assoc: &str,
    extra: Option<&str>,
) -> Option<String> {
    let assoc_wide: Vec<u16> = std::ffi::OsStr::new(assoc)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let extra_wide = extra.map(|value| {
        std::ffi::OsStr::new(value)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect::<Vec<u16>>()
    });
    let mut buf = vec![0u16; 1024];
    let mut len = buf.len() as u32;
    if unsafe {
        AssocQueryStringW(
            flags,
            str_type,
            PCWSTR(assoc_wide.as_ptr()),
            extra_wide
                .as_ref()
                .map(|wide: &Vec<u16>| PCWSTR(wide.as_ptr()))
                .unwrap_or(PCWSTR::null()),
            PWSTR(buf.as_mut_ptr()),
            &mut len,
        )
    }
    .is_err()
    {
        return None;
    }
    let end = buf.iter().position(|&c| c == 0).unwrap_or(0);
    if end == 0 {
        None
    } else {
        Some(String::from_utf16_lossy(&buf[..end]))
    }
}

fn hkcr_string_value(subkey: &str, value_name: Option<&str>) -> Option<String> {
    unsafe {
        let subkey_wide: Vec<u16> = std::ffi::OsStr::new(subkey)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let value_wide = value_name.map(|value| {
            std::ffi::OsStr::new(value)
                .encode_wide()
                .chain(std::iter::once(0))
                .collect::<Vec<u16>>()
        });
        let mut hkey = Default::default();
        if RegOpenKeyExW(
            HKEY_CLASSES_ROOT,
            PCWSTR(subkey_wide.as_ptr()),
            0,
            KEY_READ,
            &mut hkey,
        )
        .is_err()
        {
            return None;
        }
        let query_name = value_wide
            .as_ref()
            .map(|wide| PCWSTR(wide.as_ptr()))
            .unwrap_or(PCWSTR::null());
        let mut kind = REG_VALUE_TYPE::default();
        let mut len = 0u32;
        let _ = RegQueryValueExW(
            hkey,
            query_name,
            None,
            Some(&mut kind),
            None,
            Some(&mut len),
        );
        if len < 2 {
            let _ = RegCloseKey(hkey);
            return None;
        }
        let mut buf = vec![0u16; len as usize / 2 + 1];
        if RegQueryValueExW(
            hkey,
            query_name,
            None,
            Some(&mut kind),
            Some(buf.as_mut_ptr().cast()),
            Some(&mut len),
        )
        .is_err()
        {
            let _ = RegCloseKey(hkey);
            return None;
        }
        let _ = RegCloseKey(hkey);
        let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        let value = String::from_utf16_lossy(&buf[..end]).trim().to_string();
        if value.is_empty() {
            None
        } else {
            Some(value)
        }
    }
}

fn icon_for_registry_location(location: &str) -> Option<Vec<u8>> {
    let size = menu_icon_extract_px();
    let location = location.trim().to_string();
    let key = MenuIconCacheKey::Location {
        location: location.clone(),
        size,
    };
    if let Some(cached) = menu_icon_cache_get(&key) {
        return cached.map(|arc| (*arc).clone());
    }
    let png = shell_icon_png_from_location(&location, size).ok();
    menu_icon_cache_put(key, png.clone());
    png
}

fn icon_for_registry_verb(path: &Path, verb: &str) -> Option<Vec<u8>> {
    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| format!(".{ext}"));
    let progid = ext
        .as_deref()
        .and_then(|ext| assoc_query_string(Default::default(), ASSOCSTR_PROGID, ext, None));
    let mut keys = Vec::new();
    if let Some(progid) = progid.as_deref() {
        keys.push(format!("{progid}\\shell\\{verb}"));
    }
    if let Some(ext) = ext.as_deref() {
        keys.push(format!("SystemFileAssociations\\{ext}\\shell\\{verb}"));
        keys.push(format!("{ext}\\shell\\{verb}"));
    }
    keys.push(format!("*\\shell\\{verb}"));
    keys.push(format!("AllFilesystemObjects\\shell\\{verb}"));
    keys.push(format!("Explorer\\CommandStore\\shell\\{verb}"));

    for key in keys {
        if let Some(location) = hkcr_string_value(&key, Some("Icon"))
            .or_else(|| hkcr_string_value(&format!("{key}\\Icon"), None))
        {
            if let Some(png) = icon_for_registry_location(&location) {
                return Some(png);
            }
        }
    }
    None
}

fn icon_for_file_verb(path: &Path, verb: &str) -> Option<Vec<u8>> {
    let verb = verb.trim();
    if verb.is_empty() {
        return None;
    }
    if let Some(png) = icon_for_registry_verb(path, verb) {
        return Some(png);
    }

    let size = menu_icon_extract_px();
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| format!(".{ext}"))
        .unwrap_or_default();
    let verb_key = verb.to_ascii_lowercase();
    let exe_cache_key = MenuIconCacheKey::VerbExe {
        extension: extension.clone(),
        verb: verb_key,
        size,
    };
    if let Some(cached) = menu_icon_cache_get(&exe_cache_key) {
        return cached.map(|arc| (*arc).clone());
    }

    let png = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| format!(".{ext}"))
        .and_then(|ext| {
            assoc_query_string(Default::default(), ASSOCSTR_EXECUTABLE, &ext, Some(verb))
        })
        .or_else(|| {
            path.extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| format!(".{ext}"))
                .and_then(|ext| assoc_query_string(Default::default(), ASSOCSTR_PROGID, &ext, None))
                .and_then(|progid| {
                    assoc_query_string(Default::default(), ASSOCSTR_EXECUTABLE, &progid, Some(verb))
                })
        })
        .or_else(|| {
            let assoc = path.to_string_lossy();
            assoc_query_string(
                ASSOCF_INIT_BYEXENAME,
                ASSOCSTR_EXECUTABLE,
                &assoc,
                Some(verb),
            )
        })
        .and_then(|exe| {
            let exe_path = Path::new(exe.trim_matches('"'));
            shell_icon_png(exe_path, size).ok()
        });
    menu_icon_cache_put(exe_cache_key, png.clone());
    png
}

pub(crate) unsafe fn resolve_menu_item_icon(
    popup: HMENU,
    menu: &IContextMenu,
    index: u32,
    item_id: u32,
    hbmp: HBITMAP,
    primary_path: &Path,
    label: &str,
    verb: Option<&str>,
) -> Option<Vec<u8>> {
    let row_key = menu_row_cache_key(verb, label, primary_path);
    if let Some(cached) = menu_icon_cache_get(&row_key) {
        return cached.map(|arc| (*arc).clone());
    }

    let png =
        resolve_menu_item_icon_uncached(popup, menu, index, item_id, hbmp, primary_path, verb);
    menu_icon_cache_put(row_key, png.clone());
    png
}

unsafe fn resolve_menu_item_icon_uncached(
    popup: HMENU,
    menu: &IContextMenu,
    index: u32,
    item_id: u32,
    hbmp: HBITMAP,
    primary_path: &Path,
    verb: Option<&str>,
) -> Option<Vec<u8>> {
    tracing::info!(target: "shell_menu", "icon[{index}] step=hbitmap_png begin");
    if let Some(png) = menu_item_bitmap_png(hbmp) {
        return Some(png);
    }
    tracing::info!(target: "shell_menu", "icon[{index}] step=via_draw begin (WM_MEASUREITEM/WM_DRAWITEM)");
    if let Some(png) = menu_item_icon_via_draw(menu, popup, item_id) {
        return Some(png);
    }
    if is_callback_bitmap(hbmp) {
        tracing::info!(target: "shell_menu", "icon[{index}] step=callback_refresh begin");
        let refreshed = refresh_item_bitmap(popup, index);
        if let Some(png) = menu_item_bitmap_png(refreshed) {
            return Some(png);
        }
    }
    if let Some(verb) = verb {
        tracing::info!(target: "shell_menu", "icon[{index}] step=file_verb begin (AssocQueryStringW) verb={verb}");
        if let Some(png) = icon_for_file_verb(primary_path, verb) {
            return Some(png);
        }
    }
    tracing::info!(target: "shell_menu", "icon[{index}] step=none (no icon resolved)");
    None
}
