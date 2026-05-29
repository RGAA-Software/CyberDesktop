use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use cyberfiles_text_engine::FileMatches;
use gpui::{Entity, SharedString, Subscription};
use gpui_component::{input::InputState, VirtualListScrollHandle};

/// One row in the Find in File results list.
#[derive(Clone)]
pub(crate) enum SearchRow {
    Match {
        path: PathBuf,
        line: u64,
        text: SharedString,
    },
}

/// State for the Find in File side panel (current tab buffer only).
pub(crate) struct SearchPanelState {
    pub(crate) query: Entity<InputState>,
    /// Path of the tab file when the panel was last synced (for display).
    pub(crate) scope_path: PathBuf,
    pub(crate) results: Vec<FileMatches>,
    pub(crate) rows: Vec<SearchRow>,
    pub(crate) status: String,
    pub(crate) searching: bool,
    pub(crate) lines_scanned: usize,
    pub(crate) matches_so_far: usize,
    pub(crate) case_sensitive: bool,
    pub(crate) whole_word: bool,
    pub(crate) regex: bool,
    /// Monotonic id; bumped to drop stale results and stop in-flight work.
    pub(crate) generation: u64,
    /// Set while a background search runs; `store(true)` to cancel immediately.
    pub(crate) cancel: Option<Arc<AtomicBool>>,
    pub(crate) scroll: VirtualListScrollHandle,
    pub(crate) _subs: Vec<Subscription>,
}

impl SearchPanelState {
    pub(crate) fn rebuild_rows(&mut self) {
        let mut rows = Vec::new();
        for file in &self.results {
            for m in &file.matches {
                rows.push(SearchRow::Match {
                    path: file.path.clone(),
                    line: m.line_number,
                    text: SharedString::from(m.line_text.clone()),
                });
            }
        }
        self.rows = rows;
    }
}
