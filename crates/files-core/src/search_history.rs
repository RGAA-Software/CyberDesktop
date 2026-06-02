use crate::history_store::{self, HistoryKind};

const MAX_SEARCH_HISTORY: usize = 1024;

/// Recent global-search queries from the omnibar (Files search history).
pub fn search_history_list() -> Vec<String> {
    history_store::list(HistoryKind::Search, MAX_SEARCH_HISTORY)
}

/// Records a submitted search query (deduped, most recent first).
pub fn record_search_history(query: &str) {
    let _ = history_store::record(HistoryKind::Search, query, MAX_SEARCH_HISTORY);
}
