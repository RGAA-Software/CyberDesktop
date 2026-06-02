use std::path::PathBuf;

use windows::core::GUID;
use windows::Win32::System::Com::CoTaskMemFree;
use windows::Win32::UI::Shell::{SHGetKnownFolderPath, KF_FLAG_DEFAULT};

/// `{645FF040-5081-101B-9F08-00AA002F954E}`
const FOLDERID_RECYCLE_BIN: GUID = GUID::from_u128(0x645FF040_5081_101B_9F08_00AA002F954E);

/// Same namespace string as Files (`Constants.UserEnvironmentPaths.RecycleBinPath`).
pub const SHELL_RECYCLE_BIN_PATH: &str = "Shell:RecycleBinFolder";

/// True when `path` is the shell recycle-bin folder (or inside it).
pub fn is_recycle_bin_path(path: &std::path::Path) -> bool {
    recycle_bin_folder()
        .map(|root| path == root || path.starts_with(&root))
        .unwrap_or(false)
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
