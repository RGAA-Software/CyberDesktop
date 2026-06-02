use std::path::Path;

use crate::history_store::{self, HistoryKind};

const MAX_PATH_HISTORY: usize = 1024;

/// Recent paths typed in the omnibar (Files `PathHistoryList`).
pub fn path_history_list() -> Vec<String> {
    history_store::list(HistoryKind::Path, MAX_PATH_HISTORY)
}

/// Records a successfully navigated directory path (deduped, most recent first).
pub fn record_path_history(path: &Path) {
    if !path.is_dir() {
        return;
    }
    let path_string = path.to_string_lossy().to_string();
    let _ = history_store::record(HistoryKind::Path, &path_string, MAX_PATH_HISTORY);
}
