use std::path::PathBuf;

use cyberfiles_text_engine::FileMatches;
use gpui::{Entity, SharedString, Subscription};
use gpui_component::{input::InputState, VirtualListScrollHandle};

/// One flattened row in the "Find in Files" results list (file header or a
/// single matching line), so we can drive a [`v_virtual_list`].
#[derive(Clone)]
pub(crate) enum SearchRow {
    File { label: SharedString, count: usize },
    Match { path: PathBuf, line: u64, text: SharedString },
}

/// State for the "Find in Files" (global search) side panel.
pub(crate) struct SearchPanelState {
    pub(crate) query: Entity<InputState>,
    pub(crate) root: PathBuf,
    pub(crate) results: Vec<FileMatches>,
    /// Flattened rows for the virtual list (kept in sync with `results`).
    pub(crate) rows: Vec<SearchRow>,
    pub(crate) status: String,
    pub(crate) case_sensitive: bool,
    pub(crate) whole_word: bool,
    pub(crate) regex: bool,
    /// Monotonic id so a slow search that finishes late can't clobber a newer one
    /// (also bumped on tab switch to cancel an in-flight search).
    pub(crate) generation: u64,
    pub(crate) scroll: VirtualListScrollHandle,
    pub(crate) _subs: Vec<Subscription>,
}

impl SearchPanelState {
    /// Rebuilds the flattened virtual-list rows from `results`.
    pub(crate) fn rebuild_rows(&mut self) {
        let mut rows = Vec::new();
        for file in &self.results {
            let rel = file
                .path
                .strip_prefix(&self.root)
                .unwrap_or(&file.path)
                .display()
                .to_string();
            rows.push(SearchRow::File {
                label: SharedString::from(rel),
                count: file.matches.len(),
            });
            for m in &file.matches {
                rows.push(SearchRow::Match {
                    path: file.path.clone(),
                    line: m.line_number,
                    text: SharedString::from(m.line_text.trim_end().to_string()),
                });
            }
        }
        self.rows = rows;
    }
}
