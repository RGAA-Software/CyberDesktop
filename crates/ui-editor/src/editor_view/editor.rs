//! `EngineEditor` entity and constructors.

use std::ops::Range;
use std::path::PathBuf;
use std::time::SystemTime;

use cyberfiles_text_engine::{load_file, Document, SyntaxState};
use gpui::{prelude::*, px, App, Bounds, Context, Entity, FocusHandle, Pixels, Window};

use super::language::language_for_path;
use super::state::{
    FindState, GotoState, InputTarget, SearchPanelState, TabSlot, VisibleLine, WrappedVisible,
};
use super::state::read_file_meta;

/// A high-performance, engine-backed text editor surface.
pub struct EngineEditor {
    pub(crate) focus_handle: FocusHandle,
    pub(crate) document: Document,
    pub(crate) syntax: SyntaxState,
    pub(crate) parsed_revision: Option<u64>,
    pub(crate) marked_range: Option<Range<usize>>,
    pub(crate) is_selecting: bool,
    pub(crate) needs_focus: bool,
    pub(crate) input_target: InputTarget,
    pub(crate) find: Option<FindState>,
    /// Go to Line overlay.
    pub(crate) goto: Option<GotoState>,
    /// "Find in File" side panel (current tab only), when open.
    pub(crate) search_panel: Option<SearchPanelState>,
    pub(crate) show_line_numbers: bool,
    pub(crate) show_about: bool,
    /// Whether the keyboard-shortcuts reference overlay is open.
    pub(crate) show_shortcuts: bool,
    /// Active vertical scrollbar-thumb drag: `(mouse_y_at_grab, scroll_y_at_grab)`.
    pub(crate) scrollbar_drag: Option<(Pixels, Pixels)>,
    /// Active horizontal scrollbar-thumb drag: `(mouse_x_at_grab, scroll_x_at_grab)`.
    pub(crate) hscrollbar_drag: Option<(Pixels, Pixels)>,
    /// Request to scroll the caret into view on the next frame (set on edits and
    /// cursor movement; consumed in `prepaint` for horizontal reveal).
    pub(crate) reveal_caret: bool,
    // Geometry / scroll.
    pub(crate) font_size: Pixels,
    pub(crate) line_height: Pixels,
    pub(crate) gutter_width: Pixels,
    pub(crate) scroll_y: Pixels,
    pub(crate) scroll_x: Pixels,
    /// Widest shaped line seen in the last painted viewport — the horizontal
    /// scroll extent (measuring only visible lines keeps this O(viewport)).
    pub(crate) content_width: Pixels,
    /// Reserve bottom space for the horizontal scrollbar lane (sticky across frames).
    pub(crate) reserve_hscrollbar_lane: bool,
    pub(crate) last_bounds: Option<Bounds<Pixels>>,
    pub(crate) visible: Vec<VisibleLine>,
    /// Soft (word) wrap. When on, long lines wrap to the viewport width, there is
    /// no horizontal scrolling, and the viewport is anchored by document line +
    /// sub-row offset (so it stays O(viewport) even for huge files).
    pub(crate) soft_wrap: bool,
    /// Document line at the top of the viewport (wrap mode).
    pub(crate) wrap_top_line: usize,
    /// Pixels of `wrap_top_line`'s wrapped block scrolled above the viewport top.
    pub(crate) wrap_top_off: Pixels,
    /// Last document line painted in the previous frame (wrap mode), used to
    /// scroll the caret down into view without a full layout scan.
    pub(crate) wrap_bottom_line: usize,
    /// Wrapped lines retained for hit-testing (wrap mode).
    pub(crate) wrapped_visible: Vec<WrappedVisible>,
    /// Open tabs. The entry at `active` is a drained placeholder; the live tab's
    /// state is held in the fields above and swapped back on switch.
    pub(crate) tabs: Vec<TabSlot>,
    pub(crate) active: usize,
    /// `(mtime, len)` of the active document's file when last loaded/saved.
    pub(crate) file_meta: Option<(SystemTime, u64)>,
    /// The active file changed on disk underneath us.
    pub(crate) disk_changed: bool,
    /// Most-recently-used file list (newest first), shown via the Recent panel.
    pub(crate) recent: Vec<PathBuf>,
    /// Whether the Recent Files dropdown is open.
    pub(crate) show_recent: bool,
    /// Set once the background disk-watch poller has been started.
    pub(crate) watch_started: bool,
    /// A pending close awaiting the user's save/discard/cancel decision.
    pub(crate) pending_close: Option<CloseTarget>,
    /// Set once the window-should-close hook has been registered.
    pub(crate) close_hooked: bool,
    /// Set when the user confirmed closing the window despite unsaved changes.
    pub(crate) allow_window_close: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CloseTarget {
    Tab(usize),
    Window,
}


impl EngineEditor {
    pub fn new(language: &str, document: Document, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            document,
            syntax: SyntaxState::new(language),
            parsed_revision: None,
            marked_range: None,
            is_selecting: false,
            needs_focus: true,
            input_target: InputTarget::Document,
            find: None,
            goto: None,
            search_panel: None,
            show_line_numbers: true,
            show_about: false,
            show_shortcuts: false,
            scrollbar_drag: None,
            hscrollbar_drag: None,
            reveal_caret: false,
            font_size: px(14.0),
            line_height: px(20.0),
            gutter_width: px(48.0),
            scroll_y: px(0.0),
            scroll_x: px(0.0),
            content_width: px(0.0),
            reserve_hscrollbar_lane: false,
            last_bounds: None,
            visible: Vec::new(),
            soft_wrap: false,
            wrap_top_line: 0,
            wrap_top_off: px(0.0),
            wrap_bottom_line: 0,
            wrapped_visible: Vec::new(),
            tabs: vec![TabSlot::placeholder()],
            active: 0,
            file_meta: None,
            disk_changed: false,
            recent: Vec::new(),
            show_recent: false,
            pending_close: None,
            close_hooked: false,
            allow_window_close: false,
            watch_started: false,
        }
    }

    /// Convenience factory for [`open_window`]: builds an editor entity for `path`.
    pub fn view(path: Option<PathBuf>, _window: &mut Window, cx: &mut App) -> Entity<Self> {
        cx.new(|cx| Self::from_path(path, cx))
    }

    /// Loads `path` (or starts empty) and builds an editor for it.
    pub fn from_path(path: Option<PathBuf>, cx: &mut Context<Self>) -> Self {
        let language = language_for_path(path.as_deref());
        let document = match &path {
            Some(p) => match load_file(p) {
                Ok(loaded) => Document::from_loaded(loaded, Some(p.clone()), language),
                Err(_) => Document::empty(),
            },
            None => Document::empty(),
        };
        let mut editor = Self::new(language, document, cx);
        editor.document.set_caret(0);
        if let Some(p) = &path {
            editor.file_meta = read_file_meta(p);
            editor.push_recent(p.clone());
        }
        editor
    }

    pub fn document(&self) -> &Document {
        &self.document
    }

    pub fn document_mut(&mut self) -> &mut Document {
        &mut self.document
    }
}
