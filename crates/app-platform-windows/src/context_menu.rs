use crate::com::ensure_com_apartment;
use crate::shell_menu_session;
use std::cell::RefCell;
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use windows::core::{Interface, PCSTR, PCWSTR};
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, POINT, WPARAM};
use windows::Win32::System::Registry::{
    RegCloseKey, RegOpenKeyExW, RegQueryValueExW, HKEY_CLASSES_ROOT, KEY_READ, REG_VALUE_TYPE,
};
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::Shell::{
    IContextMenu, IContextMenu2, ILClone, ILFree, IShellFolder, SHBindToParent, SHParseDisplayName,
    CMF_EXTENDEDVERBS, CMF_NORMAL, CMINVOKECOMMANDINFO, GCS_HELPTEXTW, GCS_VERBA, GCS_VERBW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreatePopupMenu, DestroyMenu, GetCursorPos, GetForegroundWindow, GetMenuItemCount,
    GetMenuItemInfoW, GetSubMenu, SetForegroundWindow, TrackPopupMenu, HMENU, MENUITEMINFOW,
    MFT_SEPARATOR, MIIM_BITMAP, MIIM_FTYPE, MIIM_ID, MIIM_STRING, MIIM_SUBMENU, TPM_LEFTALIGN,
    TPM_RETURNCMD, TPM_RIGHTBUTTON, WM_INITMENUPOPUP,
};

#[cfg(test)]
use crate::shell_icon::menu_icon_pixel_size;
use crate::shell_menu_icon::{
    init_popup_menu, refresh_item_bitmap, resolve_menu_item_icon, set_menu_icon_extract_px,
};

const CMD_FIRST: u32 = 1;
const CMD_LAST: u32 = 0x7fff;
const MAX_SHELL_MENU_ITEMS: usize = 96;
const MAX_SUBMENU_DEPTH: u32 = 8;
const SHELL_MENU_ICONS_ENABLED: bool = true;

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

fn looks_like_braced_guid(label: &str) -> bool {
    let t = label.trim();
    t.starts_with('{')
        && t.ends_with('}')
        && t.len() > 2
        && t[1..t.len() - 1]
            .chars()
            .all(|c| c.is_ascii_hexdigit() || c == '-')
}

/// Shell sometimes exposes internal verb ids (`edit`, `setdesktopwallpaper`) as menu text.
fn is_likely_internal_verb_label(label: &str) -> bool {
    let t = label.trim();
    !t.is_empty() && t.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_') && !t.contains('&')
}

fn hkcr_default_display_name(subkey: &str) -> Option<String> {
    unsafe {
        let subkey_wide: Vec<u16> = OsStr::new(subkey)
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
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
        let mut kind = REG_VALUE_TYPE::default();
        let mut len = 0u32;
        let _ = RegQueryValueExW(
            hkey,
            PCWSTR::null(),
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
            PCWSTR::null(),
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
        let name = String::from_utf16_lossy(&buf[..end]);
        let name = name.trim();
        if name.is_empty() {
            None
        } else {
            Some(format_shell_menu_label(name))
        }
    }
}

fn resolve_submenu_label(raw_label: &str) -> String {
    let formatted = format_shell_menu_label(raw_label.trim());
    resolve_braced_guid_display_name(&formatted).unwrap_or(formatted)
}

fn resolve_braced_guid_display_name(label: &str) -> Option<String> {
    if !looks_like_braced_guid(label) {
        return None;
    }
    let t = label.trim();
    hkcr_default_display_name(t).or_else(|| {
        let inner = t.trim_matches(|c| c == '{' || c == '}');
        hkcr_default_display_name(&format!("CLSID\\{inner}"))
    })
}

unsafe fn command_string_wide(
    context_menu: &IContextMenu,
    command_offset: u32,
    string_type: u32,
) -> Option<String> {
    let mut buf = [0u16; 512];
    if context_menu
        .GetCommandString(
            command_offset as usize,
            string_type,
            None,
            windows::core::PSTR(buf.as_mut_ptr().cast()),
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
    Some(String::from_utf16_lossy(&buf[..len]))
}

unsafe fn resolve_menu_item_label(
    context_menu: &IContextMenu,
    command_offset: u32,
    raw_label: &str,
) -> String {
    let menu_label = format_shell_menu_label(raw_label.trim());
    if let Some(name) = resolve_braced_guid_display_name(&menu_label) {
        return name;
    }
    if let Some(help) = command_string_wide(context_menu, command_offset, GCS_HELPTEXTW) {
        let help = format_shell_menu_label(help.trim());
        if !help.is_empty()
            && (looks_like_braced_guid(&menu_label) || is_likely_internal_verb_label(&menu_label))
        {
            return help;
        }
    }
    if is_likely_internal_verb_label(&menu_label) {
        if let Some(verbw) = command_string_wide(context_menu, command_offset, GCS_VERBW) {
            let verbw = format_shell_menu_label(verbw.trim());
            if !verbw.is_empty() && !looks_like_braced_guid(&verbw) {
                return verbw;
            }
        }
    }
    menu_label
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
    primary_path: PathBuf,
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
        unsafe { enumerate_popup_menu(handle.popup, &handle.menu, 0, false, &handle.primary_path) }
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

    let primary_path = PREPARED_MENU.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|handle| handle.primary_path.clone())
            .unwrap_or_default()
    });
    enumerate_popup_menu(submenu, menu, 1, true, &primary_path)
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
            primary_path: _,
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
    // Files: `CMF_NORMAL` or `CMF_EXTENDEDVERBS` only (no `CMF_EXPLORE`).
    let flags = if extended_verbs {
        CMF_NORMAL | CMF_EXTENDEDVERBS
    } else {
        CMF_NORMAL
    };
    menu.QueryContextMenu(popup, 0, CMD_FIRST, CMD_LAST, flags)?;
    let _raw_count = GetMenuItemCount(popup);

    Ok(ContextMenuHandle {
        menu,
        popup,
        child_pidls,
        primary_path: paths[0].clone(),
    })
}

unsafe fn shell_item_icon_png(
    popup: HMENU,
    context_menu: &IContextMenu,
    index: u32,
    item_id: u32,
    _hbmp: windows::Win32::Graphics::Gdi::HBITMAP,
    primary_path: &Path,
    label: &str,
    verb: Option<&str>,
) -> Option<Vec<u8>> {
    if !SHELL_MENU_ICONS_ENABLED {
        return None;
    }
    let hbmp = refresh_item_bitmap(popup, index);
    resolve_menu_item_icon(
        popup,
        context_menu,
        index,
        item_id,
        hbmp,
        primary_path,
        label,
        verb,
    )
}

unsafe fn command_verb(context_menu: &IContextMenu, command_offset: u32) -> Option<String> {
    let mut buf = [0u8; 256];
    buf.fill(0);
    if context_menu
        .GetCommandString(
            command_offset as usize,
            GCS_VERBA,
            None,
            windows::core::PSTR(buf.as_mut_ptr()),
            buf.len() as u32,
        )
        .is_ok()
    {
        let len = buf.iter().position(|&c| c == 0).unwrap_or(0);
        if len > 0 {
            return Some(String::from_utf8_lossy(&buf[..len]).into_owned());
        }
    }

    let mut wide = [0u16; 256];
    if context_menu
        .GetCommandString(
            command_offset as usize,
            GCS_VERBW,
            None,
            windows::core::PSTR(wide.as_mut_ptr().cast()),
            wide.len() as u32,
        )
        .is_ok()
    {
        let len = wide.iter().position(|&c| c == 0).unwrap_or(0);
        if len > 0 {
            return Some(String::from_utf16_lossy(&wide[..len]));
        }
    }
    None
}

unsafe fn enumerate_popup_menu(
    popup: HMENU,
    context_menu: &IContextMenu,
    depth: u32,
    expand_submenus: bool,
    primary_path: &Path,
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

    init_popup_menu(popup, context_menu);

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
            let raw_label = String::from_utf16_lossy(&label_buf[..label_len]);
            let label = resolve_submenu_label(&raw_label);
            if expand_submenus {
                let icon_png = shell_item_icon_png(
                    popup,
                    context_menu,
                    index,
                    info.wID,
                    info.hbmpItem,
                    primary_path,
                    &label,
                    None,
                );
                if let Ok(children) =
                    enumerate_popup_menu(submenu, context_menu, depth + 1, true, primary_path)
                {
                    if !children.is_empty() {
                        entries.push(ShellContextMenuEntry::Submenu {
                            label,
                            children,
                            icon_png,
                            lazy_parent_index: None,
                        });
                    }
                }
            } else {
                let icon_png = shell_item_icon_png(
                    popup,
                    context_menu,
                    index,
                    info.wID,
                    info.hbmpItem,
                    primary_path,
                    &label,
                    None,
                );
                entries.push(ShellContextMenuEntry::Submenu {
                    label,
                    children: Vec::new(),
                    icon_png,
                    lazy_parent_index: Some(index),
                });
            }
            continue;
        }

        let label_len = label_buf.iter().position(|&c| c == 0).unwrap_or(0);
        if label_len == 0 {
            continue;
        }
        let raw_label = String::from_utf16_lossy(&label_buf[..label_len]);
        let command_offset = info.wID.saturating_sub(CMD_FIRST);
        if command_offset > CMD_LAST.saturating_sub(CMD_FIRST) {
            continue;
        }
        let label = resolve_menu_item_label(context_menu, command_offset, &raw_label);

        let verb = command_verb(context_menu, command_offset);
        if should_skip_shell_verb(verb.as_deref(), &label) {
            continue;
        }

        let icon_png = shell_item_icon_png(
            popup,
            context_menu,
            index,
            info.wID,
            info.hbmpItem,
            primary_path,
            &label,
            verb.as_deref(),
        );

        entries.push(ShellContextMenuEntry::Item {
            label,
            command_offset,
            command_string: verb,
            icon_png,
        });
    }

    Ok(entries)
}

/// Opens the system «Open with» dialog for a file (same as Explorer).
pub fn show_open_with_dialog(path: &Path) -> anyhow::Result<()> {
    let path = path.to_path_buf();
    std::thread::spawn(move || {
        if let Err(error) = show_open_with_dialog_blocking(&path) {
            tracing::error!(target: "shell_menu", ?error, "OpenAs dialog failed");
        }
    });
    Ok(())
}

/// Blocks until the Open With dialog closes; safe to call from a background thread.
pub fn show_open_with_dialog_blocking(path: &Path) -> anyhow::Result<()> {
    let path = path.to_path_buf();
    crate::com::run_sta_task(move || show_open_with_dialog_sta(&path))
}

fn show_open_with_dialog_sta(path: &Path) -> anyhow::Result<()> {
    use windows::core::PCWSTR;
    use windows::Win32::UI::Shell::{
        SHOpenWithDialog, OAIF_ALLOW_REGISTRATION, OAIF_EXEC, OPENASINFO,
    };
    use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

    let path_wide = path_to_wide(path);
    let class_wide: Vec<u16> = std::iter::once(0).collect();
    let info = OPENASINFO {
        pcszFile: PCWSTR(path_wide.as_ptr()),
        pcszClass: PCWSTR(class_wide.as_ptr()),
        oaifInFlags: OAIF_ALLOW_REGISTRATION | OAIF_EXEC,
    };
    let hwnd = unsafe { GetForegroundWindow() };
    unsafe { SHOpenWithDialog(hwnd, &info as *const _)? };
    Ok(())
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

/// Preloads Shell STA thread (lightweight) so first real QueryContextMenu doesn't need to spawn it.
pub fn warm_up_query_context_menu() {
    std::thread::Builder::new()
        .name("cyber_desktop-shell-warmup".into())
        .spawn(|| {
            use std::time::Instant;
            let thread_start = Instant::now();
            tracing::info!(target: "startup", step = "shell_warmup_thread_begin");
            shell_menu_session::init_sta_thread();
            tracing::info!(
                target: "startup",
                step = "shell_warmup_thread_done",
                block_ms = thread_start.elapsed().as_secs_f64() * 1000.0
            );
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

    #[test]
    fn looks_like_braced_guid_detects_progid() {
        assert!(looks_like_braced_guid(
            "{BFF0E2A4-C70C-4AD7-AC3D-10D1ECEBB5B4}"
        ));
        assert!(!looks_like_braced_guid("Bandizip"));
    }

    #[test]
    fn is_likely_internal_verb_label_detects_edit() {
        assert!(is_likely_internal_verb_label("edit"));
        assert!(is_likely_internal_verb_label("setdesktopwallpaper"));
        assert!(!is_likely_internal_verb_label("Open with QtAV"));
        assert!(!is_likely_internal_verb_label("压缩为 test.zip"));
    }
}

#[cfg(all(windows, test))]
mod windows_tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use std::path::{Path, PathBuf};

    fn sanitize_filename(label: &str) -> String {
        let mut s = String::with_capacity(label.len());
        for ch in label.chars() {
            s.push(match ch {
                '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | '.' => '_',
                c if c.is_control() => '_',
                c => c,
            });
        }
        let s = s.trim_matches('_').trim();
        if s.is_empty() {
            "unnamed".to_string()
        } else if s.len() > 80 {
            s.chars().take(80).collect()
        } else {
            s.to_string()
        }
    }

    fn png_stats(png: &[u8]) -> String {
        match image::ImageReader::new(std::io::Cursor::new(png))
            .with_guessed_format()
            .ok()
            .and_then(|r| r.decode().ok())
        {
            Some(img) => {
                let rgba = img.to_rgba8();
                let (w, h) = rgba.dimensions();
                let mut transparent = 0u32;
                let mut white = 0u32;
                for p in rgba.pixels() {
                    if p[3] == 0 {
                        transparent += 1;
                    } else if p[0] > 240 && p[1] > 240 && p[2] > 240 {
                        white += 1;
                    }
                }
                let total = (w * h).max(1);
                format!(
                    "decode_ok {w}x{h} bytes={} transparent={transparent} ({:.0}%) white_opaque={white} ({:.0}%)",
                    png.len(),
                    transparent as f64 * 100.0 / total as f64,
                    white as f64 * 100.0 / total as f64,
                )
            }
            None => format!("decode_fail bytes={}", png.len()),
        }
    }

    fn write_icon_png(path: &Path, png: &[u8]) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap_or_else(|e| {
                panic!("mkdir {}: {e}", parent.display());
            });
        }
        fs::write(path, png).unwrap_or_else(|e| panic!("write {}: {e}", path.display()));
    }

    fn export_entries_recursive(
        entries: &[ShellContextMenuEntry],
        out_dir: &Path,
        manifest: &mut impl Write,
        path_prefix: &str,
        submenu_stack: &str,
    ) {
        for (index, entry) in entries.iter().enumerate() {
            match entry {
                ShellContextMenuEntry::Separator => {
                    writeln!(manifest, "{submenu_stack}[{index:03}] --- separator ---").ok();
                }
                ShellContextMenuEntry::Item {
                    label,
                    command_offset,
                    command_string,
                    icon_png,
                } => {
                    let base = format!("{path_prefix}{index:03}_{}", sanitize_filename(label));
                    writeln!(
                        manifest,
                        "{submenu_stack}[{index:03}] ITEM label={label:?} verb={command_string:?} offset={command_offset}"
                    )
                    .ok();
                    if let Some(png) = icon_png.as_deref().filter(|p| !p.is_empty()) {
                        let file = out_dir.join(format!("{base}.png"));
                        write_icon_png(&file, png);
                        let _ = writeln!(
                            manifest,
                            "    icon -> {} | {}",
                            file.display(),
                            png_stats(png)
                        );
                    } else {
                        let _ = writeln!(manifest, "    icon -> (none)");
                    }
                }
                ShellContextMenuEntry::Submenu {
                    label,
                    children,
                    icon_png,
                    lazy_parent_index,
                } => {
                    let base = format!("{path_prefix}{index:03}_{}", sanitize_filename(label));
                    let stack = format!("{submenu_stack}/{label}");
                    writeln!(
                        manifest,
                        "{submenu_stack}[{index:03}] SUBMENU label={label:?} lazy={lazy_parent_index:?}"
                    )
                    .ok();
                    if let Some(png) = icon_png.as_deref().filter(|p| !p.is_empty()) {
                        let file = out_dir.join(format!("{base}__submenu.png"));
                        write_icon_png(&file, png);
                        let _ = writeln!(
                            manifest,
                            "    submenu_icon -> {} | {}",
                            file.display(),
                            png_stats(png)
                        );
                    } else {
                        let _ = writeln!(manifest, "    submenu_icon -> (none)");
                    }

                    let resolved = if children.is_empty() {
                        lazy_parent_index
                            .and_then(|idx| crate::shell_menu_session::load_lazy_submenu(idx).ok())
                            .unwrap_or_default()
                    } else {
                        children.clone()
                    };
                    let child_prefix = format!("{base}/");
                    export_entries_recursive(&resolved, out_dir, manifest, &child_prefix, &stack);
                }
            }
        }
    }

    fn export_shell_menu_for_paths(
        tag: &str,
        paths: &[PathBuf],
        extended_verbs: bool,
        out_root: &Path,
    ) {
        let icon_px = menu_icon_pixel_size(system_scale_factor());
        let entries = query_shell_context_menu_items(paths, extended_verbs, icon_px)
            .unwrap_or_else(|e| panic!("query failed ({tag}): {e:#}"));

        let out_dir = out_root.join(format!("{tag}_extended_{extended_verbs}"));
        fs::create_dir_all(&out_dir).expect("create export dir");

        let manifest_path = out_dir.join("manifest.txt");
        let mut manifest = fs::File::create(&manifest_path).expect("create manifest");
        writeln!(
            manifest,
            "tag={tag}\nextended_verbs={extended_verbs}\nicon_px={icon_px}\npaths={paths:?}\nentry_count={}\n",
            entries.len()
        )
        .ok();

        export_entries_recursive(&entries, &out_dir, &mut manifest, "", "");

        let with_icons = entries
            .iter()
            .filter(|e| match e {
                ShellContextMenuEntry::Item { icon_png, .. }
                | ShellContextMenuEntry::Submenu { icon_png, .. } => {
                    icon_png.as_ref().is_some_and(|p| !p.is_empty())
                }
                _ => false,
            })
            .count();
        writeln!(
            manifest,
            "\ntop_level_with_icon_png={with_icons} / {}",
            entries.len()
        )
        .ok();

        eprintln!(
            "exported {tag} (extended={extended_verbs}): {} top-level entries -> {}",
            entries.len(),
            out_dir.display()
        );
        eprintln!("manifest: {}", manifest_path.display());
    }

    /// Export every `icon_png` from a real Shell context menu to PNG files for visual inspection.
    ///
    /// ```powershell
    /// $env:EXPORT_SHELL_MENU_ICONS = "1"
    /// $env:SHELL_MENU_TEST_PATH = "D:\path\to\your\image.png"   # optional
    /// cargo test -p app-platform-windows export_shell_menu_icons_to_disk -- --nocapture
    /// ```
    ///
    /// Output: `target/shell_menu_icons_export/<timestamp>/`
    #[test]
    fn export_shell_menu_icons_to_disk() {
        if std::env::var("EXPORT_SHELL_MENU_ICONS").ok().as_deref() != Some("1") {
            eprintln!(
                "skip export_shell_menu_icons_to_disk: set EXPORT_SHELL_MENU_ICONS=1 to write PNGs"
            );
            return;
        }

        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let out_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("target")
            .join("shell_menu_icons_export")
            .join(stamp.to_string());
        fs::create_dir_all(&out_root).expect("create export root");

        let test_path = std::env::var("SHELL_MENU_TEST_PATH")
            .ok()
            .map(PathBuf::from)
            .filter(|p| p.exists());

        if let Some(path) = test_path {
            export_shell_menu_for_paths("user_file", &[path.clone()], false, &out_root);
            export_shell_menu_for_paths("user_file", &[path], true, &out_root);
        } else {
            eprintln!(
                "SHELL_MENU_TEST_PATH not set or missing; using temp files (set it to your image path)"
            );
            let dir = std::env::temp_dir();
            let png = dir.join("cyber_desktop_shell_icon_export.png");
            fs::write(&png, b"\x89PNG\r\n\x1a\n").expect("write temp png");
            export_shell_menu_for_paths("temp_png", &[png.clone()], false, &out_root);
            export_shell_menu_for_paths("temp_png", &[png], true, &out_root);
            let _ = fs::remove_file(dir.join("cyber_desktop_shell_icon_export.png"));
        }

        eprintln!("\nOpen folder: {}", out_root.display());
        eprintln!("Read manifest.txt in each subfolder for decode stats (white_opaque % explains UI white squares).");
    }

    /// Smoke test for shell context menu queries.
    #[test]
    fn query_shell_context_menu_items_smoke() {
        let dir = std::env::temp_dir();
        let file = dir.join("cyber_desktop_shell_menu_test.txt");
        fs::write(&file, b"test").expect("write temp file");
        let png = dir.join("cyber_desktop_shell_menu_test.png");
        fs::write(&png, b"\x89PNG\r\n\x1a\n").expect("write temp png");
        let subdir = dir.join("cyber_desktop_shell_menu_test_dir");
        fs::create_dir_all(&subdir).expect("create temp dir");

        for (label, paths) in [
            ("file", vec![file.clone()]),
            ("png", vec![png.clone()]),
            ("directory", vec![subdir.clone()]),
        ] {
            let icon_px = menu_icon_pixel_size(system_scale_factor());
            let normal =
                query_shell_context_menu_items(&paths, false, icon_px).unwrap_or_else(|e| {
                    panic!("query normal ({label}): {e:#}");
                });
            let with_icons = normal
                .iter()
                .filter(|e| match e {
                    ShellContextMenuEntry::Item { icon_png, .. } => {
                        icon_png.as_ref().is_some_and(|p| !p.is_empty())
                    }
                    ShellContextMenuEntry::Submenu { icon_png, .. } => {
                        icon_png.as_ref().is_some_and(|p| !p.is_empty())
                    }
                    _ => false,
                })
                .count();
            eprintln!(
                "shell menu {label}: {} entries, {} with icons",
                normal.len(),
                with_icons
            );
            assert!(!normal.is_empty(), "expected Shell entries for {label}");
        }

        let _ = fs::remove_file(file);
        let _ = fs::remove_file(png);
        let _ = fs::remove_dir(subdir);
    }
}
