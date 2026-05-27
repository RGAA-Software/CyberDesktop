use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;

use windows::core::{GUID, PCWSTR};
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::UI::Shell::Common::ITEMIDLIST;
use windows::Win32::UI::Shell::{
    SHGetFolderLocation, SHGetKnownFolderIDList, SHGetKnownFolderPath, SHParseDisplayName,
    CSIDL_BITBUCKET, KF_FLAG_DEFAULT,
};

/// `{645FF040-5081-101B-9F08-00AA002F954E}`
const FOLDERID_RECYCLE_BIN: GUID = GUID::from_u128(0x645FF040_5081_101B_9F08_00AA002F954E);

/// Same namespace string as Files (`Constants.UserEnvironmentPaths.RecycleBinPath`).
pub const SHELL_RECYCLE_BIN_PATH: &str = "Shell:RecycleBinFolder";

fn path_to_wide(path: &str) -> Vec<u16> {
    OsStr::new(path)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// True when `path` is the shell recycle-bin folder (or inside it).
pub fn is_recycle_bin_path(path: &std::path::Path) -> bool {
    recycle_bin_folder()
        .map(|root| path == root || path.starts_with(&root))
        .unwrap_or(false)
}

/// Returns the recycle-bin folder PIDL (caller must [`ILFree`] it).
///
/// Tries `Shell:RecycleBinFolder` first (Files/Vanara), then known-folder ID list.
pub unsafe fn recycle_bin_pidl() -> windows::core::Result<*mut ITEMIDLIST> {
    let wide = path_to_wide(SHELL_RECYCLE_BIN_PATH);
    let mut pidl: *mut ITEMIDLIST = std::ptr::null_mut();
    if SHParseDisplayName(PCWSTR(wide.as_ptr()), None, &mut pidl, 0, None).is_ok()
        && !pidl.is_null()
    {
        return Ok(pidl);
    }
    if !pidl.is_null() {
        windows::Win32::UI::Shell::ILFree(Some(pidl));
    }

    if let Ok(pidl) = SHGetKnownFolderIDList(&FOLDERID_RECYCLE_BIN, KF_FLAG_DEFAULT.0 as u32, None)
    {
        return Ok(pidl);
    }

    SHGetFolderLocation(HWND::default(), CSIDL_BITBUCKET as i32, None, 0)
}

/// Returns the shell recycle-bin folder path when available.
pub fn recycle_bin_folder() -> Option<PathBuf> {
    unsafe {
        let raw = SHGetKnownFolderPath(&FOLDERID_RECYCLE_BIN, KF_FLAG_DEFAULT, None).ok()?;
        let path = raw.to_string().ok().map(PathBuf::from);
        let _ = CoTaskMemFree(Some(raw.0.cast()));
        path
    }
}
