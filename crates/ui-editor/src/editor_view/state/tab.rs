use std::path::Path;
use std::time::SystemTime;

use cyberfiles_text_engine::{Document, SyntaxState};
use gpui::{px, Pixels};

/// Per-tab state parked while another tab is active. The currently active tab's
/// data lives in the [`EngineEditor`](super::super::EngineEditor) fields directly
/// (the slot at `active` is a drained placeholder); everything is swapped in/out on
/// tab switch.
pub(crate) struct TabSlot {
    pub(crate) document: Document,
    pub(crate) syntax: SyntaxState,
    pub(crate) parsed_revision: Option<u64>,
    pub(crate) scroll_x: Pixels,
    pub(crate) scroll_y: Pixels,
    /// Last-seen `(mtime, len)` of the on-disk file, for external-change detection.
    pub(crate) file_meta: Option<(SystemTime, u64)>,
    /// Set when the file changed on disk since we last loaded/saved it.
    pub(crate) disk_changed: bool,
}

impl TabSlot {
    /// A cheap, empty placeholder used while a tab is the active (live) one.
    pub(crate) fn placeholder() -> Self {
        Self {
            document: Document::empty(),
            syntax: SyntaxState::new("text"),
            parsed_revision: None,
            scroll_x: px(0.0),
            scroll_y: px(0.0),
            file_meta: None,
            disk_changed: false,
        }
    }
}

/// Reads `(modified_time, len)` for external-modification detection.
pub(crate) fn read_file_meta(path: &Path) -> Option<(SystemTime, u64)> {
    let meta = std::fs::metadata(path).ok()?;
    Some((meta.modified().ok()?, meta.len()))
}
