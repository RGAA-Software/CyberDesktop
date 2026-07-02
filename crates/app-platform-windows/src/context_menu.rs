use crate::com::ensure_com_apartment;
use crate::shell_menu_session;
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
use crate::shell_menu_icon::{init_popup_menu, refresh_item_bitmap, resolve_menu_item_icon};

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
        tracing::info!(target: "shell_menu", "{}", format_args!($($t)*));
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
        /// CLSID of the owning shell extension for Layer B items; `None` for Layer A.
        handler_clsid: Option<String>,
    },
    /// Nested Shell popup; `lazy_parent_index` is set when children load on expand (Files).
    Submenu {
        label: String,
        children: Vec<ShellContextMenuEntry>,
        icon_png: Option<Vec<u8>>,
        /// Parent HMENU index for [`expand_lazy_submenu`]; `None` if `children` are populated.
        lazy_parent_index: Option<u32>,
        /// CLSID of the owning shell extension for Layer B submenus; `None` for Layer A.
        handler_clsid: Option<String>,
    },
}

fn path_to_wide(path: &Path) -> Vec<u16> {
    OsStr::new(path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

pub(crate) fn same_parent(paths: &[PathBuf]) -> bool {
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

pub(crate) unsafe fn bind_parent_and_relative(
    path: &Path,
) -> anyhow::Result<(IShellFolder, *mut ITEMIDLIST)> {
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

pub(crate) unsafe fn free_pidl(pidl: *mut ITEMIDLIST) {
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

pub(crate) struct ContextMenuHandle {
    pub(crate) menu: IContextMenu,
    pub(crate) popup: HMENU,
    pub(crate) child_pidls: Vec<*mut ITEMIDLIST>,
    pub(crate) primary_path: PathBuf,
}

/// Build menu and enumerate top-level only (Files: `EnumMenuItems(..., loadSubmenus: false)`).
pub(crate) fn prepare_and_enumerate_top_level(
    paths: &[PathBuf],
    extended_verbs: bool,
    menu_icon_extract_px: u32,
) -> anyhow::Result<Vec<ShellContextMenuEntry>> {
    shell_log!("prepare: ENTER (job body start, before any COM)");
    shell_log!(
        "prepare_and_enumerate_top_level: paths={:?} extended={} icon_px={menu_icon_extract_px}",
        paths,
        extended_verbs
    );
    shell_log!("prepare: release_hybrid_session begin");
    crate::hybrid_shell_session::release_prepared_hybrid_session();
    shell_log!("prepare: release_hybrid_session done; prepare_and_store begin");
    let result = unsafe {
        crate::hybrid_shell_session::HybridSession::prepare_and_store(
            paths,
            extended_verbs,
            menu_icon_extract_px,
        )
    };
    shell_log!(
        "prepare: enumerate done (ok={}, n={})",
        result.is_ok(),
        result.as_ref().map(|v| v.len()).unwrap_or(0)
    );
    result
}

/// Files `LoadSubMenu`: `HandleMenuMsg(WM_INITMENUPOPUP)` then enumerate that HMENU.
pub(crate) fn expand_lazy_submenu(
    handler_clsid: Option<&str>,
    parent_index: u32,
) -> anyhow::Result<Vec<ShellContextMenuEntry>> {
    unsafe {
        crate::hybrid_shell_session::HybridSession::expand_lazy_submenu(handler_clsid, parent_index)
    }
}

pub(crate) unsafe fn expand_lazy_submenu_inner(
    popup: HMENU,
    menu: &IContextMenu,
    parent_index: u32,
    primary_path: &Path,
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

    enumerate_popup_menu(submenu, menu, 1, true, primary_path, true, None)
}

pub(crate) fn invoke_prepared_menu(
    handler_clsid: Option<&str>,
    command_offset: u32,
) -> anyhow::Result<()> {
    unsafe {
        crate::hybrid_shell_session::HybridSession::invoke_prepared(handler_clsid, command_offset)
    }
}

impl ContextMenuHandle {
    pub(crate) unsafe fn release(self) {
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

pub(crate) unsafe fn create_context_menu(
    paths: &[PathBuf],
    extended_verbs: bool,
) -> anyhow::Result<ContextMenuHandle> {
    let t0 = std::time::Instant::now();
    let (parent_sf, first_child) = bind_parent_and_relative(&paths[0])?;
    let mut child_pidls = vec![first_child];

    for path in paths.iter().skip(1) {
        let (_, relative) = bind_parent_and_relative(path)?;
        child_pidls.push(relative);
    }
    shell_log!(
        "create: bound {} pidl(s) in {:.1}ms",
        child_pidls.len(),
        t0.elapsed().as_secs_f64() * 1000.0
    );

    let apidl: Vec<*const ITEMIDLIST> = child_pidls
        .iter()
        .map(|p| *p as *const ITEMIDLIST)
        .collect();

    shell_log!("create: GetUIObjectOf begin");
    let t1 = std::time::Instant::now();
    let menu: IContextMenu = parent_sf.GetUIObjectOf(HWND::default(), &apidl, None)?;
    shell_log!(
        "create: GetUIObjectOf done in {:.1}ms",
        t1.elapsed().as_secs_f64() * 1000.0
    );

    let popup = CreatePopupMenu()?;
    // Files: `CMF_NORMAL` or `CMF_EXTENDEDVERBS` only (no `CMF_EXPLORE`).
    // Additionally ask handlers to enumerate verbs asynchronously; some misbehaving
    // extensions (notably WPS "Open With") block the aggregate QueryContextMenu otherwise.
    const CMF_ASYNCVERBENUM: u32 = 0x00000400;
    let flags = if extended_verbs {
        CMF_NORMAL | CMF_EXTENDEDVERBS | CMF_ASYNCVERBENUM
    } else {
        CMF_NORMAL | CMF_ASYNCVERBENUM
    };
    shell_log!("create: QueryContextMenu begin (extended={extended_verbs})");
    let t2 = std::time::Instant::now();
    menu.QueryContextMenu(popup, 0, CMD_FIRST, CMD_LAST, flags)?;
    let _raw_count = GetMenuItemCount(popup);
    shell_log!(
        "create: QueryContextMenu done in {:.1}ms (raw_count={_raw_count})",
        t2.elapsed().as_secs_f64() * 1000.0
    );

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

pub(crate) unsafe fn enumerate_popup_menu(
    popup: HMENU,
    context_menu: &IContextMenu,
    depth: u32,
    expand_submenus: bool,
    primary_path: &Path,
    skip_known_verbs: bool,
    handler_clsid: Option<&str>,
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
    shell_log!("enumerate: depth={depth} count={count} init_popup_menu begin");

    let t_init = std::time::Instant::now();
    init_popup_menu(popup, context_menu);
    shell_log!(
        "enumerate: depth={depth} init_popup_menu done in {:.1}ms",
        t_init.elapsed().as_secs_f64() * 1000.0
    );

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
                shell_log!("enumerate: item[{index}] submenu '{label}' icon begin");
                let t_icon = std::time::Instant::now();
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
                shell_log!(
                    "enumerate: item[{index}] submenu '{label}' icon done in {:.1}ms",
                    t_icon.elapsed().as_secs_f64() * 1000.0
                );
                if let Ok(children) = enumerate_popup_menu(
                    submenu,
                    context_menu,
                    depth + 1,
                    true,
                    primary_path,
                    true,
                    handler_clsid,
                ) {
                    if !children.is_empty() {
                        entries.push(ShellContextMenuEntry::Submenu {
                            label,
                            children,
                            icon_png,
                            lazy_parent_index: None,
                            handler_clsid: handler_clsid.map(|s| s.to_string()),
                        });
                    }
                }
            } else {
                shell_log!("enumerate: item[{index}] lazy-submenu '{label}' icon begin");
                let t_icon = std::time::Instant::now();
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
                shell_log!(
                    "enumerate: item[{index}] lazy-submenu '{label}' icon done in {:.1}ms",
                    t_icon.elapsed().as_secs_f64() * 1000.0
                );
                entries.push(ShellContextMenuEntry::Submenu {
                    label,
                    children: Vec::new(),
                    icon_png,
                    lazy_parent_index: Some(index),
                    handler_clsid: handler_clsid.map(|s| s.to_string()),
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
        if skip_known_verbs && should_skip_shell_verb(verb.as_deref(), &label) {
            continue;
        }

        shell_log!(
            "enumerate: item[{index}] '{label}' verb={:?} icon begin",
            verb.as_deref()
        );
        let t_icon = std::time::Instant::now();
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
        shell_log!(
            "enumerate: item[{index}] '{label}' icon done in {:.1}ms",
            t_icon.elapsed().as_secs_f64() * 1000.0
        );

        entries.push(ShellContextMenuEntry::Item {
            label,
            command_offset,
            command_string: verb,
            icon_png,
            handler_clsid: handler_clsid.map(|s| s.to_string()),
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

/// Warms up the Shell context menu exactly like Files' `WarmUpQueryContextMenuAsync`: build the
/// full merged context menu for the system drive root once, in the background, at startup. This
/// loads and initializes every registered shell-extension DLL (and lets them establish whatever
/// agent/IPC connections they need) BEFORE the user's first right-click, instead of paying that
/// cold cost — which can deadlock a misbehaving extension — on the interaction path.
pub fn warm_up_query_context_menu() {
    warm_up_hybrid_shell_menu();
}

/// Fire-and-forget warm-up of the Hybrid Shell query path.
pub fn warm_up_hybrid_shell_menu() {
    std::thread::Builder::new()
        .name("cyber_desktop-shell-warmup".into())
        .spawn(|| {
            use std::time::Instant;
            let thread_start = Instant::now();
            tracing::info!(target: "startup", step = "shell_warmup_thread_begin");
            crate::com::log_current_apartment("warmup-thread-start");

            let _ = crate::com::ensure_com_apartment();
            crate::com::log_current_apartment("warmup-thread-after-ensure");

            crate::shell_menu_session::set_warmup_in_progress(true);

            // Serialize only the warm-up query itself — do not hold the lock across setup/teardown.
            let dir = crate::hybrid_shell_session::warmup_directory();
            let icon_px = crate::hybrid_shell_session::warmup_icon_px();

            let outcome = {
                let Some(_op_guard) = crate::shell_menu_session::try_shell_op_lock() else {
                    tracing::info!(
                        target: "startup",
                        step = "shell_warmup_thread_skipped",
                        reason = "shell menu op already in flight"
                    );
                    crate::shell_menu_session::set_warmup_in_progress(false);
                    return;
                };
                unsafe {
                    crate::hybrid_shell_session::query_hybrid_entries_for_warmup(
                        &[dir.clone()],
                        false,
                        icon_px,
                        std::time::Duration::from_secs(5),
                    )
                }
            };

            crate::shell_menu_session::set_warmup_in_progress(false);

            match &outcome {
                Ok(entries) => {
                    tracing::info!(
                        target: "startup",
                        step = "shell_warmup_thread_done",
                        ok = true,
                        entries = entries.len(),
                        block_ms = thread_start.elapsed().as_secs_f64() * 1000.0,
                        warmup_dir = %dir.display(),
                    );
                }
                Err(error) => {
                    tracing::warn!(
                        target: "startup",
                        step = "shell_warmup_thread_done",
                        ok = false,
                        error = ?error,
                        block_ms = thread_start.elapsed().as_secs_f64() * 1000.0,
                        warmup_dir = %dir.display(),
                    );
                }
            }
        })
        .ok();
}

/// Enumerates Shell entries on a dedicated STA thread (Files `ThreadWithMessageQueue`).
/// Diagnostic: run the merged QueryContextMenu on the **calling** thread.
/// Used to test whether the hang is specific to background STA threads.
pub fn create_context_menu_on_current_thread(
    paths: &[PathBuf],
    extended_verbs: bool,
) -> anyhow::Result<u32> {
    unsafe {
        let handle = create_context_menu(paths, extended_verbs)?;
        let count = GetMenuItemCount(handle.popup);
        handle.release();
        Ok(count as u32)
    }
}

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
    handler_clsid: Option<&str>,
    _extended_verbs: bool,
) -> anyhow::Result<()> {
    let offset = command_offset;
    let clsid = handler_clsid.map(|s| s.to_string());
    std::thread::spawn(move || {
        if let Err(error) = shell_menu_session::invoke_on_session(clsid, offset) {
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
    use crate::system_scale_factor;
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
                    ..
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
                    ..
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
                            .and_then(|idx| {
                                crate::shell_menu_session::load_lazy_submenu(None, idx).ok()
                            })
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
            // NOTE: we intentionally do NOT assert non-empty. On machines with a misbehaving
            // shell extension (e.g. Baidu Netdisk's YunShellExtV164.dll wedging QueryContextMenu),
            // the query correctly times out and returns an empty list instead of hanging forever.
            // Asserting non-empty here would make the suite fail on such machines. The contract we
            // verify is that the call RETURNS (no infinite hang); see hang_repro_tests for the
            // dedicated reproduction/diagnosis.
            if normal.is_empty() {
                eprintln!(
                    "  (warning) no Shell entries for {label}; a shell extension likely timed out"
                );
            }
        }

        let _ = fs::remove_file(file);
        let _ = fs::remove_file(png);
        let _ = fs::remove_dir(subdir);
    }
}

/// Diagnostics for the "shell context menu hangs in QueryContextMenu" investigation.
///
/// These run against the LIVE machine's registered shell extensions, so they are `#[ignore]`d
/// by default. Run individually (each in a fresh process) so a loader-lock deadlock from one
/// handler can't poison the next test:
///
/// ```text
/// cargo test -p app-platform-windows repro_aggregate_directory_menu -- --ignored --nocapture
/// cargo test -p app-platform-windows enumerate_handlers_with_timeout_merge -- --ignored --nocapture
/// cargo test -p app-platform-windows isolate_baidu_directory_handler -- --ignored --nocapture
/// ```
///
/// Override the target directory with `SHELL_MENU_TEST_DIR=D:\some\folder`.
#[cfg(all(windows, test))]
mod hang_repro_tests {
    use super::*;
    use crate::com::ThreadWithMessageQueue;
    use std::time::{Duration, Instant};
    use windows::core::{IUnknown, GUID};
    use windows::Win32::System::Com::{CoCreateInstance, IDataObject, CLSCTX_INPROC_SERVER};
    use windows::Win32::System::Registry::HKEY;
    use windows::Win32::UI::Shell::IShellExtInit;

    // Third-party context-menu handlers found loaded in the hung process.
    // Baidu Netdisk is registered under `Directory` (every filesystem folder).
    const BAIDU_NETDISK: GUID = GUID::from_values(
        0x6D85624F,
        0x305A,
        0x491d,
        [0x88, 0x48, 0xC1, 0x92, 0x7A, 0xA0, 0xD7, 0x90],
    );
    // Adobe CoreSync is registered under `Folder`.
    const ADOBE_CORESYNC: GUID = GUID::from_values(
        0x2A118EB5,
        0x5797,
        0x4F5E,
        [0x8B, 0x3D, 0xF4, 0xEC, 0xBA, 0x3C, 0x98, 0xE4],
    );
    // Tencent QQ (qq_shell_extension_64.dll).
    const TENCENT_QQ: GUID = GUID::from_values(
        0xBB4CB47C,
        0x6258,
        0x4502,
        [0xAC, 0x4C, 0xAF, 0xA7, 0x3A, 0xFB, 0x43, 0x19],
    );

    const HANDLER_TIMEOUT: Duration = Duration::from_secs(8);

    fn test_dir() -> PathBuf {
        std::env::var("SHELL_MENU_TEST_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::temp_dir())
    }

    /// Instantiate ONE shell-extension by CLSID and run its `QueryContextMenu` in isolation.
    ///
    /// NOTE: this is a *lower bound* probe. A handler can pass here yet still hang inside the
    /// shell's real merged menu, because the merge initializes it differently (real folder
    /// `IDataObject` + ProgID key + existing menu state) and may drive code paths this skips.
    /// `dump_aggregate_hang_stack` is the authoritative reproduction.
    unsafe fn query_single_handler(clsid: GUID, path: &Path) -> anyhow::Result<usize> {
        Ok(query_single_handler_labels(clsid, path)?.len())
    }

    /// Like [`query_single_handler`], but returns menu item labels from the handler's popup.
    unsafe fn query_single_handler_labels(clsid: GUID, path: &Path) -> anyhow::Result<Vec<String>> {
        let (parent_sf, child) = bind_parent_and_relative(path)?;
        let apidl = [child as *const ITEMIDLIST];
        let data_object: IDataObject = parent_sf.GetUIObjectOf(HWND::default(), &apidl, None)?;

        let wide = path_to_wide(path);
        let mut full_pidl: *mut ITEMIDLIST = std::ptr::null_mut();
        SHParseDisplayName(PCWSTR(wide.as_ptr()), None, &mut full_pidl, 0, None)?;

        let unknown: IUnknown = CoCreateInstance(&clsid, None, CLSCTX_INPROC_SERVER)?;
        let init: IShellExtInit = unknown.cast()?;
        init.Initialize(
            Some(full_pidl as *const ITEMIDLIST),
            &data_object,
            HKEY::default(),
        )?;

        let menu: IContextMenu = unknown.cast()?;
        let popup = CreatePopupMenu()?;
        menu.QueryContextMenu(popup, 0, CMD_FIRST, CMD_LAST, CMF_NORMAL)?;
        let labels = read_popup_item_labels(popup);

        let _ = DestroyMenu(popup);
        free_pidl(full_pidl);
        free_pidl(child);
        Ok(labels)
    }

    /// Read display strings from an HMENU without icon/submenu expansion (fast probe).
    unsafe fn read_popup_item_labels(popup: HMENU) -> Vec<String> {
        let count = GetMenuItemCount(popup).max(0);
        let mut labels = Vec::new();
        let mut info = MENUITEMINFOW {
            cbSize: std::mem::size_of::<MENUITEMINFOW>() as u32,
            fMask: MIIM_FTYPE | MIIM_STRING,
            ..Default::default()
        };
        for index in 0..count as u32 {
            let mut label_buf = [0u16; 512];
            info.dwTypeData = windows::core::PWSTR(label_buf.as_mut_ptr());
            info.cch = label_buf.len() as u32;
            if GetMenuItemInfoW(popup, index, true, &mut info).is_err() {
                continue;
            }
            if info.fType.0 & MFT_SEPARATOR.0 != 0 {
                continue;
            }
            let label_len = label_buf.iter().position(|&c| c == 0).unwrap_or(0);
            if label_len == 0 {
                continue;
            }
            let raw = String::from_utf16_lossy(&label_buf[..label_len]);
            let label = format_shell_menu_label(&raw);
            if !label.is_empty() {
                labels.push(label);
            }
        }
        labels
    }

    #[derive(Debug)]
    enum HandlerProbeOutcome {
        Ok(Vec<String>),
        Error(String),
        Hang,
        Wedged,
    }

    fn isolate_handler(clsid: GUID, label: &str) {
        let path = test_dir();
        eprintln!("[{label}] target dir: {}", path.display());
        eprintln!("[{label}] CLSID: {clsid:?}");

        let sta = ThreadWithMessageQueue::new("cyber_desktop-isolate-test");
        let target = path.clone();
        let t0 = Instant::now();
        let outcome = sta.post_with_timeout(
            move || unsafe { query_single_handler(clsid, &target) },
            HANDLER_TIMEOUT,
        );
        let elapsed = t0.elapsed();

        match outcome {
            Some(Ok(n)) => eprintln!("[{label}] OK: returned {n} menu items in {elapsed:?}"),
            Some(Err(e)) => eprintln!("[{label}] returned an error in {elapsed:?}: {e:#}"),
            None => eprintln!(
                "[{label}] *** HANG *** QueryContextMenu did not return within {HANDLER_TIMEOUT:?} \
                 -> THIS handler is the culprit"
            ),
        }
        // Leak the (possibly wedged) STA thread on purpose; the process exits after the test.
        std::mem::forget(sta);
    }

    /// Reproduce the real path: the aggregate Shell query our app runs on right-click.
    #[test]
    #[ignore = "touches live shell extensions; run explicitly"]
    fn repro_aggregate_directory_menu() {
        let path = test_dir();
        let t0 = Instant::now();
        let result = query_shell_context_menu_items(&[path.clone()], false, 24);
        let elapsed = t0.elapsed();
        eprintln!(
            "aggregate query {} -> {:?} entries in {elapsed:?}",
            path.display(),
            result.as_ref().map(|v| v.len())
        );
        // If a handler hangs, our 6s timeout makes this return Ok(empty) at ~6s rather than hang.
        assert!(result.is_ok(), "aggregate query errored: {result:?}");
    }

    /// Mirror the real app: warm up the system-drive root (Files `WarmUpQueryContextMenuAsync`),
    /// then query a folder. If warm-up works, the second query should be fast even when a cold
    /// query would have timed out.
    #[test]
    #[ignore = "touches live shell extensions; run explicitly"]
    fn repro_warmup_then_query() {
        let system_drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".to_string());
        let root = PathBuf::from(format!("{}\\", system_drive.trim_end_matches('\\')));
        let folder = test_dir();
        let icon_px = crate::shell_icon::menu_icon_pixel_size(crate::system_scale_factor());

        for round in 0..3 {
            let t0 = Instant::now();
            let warm = query_shell_context_menu_items(&[root.clone()], false, icon_px);
            let warm_ms = t0.elapsed();
            shell_menu_session::clear_session();

            let t1 = Instant::now();
            let q = query_shell_context_menu_items(&[folder.clone()], false, icon_px);
            let q_ms = t1.elapsed();
            shell_menu_session::clear_session();

            eprintln!(
                "[round {round}] warmup({}) -> {:?} in {warm_ms:?} | query({}) -> {:?} in {q_ms:?}",
                root.display(),
                warm.as_ref().map(|v| v.len()),
                folder.display(),
                q.as_ref().map(|v| v.len()),
            );
        }
    }

    /// Does the merged `QueryContextMenu` EVER return, or is it a permanent deadlock?
    /// Runs the real merged build on an owning thread with a long (90s) timeout and reports when
    /// (if ever) it returns. This decides whether warm-up can possibly help.
    #[test]
    #[ignore = "touches live shell extensions; run explicitly"]
    fn repro_long_timeout_merged_query() {
        let folder = test_dir();
        eprintln!(
            "[long] merged create_context_menu for {} (waiting up to 90s)",
            folder.display()
        );
        let thread = ThreadWithMessageQueue::new("cyber_desktop-long-test");
        let p = folder.clone();
        let t0 = Instant::now();
        let outcome = thread.post_with_timeout(
            move || unsafe {
                let r = create_context_menu(&[p], false);
                let ok = r.is_ok();
                if let Ok(h) = r {
                    h.release();
                }
                ok
            },
            Duration::from_secs(90),
        );
        let elapsed = t0.elapsed();
        match outcome {
            Some(ok) => eprintln!("[long] *** RETURNED after {elapsed:?} (ok={ok}) -> NOT a permanent deadlock; warm-up CAN help"),
            None => eprintln!("[long] still hung after {elapsed:?} -> permanent deadlock; warm-up canNOT help, needs out-of-process"),
        }
        std::mem::forget(thread);
        std::process::exit(0);
    }

    /// Hypothesis: the wedged wait is a cross-apartment/SendMessage wait that needs SOME thread in
    /// the process to be a pumping STA (like Explorer's UI thread, or .NET's CLR-pumped STA).
    /// Spin up a dedicated message-pumping STA "host" thread, then run the merged query on a worker
    /// and see if it now completes (vs the 90s permanent hang with no pumping host).
    #[test]
    #[ignore = "touches live shell extensions; run explicitly"]
    fn repro_with_pumping_sta_host() {
        use windows::Win32::System::Ole::{OleInitialize, OleUninitialize};
        use windows::Win32::UI::WindowsAndMessaging::{
            DispatchMessageW, GetMessageW, TranslateMessage, MSG,
        };

        // Pumping STA host: OleInitialize + a real Win32 message loop, running for the test.
        std::thread::Builder::new()
            .name("cyber_desktop-pump-host".into())
            .spawn(|| unsafe {
                let _ = OleInitialize(None);
                let mut msg = MSG::default();
                while GetMessageW(&mut msg, None, 0, 0).0 > 0 {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
                OleUninitialize();
            })
            .ok();
        std::thread::sleep(Duration::from_millis(200));

        let folder = test_dir();
        eprintln!(
            "[pump] merged query for {} with a pumping STA host present (up to 30s)",
            folder.display()
        );
        let thread = ThreadWithMessageQueue::new("cyber_desktop-pump-worker");
        let p = folder.clone();
        let t0 = Instant::now();
        let outcome = thread.post_with_timeout(
            move || unsafe {
                let r = create_context_menu(&[p], false);
                let ok = r.is_ok();
                if let Ok(h) = r {
                    h.release();
                }
                ok
            },
            Duration::from_secs(30),
        );
        let elapsed = t0.elapsed();
        match outcome {
            Some(ok) => eprintln!(
                "[pump] *** RETURNED after {elapsed:?} (ok={ok}) -> a pumping STA host FIXES it"
            ),
            None => {
                eprintln!("[pump] still hung after {elapsed:?} -> pumping STA host does NOT help")
            }
        }
        std::mem::forget(thread);
        std::process::exit(0);
    }

    #[test]
    #[ignore = "touches live shell extensions; run explicitly"]
    fn isolate_baidu_directory_handler() {
        isolate_handler(BAIDU_NETDISK, "baidu");
    }

    #[test]
    #[ignore = "touches live shell extensions; run explicitly"]
    fn isolate_adobe_folder_handler() {
        isolate_handler(ADOBE_CORESYNC, "adobe");
    }

    #[test]
    #[ignore = "touches live shell extensions; run explicitly"]
    fn isolate_qq_handler() {
        isolate_handler(TENCENT_QQ, "qq");
    }

    unsafe fn pe_size_of_image(base: usize) -> usize {
        // IMAGE_DOS_HEADER.e_lfanew @ +0x3C; IMAGE_OPTIONAL_HEADER64.SizeOfImage @ NT + 80.
        let e_lfanew = *((base + 0x3C) as *const u32) as usize;
        *((base + e_lfanew + 80) as *const u32) as usize
    }

    unsafe fn unicode_string_to_string(us: &windows::Win32::Foundation::UNICODE_STRING) -> String {
        let len = (us.Length / 2) as usize;
        if us.Buffer.is_null() || len == 0 {
            return String::new();
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(us.Buffer.0, len))
    }

    /// Build a (base, end, name) map of every loaded module by walking the PEB loader list.
    /// Unlike ToolHelp, this reads memory directly and does NOT take the loader lock — essential
    /// because the wedged thread holds that lock.
    unsafe fn module_ranges() -> Vec<(usize, usize, String)> {
        use windows::Win32::System::Kernel::LIST_ENTRY;
        use windows::Win32::System::Threading::PEB;
        use windows::Win32::System::WindowsProgramming::LDR_DATA_TABLE_ENTRY;

        let mut out = Vec::new();
        let peb: *const PEB;
        std::arch::asm!("mov {}, gs:[0x60]", out(reg) peb, options(nostack, preserves_flags));
        if peb.is_null() || (*peb).Ldr.is_null() {
            return out;
        }
        let ldr = (*peb).Ldr;
        let head = &(*ldr).InMemoryOrderModuleList as *const LIST_ENTRY as usize;
        let mut cur = (*ldr).InMemoryOrderModuleList.Flink as usize;
        // InMemoryOrderLinks is the 2nd field (after Reserved1: [*mut c_void; 2] = 16 bytes).
        const IN_MEMORY_ORDER_OFFSET: usize = 16;
        let mut guard = 0;
        while cur != head && cur != 0 && guard < 1024 {
            guard += 1;
            let entry = (cur - IN_MEMORY_ORDER_OFFSET) as *const LDR_DATA_TABLE_ENTRY;
            let base = (*entry).DllBase as usize;
            if base != 0 {
                let size = pe_size_of_image(base).max(0x1000);
                let full = unicode_string_to_string(&(*entry).FullDllName);
                let name = full
                    .rsplit(|c| c == '\\' || c == '/')
                    .next()
                    .unwrap_or(&full)
                    .to_string();
                out.push((base, base + size, name));
            }
            cur = (*(cur as *const LIST_ENTRY)).Flink as usize;
        }
        out
    }

    fn module_for(addr: usize, mods: &[(usize, usize, String)]) -> Option<&str> {
        mods.iter()
            .find(|(b, e, _)| addr >= *b && addr < *e)
            .map(|(_, _, n)| n.as_str())
    }

    /// Suspend a wedged thread (same process) and print the modules present on its stack,
    /// revealing which DLL is blocked inside `QueryContextMenu`.
    unsafe fn dump_thread_stack(hthread: windows::Win32::Foundation::HANDLE) {
        use std::ffi::c_void;
        use windows::Win32::System::Diagnostics::Debug::{
            GetThreadContext, ReadProcessMemory, CONTEXT, CONTEXT_FLAGS,
        };
        use windows::Win32::System::Threading::{GetCurrentProcess, ResumeThread, SuspendThread};

        let mods = module_ranges();
        let _ = SuspendThread(hthread);

        #[repr(align(16))]
        struct AlignedContext(CONTEXT);
        let mut ctx = AlignedContext(std::mem::zeroed());
        ctx.0.ContextFlags = CONTEXT_FLAGS(0x0010_000B); // CONTEXT_FULL (AMD64)
        if GetThreadContext(hthread, &mut ctx.0).is_err() {
            eprintln!("[stack] GetThreadContext failed");
            let _ = ResumeThread(hthread);
            return;
        }
        let rip = ctx.0.Rip as usize;
        let rsp = ctx.0.Rsp as usize;
        eprintln!(
            "[stack] RIP in module: {}",
            module_for(rip, &mods).unwrap_or("<unknown>")
        );

        let proc = GetCurrentProcess();
        let mut printed: Vec<String> = Vec::new();
        let mut addr = rsp;
        let end = rsp + 0x1_0000; // scan 64KB of stack
        while addr < end {
            let mut val: usize = 0;
            let mut read: usize = 0;
            let ok = ReadProcessMemory(
                proc,
                addr as *const c_void,
                &mut val as *mut usize as *mut c_void,
                std::mem::size_of::<usize>(),
                Some(&mut read),
            )
            .is_ok();
            if ok && read == std::mem::size_of::<usize>() {
                if let Some(name) = module_for(val, &mods) {
                    // Only report return addresses into real DLLs/EXEs, de-duplicated in order.
                    if !printed.last().map(|s| s == name).unwrap_or(false) {
                        eprintln!("[stack]   <- {name}");
                        printed.push(name.to_string());
                    }
                }
            }
            addr += std::mem::size_of::<usize>();
        }
        let _ = ResumeThread(hthread);

        let third_party: Vec<&String> = printed
            .iter()
            .filter(|n| {
                let l = n.to_ascii_lowercase();
                !l.starts_with("ntdll")
                    && !l.starts_with("kernel")
                    && !l.starts_with("kernelbase")
                    && !l.starts_with("combase")
                    && !l.starts_with("ole32")
                    && !l.starts_with("rpcrt4")
                    && !l.starts_with("shell32")
                    && !l.starts_with("shcore")
                    && !l.starts_with("windows.storage")
                    && !l.starts_with("user32")
                    && !l.starts_with("win32u")
                    && !l.starts_with("msvcrt")
                    && !l.starts_with("ucrtbase")
                    && !l.starts_with("app_platform")
                    && !l.starts_with("app-platform")
            })
            .collect();
        eprintln!("\n[stack] === third-party modules on the wedged stack ===");
        for n in &third_party {
            eprintln!("[stack]   {n}");
        }
    }

    /// Run the REAL merged `QueryContextMenu` on a thread we own, let it hang, then dump its
    /// stack to identify the wedged DLL. This mirrors exactly what the app does on right-click.
    #[test]
    #[ignore = "touches live shell extensions; run explicitly"]
    fn dump_aggregate_hang_stack() {
        use std::os::windows::io::AsRawHandle;
        use windows::Win32::Foundation::HANDLE;
        use windows::Win32::System::Ole::{OleInitialize, OleUninitialize};

        let path = test_dir();
        eprintln!(
            "[stack] running merged create_context_menu for: {}",
            path.display()
        );
        let p = path.clone();
        let worker = std::thread::spawn(move || unsafe {
            let _ = OleInitialize(None);
            let r = create_context_menu(&[p.clone()], false);
            eprintln!("[stack] worker returned ok={}", r.is_ok());
            if let Ok(h) = r {
                h.release();
            }
            OleUninitialize();
        });

        let hthread = HANDLE(worker.as_raw_handle());
        std::thread::sleep(Duration::from_secs(4));
        unsafe { dump_thread_stack(hthread) };
        std::thread::sleep(Duration::from_millis(300));
        // The worker is wedged holding the loader lock, so normal process teardown would hang
        // (CoUninitialize / thread join block forever). Force-exit so this never leaves a zombie.
        eprintln!("[stack] done; force-exiting (worker is permanently wedged)");
        std::process::exit(0);
    }

    /// Probe an arbitrary handler: `SHELL_MENU_TEST_CLSID={GUID}` (braces optional).
    #[test]
    #[ignore = "touches live shell extensions; run explicitly"]
    fn isolate_clsid_from_env() {
        let raw =
            std::env::var("SHELL_MENU_TEST_CLSID").expect("set SHELL_MENU_TEST_CLSID to a {GUID}");
        let clsid = GUID::from(raw.trim().trim_start_matches('{').trim_end_matches('}'));
        isolate_handler(clsid, "env");
    }

    fn reg_query_lines(key: &str, extra: &[&str]) -> Vec<String> {
        let mut args = vec!["query", key];
        args.extend_from_slice(extra);
        std::process::Command::new("reg")
            .args(&args)
            .output()
            .ok()
            .map(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .map(|l| l.to_string())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Resolve a `ContextMenuHandlers` subkey to its CLSID (subkey name, or its default value).
    fn resolve_handler_clsid(subkey_full: &str) -> Option<String> {
        let name = subkey_full.rsplit('\\').next().unwrap_or("").trim();
        if name.starts_with('{') && name.ends_with('}') {
            return Some(name.to_string());
        }
        // Named handler: default value holds the CLSID.
        for line in reg_query_lines(subkey_full, &["/ve"]) {
            if let Some(idx) = line.find("REG_SZ") {
                let val = line[idx + "REG_SZ".len()..].trim();
                if val.starts_with('{') {
                    return Some(val.to_string());
                }
            }
        }
        None
    }

    fn clsid_dll(clsid: &str) -> String {
        let key = format!("HKCR\\CLSID\\{clsid}\\InprocServer32");
        for line in reg_query_lines(&key, &["/ve"]) {
            if let Some(idx) = line.find("REG_SZ") {
                let v = line[idx + "REG_SZ".len()..].trim();
                if !v.is_empty() {
                    return v.to_string();
                }
            }
        }
        "<unknown dll>".to_string()
    }

    fn list_folder_context_handlers() -> Vec<(String, String, String)> {
        let scopes = [
            "HKCR\\Directory\\shellex\\ContextMenuHandlers",
            "HKCR\\Directory\\Background\\shellex\\ContextMenuHandlers",
            "HKCR\\Folder\\shellex\\ContextMenuHandlers",
            "HKCR\\AllFilesystemObjects\\shellex\\ContextMenuHandlers",
        ];
        let mut seen: Vec<String> = Vec::new();
        let mut out = Vec::new();
        for scope in scopes {
            for sub in reg_query_lines(scope, &[]) {
                let sub = sub.trim();
                if !sub.starts_with("HKEY_CLASSES_ROOT\\") || !sub.contains("ContextMenuHandlers\\")
                {
                    continue;
                }
                let Some(clsid) = resolve_handler_clsid(sub) else {
                    continue;
                };
                let clsid_up = clsid.to_uppercase();
                if seen.contains(&clsid_up) {
                    continue;
                }
                seen.push(clsid_up);
                let short = sub.rsplit('\\').next().unwrap_or(sub).to_string();
                let dll = clsid_dll(&clsid);
                out.push((short, clsid, dll));
            }
        }
        out
    }

    /// Probe one CLSID in a child process; returns menu labels on success.
    fn probe_handler_labels_in_child(clsid: &str, dir: &Path) -> HandlerProbeOutcome {
        use std::process::{Command, Stdio};
        let exe = match std::env::current_exe() {
            Ok(e) => e,
            Err(e) => return HandlerProbeOutcome::Error(format!("current_exe: {e}")),
        };
        let mut child = match Command::new(&exe)
            .args([
                "--ignored",
                "--nocapture",
                "--exact",
                "context_menu::hang_repro_tests::per_handler_probe_from_env",
            ])
            .env("SHELL_MENU_TEST_CLSID", clsid)
            .env("SHELL_MENU_TEST_DIR", dir)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => return HandlerProbeOutcome::Error(format!("spawn: {e}")),
        };

        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) if Instant::now() >= deadline => {
                    let _ = child.kill();
                    return HandlerProbeOutcome::Wedged;
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(50)),
                Err(e) => return HandlerProbeOutcome::Error(format!("wait: {e}")),
            }
        }
        let out = child
            .wait_with_output()
            .map(|o| o.stderr)
            .unwrap_or_default();
        let text = String::from_utf8_lossy(&out);
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("[probe] OK n=") {
                let Some((n_str, labels_part)) = rest.split_once(" labels=") else {
                    continue;
                };
                let n: usize = n_str.parse().unwrap_or(0);
                let labels: Vec<String> = if labels_part.is_empty() {
                    Vec::new()
                } else {
                    labels_part.split('\x1f').map(str::to_string).collect()
                };
                if labels.len() != n {
                    eprintln!(
                        "[warn] probe label count mismatch for {clsid}: parsed {} != {n}",
                        labels.len()
                    );
                }
                return HandlerProbeOutcome::Ok(labels);
            }
            if line.contains("[probe] HANG") {
                return HandlerProbeOutcome::Hang;
            }
            if let Some(msg) = line.strip_prefix("[probe] ERR ") {
                return HandlerProbeOutcome::Error(msg.to_string());
            }
        }
        HandlerProbeOutcome::Error("<no probe line in child stderr>".to_string())
    }

    /// Child entry point: probe one handler and print `[probe] OK|HANG|ERR` to stderr.
    #[test]
    #[ignore = "touches live shell extensions; run explicitly"]
    fn per_handler_probe_from_env() {
        let raw =
            std::env::var("SHELL_MENU_TEST_CLSID").expect("set SHELL_MENU_TEST_CLSID to a {GUID}");
        let clsid = GUID::from(raw.trim().trim_start_matches('{').trim_end_matches('}'));
        let path = test_dir();
        let sta = ThreadWithMessageQueue::new("cyber_desktop-per-handler-probe");
        let target = path.clone();
        let outcome = sta.post_with_timeout(
            move || unsafe { query_single_handler_labels(clsid, &target) },
            HANDLER_TIMEOUT,
        );
        match outcome {
            Some(Ok(labels)) => {
                let joined = labels.join("\x1f");
                eprintln!("[probe] OK n={} labels={joined}", labels.len());
            }
            Some(Err(e)) => {
                let msg = format!("{e:#}")
                    .lines()
                    .next()
                    .unwrap_or("error")
                    .to_string();
                eprintln!("[probe] ERR {msg}");
            }
            None => eprintln!("[probe] HANG"),
        }
        std::mem::forget(sta);
    }

    /// Test 2: skip CDefFolderMenu merge — probe each handler in its own child process with a
    /// timeout, collect successful labels, and compare against the merged aggregate path.
    #[test]
    #[ignore = "touches live shell extensions; run explicitly"]
    fn enumerate_handlers_with_timeout_merge() {
        let path = test_dir();
        eprintln!("=== test2: per-handler merge (no blocklist) ===");
        eprintln!("target: {}\n", path.display());

        let t_agg = Instant::now();
        let agg = query_shell_context_menu_items(&[path.clone()], false, 24);
        shell_menu_session::clear_session();
        let agg_ms = t_agg.elapsed();
        let agg_n = agg.as_ref().map(|v| v.len()).unwrap_or(0);
        eprintln!("aggregate (CDefFolderMenu): {agg_n} entries in {agg_ms:?}\n");

        let handlers = list_folder_context_handlers();
        eprintln!(
            "probing {} unique handlers (child-isolated, {HANDLER_TIMEOUT:?} each)...\n",
            handlers.len()
        );

        let t_merge = Instant::now();
        let mut ok_handlers = 0usize;
        let mut hang_handlers = 0usize;
        let mut err_handlers = 0usize;
        let mut merged_labels: Vec<String> = Vec::new();

        for (short, clsid, dll) in &handlers {
            let t0 = Instant::now();
            match probe_handler_labels_in_child(clsid, &path) {
                HandlerProbeOutcome::Ok(labels) => {
                    ok_handlers += 1;
                    eprintln!(
                        "  {short:<28} OK {elapsed:?} n={}  {dll}",
                        labels.len(),
                        elapsed = t0.elapsed()
                    );
                    for label in labels {
                        if !merged_labels.iter().any(|l| l.eq_ignore_ascii_case(&label)) {
                            merged_labels.push(label);
                        }
                    }
                }
                HandlerProbeOutcome::Hang => {
                    hang_handlers += 1;
                    eprintln!(
                        "  {short:<28} HANG (skipped) {elapsed:?}  {dll}",
                        elapsed = t0.elapsed()
                    );
                }
                HandlerProbeOutcome::Wedged => {
                    hang_handlers += 1;
                    eprintln!(
                        "  {short:<28} WEDGED/killed {elapsed:?}  {dll}",
                        elapsed = t0.elapsed()
                    );
                }
                HandlerProbeOutcome::Error(e) => {
                    err_handlers += 1;
                    eprintln!(
                        "  {short:<28} ERR {elapsed:?}: {e}  {dll}",
                        elapsed = t0.elapsed()
                    );
                }
            }
        }

        let merge_ms = t_merge.elapsed();
        eprintln!("\n=== summary ===");
        eprintln!("handlers total:    {}", handlers.len());
        eprintln!("handlers ok:       {ok_handlers}");
        eprintln!("handlers hang:     {hang_handlers}");
        eprintln!("handlers error:    {err_handlers}");
        eprintln!("aggregate entries: {agg_n} in {agg_ms:?}");
        eprintln!(
            "merged labels:     {} unique in {merge_ms:?}",
            merged_labels.len()
        );
        eprintln!("\nmerged menu (unique labels):");
        for label in &merged_labels {
            eprintln!("  - {label}");
        }

        assert!(
            merge_ms < Duration::from_secs(handlers.len() as u64 * 25 + 30),
            "per-handler merge took too long: {merge_ms:?}"
        );
        assert!(
            !merged_labels.is_empty(),
            "per-handler merge returned no labels; aggregate had {agg_n} entries"
        );
        if agg_n == 0 {
            eprintln!(
                "\n*** aggregate empty but per-handler merge got {} labels — strategy works ***",
                merged_labels.len()
            );
        }
    }

    /// Probe one CLSID in a CHILD process (re-exec of this test binary running
    /// `isolate_clsid_from_env`). Returns the child's one-line verdict. Child isolation avoids
    /// loader-lock / leftover-apartment contamination between handlers.
    fn probe_clsid_in_child(clsid: &str, dir: &Path) -> String {
        use std::process::{Command, Stdio};
        let exe = match std::env::current_exe() {
            Ok(e) => e,
            Err(e) => return format!("<no current_exe: {e}>"),
        };
        let mut child = match Command::new(&exe)
            .args([
                "--ignored",
                "--nocapture",
                "--exact",
                "context_menu::hang_repro_tests::isolate_clsid_from_env",
            ])
            .env("SHELL_MENU_TEST_CLSID", clsid)
            .env("SHELL_MENU_TEST_DIR", dir)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => return format!("<spawn failed: {e}>"),
        };

        // The child self-bounds to HANDLER_TIMEOUT (8s); give it headroom, else kill (= wedged).
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) if Instant::now() >= deadline => {
                    let _ = child.kill();
                    return "*** WEDGED (child did not exit; process poisoned) ***".to_string();
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(100)),
                Err(e) => return format!("<wait error: {e}>"),
            }
        }
        let out = child
            .wait_with_output()
            .map(|o| o.stderr)
            .unwrap_or_default();
        let text = String::from_utf8_lossy(&out);
        text.lines()
            .find(|l| {
                l.contains("[env]")
                    && (l.contains("OK") || l.contains("error") || l.contains("HANG"))
            })
            .map(|l| l.trim().to_string())
            .unwrap_or_else(|| "<no verdict line>".to_string())
    }

    /// Enumerate EVERY folder-scope context-menu handler and probe each in its own process.
    /// Whichever reports HANG is the one wedging `QueryContextMenu`.
    #[test]
    #[ignore = "touches live shell extensions; run explicitly"]
    fn isolate_all_folder_handlers() {
        let path = test_dir();
        eprintln!(
            "probing folder-scope handlers (child-process isolated) against: {}\n",
            path.display()
        );

        let scopes = [
            "HKCR\\Directory\\shellex\\ContextMenuHandlers",
            "HKCR\\Directory\\Background\\shellex\\ContextMenuHandlers",
            "HKCR\\Folder\\shellex\\ContextMenuHandlers",
            "HKCR\\AllFilesystemObjects\\shellex\\ContextMenuHandlers",
        ];

        let mut seen: Vec<String> = Vec::new();
        let mut culprits: Vec<String> = Vec::new();
        for scope in scopes {
            for sub in reg_query_lines(scope, &[]) {
                let sub = sub.trim();
                if !sub.starts_with("HKEY_CLASSES_ROOT\\") || !sub.contains("ContextMenuHandlers\\")
                {
                    continue;
                }
                let Some(clsid) = resolve_handler_clsid(sub) else {
                    continue;
                };
                let clsid_up = clsid.to_uppercase();
                if seen.contains(&clsid_up) {
                    continue;
                }
                seen.push(clsid_up.clone());

                let dll = clsid_dll(&clsid);
                let short = sub.rsplit('\\').next().unwrap_or(sub);
                let verdict = probe_clsid_in_child(&clsid, &path);
                eprintln!("  {short:<28} {clsid}  {dll}\n      -> {verdict}");
                if verdict.contains("HANG") || verdict.contains("WEDGED") {
                    culprits.push(format!("{short} {clsid} {dll}"));
                }
            }
        }
        eprintln!("\n=== culprits (hang in isolation) ===");
        if culprits.is_empty() {
            eprintln!(
                "(none — the hang is an aggregation/interaction effect, not a single handler)"
            );
        } else {
            for c in &culprits {
                eprintln!("  {c}");
            }
        }
    }
}
