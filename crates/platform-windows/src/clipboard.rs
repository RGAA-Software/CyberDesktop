use std::path::PathBuf;

use clipboard_win::{formats::FileList, get_clipboard};

/// Reads file paths from the Windows clipboard (`CF_HDROP`), if present.
pub fn read_clipboard_file_paths() -> Vec<PathBuf> {
    let paths: Vec<PathBuf> = get_clipboard(FileList).unwrap_or_default();
    paths.into_iter().filter(|path| path.exists()).collect()
}
