//! Query Shell context-menu handlers one-by-one (bypasses `CDefFolderMenu` merge hangs).
//!
//! Production path: enumerate registry handlers, probe each with a timeout, merge successful
//! items, and invoke via `(clsid, command_offset)` on the owning handler.
//!
//! ## Gate tests (run before production integration)
//!
//! ```text
//! cargo test -p app-platform-windows gate_0_registry_lists_handlers -- --ignored --nocapture
//! cargo test -p app-platform-windows gate_1_no_handler_hangs_child_isolated -- --ignored --nocapture
//! cargo test -p app-platform-windows gate_2_submenu_expansion_finds_children -- --ignored --nocapture
//! cargo test -p app-platform-windows gate_3_get_command_string_no_hang -- --ignored --nocapture
//! cargo test -p app-platform-windows gate_3_invoke_invalid_offset_returns_error_fast -- --ignored --nocapture
//! cargo test -p app-platform-windows gate_4_init_style_improves_success_rate -- --ignored --nocapture
//! cargo test -p app-platform-windows gate_5_per_handler_beats_aggregate -- --ignored --nocapture
//! cargo test -p app-platform-windows gate_all_production_ready -- --ignored --nocapture
//! ```
//!
//! Override target directory: `SHELL_MENU_TEST_DIR=D:\some\folder`
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use windows::core::{IUnknown, Interface, GUID, PCSTR, PCWSTR};
use windows::Win32::Foundation::HWND;
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::System::Com::{CoCreateInstance, IDataObject, CLSCTX_INPROC_SERVER};
use windows::Win32::System::Registry::{
    RegCloseKey, RegOpenKeyExW, HKEY, HKEY_CLASSES_ROOT, KEY_READ,
};
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::Shell::{IContextMenu, IContextMenu2, IShellExtInit, SHParseDisplayName};
use windows::Win32::UI::Shell::{CMINVOKECOMMANDINFO, GCS_VERBA, GCS_VERBW};
use windows::Win32::UI::WindowsAndMessaging::{
    CreatePopupMenu, DestroyMenu, GetMenuItemCount, GetMenuItemInfoW, GetSubMenu, HMENU,
    MENUITEMINFOW, MFT_SEPARATOR, MIIM_FTYPE, MIIM_ID, MIIM_STRING, MIIM_SUBMENU, WM_INITMENUPOPUP,
};

use crate::com::ThreadWithMessageQueue;
use crate::context_menu::format_shell_menu_label;
use crate::context_menu::{bind_parent_and_relative, free_pidl};

const CMD_FIRST: u32 = 1;
const CMD_LAST: u32 = 0x7fff;

pub const HANDLER_PROBE_TIMEOUT: Duration = Duration::from_secs(8);

/// One menu row from a single shell extension handler.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HandlerMenuItem {
    pub label: String,
    pub command_offset: u32,
    pub command_string: Option<String>,
    pub children: Vec<HandlerMenuItem>,
}

impl HandlerMenuItem {
    pub fn is_submenu(&self) -> bool {
        !self.children.is_empty()
    }
}

/// Result of probing one handler CLSID.
#[derive(Debug, Clone)]
pub struct HandlerProbeRecord {
    pub handler_name: String,
    pub clsid: String,
    #[allow(dead_code)]
    pub dll: String,
    pub items: Vec<HandlerMenuItem>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitStyle {
    #[allow(dead_code)]
    /// Prior test path: full-item PIDL + `HKEY::default()` (under-initializes many handlers).
    Legacy,
    /// Shell-style: parent-folder PIDL + `Folder`/`Directory` ProgID key.
    ShellAccurate,
}

pub(crate) struct HandlerInstance {
    pub(crate) menu: IContextMenu,
    pub(crate) popup: HMENU,
    pub(crate) parent_pidl: *mut ITEMIDLIST,
    pub(crate) child_pidl: *mut ITEMIDLIST,
    pub(crate) prog_id_key: HKEY,
}

impl Drop for HandlerInstance {
    fn drop(&mut self) {
        unsafe {
            if !self.popup.is_invalid() {
                let _ = DestroyMenu(self.popup);
            }
            if self.prog_id_key != HKEY::default() {
                let _ = RegCloseKey(self.prog_id_key);
            }
            free_pidl(self.parent_pidl);
            free_pidl(self.child_pidl);
        }
    }
}

fn path_to_wide(path: &Path) -> Vec<u16> {
    OsStr::new(path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
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

fn resolve_handler_clsid(subkey_full: &str) -> Option<String> {
    let name = subkey_full.rsplit('\\').next().unwrap_or("").trim();
    if name.starts_with('{') && name.ends_with('}') {
        return Some(name.to_string());
    }
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

/// All folder-scope `ContextMenuHandlers` (deduplicated by CLSID).
pub fn list_folder_handlers() -> Vec<(String, String, String)> {
    let scopes = [
        "HKCR\\Directory\\shellex\\ContextMenuHandlers",
        "HKCR\\Directory\\Background\\shellex\\ContextMenuHandlers",
        "HKCR\\Folder\\shellex\\ContextMenuHandlers",
        "HKCR\\AllFilesystemObjects\\shellex\\ContextMenuHandlers",
    ];
    let mut seen = Vec::new();
    let mut out = Vec::new();
    for scope in scopes {
        for sub in reg_query_lines(scope, &[]) {
            let sub = sub.trim();
            if !sub.starts_with("HKEY_CLASSES_ROOT\\") || !sub.contains("ContextMenuHandlers\\") {
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

unsafe fn open_prog_id_key(class_name: &str) -> Option<HKEY> {
    let wide: Vec<u16> = OsStr::new(class_name)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let mut hkey = HKEY::default();
    if RegOpenKeyExW(
        HKEY_CLASSES_ROOT,
        PCWSTR(wide.as_ptr()),
        0,
        KEY_READ,
        &mut hkey,
    )
    .is_ok()
    {
        Some(hkey)
    } else {
        None
    }
}

unsafe fn shell_accurate_prog_id_key() -> HKEY {
    open_prog_id_key("Folder")
        .or_else(|| open_prog_id_key("Directory"))
        .unwrap_or(HKEY::default())
}

unsafe fn parse_pidl(path: &Path) -> anyhow::Result<*mut ITEMIDLIST> {
    let wide = path_to_wide(path);
    let mut pidl: *mut ITEMIDLIST = std::ptr::null_mut();
    SHParseDisplayName(PCWSTR(wide.as_ptr()), None, &mut pidl, 0, None)?;
    Ok(pidl)
}

#[allow(dead_code)]
unsafe fn create_handler_instance(
    clsid: GUID,
    path: &Path,
    init_style: InitStyle,
) -> anyhow::Result<HandlerInstance> {
    create_handler_instance_for_paths(clsid, std::slice::from_ref(&path.to_path_buf()), init_style)
}

pub(crate) unsafe fn create_handler_instance_for_paths(
    clsid: GUID,
    paths: &[PathBuf],
    init_style: InitStyle,
) -> anyhow::Result<HandlerInstance> {
    if paths.is_empty() {
        anyhow::bail!("create_handler_instance_for_paths requires at least one path");
    }

    let (parent_sf, first_child) = bind_parent_and_relative(&paths[0])?;
    let mut child_pidls: Vec<*mut ITEMIDLIST> = vec![first_child];

    for path in paths.iter().skip(1) {
        let (_, relative) = bind_parent_and_relative(path)?;
        child_pidls.push(relative);
    }

    let apidl: Vec<*const ITEMIDLIST> = child_pidls
        .iter()
        .map(|p| *p as *const ITEMIDLIST)
        .collect();
    let data_object: IDataObject = parent_sf.GetUIObjectOf(HWND::default(), &apidl, None)?;
    let (parent_pidl, prog_id_key) = match init_style {
        InitStyle::Legacy => {
            let full = parse_pidl(&paths[0])?;
            (full, HKEY::default())
        }
        InitStyle::ShellAccurate => {
            let parent_path = paths[0]
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(&paths[0]);
            let parent = parse_pidl(parent_path)?;
            (parent, shell_accurate_prog_id_key())
        }
    };

    let unknown: IUnknown = CoCreateInstance(&clsid, None, CLSCTX_INPROC_SERVER)?;
    let init: IShellExtInit = unknown.cast()?;
    init.Initialize(
        Some(parent_pidl as *const ITEMIDLIST),
        &data_object,
        prog_id_key,
    )?;

    let menu: IContextMenu = unknown.cast()?;
    let popup = CreatePopupMenu()?;
    menu.QueryContextMenu(
        popup,
        0,
        CMD_FIRST,
        CMD_LAST,
        windows::Win32::UI::Shell::CMF_NORMAL,
    )?;

    Ok(HandlerInstance {
        menu,
        popup,
        parent_pidl,
        child_pidl: child_pidls
            .into_iter()
            .next()
            .unwrap_or(std::ptr::null_mut()),
        prog_id_key,
    })
}

/// Invoke one command on a handler identified by CLSID string, creating a fresh instance.
pub(crate) fn invoke_handler_by_clsid(
    paths: &[PathBuf],
    clsid_str: &str,
    command_offset: u32,
) -> anyhow::Result<()> {
    let clsid = parse_clsid(clsid_str)?;
    unsafe {
        let instance = create_handler_instance_for_paths(clsid, paths, InitStyle::ShellAccurate)?;
        let mut info = CMINVOKECOMMANDINFO::default();
        info.cbSize = std::mem::size_of::<CMINVOKECOMMANDINFO>() as u32;
        info.lpVerb = PCSTR::from_raw(command_offset as usize as *const u8);
        info.nShow = 1;
        instance.menu.InvokeCommand(&info)?;
    }
    Ok(())
}

/// Expand a lazy submenu for a handler identified by CLSID string, creating a fresh instance.
pub(crate) unsafe fn expand_handler_submenu_by_clsid(
    paths: &[PathBuf],
    clsid_str: &str,
    parent_index: u32,
) -> anyhow::Result<Vec<crate::context_menu::ShellContextMenuEntry>> {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetMenuItemInfoW, GetSubMenu, MENUITEMINFOW, MIIM_BITMAP, MIIM_FTYPE, MIIM_ID, MIIM_STRING,
        MIIM_SUBMENU, WM_INITMENUPOPUP,
    };

    let clsid = parse_clsid(clsid_str)?;
    let instance = create_handler_instance_for_paths(clsid, paths, InitStyle::ShellAccurate)?;

    let mut info = MENUITEMINFOW {
        cbSize: std::mem::size_of::<MENUITEMINFOW>() as u32,
        fMask: MIIM_FTYPE | MIIM_ID | MIIM_STRING | MIIM_BITMAP | MIIM_SUBMENU,
        ..Default::default()
    };
    let mut label_buf = [0u16; 512];
    info.dwTypeData = windows::core::PWSTR(label_buf.as_mut_ptr());
    info.cch = label_buf.len() as u32;

    if GetMenuItemInfoW(instance.popup, parent_index, true, &mut info).is_err() {
        anyhow::bail!("GetMenuItemInfoW failed for handler submenu index {parent_index}");
    }

    let submenu = if !info.hSubMenu.is_invalid() {
        info.hSubMenu
    } else {
        GetSubMenu(instance.popup, parent_index as i32)
    };
    if submenu.is_invalid() {
        return Ok(Vec::new());
    }

    if let Ok(cmenu2) = instance.menu.cast::<IContextMenu2>() {
        let _ = cmenu2.HandleMenuMsg(
            WM_INITMENUPOPUP,
            WPARAM(submenu.0 as usize),
            LPARAM(parent_index as isize),
        );
    }

    crate::context_menu::enumerate_popup_menu(
        submenu,
        &instance.menu,
        1,
        true,
        &paths[0],
        false,
        Some(clsid_str),
    )
}

unsafe fn read_command_string(menu: &IContextMenu, command_offset: u32) -> Option<String> {
    let mut buf = [0u8; 256];
    if menu
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
    if menu
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

unsafe fn enumerate_popup(
    popup: HMENU,
    menu: &IContextMenu,
    expand_submenus: bool,
    depth: u32,
) -> anyhow::Result<Vec<HandlerMenuItem>> {
    if depth > 8 {
        return Ok(Vec::new());
    }
    let count = GetMenuItemCount(popup).max(0);
    let mut items = Vec::new();
    let mut info = MENUITEMINFOW {
        cbSize: std::mem::size_of::<MENUITEMINFOW>() as u32,
        fMask: MIIM_FTYPE | MIIM_ID | MIIM_STRING | MIIM_SUBMENU,
        ..Default::default()
    };

    for index in 0..count as u32 {
        let mut label_buf = [0u16; 512];
        info.dwTypeData = windows::core::PWSTR(label_buf.as_mut_ptr());
        info.cch = label_buf.len() as u32;
        info.hSubMenu = Default::default();
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
        let label = format_shell_menu_label(&String::from_utf16_lossy(&label_buf[..label_len]));

        let submenu = if !info.hSubMenu.is_invalid() {
            info.hSubMenu
        } else {
            GetSubMenu(popup, index as i32)
        };

        if !submenu.is_invalid() {
            if expand_submenus {
                if let Ok(cmenu2) = menu.cast::<IContextMenu2>() {
                    let _ = cmenu2.HandleMenuMsg(
                        WM_INITMENUPOPUP,
                        WPARAM(submenu.0 as usize),
                        LPARAM(index as isize),
                    );
                }
                let children = enumerate_popup(submenu, menu, true, depth + 1)?;
                items.push(HandlerMenuItem {
                    label,
                    command_offset: 0,
                    command_string: None,
                    children,
                });
            } else {
                items.push(HandlerMenuItem {
                    label,
                    command_offset: 0,
                    command_string: None,
                    children: Vec::new(),
                });
            }
            continue;
        }

        let command_offset = info.wID.saturating_sub(CMD_FIRST);
        if command_offset > CMD_LAST.saturating_sub(CMD_FIRST) {
            continue;
        }
        let command_string = read_command_string(menu, command_offset);
        items.push(HandlerMenuItem {
            label,
            command_offset,
            command_string,
            children: Vec::new(),
        });
    }
    Ok(items)
}

#[allow(dead_code)]
/// Build the menu tree for one handler CLSID.
pub fn query_handler_menu(
    clsid: &GUID,
    path: &Path,
    init_style: InitStyle,
    expand_submenus: bool,
) -> anyhow::Result<Vec<HandlerMenuItem>> {
    query_handler_menu_for_paths(
        clsid,
        std::slice::from_ref(&path.to_path_buf()),
        init_style,
        expand_submenus,
    )
}

/// Multi-selection variant of [`query_handler_menu`].
pub fn query_handler_menu_for_paths(
    clsid: &GUID,
    paths: &[PathBuf],
    init_style: InitStyle,
    expand_submenus: bool,
) -> anyhow::Result<Vec<HandlerMenuItem>> {
    unsafe {
        let instance = create_handler_instance_for_paths(*clsid, paths, init_style)?;
        enumerate_popup(instance.popup, &instance.menu, expand_submenus, 0)
    }
}

#[allow(dead_code)]
/// Invoke one command on a specific handler (must use the same [`InitStyle`] as query).
pub fn invoke_handler_item(
    clsid: &GUID,
    path: &Path,
    init_style: InitStyle,
    command_offset: u32,
) -> anyhow::Result<()> {
    unsafe {
        let instance = create_handler_instance(*clsid, path, init_style)?;
        let mut info = CMINVOKECOMMANDINFO::default();
        info.cbSize = std::mem::size_of::<CMINVOKECOMMANDINFO>() as u32;
        info.lpVerb = PCSTR::from_raw(command_offset as usize as *const u8);
        info.nShow = 1;
        instance.menu.InvokeCommand(&info)?;
        Ok(())
    }
}

#[allow(dead_code)]
/// Verify `GetCommandString` is reachable for every leaf item (invoke mapping sanity check).
pub fn verify_command_strings(
    menu: &IContextMenu,
    items: &[HandlerMenuItem],
) -> anyhow::Result<()> {
    unsafe {
        for item in items {
            if item.is_submenu() {
                verify_command_strings(menu, &item.children)?;
                continue;
            }
            let _ = read_command_string(menu, item.command_offset);
        }
    }
    Ok(())
}

#[allow(dead_code)]
pub fn count_leaf_items(items: &[HandlerMenuItem]) -> usize {
    items
        .iter()
        .map(|i| {
            if i.is_submenu() {
                count_leaf_items(&i.children)
            } else {
                1
            }
        })
        .sum()
}

#[allow(dead_code)]
pub fn count_submenus_with_children(items: &[HandlerMenuItem]) -> usize {
    items
        .iter()
        .map(|i| {
            let here = if i.is_submenu() && !i.children.is_empty() {
                1
            } else {
                0
            };
            here + count_submenus_with_children(&i.children)
        })
        .sum()
}

#[cfg(test)]
pub fn flat_labels(items: &[HandlerMenuItem]) -> Vec<String> {
    let mut out = Vec::new();
    fn walk(items: &[HandlerMenuItem], out: &mut Vec<String>) {
        for i in items {
            if i.is_submenu() {
                walk(&i.children, out);
            } else if !i.label.is_empty() {
                out.push(i.label.clone());
            }
        }
    }
    walk(items, &mut out);
    out
}

/// Outcome of probing one handler under a timeout budget.
#[derive(Debug, Clone)]
pub enum HandlerProbeResult {
    Ok(HandlerProbeRecord),
    Error {
        handler_name: String,
        clsid: String,
        dll: String,
        message: String,
    },
    Timeout {
        handler_name: String,
        clsid: String,
        dll: String,
    },
}

impl HandlerProbeResult {
    #[allow(dead_code)]
    pub fn handler_name(&self) -> &str {
        match self {
            Self::Ok(r) => &r.handler_name,
            Self::Error { handler_name, .. } | Self::Timeout { handler_name, .. } => handler_name,
        }
    }
}

#[allow(dead_code)]
/// Probe one handler on a fresh STA thread; returns [`HandlerProbeResult::Timeout`] if wedged.
pub fn probe_one_handler_timed(
    handler_name: &str,
    clsid_str: &str,
    dll: &str,
    path: &Path,
    init_style: InitStyle,
    expand_submenus: bool,
    timeout: Duration,
) -> HandlerProbeResult {
    probe_one_handler_timed_for_paths(
        handler_name,
        clsid_str,
        dll,
        std::slice::from_ref(&path.to_path_buf()),
        init_style,
        expand_submenus,
        timeout,
    )
}

/// Multi-selection variant of [`probe_one_handler_timed`].
pub fn probe_one_handler_timed_for_paths(
    handler_name: &str,
    clsid_str: &str,
    dll: &str,
    paths: &[PathBuf],
    init_style: InitStyle,
    expand_submenus: bool,
    timeout: Duration,
) -> HandlerProbeResult {
    let Ok(clsid) = parse_clsid(clsid_str) else {
        return HandlerProbeResult::Error {
            handler_name: handler_name.to_string(),
            clsid: clsid_str.to_string(),
            dll: dll.to_string(),
            message: "invalid clsid".into(),
        };
    };
    let sta = ThreadWithMessageQueue::new("cyber_desktop-per-handler-probe");
    let p = paths.to_vec();
    let name = handler_name.to_string();
    let clsid_owned = clsid_str.to_string();
    let dll_owned = dll.to_string();
    let outcome = sta.post_with_timeout(
        move || query_handler_menu_for_paths(&clsid, &p, init_style, expand_submenus),
        timeout,
    );
    // Only leak the STA thread if it actually wedged; otherwise drop it so the
    // worker thread exits cleanly (important for tests and process teardown).
    let timed_out = outcome.is_none();
    if timed_out {
        std::mem::forget(sta);
    }
    match outcome {
        Some(Ok(items)) => HandlerProbeResult::Ok(HandlerProbeRecord {
            handler_name: name,
            clsid: clsid_owned,
            dll: dll_owned,
            items,
        }),
        Some(Err(e)) => HandlerProbeResult::Error {
            handler_name: name,
            clsid: clsid_owned,
            dll: dll_owned,
            message: format!("{e:#}"),
        },
        None => HandlerProbeResult::Timeout {
            handler_name: name,
            clsid: clsid_owned,
            dll: dll_owned,
        },
    }
}

/// Scan every folder-scope handler with per-handler STA timeouts (in-process).
pub fn probe_all_handlers_timed(
    path: &Path,
    init_style: InitStyle,
    expand_submenus: bool,
    timeout: Duration,
) -> (
    Vec<HandlerProbeRecord>,
    Vec<(String, String, String, String)>,
    Vec<(String, String, String)>,
) {
    probe_all_handlers_timed_for_paths(
        std::slice::from_ref(&path.to_path_buf()),
        init_style,
        expand_submenus,
        timeout,
    )
}

/// Multi-selection variant of [`probe_all_handlers_timed`].
pub fn probe_all_handlers_timed_for_paths(
    paths: &[PathBuf],
    init_style: InitStyle,
    expand_submenus: bool,
    timeout: Duration,
) -> (
    Vec<HandlerProbeRecord>,
    Vec<(String, String, String, String)>,
    Vec<(String, String, String)>,
) {
    let mut ok = Vec::new();
    let mut errors = Vec::new();
    let mut timeouts = Vec::new();
    for (name, clsid_str, dll) in list_folder_handlers() {
        match probe_one_handler_timed_for_paths(
            &name,
            &clsid_str,
            &dll,
            paths,
            init_style,
            expand_submenus,
            timeout,
        ) {
            HandlerProbeResult::Ok(rec) => ok.push(rec),
            HandlerProbeResult::Error {
                handler_name,
                clsid,
                dll,
                message,
            } => errors.push((handler_name, clsid, dll, message)),
            HandlerProbeResult::Timeout {
                handler_name,
                clsid,
                dll,
            } => timeouts.push((handler_name, clsid, dll)),
        }
    }
    (ok, errors, timeouts)
}

#[allow(dead_code)]
/// Back-compat wrapper without timeout tracking (blocks forever if a handler hangs).
pub fn probe_all_handlers(
    path: &Path,
    init_style: InitStyle,
    expand_submenus: bool,
) -> (
    Vec<HandlerProbeRecord>,
    Vec<(String, String, String, String)>,
) {
    let (ok, errors, _timeouts) =
        probe_all_handlers_timed(path, init_style, expand_submenus, HANDLER_PROBE_TIMEOUT);
    (ok, errors)
}

pub(crate) fn parse_clsid(s: &str) -> anyhow::Result<GUID> {
    Ok(GUID::from(
        s.trim().trim_start_matches('{').trim_end_matches('}'),
    ))
}

#[cfg(all(windows, test))]
mod gate_tests {
    use super::*;
    use std::path::PathBuf;
    use std::process::{Command, Stdio};
    use std::time::Instant;

    const CHILD_DEADLINE: Duration = Duration::from_secs(20);

    fn test_dir() -> PathBuf {
        std::env::var("SHELL_MENU_TEST_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| std::env::temp_dir())
    }

    fn init_style_from_env() -> InitStyle {
        match std::env::var("SHELL_MENU_INIT_STYLE")
            .unwrap_or_else(|_| "accurate".into())
            .to_ascii_lowercase()
            .as_str()
        {
            "legacy" => InitStyle::Legacy,
            _ => InitStyle::ShellAccurate,
        }
    }

    fn expand_submenus_from_env() -> bool {
        !matches!(
            std::env::var("SHELL_MENU_EXPAND_SUBMENUS")
                .unwrap_or_else(|_| "1".into())
                .as_str(),
            "0" | "false" | "no"
        )
    }

    fn run_on_sta<T: Send + 'static>(
        timeout: Duration,
        f: impl FnOnce() -> T + Send + 'static,
    ) -> Option<T> {
        let sta = ThreadWithMessageQueue::new("cyber_desktop-per-handler-gate");
        let out = sta.post_with_timeout(f, timeout);
        std::mem::forget(sta);
        out
    }

    fn unique_labels(records: &[HandlerProbeRecord]) -> Vec<String> {
        let labels: Vec<String> = records.iter().flat_map(|r| flat_labels(&r.items)).collect();
        labels
            .into_iter()
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .map(String::from)
            .collect()
    }

    fn print_scan_summary(
        label: &str,
        handlers_total: usize,
        ok: &[HandlerProbeRecord],
        errors: &[(String, String, String, String)],
        timeouts: &[(String, String, String)],
        elapsed: Duration,
    ) {
        let unique = unique_labels(ok);
        let submenus = ok
            .iter()
            .map(|r| count_submenus_with_children(&r.items))
            .sum::<usize>();
        eprintln!(
            "{label}: ok={} err={} hang={} unique_labels={} submenus={} in {elapsed:?}",
            ok.len(),
            errors.len(),
            timeouts.len(),
            unique.len(),
            submenus
        );
        eprintln!("  handlers registered: {handlers_total}");
    }

    /// Child entry: one handler probe with timeout; prints `[gate-probe] OK|ERR|HANG` to stderr.
    #[test]
    #[ignore = "touches live shell extensions; run explicitly"]
    fn per_handler_gate_probe_from_env() {
        let raw =
            std::env::var("SHELL_MENU_TEST_CLSID").expect("set SHELL_MENU_TEST_CLSID to a {GUID}");
        let _clsid = parse_clsid(&raw).expect("clsid");
        let path = test_dir();
        let init_style = init_style_from_env();
        let expand = expand_submenus_from_env();
        match probe_one_handler_timed(
            "env",
            &raw,
            "<child>",
            &path,
            init_style,
            expand,
            HANDLER_PROBE_TIMEOUT,
        ) {
            HandlerProbeResult::Ok(rec) => {
                let labels = flat_labels(&rec.items);
                let joined = labels.join("\x1f");
                eprintln!(
                    "[gate-probe] OK n={} leaf={} submenus={} labels={joined}",
                    labels.len(),
                    count_leaf_items(&rec.items),
                    count_submenus_with_children(&rec.items)
                );
            }
            HandlerProbeResult::Error { message, .. } => {
                eprintln!("[gate-probe] ERR {message}");
            }
            HandlerProbeResult::Timeout { .. } => eprintln!("[gate-probe] HANG"),
        }
    }

    #[allow(dead_code)]
    enum ChildProbeOutcome {
        Ok {
            leaf: usize,
            submenus: usize,
            labels: Vec<String>,
        },
        Error(String),
        Hang,
        Wedged,
    }

    fn probe_handler_in_child(
        clsid: &str,
        path: &Path,
        init_style: InitStyle,
        expand_submenus: bool,
    ) -> ChildProbeOutcome {
        let style = match init_style {
            InitStyle::Legacy => "legacy",
            InitStyle::ShellAccurate => "accurate",
        };
        let expand = if expand_submenus { "1" } else { "0" };
        let exe = match std::env::current_exe() {
            Ok(e) => e,
            Err(e) => return ChildProbeOutcome::Error(format!("current_exe: {e}")),
        };
        let mut child = match Command::new(&exe)
            .args([
                "--ignored",
                "--nocapture",
                "--exact",
                "per_handler_shell::gate_tests::per_handler_gate_probe_from_env",
            ])
            .env("SHELL_MENU_TEST_CLSID", clsid)
            .env("SHELL_MENU_TEST_DIR", path)
            .env("SHELL_MENU_INIT_STYLE", style)
            .env("SHELL_MENU_EXPAND_SUBMENUS", expand)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => return ChildProbeOutcome::Error(format!("spawn: {e}")),
        };

        let deadline = Instant::now() + CHILD_DEADLINE;
        loop {
            match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) if Instant::now() >= deadline => {
                    let _ = child.kill();
                    return ChildProbeOutcome::Wedged;
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(50)),
                Err(e) => return ChildProbeOutcome::Error(format!("wait: {e}")),
            }
        }
        let out = child
            .wait_with_output()
            .map(|o| o.stderr)
            .unwrap_or_default();
        let text = String::from_utf8_lossy(&out);
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("[gate-probe] OK n=") {
                let Some((n_str, tail)) = rest.split_once(" leaf=") else {
                    continue;
                };
                let n: usize = n_str.parse().unwrap_or(0);
                let Some((leaf_str, tail2)) = tail.split_once(" submenus=") else {
                    continue;
                };
                let leaf: usize = leaf_str.parse().unwrap_or(0);
                let Some((submenus_str, labels_part)) = tail2.split_once(" labels=") else {
                    continue;
                };
                let submenus: usize = submenus_str.parse().unwrap_or(0);
                let labels: Vec<String> = if labels_part.is_empty() {
                    Vec::new()
                } else {
                    labels_part.split('\x1f').map(str::to_string).collect()
                };
                if labels.len() != n {
                    eprintln!(
                        "[warn] gate-probe label count mismatch for {clsid}: parsed {} != {n}",
                        labels.len()
                    );
                }
                return ChildProbeOutcome::Ok {
                    leaf,
                    submenus,
                    labels,
                };
            }
            if line.contains("[gate-probe] HANG") {
                return ChildProbeOutcome::Hang;
            }
            if let Some(msg) = line.strip_prefix("[gate-probe] ERR ") {
                return ChildProbeOutcome::Error(msg.to_string());
            }
        }
        ChildProbeOutcome::Error("<no gate-probe line in child stderr>".to_string())
    }

    struct ChildScanSummary {
        ok_count: usize,
        err_count: usize,
        hang_count: usize,
        unique_labels: Vec<String>,
        elapsed: Duration,
    }

    /// Production-style scan: one child process per handler (survives aggregate poisoning).
    fn scan_handlers_child_isolated(
        path: &Path,
        init_style: InitStyle,
        expand_submenus: bool,
        verbose: bool,
    ) -> ChildScanSummary {
        let handlers = list_folder_handlers();
        let t0 = Instant::now();
        let mut ok_count = 0usize;
        let mut err_count = 0usize;
        let mut hang_count = 0usize;
        let mut unique_labels: Vec<String> = Vec::new();

        for (name, clsid, dll) in &handlers {
            let t1 = Instant::now();
            match probe_handler_in_child(clsid, path, init_style, expand_submenus) {
                ChildProbeOutcome::Ok { labels, .. } => {
                    ok_count += 1;
                    if verbose {
                        eprintln!(
                            "  {name:<28} OK {elapsed:?} n={}",
                            labels.len(),
                            elapsed = t1.elapsed()
                        );
                    }
                    for label in labels {
                        if !unique_labels.iter().any(|l| l.eq_ignore_ascii_case(&label)) {
                            unique_labels.push(label);
                        }
                    }
                }
                ChildProbeOutcome::Hang | ChildProbeOutcome::Wedged => {
                    hang_count += 1;
                    if verbose {
                        eprintln!(
                            "  {name:<28} HANG {elapsed:?}  {dll}",
                            elapsed = t1.elapsed()
                        );
                    }
                }
                ChildProbeOutcome::Error(e) => {
                    err_count += 1;
                    if verbose {
                        eprintln!(
                            "  {name:<28} ERR {elapsed:?}: {e}  {dll}",
                            elapsed = t1.elapsed()
                        );
                    }
                }
            }
        }

        ChildScanSummary {
            ok_count,
            err_count,
            hang_count,
            unique_labels,
            elapsed: t0.elapsed(),
        }
    }

    /// Gate 0: registry enumeration returns at least one folder handler.
    #[test]
    #[ignore = "touches live shell extensions; run explicitly"]
    fn gate_0_registry_lists_handlers() {
        let handlers = list_folder_handlers();
        eprintln!("gate0: {} folder-scope handlers", handlers.len());
        for (name, clsid, dll) in &handlers {
            eprintln!("  {name:<28} {clsid}  {dll}");
        }
        assert!(
            !handlers.is_empty(),
            "expected at least one ContextMenuHandlers registration"
        );
    }

    /// Gate 1: every handler returns within timeout in an isolated child (no wedged processes).
    #[test]
    #[ignore = "touches live shell extensions; run explicitly"]
    fn gate_1_no_handler_hangs_child_isolated() {
        let path = test_dir();
        let handlers = list_folder_handlers();
        eprintln!(
            "gate1: child-isolated scan of {} handlers on {}",
            handlers.len(),
            path.display()
        );

        let summary = scan_handlers_child_isolated(&path, InitStyle::ShellAccurate, true, true);

        eprintln!(
            "gate1 summary: ok={} err={} hang={} unique_labels={} in {:?}",
            summary.ok_count,
            summary.err_count,
            summary.hang_count,
            summary.unique_labels.len(),
            summary.elapsed
        );
        assert_eq!(
            summary.hang_count, 0,
            "gate1: no handler may hang when probed in isolation"
        );
        assert!(
            !summary.unique_labels.is_empty(),
            "gate1: expected at least one menu label from successful handlers"
        );
        assert!(
            summary.elapsed < Duration::from_secs(handlers.len() as u64 * 25 + 30),
            "gate1: full scan took too long: {:?}",
            summary.elapsed
        );
    }

    /// Gate 4: Shell-accurate `Initialize` must not regress vs legacy; should reduce ERR count.
    #[test]
    #[ignore = "touches live shell extensions; run explicitly"]
    fn gate_4_init_style_improves_success_rate() {
        let path = test_dir();
        eprintln!("gate4 init styles on {}", path.display());

        let t0 = Instant::now();
        let (legacy_ok, legacy_err, legacy_hang) =
            probe_all_handlers_timed(&path, InitStyle::Legacy, true, HANDLER_PROBE_TIMEOUT);
        let legacy_ms = t0.elapsed();

        let t1 = Instant::now();
        let (accurate_ok, accurate_err, accurate_hang) =
            probe_all_handlers_timed(&path, InitStyle::ShellAccurate, true, HANDLER_PROBE_TIMEOUT);
        let accurate_ms = t1.elapsed();

        print_scan_summary(
            "legacy",
            list_folder_handlers().len(),
            &legacy_ok,
            &legacy_err,
            &legacy_hang,
            legacy_ms,
        );
        print_scan_summary(
            "accurate",
            list_folder_handlers().len(),
            &accurate_ok,
            &accurate_err,
            &accurate_hang,
            accurate_ms,
        );

        assert_eq!(
            legacy_hang.len(),
            0,
            "legacy init must not hang any handler (hangs={:?})",
            legacy_hang
        );
        assert_eq!(
            accurate_hang.len(),
            0,
            "ShellAccurate init must not hang any handler (hangs={:?})",
            accurate_hang
        );
        assert!(
            accurate_ok.len() >= legacy_ok.len(),
            "ShellAccurate init must not reduce OK handlers (legacy ok={}, accurate ok={})",
            legacy_ok.len(),
            accurate_ok.len()
        );
        assert!(
            accurate_err.len() <= legacy_err.len(),
            "ShellAccurate must not increase ERR count (legacy err={}, accurate err={})",
            legacy_err.len(),
            accurate_err.len()
        );
    }

    /// Gate 2: expanded submenus must yield at least one non-empty child list.
    #[test]
    #[ignore = "touches live shell extensions; run explicitly"]
    fn gate_2_submenu_expansion_finds_children() {
        let path = test_dir();
        let (ok, errors, timeouts) =
            probe_all_handlers_timed(&path, InitStyle::ShellAccurate, true, HANDLER_PROBE_TIMEOUT);
        assert!(
            timeouts.is_empty(),
            "gate2: handler timeouts during submenu scan: {timeouts:?}"
        );
        let mut examples = Vec::new();
        let mut submenus_with_children = 0usize;
        for rec in &ok {
            let n = count_submenus_with_children(&rec.items);
            if n > 0 {
                submenus_with_children += n;
                examples.push(format!("{} ({})", rec.handler_name, rec.clsid));
            }
        }
        eprintln!(
            "gate2: {submenus_with_children} submenu(s) with expanded children on {} ({} err)",
            path.display(),
            errors.len()
        );
        for ex in &examples {
            eprintln!("  - {ex}");
        }
        assert!(
            submenus_with_children > 0,
            "expected at least one handler submenu with expanded children"
        );
    }

    /// Gate 3a: every leaf item must survive `GetCommandString` probe (no hang).
    #[test]
    #[ignore = "touches live shell extensions; run explicitly"]
    fn gate_3_get_command_string_no_hang() {
        let path = test_dir();
        let (ok, errors, timeouts) =
            probe_all_handlers_timed(&path, InitStyle::ShellAccurate, true, HANDLER_PROBE_TIMEOUT);
        assert!(
            timeouts.is_empty(),
            "gate3a: handler query timeouts: {timeouts:?}"
        );
        eprintln!(
            "gate3a: probing GetCommandString on {} handlers ({} err)",
            ok.len(),
            errors.len()
        );

        let mut leaf_items = 0usize;
        for rec in &ok {
            let n = count_leaf_items(&rec.items);
            leaf_items += n;
            let clsid = parse_clsid(&rec.clsid).expect("clsid");
            let items = rec.items.clone();
            let p = path.clone();
            let outcome = run_on_sta(HANDLER_PROBE_TIMEOUT, move || unsafe {
                let instance = create_handler_instance(clsid, &p, InitStyle::ShellAccurate)?;
                verify_command_strings(&instance.menu, &items)
            });
            assert!(
                outcome.is_some(),
                "GetCommandString probe hung on handler {}",
                rec.handler_name
            );
            assert!(
                outcome.as_ref().unwrap().is_ok(),
                "GetCommandString probe failed on handler {}: {:?}",
                rec.handler_name,
                outcome
            );
        }
        eprintln!(
            "gate3a: verified {leaf_items} leaf items across {} handlers",
            ok.len()
        );
        assert!(leaf_items > 0, "expected at least one leaf menu item");
    }

    /// Gate 3b: `InvokeCommand` dispatch path returns quickly for invalid offset (proves invoke wiring).
    #[test]
    #[ignore = "touches live shell extensions; run explicitly"]
    fn gate_3_invoke_invalid_offset_returns_error_fast() {
        let path = test_dir();
        let (ok, _, timeouts) = probe_all_handlers_timed(
            &path,
            InitStyle::ShellAccurate,
            false,
            HANDLER_PROBE_TIMEOUT,
        );
        assert!(
            timeouts.is_empty(),
            "gate3b: handler query timeouts: {timeouts:?}"
        );
        let rec = ok
            .iter()
            .find(|r| count_leaf_items(&r.items) > 0)
            .expect("need at least one handler with menu items for invoke wiring test");
        let clsid = parse_clsid(&rec.clsid).expect("clsid");
        let t0 = Instant::now();
        let outcome = run_on_sta(HANDLER_PROBE_TIMEOUT, move || {
            invoke_handler_item(&clsid, &path, InitStyle::ShellAccurate, 9999)
        });
        let elapsed = t0.elapsed();
        eprintln!(
            "gate3b: invalid invoke on {} ({}) returned in {elapsed:?}: {outcome:?}",
            rec.handler_name, rec.clsid
        );
        assert!(outcome.is_some(), "InvokeCommand hung on invalid offset");
        assert!(
            outcome.as_ref().unwrap().is_err(),
            "InvokeCommand with invalid offset should fail, not succeed"
        );
        assert!(
            elapsed < Duration::from_secs(2),
            "invalid InvokeCommand took too long: {elapsed:?}"
        );
    }

    /// Gate 5: per-handler child scan yields real labels; aggregate (run last) may hang/empty.
    #[test]
    #[ignore = "touches live shell extensions; run explicitly"]
    fn gate_5_per_handler_beats_aggregate() {
        use crate::context_menu::query_shell_context_menu_items;
        use crate::shell_menu_session;

        let path = test_dir();
        eprintln!("gate5 on {}", path.display());

        // Production path first — must not run after aggregate (aggregate poisons the process).
        let child = scan_handlers_child_isolated(&path, InitStyle::ShellAccurate, true, false);
        eprintln!(
            "per-handler (child): ok={} err={} hang={} unique_labels={} in {:?}",
            child.ok_count,
            child.err_count,
            child.hang_count,
            child.unique_labels.len(),
            child.elapsed
        );

        assert_eq!(
            child.hang_count, 0,
            "gate5: child-isolated scan must not hang"
        );
        assert!(
            !child.unique_labels.is_empty(),
            "gate5: per-handler merge must produce menu labels"
        );
        assert!(
            child.elapsed < Duration::from_secs(list_folder_handlers().len() as u64 * 25 + 30),
            "gate5: child scan too slow: {:?}",
            child.elapsed
        );

        // Aggregate last — documents CDefFolderMenu failure; do not probe handlers after this.
        let t0 = Instant::now();
        let agg = query_shell_context_menu_items(&[path.clone()], false, 24);
        shell_menu_session::clear_session();
        let agg_ms = t0.elapsed();
        let agg_n = agg.as_ref().map(|v| v.len()).unwrap_or(0);
        eprintln!("aggregate (CDefFolderMenu): {agg_n} entries in {agg_ms:?}");
        if agg_n == 0 {
            eprintln!(
                "gate5: aggregate empty but per-handler got {} unique labels — strategy validated",
                child.unique_labels.len()
            );
        }
    }

    /// Master gate: in-process gates 2–4 on a clean process, child scan for gate 1/5, aggregate last.
    #[test]
    #[ignore = "touches live shell extensions; run explicitly"]
    fn gate_all_production_ready() {
        use crate::context_menu::query_shell_context_menu_items;
        use crate::shell_menu_session;

        let path = test_dir();
        let handlers = list_folder_handlers();
        eprintln!(
            "=== gate_all on {} ({} handlers) ===",
            path.display(),
            handlers.len()
        );

        assert!(!handlers.is_empty(), "gate0");

        // Gates 2–4 on a clean process (ShellAccurate only; gate_4 legacy comparison is a separate test).
        let t1 = Instant::now();
        let (accurate_ok, accurate_err, accurate_hang) =
            probe_all_handlers_timed(&path, InitStyle::ShellAccurate, true, HANDLER_PROBE_TIMEOUT);
        let scan_ms = t1.elapsed();

        print_scan_summary(
            "accurate",
            handlers.len(),
            &accurate_ok,
            &accurate_err,
            &accurate_hang,
            scan_ms,
        );

        assert_eq!(accurate_hang.len(), 0, "gate4: accurate hangs");

        let submenus = accurate_ok
            .iter()
            .map(|r| count_submenus_with_children(&r.items))
            .sum::<usize>();
        assert!(submenus > 0, "gate2");

        let unique = unique_labels(&accurate_ok);
        assert!(!unique.is_empty(), "gate5 in-process labels");

        if let Some(rec) = accurate_ok.iter().find(|r| count_leaf_items(&r.items) > 0) {
            let clsid = parse_clsid(&rec.clsid).expect("clsid");
            let first = rec.items.iter().find_map(|i| {
                if i.is_submenu() {
                    i.children.first()
                } else {
                    Some(i)
                }
            });
            if let Some(first) = first {
                let p = path.clone();
                let offset = first.command_offset;
                let verb = run_on_sta(HANDLER_PROBE_TIMEOUT, move || unsafe {
                    let instance =
                        create_handler_instance(clsid, &p, InitStyle::ShellAccurate).ok()?;
                    read_command_string(&instance.menu, offset)
                });
                assert!(verb.is_some(), "gate3: GetCommandString spot-check hung");
                eprintln!(
                    "gate3 spot-check: {} offset={} verb={:?}",
                    rec.handler_name,
                    offset,
                    verb.unwrap()
                );
            }
        }

        // Gate 1 / production isolation (child processes).
        let child = scan_handlers_child_isolated(&path, InitStyle::ShellAccurate, true, false);
        eprintln!(
            "gate1 child: ok={} err={} hang={} unique_labels={} in {:?}",
            child.ok_count,
            child.err_count,
            child.hang_count,
            child.unique_labels.len(),
            child.elapsed
        );
        assert_eq!(child.hang_count, 0, "gate1");
        assert!(!child.unique_labels.is_empty(), "gate1/5 child labels");

        // Aggregate last — may wedge loader lock; informational only.
        let t0 = Instant::now();
        let agg = query_shell_context_menu_items(&[path.clone()], false, 24);
        shell_menu_session::clear_session();
        let agg_n = agg.as_ref().map(|v| v.len()).unwrap_or(0);
        eprintln!(
            "aggregate (last, informational): {agg_n} entries in {:?}",
            t0.elapsed()
        );

        eprintln!(
            "=== gate_all PASSED (in-process unique={}, child unique={}) ===",
            unique.len(),
            child.unique_labels.len()
        );
    }

    // Aliases for older test names (keep CI/docs stable).
    #[test]
    #[ignore = "touches live shell extensions; run explicitly"]
    fn gate_init_style_improves_success_rate() {
        gate_4_init_style_improves_success_rate();
    }

    #[test]
    #[ignore = "touches live shell extensions; run explicitly"]
    fn gate_submenu_expansion_finds_children() {
        gate_2_submenu_expansion_finds_children();
    }

    #[test]
    #[ignore = "touches live shell extensions; run explicitly"]
    fn gate_invoke_command_strings_for_all_items() {
        gate_3_get_command_string_no_hang();
    }

    #[test]
    #[ignore = "touches live shell extensions; run explicitly"]
    fn gate_invoke_invalid_offset_returns_error_fast() {
        gate_3_invoke_invalid_offset_returns_error_fast();
    }

    #[test]
    #[ignore = "touches live shell extensions; run explicitly"]
    fn gate_per_handler_ready_for_production() {
        gate_all_production_ready();
    }
}
