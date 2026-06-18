use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use windows::core::implement;
use windows::Win32::Foundation::{
    DRAGDROP_S_CANCEL, DRAGDROP_S_DROP, DRAGDROP_S_USEDEFAULTCURSORS, HWND, POINT, RECT, S_OK,
};
use windows::Win32::System::Com::{CoTaskMemFree, IDataObject};
use windows::Win32::System::Ole::{
    DoDragDrop, IDropSource, IDropSource_Impl, OleInitialize, OleUninitialize, DROPEFFECT,
    DROPEFFECT_COPY, DROPEFFECT_MOVE, DROPEFFECT_NONE,
};
use windows::Win32::System::SystemServices::MK_LBUTTON;
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::Shell::{
    ILClone, ILCreateFromPathW, ILFree, IShellFolder, SHBindToParent, SHCreateDataObject,
    SHGetDesktopFolder, SHGetIDListFromObject, SHParseDisplayName,
};
use windows::Win32::UI::WindowsAndMessaging::{GetAncestor, GetCursorPos, GetWindowRect, GA_ROOT};

use crate::com::ensure_com_apartment;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragEffect {
    None,
    Copy,
    Move,
}

fn path_to_wide(path: &Path) -> Vec<u16> {
    OsStr::new(path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

fn sanitize_paths(paths: &[PathBuf]) -> Vec<PathBuf> {
    paths.iter().filter(|p| p.exists()).cloned().collect()
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

struct DragPidls {
    child: Vec<*mut ITEMIDLIST>,
    desktop: Option<*mut ITEMIDLIST>,
}

impl Drop for DragPidls {
    fn drop(&mut self) {
        unsafe {
            for pidl in &self.child {
                if !pidl.is_null() {
                    ILFree(Some(*pidl));
                }
            }
            if let Some(desktop) = self.desktop.filter(|p| !p.is_null()) {
                CoTaskMemFree(Some(desktop as *const _));
            }
        }
    }
}

unsafe fn bind_parent_and_relative(path: &Path) -> anyhow::Result<(IShellFolder, *mut ITEMIDLIST)> {
    let wide = path_to_wide(path);
    let mut full_pidl: *mut ITEMIDLIST = std::ptr::null_mut();
    SHParseDisplayName(
        windows::core::PCWSTR(wide.as_ptr()),
        None,
        &mut full_pidl,
        0,
        None,
    )?;

    let mut relative: *mut ITEMIDLIST = std::ptr::null_mut();
    let parent: IShellFolder = SHBindToParent(full_pidl, Some(&mut relative))?;
    let relative_owned = ILClone(relative);
    ILFree(Some(full_pidl));
    if relative_owned.is_null() {
        anyhow::bail!("ILClone failed for {}", path.display());
    }
    Ok((parent, relative_owned))
}

/// Shell drag source data object — same approach as Files `GetChildrenUIObjects<IDataObject>`.
unsafe fn build_shell_data_object(paths: &[PathBuf]) -> anyhow::Result<(IDataObject, DragPidls)> {
    if paths.is_empty() {
        anyhow::bail!("no valid filesystem items for drag out");
    }

    if same_parent(paths) {
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
        let data: IDataObject = parent_sf.GetUIObjectOf(HWND::default(), &apidl, None)?;
        return Ok((
            data,
            DragPidls {
                child: child_pidls,
                desktop: None,
            },
        ));
    }

    // Mixed-parent selection (ShelfPane-style): desktop PIDL + absolute child PIDLs.
    let desktop: IShellFolder = SHGetDesktopFolder()?;
    let desktop_pidl = SHGetIDListFromObject(&desktop)?;
    let mut child_pidls = Vec::new();
    for path in paths {
        let wide = path_to_wide(path);
        let pidl = ILCreateFromPathW(windows::core::PCWSTR(wide.as_ptr()));
        if pidl.is_null() {
            continue;
        }
        child_pidls.push(pidl);
    }
    if child_pidls.is_empty() {
        anyhow::bail!("no valid filesystem items for drag out");
    }
    let apidl: Vec<*const ITEMIDLIST> = child_pidls
        .iter()
        .map(|p| *p as *const ITEMIDLIST)
        .collect();
    let data: IDataObject =
        SHCreateDataObject(Some(desktop_pidl), Some(&apidl), None::<&IDataObject>)?;
    Ok((
        data,
        DragPidls {
            child: child_pidls,
            desktop: Some(desktop_pidl),
        },
    ))
}

#[implement(IDropSource)]
struct ShellDropSource;

impl IDropSource_Impl for ShellDropSource_Impl {
    fn QueryContinueDrag(
        &self,
        fescapepressed: windows::Win32::Foundation::BOOL,
        grfkeystate: windows::Win32::System::SystemServices::MODIFIERKEYS_FLAGS,
    ) -> windows::core::HRESULT {
        if fescapepressed.as_bool() {
            return DRAGDROP_S_CANCEL;
        }
        if (grfkeystate.0 & MK_LBUTTON.0) == 0 {
            return DRAGDROP_S_DROP;
        }
        S_OK
    }

    fn GiveFeedback(&self, _dweffect: DROPEFFECT) -> windows::core::HRESULT {
        DRAGDROP_S_USEDEFAULTCURSORS
    }
}

/// Returns true when the cursor is outside the given window (uses root HWND for bounds).
pub fn is_cursor_outside_window(host_hwnd: isize) -> bool {
    if host_hwnd == 0 {
        return false;
    }
    unsafe { !cursor_in_window(root_hwnd(HWND(host_hwnd as _))) }
}

/// Top-level HWND for leave tracking (child HWND from GPUI is normalized).
pub(crate) fn root_window_hwnd(host_hwnd: isize) -> isize {
    if host_hwnd == 0 {
        return 0;
    }
    unsafe { root_hwnd(HWND(host_hwnd as _)).0 as isize }
}

unsafe fn root_hwnd(hwnd: HWND) -> HWND {
    if hwnd.0.is_null() {
        return hwnd;
    }
    let root = GetAncestor(hwnd, GA_ROOT);
    if root.0.is_null() {
        hwnd
    } else {
        root
    }
}

unsafe fn cursor_in_window(hwnd: HWND) -> bool {
    let mut screen = POINT::default();
    if GetCursorPos(&mut screen).is_err() {
        return false;
    }
    let mut window_rect = RECT::default();
    if GetWindowRect(hwnd, &mut window_rect).is_err() {
        return false;
    }
    screen.x >= window_rect.left
        && screen.x < window_rect.right
        && screen.y >= window_rect.top
        && screen.y < window_rect.bottom
}

fn map_drop_effect(effect: DROPEFFECT) -> DragEffect {
    if (effect.0 & DROPEFFECT_MOVE.0) != 0 {
        DragEffect::Move
    } else if (effect.0 & DROPEFFECT_COPY.0) != 0 {
        DragEffect::Copy
    } else {
        DragEffect::None
    }
}

/// Starts native Explorer drag-out (`DoDragDrop`) for selected filesystem paths.
pub fn begin_drag_out(
    paths: &[PathBuf],
    allow_move: bool,
    _host_hwnd: Option<isize>,
) -> anyhow::Result<DragEffect> {
    let list = sanitize_paths(paths);
    if list.is_empty() {
        return Ok(DragEffect::None);
    }

    ensure_com_apartment()?;

    unsafe {
        let should_uninit = match OleInitialize(None) {
            Ok(()) => true,
            Err(e) if e.code() == windows::Win32::Foundation::RPC_E_CHANGED_MODE => false,
            Err(e) => anyhow::bail!("OleInitialize failed: {e}"),
        };

        let (data, _pidls) = build_shell_data_object(&list)?;
        let source: IDropSource = ShellDropSource.into();
        let allowed = if allow_move {
            DROPEFFECT_COPY | DROPEFFECT_MOVE
        } else {
            DROPEFFECT_COPY
        };
        let mut effect = DROPEFFECT_NONE;
        let drag_hr = DoDragDrop(&data, &source, allowed, &mut effect);

        if should_uninit {
            OleUninitialize();
        }

        if drag_hr == DRAGDROP_S_CANCEL {
            return Ok(DragEffect::None);
        }
        if drag_hr != DRAGDROP_S_DROP && !drag_hr.is_ok() {
            anyhow::bail!("DoDragDrop failed: {drag_hr:?}");
        }

        Ok(map_drop_effect(effect))
    }
}
