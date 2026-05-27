use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};

use windows::core::{w, PCWSTR};
use windows::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_INVOKEIDLIST, SHELLEXECUTEINFOW};
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, SW_SHOW};

/// Shows the system properties dialog for a file or folder.
pub fn open_item_properties(path: &Path) -> anyhow::Result<()> {
    let path_wide: Vec<u16> = OsStr::new(path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let mut info = SHELLEXECUTEINFOW {
        cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
        fMask: SEE_MASK_INVOKEIDLIST,
        hwnd: unsafe { GetForegroundWindow() },
        lpVerb: w!("properties"),
        lpFile: PCWSTR(path_wide.as_ptr()),
        nShow: SW_SHOW.0,
        ..Default::default()
    };

    unsafe {
        ShellExecuteExW(&mut info)?;
    }
    Ok(())
}

/// Opens the system properties sheet without blocking the GPUI thread.
pub fn invoke_shell_properties(paths: &[PathBuf]) -> anyhow::Result<()> {
    if paths.len() != 1 {
        anyhow::bail!("properties requires a single path");
    }
    let path = paths[0].clone();
    std::thread::spawn(move || {
        let _ = open_item_properties(&path);
    });
    Ok(())
}

pub use crate::context_menu::{
    format_shell_menu_label, invoke_shell_context_menu_item, open_in_new_explorer_window,
    query_shell_context_menu_items, show_open_with_dialog, warm_up_query_context_menu,
    ShellContextMenuEntry,
};
pub use crate::shell_menu_session::{clear_session as clear_shell_menu_session, load_lazy_submenu};

/// Optional Explorer-style popup at the cursor (not the default Files parity UX).
pub fn show_shell_context_menu(paths: &[std::path::PathBuf]) -> anyhow::Result<()> {
    crate::context_menu::show_shell_context_menu(paths)
        .or_else(|_| crate::context_menu::show_shell_context_menu_fallback(paths))
}
