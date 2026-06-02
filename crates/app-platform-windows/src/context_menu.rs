use crate::com::ensure_com_apartment;
use crate::shell_menu_session;
use std::cell::{Cell, RefCell};
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use windows::core::{Interface, PCSTR, PCWSTR};
use windows::Win32::Foundation::{BOOL, HANDLE, HWND, LPARAM, POINT, WPARAM};
use windows::Win32::Graphics::Gdi::HBITMAP;
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::Shell::{
    IContextMenu, IContextMenu2, ILClone, ILFree, IShellFolder, SHBindToParent, SHParseDisplayName,
    CMF_EXTENDEDVERBS, CMF_NORMAL, CMF_OPTIMIZEFORINVOKE, CMINVOKECOMMANDINFO, GCS_VERBA,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CopyImage, CreatePopupMenu, DestroyMenu, GetCursorPos, GetForegroundWindow, GetMenuItemCount,
    GetMenuItemInfoW, GetSubMenu, SetForegroundWindow, TrackPopupMenu, HMENU, IMAGE_BITMAP,
    LR_COPYRETURNORG, MENUITEMINFOW, MFT_SEPARATOR, MIIM_BITMAP, MIIM_FTYPE, MIIM_ID, MIIM_STRING,
    MIIM_SUBMENU, TPM_LEFTALIGN, TPM_RETURNCMD, TPM_RIGHTBUTTON,
};

use crate::shell_icon::{bitmap_to_png, menu_icon_pixel_size, system_scale_factor};

const CMD_FIRST: u32 = 1;
const CMD_LAST: u32 = 0x7fff;
const MAX_SHELL_MENU_ITEMS: usize = 96;
const MAX_SUBMENU_DEPTH: u32 = 8;
const SHELL_MENU_ICONS_ENABLED: bool = true;

thread_local! {
    /// STA thread: physical pixels for `CopyImage` when rasterizing menu bitmaps.
    static MENU_ICON_EXTRACT_PX: Cell<u32> = const { Cell::new(16) };
}

fn set_menu_icon_extract_px(px: u32) {
    MENU_ICON_EXTRACT_PX.with(|c| {
        c.set(px.clamp(16, crate::shell_icon::MAX_ICON_SIZE));
    });
}

fn menu_icon_extract_px() -> u32 {
    MENU_ICON_EXTRACT_PX.with(|c| c.get())
}

/// Strip Win32 menu mnemonics (`&`) the same way as Files `ExtractLabelAndAccessKey`.
pub fn format_shell_menu_label(raw: &str) -> String {
    let mut label = String::new();
    let mut chars = raw.chars().peekable();
    while let Some(current) = chars.next() {
        if current != '&' {
            label.push(current);
            continue;
        }
        let Some(next) = chars.next() else {
            label.push('&');
            break;
        };
        if next == '&' {
            label.push('&');
            continue;
        }
        label.push(next);
    }
    label
}

macro_rules! shell_log {
    ($($t:tt)*) => {{
        let _ = format_args!($($t)*);
    }};
}

/// One row in a Files-style merged context flyout (not a native `TrackPopupMenu` surface).
#[derive(Debug, Clone)]
pub enum ShellContextMenuEntry {
    Separator,
    Item {
        label: String,
        command_offset: u32,
        command_string: Option<String>,
        /// PNG bytes (16×16) from the Shell menu bitmap, when present.
        icon_png: Option<Vec<u8>>,
    },
    /// Nested Shell popup; `lazy_parent_index` is set when children load on expand (Files).
    Submenu {
        label: String,
        children: Vec<ShellContextMenuEntry>,
        icon_png: Option<Vec<u8>>,
        /// Parent HMENU index for [`expand_lazy_submenu`]; `None` if `children` are populated.
        lazy_parent_index: Option<u32>,
    },
}

const WM_INITMENUPOPUP: u32 = 0x0117;

fn path_to_wide(path: &Path) -> Vec<u16> {
    OsStr::new(path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

fn same_parent(paths: &[PathBuf]) -> bool {
    let Some(first) = paths
        .first()
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
    else {
        return false;
    };
    paths
        .iter()
        .all(|p| p.parent().map(|p| p.to_path_buf()) == Some(first.clone()))
}

unsafe fn bind_parent_and_relative(path: &Path) -> anyhow::Result<(IShellFolder, *mut ITEMIDLIST)> {
    let wide = path_to_wide(path);
    let mut full_pidl: *mut ITEMIDLIST = std::ptr::null_mut();
    SHParseDisplayName(PCWSTR(wide.as_ptr()), None, &mut full_pidl, 0, None)?;

    let mut relative: *mut ITEMIDLIST = std::ptr::null_mut();
    let parent: IShellFolder = SHBindToParent(full_pidl, Some(&mut relative))?;
    // `relative` points into `full_pidl` memory; clone before freeing the full PIDL.
    let relative_owned = ILClone(relative);
    ILFree(Some(full_pidl));
    if relative_owned.is_null() {
        anyhow::bail!("ILClone failed for {}", path.display());
    }
    Ok((parent, relative_owned))
}

unsafe fn free_pidl(pidl: *mut ITEMIDLIST) {
    if !pidl.is_null() {
        ILFree(Some(pidl));
    }
}

fn should_skip_shell_verb(command_string: Option<&str>, label: &str) -> bool {
    const KNOWN: &[&str] = &[
        "open",
        "opennew",
        "opencontaining",
        "opennewprocess",
        "runas",
        "runasuser",
        "cut",
        "copy",
        "paste",
        "delete",
        "properties",
        "link",
        "rename",
        "explore",
        "openinfiles",
        "extract",
        "copyaspath",
        "undelete",
        "empty",
        "format",
    ];
    if let Some(verb) = command_string {
        if KNOWN.iter().any(|k| verb.eq_ignore_ascii_case(k)) {
            return true;
        }
    }
    let lower = label.to_ascii_lowercase();
    KNOWN.iter().any(|k| lower == *k)
}

struct ContextMenuHandle {
    menu: IContextMenu,
    popup: HMENU,
    child_pidls: Vec<*mut ITEMIDLIST>,
}

thread_local! {
    static PREPARED_MENU: RefCell<Option<ContextMenuHandle>> = const { RefCell::new(None) };
}

pub(crate) fn release_prepared_menu() {
    PREPARED_MENU.with(|slot| {
        if let Some(handle) = slot.borrow_mut().take() {
            unsafe {
                handle.release();
            }
        }
    });
}

/// Build menu and enumerate top-level only (Files: `EnumMenuItems(..., loadSubmenus: false)`).
pub(crate) fn prepare_and_enumerate_top_level(
    paths: &[PathBuf],
    extended_verbs: bool,
    menu_icon_extract_px: u32,
) -> anyhow::Result<Vec<ShellContextMenuEntry>> {
    set_menu_icon_extract_px(menu_icon_extract_px);
    shell_log!(
        "prepare_and_enumerate_top_level: paths={:?} extended={} icon_px={menu_icon_extract_px}",
        paths,
        extended_verbs
    );
    release_prepared_menu();
    unsafe {
        let handle = create_context_menu(paths, extended_verbs)?;
        PREPARED_MENU.with(|slot| *slot.borrow_mut() = Some(handle));
    }
    enumerate_prepared_menu_top_level()
}

pub(crate) fn enumerate_prepared_menu_top_level() -> anyhow::Result<Vec<ShellContextMenuEntry>> {
    PREPARED_MENU.with(|slot| {
        let guard = slot.borrow();
        let Some(handle) = guard.as_ref() else {
            anyhow::bail!("no prepared shell context menu");
        };
        unsafe { enumerate_popup_menu(handle.popup, &handle.menu, 0, false) }
    })
}

/// Files `LoadSubMenu`: `HandleMenuMsg(WM_INITMENUPOPUP)` then enumerate that HMENU.
pub(crate) fn expand_lazy_submenu(parent_index: u32) -> anyhow::Result<Vec<ShellContextMenuEntry>> {
    PREPARED_MENU.with(|slot| {
        let guard = slot.borrow();
        let Some(handle) = guard.as_ref() else {
            anyhow::bail!("no prepared shell context menu");
        };
        unsafe { expand_lazy_submenu_inner(handle.popup, &handle.menu, parent_index) }
    })
}

unsafe fn expand_lazy_submenu_inner(
    popup: HMENU,
    menu: &IContextMenu,
    parent_index: u32,
) -> anyhow::Result<Vec<ShellContextMenuEntry>> {
    let mut info = MENUITEMINFOW {
        cbSize: std::mem::size_of::<MENUITEMINFOW>() as u32,
        fMask: MIIM_FTYPE | MIIM_ID | MIIM_STRING | MIIM_BITMAP | MIIM_SUBMENU,
        ..Default::default()
    };
    let mut label_buf = [0u16; 512];
    info.dwTypeData = windows::core::PWSTR(label_buf.as_mut_ptr());
    info.cch = label_buf.len() as u32;

    if GetMenuItemInfoW(popup, parent_index, true, &mut info).is_err() {
        anyhow::bail!("GetMenuItemInfoW failed for submenu index {parent_index}");
    }

    let submenu = if !info.hSubMenu.is_invalid() {
        info.hSubMenu
    } else {
        GetSubMenu(popup, parent_index as i32)
    };
    if submenu.is_invalid() {
        return Ok(Vec::new());
    }

    if let Ok(cmenu2) = menu.cast::<IContextMenu2>() {
        let _ = cmenu2.HandleMenuMsg(
            WM_INITMENUPOPUP,
            WPARAM(submenu.0 as usize),
            LPARAM(parent_index as isize),
        );
    }

    enumerate_popup_menu(submenu, menu, 1, true)
}

pub(crate) fn invoke_prepared_menu(command_offset: u32) -> anyhow::Result<()> {
    unsafe {
        let Some(menu) = PREPARED_MENU.with(|slot| slot.borrow().as_ref().map(|h| h.menu.clone()))
        else {
            anyhow::bail!("no prepared shell context menu for invoke");
        };
        let mut info = CMINVOKECOMMANDINFO::default();
        info.cbSize = std::mem::size_of::<CMINVOKECOMMANDINFO>() as u32;
        info.lpVerb = PCSTR::from_raw(command_offset as usize as *const u8);
        info.nShow = 1;
        menu.InvokeCommand(&info)?;
        Ok(())
    }
}

impl ContextMenuHandle {
    unsafe fn release(self) {
        let ContextMenuHandle {
            menu,
            popup,
            child_pidls,
        } = self;
        drop(menu);
        let _ = DestroyMenu(popup);
        for pidl in child_pidls {
            free_pidl(pidl);
        }
    }
}

unsafe fn create_context_menu(
    paths: &[PathBuf],
    extended_verbs: bool,
) -> anyhow::Result<ContextMenuHandle> {
    let (parent_sf, first_child) = bind_parent_and_relative(&paths[0])?;
    let mut child_pidls = vec![first_child];

    for path in paths.iter().skip(1) {
        let (_, relative) = bind_parent_and_relative(path)?;
        child_pidls.push(relative);
    }

    let apidl: Vec<*const ITEMIDLIST> = child_pidls
        .iter()
        .map(|p| *p as *const ITEMIDLIST)
        .collect();

    let menu: IContextMenu = parent_sf.GetUIObjectOf(HWND::default(), &apidl, None)?;

    let popup = CreatePopupMenu()?;
    let flags = if extended_verbs {
        CMF_NORMAL | CMF_EXTENDEDVERBS | CMF_OPTIMIZEFORINVOKE
    } else {
        CMF_NORMAL | CMF_OPTIMIZEFORINVOKE
    };
    menu.QueryContextMenu(popup, 0, CMD_FIRST, CMD_LAST, flags)?;
    let _raw_count = GetMenuItemCount(popup);

    Ok(ContextMenuHandle {
        menu,
        popup,
        child_pidls,
    })
}

unsafe fn command_verb(context_menu: &IContextMenu, command_offset: u32) -> Option<String> {
    let id = CMD_FIRST.saturating_add(command_offset) as usize;
    let mut buf = [0u8; 256];
    buf.fill(0);
    if context_menu
        .GetCommandString(
            id,
            GCS_VERBA,
            None,
            windows::core::PSTR(buf.as_mut_ptr()),
            buf.len() as u32,
        )
        .is_err()
    {
        return None;
    }
    let len = buf.iter().position(|&c| c == 0).unwrap_or(0);
    if len == 0 {
        return None;
    }
    Some(String::from_utf8_lossy(&buf[..len]).into_owned())
}

unsafe fn enumerate_popup_menu(
    popup: HMENU,
    context_menu: &IContextMenu,
    depth: u32,
    expand_submenus: bool,
) -> anyhow::Result<Vec<ShellContextMenuEntry>> {
    if depth >= MAX_SUBMENU_DEPTH {
        shell_log!("enumerate: max submenu depth {}", depth);
        return Ok(Vec::new());
    }

    let count = GetMenuItemCount(popup);
    if count == 0 {
        shell_log!("enumerate: empty HMENU");
        return Ok(Vec::new());
    }

    let mut entries = Vec::new();
    let mut info = MENUITEMINFOW {
        cbSize: std::mem::size_of::<MENUITEMINFOW>() as u32,
        fMask: MIIM_FTYPE | MIIM_ID | MIIM_STRING | MIIM_BITMAP | MIIM_SUBMENU,
        ..Default::default()
    };

    for index in 0..count as u32 {
        if entries.len() >= MAX_SHELL_MENU_ITEMS {
            break;
        }

        let mut label_buf = [0u16; 512];
        info.dwTypeData = windows::core::PWSTR(label_buf.as_mut_ptr());
        info.cch = label_buf.len() as u32;
        info.hSubMenu = Default::default();

        if GetMenuItemInfoW(popup, index, true, &mut info).is_err() {
            continue;
        }

        if info.fType.0 & MFT_SEPARATOR.0 != 0 {
            entries.push(ShellContextMenuEntry::Separator);
            continue;
        }

        let submenu = if !info.hSubMenu.is_invalid() {
            info.hSubMenu
        } else {
            GetSubMenu(popup, index as i32)
        };
        if !submenu.is_invalid() {
            let label_len = label_buf.iter().position(|&c| c == 0).unwrap_or(0);
            let label = format_shell_menu_label(&String::from_utf16_lossy(&label_buf[..label_len]));
            if expand_submenus {
                if let Ok(children) = enumerate_popup_menu(submenu, context_menu, depth + 1, true) {
                    if !children.is_empty() {
                        entries.push(ShellContextMenuEntry::Submenu {
                            label,
                            children,
                            icon_png: menu_item_icon_png(info.hbmpItem),
                            lazy_parent_index: None,
                        });
                    }
                }
            } else {
                entries.push(ShellContextMenuEntry::Submenu {
                    label,
                    children: Vec::new(),
                    icon_png: menu_item_icon_png(info.hbmpItem),
                    lazy_parent_index: Some(index),
                });
            }
            continue;
        }

        let label_len = label_buf.iter().position(|&c| c == 0).unwrap_or(0);
        if label_len == 0 {
            continue;
        }
        let label = format_shell_menu_label(&String::from_utf16_lossy(&label_buf[..label_len]));
        let command_offset = info.wID.saturating_sub(CMD_FIRST);
        if command_offset > CMD_LAST.saturating_sub(CMD_FIRST) {
            continue;
        }

        let verb = command_verb(context_menu, command_offset);
        if should_skip_shell_verb(verb.as_deref(), &label) {
            continue;
        }

        entries.push(ShellContextMenuEntry::Item {
            label,
            command_offset,
            command_string: verb,
            icon_png: menu_item_icon_png(info.hbmpItem),
        });
    }

    Ok(entries)
}

unsafe fn menu_item_icon_png(hbmp: HBITMAP) -> Option<Vec<u8>> {
    if !SHELL_MENU_ICONS_ENABLED || hbmp.is_invalid() {
        return None;
    }
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let extract_px = menu_icon_extract_px() as i32;
        let copy = CopyImage(
            HANDLE(hbmp.0),
            IMAGE_BITMAP,
            extract_px,
            extract_px,
            LR_COPYRETURNORG,
        )
        .ok()?;
        bitmap_to_png(HBITMAP(copy.0)).ok()
    }))
    .ok()
    .flatten()
}

/// Opens the system «Open with» dialog for a file (same as Explorer).
pub fn show_open_with_dialog(path: &Path) -> anyhow::Result<()> {
    use std::process::Command;

    let path = path.to_string_lossy();
    let status = Command::new("rundll32.exe")
        .arg("shell32.dll,OpenAs_RunDLL")
        .arg(path.as_ref())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        anyhow::bail!("OpenAs dialog exited with {status}")
    }
}

/// Opens the parent folder in a new Explorer window (Files «Open in new window» subset).
pub fn open_in_new_explorer_window(path: &Path) -> anyhow::Result<()> {
    use std::process::Command;

    let target = if path.is_dir() {
        path.to_path_buf()
    } else {
        path.parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| path.to_path_buf())
    };
    let status = Command::new("explorer.exe")
        .arg(target.as_os_str())
        .status()?;
    if status.success() {
        Ok(())
    } else {
        anyhow::bail!("explorer exited with {status}")
    }
}

/// Preloads Shell `QueryContextMenu` on a background thread (Files `WarmUpQueryContextMenuAsync`).
pub fn warm_up_query_context_menu() {
    std::thread::Builder::new()
        .name("cyber_desktop-shell-warmup".into())
        .spawn(|| {
            let path = std::env::temp_dir().join("cyber_desktop_shell_warmup.txt");
            shell_log!("warm_up start: {}", path.display());
            let result: anyhow::Result<Vec<ShellContextMenuEntry>> = (|| {
                std::fs::write(&path, b"")?;
                let icon_px = menu_icon_pixel_size(system_scale_factor());
                let entries =
                    shell_menu_session::query_with_session(&[path.clone()], false, icon_px)?;
                let _ = std::fs::remove_file(&path);
                Ok(entries)
            })();
            match result {
                Ok(entries) => {
                    shell_log!("warm_up ok: entries={}", entries.len());
                }
                Err(error) => {
                    shell_log!("warm_up err: {error:#}");
                    let _ = std::fs::remove_file(&path);
                }
            }
            shell_menu_session::clear_session();
        })
        .ok();
}

/// Enumerates Shell entries on a dedicated STA thread (Files `ThreadWithMessageQueue`).
pub fn query_shell_context_menu_items(
    paths: &[PathBuf],
    extended_verbs: bool,
    menu_icon_extract_px: u32,
) -> anyhow::Result<Vec<ShellContextMenuEntry>> {
    shell_log!(
        "query start: n_paths={} extended={} icon_px={menu_icon_extract_px} paths={:?}",
        paths.len(),
        extended_verbs,
        paths
    );
    if paths.is_empty() {
        shell_log!("query abort: no paths");
        return Ok(Vec::new());
    }
    if !same_parent(paths) {
        shell_log!("query abort: not same_parent");
        return Ok(Vec::new());
    }
    let entries =
        shell_menu_session::query_with_session(paths, extended_verbs, menu_icon_extract_px)?;
    Ok(entries)
}

/// Invokes one Shell menu command by offset (from [`query_shell_context_menu_items`]).
pub fn invoke_shell_context_menu_item(
    _paths: &[PathBuf],
    command_offset: u32,
    _extended_verbs: bool,
) -> anyhow::Result<()> {
    let offset = command_offset;
    std::thread::spawn(move || {
        if let Err(error) = shell_menu_session::invoke_on_session(offset) {
            tracing::error!(target: "shell_menu", offset, error = ?error, "invoke failed");
        }
    });
    Ok(())
}

/// Optional Explorer-style popup (not the default Files parity UX).
pub fn show_shell_context_menu(paths: &[PathBuf]) -> anyhow::Result<()> {
    if paths.is_empty() {
        return Ok(());
    }

    if !same_parent(paths) {
        return show_shell_context_menu_fallback(paths);
    }

    ensure_com_apartment()?;
    unsafe { show_shell_context_menu_inner(paths) }
}

unsafe fn show_shell_context_menu_inner(paths: &[PathBuf]) -> anyhow::Result<()> {
    let handle = create_context_menu(paths, false)?;
    let hwnd = GetForegroundWindow();

    let mut cursor = POINT::default();
    GetCursorPos(&mut cursor)?;
    let _ = SetForegroundWindow(hwnd);

    let cmd = TrackPopupMenu(
        handle.popup,
        TPM_RETURNCMD | TPM_LEFTALIGN | TPM_RIGHTBUTTON,
        cursor.x,
        cursor.y,
        0,
        hwnd,
        None,
    );

    if cmd == BOOL(0) {
        handle.release();
        return Ok(());
    }

    let offset = cmd.0 as u32 - CMD_FIRST;
    let mut info = CMINVOKECOMMANDINFO::default();
    info.cbSize = std::mem::size_of::<CMINVOKECOMMANDINFO>() as u32;
    info.hwnd = hwnd;
    info.lpVerb = PCSTR::from_raw(offset as usize as *const u8);
    info.nShow = 1;
    handle.menu.InvokeCommand(&info)?;
    handle.release();
    Ok(())
}

/// Fallback when COM menu setup fails: open parent folder in Explorer.
pub fn show_shell_context_menu_fallback(paths: &[PathBuf]) -> anyhow::Result<()> {
    use windows::core::w;
    use windows::Win32::UI::Shell::{ShellExecuteExW, SHELLEXECUTEINFOW};
    use windows::Win32::UI::WindowsAndMessaging::SW_SHOW;

    let primary = &paths[0];
    let parent = primary
        .parent()
        .filter(|p| p.exists())
        .unwrap_or(primary.as_path());
    let parent_wide = path_to_wide(parent);
    let args = if paths.len() == 1 {
        format!(
            "/select,\"{}\"",
            primary.display().to_string().replace('"', "")
        )
    } else {
        String::new()
    };
    let args_wide = path_to_wide(Path::new(&args));

    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        hwnd: HWND::default(),
        lpVerb: w!("open"),
        lpFile: PCWSTR(parent_wide.as_ptr()),
        lpParameters: if args.is_empty() {
            PCWSTR::null()
        } else {
            PCWSTR(args_wide.as_ptr())
        },
        nShow: SW_SHOW.0,
        ..Default::default()
    };

    unsafe {
        ShellExecuteExW(&mut info)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_shell_menu_label_strips_mnemonics() {
        assert_eq!(format_shell_menu_label("Open &with..."), "Open with...");
        assert_eq!(format_shell_menu_label("Copy && paste"), "Copy & paste");
        assert_eq!(format_shell_menu_label("Pr&operties"), "Properties");
    }
}

#[cfg(all(windows, test))]
mod windows_tests {
    use super::*;
    use std::fs;

    /// Smoke test for shell context menu queries.
    #[test]
    fn query_shell_context_menu_items_smoke() {
        let dir = std::env::temp_dir();
        let file = dir.join("cyber_desktop_shell_menu_test.txt");
        fs::write(&file, b"test").expect("write temp file");
        let subdir = dir.join("cyber_desktop_shell_menu_test_dir");
        fs::create_dir_all(&subdir).expect("create temp dir");

        for (label, paths) in [
            ("file", vec![file.clone()]),
            ("directory", vec![subdir.clone()]),
        ] {
            let icon_px = menu_icon_pixel_size(system_scale_factor());
            let normal =
                query_shell_context_menu_items(&paths, false, icon_px).unwrap_or_else(|e| {
                    panic!("query normal ({label}): {e:#}");
                });
            let _ = (label, normal.len());
            assert!(!normal.is_empty(), "expected Shell entries for {label}");
        }

        let _ = fs::remove_file(file);
        let _ = fs::remove_dir(subdir);
    }
}
