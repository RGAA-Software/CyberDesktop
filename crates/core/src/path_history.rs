use std::path::Path;

use crate::{load_config, save_config};

const MAX_PATH_HISTORY: usize = 10;

/// Recent paths typed in the omnibar (Files `PathHistoryList`).
pub fn path_history_list() -> Vec<String> {
    load_config().map(|c| c.path_history).unwrap_or_default()
}

/// Records a successfully navigated directory path (deduped, most recent first).
pub fn record_path_history(path: &Path) {
    if !path.is_dir() {
        return;
    }
    let path_string = path.to_string_lossy().to_string();
    let mut config = load_config().unwrap_or_default();
    config.path_history.retain(|p| p != &path_string);
    config.path_history.insert(0, path_string);
    config.path_history.truncate(MAX_PATH_HISTORY);
    let _ = save_config(&config);
}
