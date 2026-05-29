//! Engine-backed editor view.
//!
//! The text surface is a custom GPUI [`Element`] (`EditorCanvas`). On every
//! frame it:
//! - computes the visible line range from the current scroll offset (only the
//!   visible lines are shaped/painted — virtualization for huge files),
//! - shapes each visible line with syntax-colored [`TextRun`]s,
//! - paints selection + caret quads,
//! - registers an [`ElementInputHandler`] so the OS routes typed text and IME
//!   composition through [`EntityInputHandler`],
//! - and stores its bounds + shaped lines back on the entity so mouse handlers
//!   can hit-test precisely.
//!
//! All edits funnel through the engine [`Document`].

use std::ops::Range;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use cyberfiles_text_engine::{
    load_file, search_directory, Cursor, Document, FileMatches, GlobalSearchOptions, HighlightKind,
    Match, Position, SearchOptions, Searcher, SelectionSet, SyntaxState, TextBuffer,
};
use cyberfiles_ui::{
    editor_menu_bar, set_view_toggles, AboutEditor, EditorCopy, EditorCut, EditorPaste, EditorRedo,
    EditorUndo, ExitEditor, FindInFiles, FindNext, FindPrevious, FindText, GoToLine,
    IndentSelection, KeyboardShortcuts, NewFile, OpenFile, OutdentSelection, ReplaceAllText,
    ReplaceText, SaveFile, SaveFileAs, SelectAll, TitleBar, ToggleComment, ToggleLineNumbers,
    ToggleSoftWrap,
};
use gpui::{
    div, fill, point, prelude::*, px, relative, rgb, size, App, Bounds, ClickEvent, ClipboardItem,
    Context, Element, ElementId, ElementInputHandler, Entity, EntityInputHandler, FocusHandle,
    Focusable, Font, GlobalElementId, Hsla, KeyDownEvent, LayoutId, MouseButton, MouseDownEvent,
    MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point, ScrollWheelEvent, ShapedLine,
    SharedString, Size, Stateful, Style, Subscription, TextRun, UTF16Selection, Window, WrappedLine,
};
use std::rc::Rc;
use gpui_component::{
    button::{Button, ButtonVariants as _},
    input::{Input, InputEvent, InputState},
    scroll::{ScrollableElement as _, ScrollbarAxis},
    v_virtual_list, Selectable as _, Sizable as _, VirtualListScrollHandle,
};

/// Maps a file extension to a language id understood by the engine's highlighter.
pub fn language_for_path(path: Option<&Path>) -> &'static str {
    let Some(ext) = path
        .and_then(|p| p.extension())
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
    else {
        return "text";
    };
    match ext.as_str() {
        "rs" => "rust",
        "js" | "cjs" | "mjs" | "jsx" => "javascript",
        "ts" | "tsx" => "typescript",
        "py" => "python",
        "json" => "json",
        "c" | "h" => "c",
        "cc" | "cpp" | "cxx" | "hpp" => "cpp",
        "sh" | "bash" => "bash",
        _ => "text",
    }
}

/// Where typed text currently goes. The Find / Find-in-Files panels use real
/// gpui-component inputs, so only the document and the lightweight Go to Line
/// overlay route text through the editor itself.
#[derive(Clone, Copy, PartialEq, Eq)]
enum InputTarget {
    Document,
    GotoLine,
}

/// State for the Find / Replace bar. The query/replace fields are real
/// gpui-component text inputs; searching happens only on Enter or button press.
struct FindState {
    query: Entity<InputState>,
    replace: Entity<InputState>,
    replace_mode: bool,
    case_sensitive: bool,
    whole_word: bool,
    regex: bool,
    status: String,
    _subs: Vec<Subscription>,
}

impl FindState {
    fn options(&self) -> SearchOptions {
        SearchOptions {
            case_sensitive: self.case_sensitive,
            whole_word: self.whole_word,
            regex: self.regex,
        }
    }
}

/// One flattened row in the "Find in Files" results list (file header or a
/// single matching line), so we can drive a [`v_virtual_list`].
#[derive(Clone)]
enum SearchRow {
    File { label: SharedString, count: usize },
    Match { path: PathBuf, line: u64, text: SharedString },
}

/// State for the "Find in Files" (global search) side panel.
struct SearchPanelState {
    query: Entity<InputState>,
    root: PathBuf,
    results: Vec<FileMatches>,
    /// Flattened rows for the virtual list (kept in sync with `results`).
    rows: Vec<SearchRow>,
    status: String,
    case_sensitive: bool,
    whole_word: bool,
    regex: bool,
    /// Monotonic id so a slow search that finishes late can't clobber a newer one
    /// (also bumped on tab switch to cancel an in-flight search).
    generation: u64,
    scroll: VirtualListScrollHandle,
    _subs: Vec<Subscription>,
}

impl SearchPanelState {
    /// Rebuilds the flattened virtual-list rows from `results`.
    fn rebuild_rows(&mut self) {
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

/// Geometry of the vertical scrollbar for the current frame.
struct ScrollbarMetrics {
    viewport: Pixels,
    thumb_top: Pixels,
    thumb_h: Pixels,
    max_scroll: Pixels,
}

/// Geometry of the horizontal scrollbar for the current frame.
struct HScrollbarMetrics {
    /// Track length (the scrollable gutter-to-edge span).
    track: Pixels,
    thumb_left: Pixels,
    thumb_w: Pixels,
    max_scroll: Pixels,
}

/// A shaped, currently-visible line retained for hit-testing.
struct VisibleLine {
    line: usize,
    start_char: usize,
    top: Pixels,
    shaped: ShapedLine,
}

/// A wrapped, currently-visible logical line retained for hit-testing (wrap mode).
struct WrappedVisible {
    line: usize,
    start_char: usize,
    top: Pixels,
    wrapped: WrappedLine,
}

/// Number of visual rows a [`WrappedLine`] occupies.
fn wrap_rows(wrapped: &WrappedLine) -> usize {
    wrapped.wrap_boundaries().len() + 1
}

/// Per-tab state parked while another tab is active. The currently active tab's
/// data lives in the [`EngineEditor`] fields directly (the slot at `active` is a
/// drained placeholder); everything is swapped in/out on tab switch.
struct TabSlot {
    document: Document,
    syntax: SyntaxState,
    parsed_revision: Option<u64>,
    scroll_x: Pixels,
    scroll_y: Pixels,
    /// Last-seen `(mtime, len)` of the on-disk file, for external-change detection.
    file_meta: Option<(SystemTime, u64)>,
    /// Set when the file changed on disk since we last loaded/saved it.
    disk_changed: bool,
}

impl TabSlot {
    /// A cheap, empty placeholder used while a tab is the active (live) one.
    fn placeholder() -> Self {
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
fn read_file_meta(path: &Path) -> Option<(SystemTime, u64)> {
    let meta = std::fs::metadata(path).ok()?;
    Some((meta.modified().ok()?, meta.len()))
}

/// A high-performance, engine-backed text editor surface.
pub struct EngineEditor {
    focus_handle: FocusHandle,
    document: Document,
    syntax: SyntaxState,
    parsed_revision: Option<u64>,
    marked_range: Option<Range<usize>>,
    is_selecting: bool,
    needs_focus: bool,
    input_target: InputTarget,
    find: Option<FindState>,
    /// Pending "Go to Line" input buffer (digits typed so far).
    goto: Option<String>,
    /// "Find in Files" side panel, when open.
    search_panel: Option<SearchPanelState>,
    show_line_numbers: bool,
    show_about: bool,
    /// Whether the keyboard-shortcuts reference overlay is open.
    show_shortcuts: bool,
    /// Active vertical scrollbar-thumb drag: `(mouse_y_at_grab, scroll_y_at_grab)`.
    scrollbar_drag: Option<(Pixels, Pixels)>,
    /// Active horizontal scrollbar-thumb drag: `(mouse_x_at_grab, scroll_x_at_grab)`.
    hscrollbar_drag: Option<(Pixels, Pixels)>,
    /// Request to scroll the caret into view on the next frame (set on edits and
    /// cursor movement; consumed in `prepaint` for horizontal reveal).
    reveal_caret: bool,
    // Geometry / scroll.
    font_size: Pixels,
    line_height: Pixels,
    gutter_width: Pixels,
    scroll_y: Pixels,
    scroll_x: Pixels,
    /// Widest shaped line seen in the last painted viewport — the horizontal
    /// scroll extent (measuring only visible lines keeps this O(viewport)).
    content_width: Pixels,
    last_bounds: Option<Bounds<Pixels>>,
    visible: Vec<VisibleLine>,
    /// Soft (word) wrap. When on, long lines wrap to the viewport width, there is
    /// no horizontal scrolling, and the viewport is anchored by document line +
    /// sub-row offset (so it stays O(viewport) even for huge files).
    soft_wrap: bool,
    /// Document line at the top of the viewport (wrap mode).
    wrap_top_line: usize,
    /// Pixels of `wrap_top_line`'s wrapped block scrolled above the viewport top.
    wrap_top_off: Pixels,
    /// Last document line painted in the previous frame (wrap mode), used to
    /// scroll the caret down into view without a full layout scan.
    wrap_bottom_line: usize,
    /// Wrapped lines retained for hit-testing (wrap mode).
    wrapped_visible: Vec<WrappedVisible>,
    /// Open tabs. The entry at `active` is a drained placeholder; the live tab's
    /// state is held in the fields above and swapped back on switch.
    tabs: Vec<TabSlot>,
    active: usize,
    /// `(mtime, len)` of the active document's file when last loaded/saved.
    file_meta: Option<(SystemTime, u64)>,
    /// The active file changed on disk underneath us.
    disk_changed: bool,
    /// Most-recently-used file list (newest first), shown via the Recent panel.
    recent: Vec<PathBuf>,
    /// Whether the Recent Files dropdown is open.
    show_recent: bool,
    /// Set once the background disk-watch poller has been started.
    watch_started: bool,
    /// A pending close awaiting the user's save/discard/cancel decision.
    pending_close: Option<CloseTarget>,
    /// Set once the window-should-close hook has been registered.
    close_hooked: bool,
    /// Set when the user confirmed closing the window despite unsaved changes.
    allow_window_close: bool,
}

/// What a confirmation overlay is gating: closing one tab, or the whole window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CloseTarget {
    Tab(usize),
    Window,
}

impl EngineEditor {
    /// Creates an editor for `document`, highlighting as `language`.
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

    fn refresh_syntax(&mut self) {
        let rev = self.document.revision();
        if self.parsed_revision == Some(rev) {
            return;
        }
        // Incremental on edits (feed byte-range edits to the old tree); full
        // parse only when we have no prior tree (load / language switch).
        if self.parsed_revision.is_some() {
            let edits = self.document.take_syntax_edits();
            for edit in &edits {
                self.syntax.edit(edit);
            }
        } else {
            self.document.take_syntax_edits();
        }
        self.syntax.reparse(self.document.buffer().rope());
        self.parsed_revision = Some(rev);
    }

    fn changed(&mut self, cx: &mut Context<Self>) {
        self.ensure_caret_visible();
        cx.notify();
    }

    fn max_scroll(&self) -> Pixels {
        let Some(b) = self.last_bounds else {
            return px(0.0);
        };
        let total = self.line_height * self.document.buffer().line_count() as f32;
        (total - b.size.height).max(px(0.0))
    }

    fn ensure_caret_visible(&mut self) {
        // Horizontal reveal needs shaped glyph metrics, so it is resolved in
        // `prepaint`; here we just flag it and handle the vertical axis.
        self.reveal_caret = true;
        let Some(b) = self.last_bounds else {
            return;
        };
        let line = self
            .document
            .buffer()
            .char_to_position(self.document.selections().primary().head)
            .line;
        if self.soft_wrap {
            // Anchor by document line: exact sub-row visibility is resolved when
            // `prepaint` lays the wrapped block out, so we only correct the line.
            if line < self.wrap_top_line {
                self.wrap_top_line = line;
                self.wrap_top_off = px(0.0);
            } else if line > self.wrap_bottom_line {
                self.wrap_top_line += line - self.wrap_bottom_line;
                self.wrap_top_off = px(0.0);
            }
            return;
        }
        let top = self.line_height * line as f32;
        let bottom = top + self.line_height;
        if top < self.scroll_y {
            self.scroll_y = top;
        } else if bottom > self.scroll_y + b.size.height {
            self.scroll_y = bottom - b.size.height;
        }
    }

    /// Width available for text (viewport minus gutter and scrollbar lane).
    fn view_width(&self) -> Pixels {
        let Some(b) = self.last_bounds else {
            return px(0.0);
        };
        (b.size.width - self.gutter_width - px(14.0)).max(px(0.0))
    }

    fn toggle_soft_wrap(&mut self, cx: &mut Context<Self>) {
        self.soft_wrap = !self.soft_wrap;
        if self.soft_wrap {
            // Seed the wrap anchor from the current pixel scroll position.
            self.wrap_top_line =
                (f32::from(self.scroll_y) / f32::from(self.line_height)).floor() as usize;
            self.wrap_top_off = px(0.0);
            self.scroll_x = px(0.0);
        } else {
            self.scroll_y = self.line_height * self.wrap_top_line as f32;
        }
        set_view_toggles(self.show_line_numbers, self.soft_wrap, cx);
        cx.notify();
    }

    /// Steps the wrap anchor so `wrap_top_off` lands inside `wrap_top_line`'s
    /// block. Only ever measures lines adjacent to the viewport (O(scroll step)).
    fn normalize_wrap_scroll(&mut self, window: &mut Window) {
        let line_count = self.document.buffer().line_count();
        if line_count == 0 {
            self.wrap_top_line = 0;
            self.wrap_top_off = px(0.0);
            return;
        }
        if self.wrap_top_line >= line_count {
            self.wrap_top_line = line_count - 1;
        }
        let view_w = self.view_width();
        let lh = self.line_height;
        loop {
            if self.wrap_top_off < px(0.0) {
                if self.wrap_top_line == 0 {
                    self.wrap_top_off = px(0.0);
                    break;
                }
                self.wrap_top_line -= 1;
                let rows = self.measure_wrap_rows(self.wrap_top_line, view_w, window);
                self.wrap_top_off += lh * rows as f32;
                continue;
            }
            let rows = self.measure_wrap_rows(self.wrap_top_line, view_w, window);
            let block = lh * rows as f32;
            if self.wrap_top_off >= block {
                if self.wrap_top_line + 1 >= line_count {
                    // Don't scroll the last line entirely off-screen.
                    self.wrap_top_off = (block - lh).max(px(0.0));
                    break;
                }
                self.wrap_top_off -= block;
                self.wrap_top_line += 1;
                continue;
            }
            break;
        }
    }

    /// Visual-row count of `line` at `width` (cheap single-run shaping; colours
    /// don't affect wrap boundaries, so this matches the painted layout).
    fn measure_wrap_rows(&self, line: usize, width: Pixels, window: &mut Window) -> usize {
        if width <= px(0.0) {
            return 1;
        }
        let text = self.document.buffer().line_text(line);
        if text.is_empty() {
            return 1;
        }
        let font = window.text_style().font();
        let run = TextRun {
            len: text.len(),
            font,
            color: rgb(0xffffff).into(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        match window.text_system().shape_text(
            SharedString::from(text),
            self.font_size,
            &[run],
            Some(width),
            None,
        ) {
            Ok(lines) => lines.first().map(wrap_rows).unwrap_or(1),
            Err(_) => 1,
        }
    }

    // ---- Movement (operates on every cursor) -----------------------------

    /// Remaps every cursor's head via `f`, extending the selection or collapsing
    /// to a caret, then re-normalizes (merging any cursors that collide).
    fn move_cursors(&mut self, extend: bool, f: impl Fn(&TextBuffer, Cursor) -> usize) {
        let buf = self.document.buffer();
        let new_cursors: Vec<Cursor> = self
            .document
            .selections()
            .cursors()
            .iter()
            .map(|c| {
                let head = f(buf, *c);
                if extend {
                    Cursor::new(c.anchor, head)
                } else {
                    Cursor::caret(head)
                }
            })
            .collect();
        self.document
            .set_selections(SelectionSet::from_cursors(new_cursors));
    }

    fn move_horizontal(&mut self, dir: isize, extend: bool) {
        let len = self.document.buffer().len_chars();
        let new_cursors: Vec<Cursor> = self
            .document
            .selections()
            .cursors()
            .iter()
            .map(|c| {
                if !extend && !c.is_empty() {
                    // Collapse a selection to the edge in the move direction.
                    Cursor::caret(if dir < 0 { c.start() } else { c.end() })
                } else {
                    let head = if dir < 0 {
                        c.head.saturating_sub(1)
                    } else {
                        (c.head + 1).min(len)
                    };
                    if extend {
                        Cursor::new(c.anchor, head)
                    } else {
                        Cursor::caret(head)
                    }
                }
            })
            .collect();
        self.document
            .set_selections(SelectionSet::from_cursors(new_cursors));
    }

    fn move_vertical(&mut self, dir: isize, extend: bool) {
        self.move_cursors(extend, |buf, c| {
            let pos = buf.char_to_position(c.head);
            let last_line = buf.line_count().saturating_sub(1) as isize;
            let target_line = (pos.line as isize + dir).clamp(0, last_line) as usize;
            buf.position_to_char(Position::new(target_line, pos.column))
        });
    }

    fn move_home(&mut self, extend: bool) {
        self.move_cursors(extend, |buf, c| {
            let line = buf.char_to_position(c.head).line;
            buf.position_to_char(Position::new(line, 0))
        });
    }

    fn move_end(&mut self, extend: bool) {
        self.move_cursors(extend, |buf, c| {
            let line = buf.char_to_position(c.head).line;
            let col = buf.line_len_chars(line);
            buf.position_to_char(Position::new(line, col))
        });
    }

    // ---- Multi-cursor ----------------------------------------------------

    /// Adds a caret at `idx`, keeping existing cursors (Alt+Click).
    fn add_caret(&mut self, idx: usize, cx: &mut Context<Self>) {
        let mut set = self.document.selections().clone();
        set.add(Cursor::caret(idx));
        self.document.set_selections(set);
        cx.notify();
    }

    /// Selects the word around the primary caret (the seed for "add next match").
    fn select_word(&mut self, cx: &mut Context<Self>) {
        let primary = self.document.selections().primary();
        let buf = self.document.buffer();
        let pos = buf.char_to_position(primary.head);
        let line_start = buf.position_to_char(Position::new(pos.line, 0));
        let chars: Vec<char> = buf.line_text(pos.line).chars().collect();
        let is_word = |c: char| c.is_alphanumeric() || c == '_';
        let mut start = pos.column.min(chars.len());
        let mut end = start;
        while start > 0 && is_word(chars[start - 1]) {
            start -= 1;
        }
        while end < chars.len() && is_word(chars[end]) {
            end += 1;
        }
        if end > start {
            self.document
                .set_selection(line_start + start, line_start + end);
            self.changed(cx);
        }
    }

    /// Ctrl+D: select the word, or add the next occurrence of the current
    /// selection as an additional cursor (wrapping around the document).
    fn add_next_occurrence(&mut self, cx: &mut Context<Self>) {
        let primary = self.document.selections().primary();
        if primary.is_empty() {
            return self.select_word(cx);
        }
        let needle = self.document.buffer().slice_text(primary.range());
        if needle.is_empty() || needle.contains('\n') {
            return;
        }
        let options = SearchOptions {
            case_sensitive: true,
            whole_word: false,
            regex: false,
        };
        let Ok(searcher) = Searcher::new(&needle, options) else {
            return;
        };
        let found = searcher
            .find_next(self.document.buffer(), primary.end())
            .or_else(|| searcher.find_next(self.document.buffer(), 0));
        if let Some(m) = found {
            let exists = self
                .document
                .selections()
                .cursors()
                .iter()
                .any(|c| c.start() == m.start && c.end() == m.end);
            if !exists {
                let mut set = self.document.selections().clone();
                set.add(Cursor::new(m.start, m.end));
                self.document.set_selections(set);
                self.ensure_caret_visible();
                cx.notify();
            }
        }
    }

    /// Collapses any multi-selection back to a single primary caret.
    fn collapse_carets(&mut self, cx: &mut Context<Self>) {
        if self.document.selections().len() > 1 {
            let head = self.document.selections().primary().head;
            self.document.set_caret(head);
            cx.notify();
        }
    }

    // ---- File operations -------------------------------------------------

    // ---- Tabs ------------------------------------------------------------

    /// Title for the tab at `index` (file name or "Untitled", with a `•` dirty
    /// marker). The active tab reads from the live fields; others from the slot.
    fn tab_title(&self, index: usize) -> String {
        let doc = if index == self.active {
            &self.document
        } else {
            &self.tabs[index].document
        };
        let name = doc
            .path()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("Untitled")
            .to_string();
        if doc.dirty() {
            format!("{name} \u{2022}")
        } else {
            name
        }
    }

    /// Moves the live fields into `tabs[active]` so the tab can be parked.
    fn park_active(&mut self) {
        let document = std::mem::replace(&mut self.document, Document::empty());
        let syntax = std::mem::replace(&mut self.syntax, SyntaxState::new("text"));
        let slot = &mut self.tabs[self.active];
        slot.document = document;
        slot.syntax = syntax;
        slot.parsed_revision = self.parsed_revision;
        slot.scroll_x = self.scroll_x;
        slot.scroll_y = self.scroll_y;
        slot.file_meta = self.file_meta;
        slot.disk_changed = self.disk_changed;
    }

    /// Pulls `tabs[index]` into the live fields and makes it active.
    fn activate(&mut self, index: usize) {
        let slot = &mut self.tabs[index];
        self.document = std::mem::replace(&mut slot.document, Document::empty());
        self.syntax = std::mem::replace(&mut slot.syntax, SyntaxState::new("text"));
        self.parsed_revision = slot.parsed_revision;
        self.scroll_x = slot.scroll_x;
        self.scroll_y = slot.scroll_y;
        self.file_meta = slot.file_meta;
        self.disk_changed = slot.disk_changed;
        self.active = index;
        self.marked_range = None;
        self.input_target = InputTarget::Document;
        self.needs_focus = true;
    }

    fn switch_to_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if index == self.active || index >= self.tabs.len() {
            return;
        }
        self.park_active();
        self.activate(index);
        // The Find-in-Files scope follows the active file; cancel any in-flight
        // search and clear stale results.
        self.retarget_search_panel();
        cx.notify();
    }

    fn next_tab(&mut self, delta: isize, cx: &mut Context<Self>) {
        let n = self.tabs.len();
        if n <= 1 {
            return;
        }
        let next = (self.active as isize + delta).rem_euclid(n as isize) as usize;
        self.switch_to_tab(next, cx);
    }

    /// Opens a fresh empty tab and switches to it.
    fn new_tab(&mut self, cx: &mut Context<Self>) {
        self.park_active();
        self.tabs.push(TabSlot::placeholder());
        let index = self.tabs.len() - 1;
        // The slot is already an empty placeholder; activating drains it.
        self.activate(index);
        self.document.set_caret(0);
        cx.notify();
    }

    fn close_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.tabs.len() {
            return;
        }
        let dirty = if index == self.active {
            self.document.dirty()
        } else {
            self.tabs[index].document.dirty()
        };
        if dirty && self.pending_close.is_none() {
            self.pending_close = Some(CloseTarget::Tab(index));
            cx.notify();
            return;
        }
        self.force_close_tab(index, cx);
    }

    /// Closes tab `index` unconditionally (no unsaved-changes prompt).
    fn force_close_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.tabs.len() {
            return;
        }
        if self.tabs.len() == 1 {
            // Last tab: reset it to an empty untitled buffer instead of closing.
            self.new_file(cx);
            return;
        }
        if index == self.active {
            // Park then drop the active slot, then activate a neighbour.
            self.park_active();
            self.tabs.remove(index);
            let next = index.min(self.tabs.len() - 1);
            self.activate(next);
        } else {
            self.tabs.remove(index);
            if self.active > index {
                self.active -= 1;
            }
        }
        cx.notify();
    }

    // ---- Close confirmation ---------------------------------------------

    /// Indices of all tabs with unsaved changes.
    fn dirty_tabs(&self) -> Vec<usize> {
        (0..self.tabs.len())
            .filter(|&i| {
                if i == self.active {
                    self.document.dirty()
                } else {
                    self.tabs[i].document.dirty()
                }
            })
            .collect()
    }

    /// Clean display name (no dirty marker) for tab `index`.
    fn tab_name(&self, index: usize) -> String {
        let doc = if index == self.active {
            &self.document
        } else {
            &self.tabs[index].document
        };
        doc.path()
            .and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".to_string())
    }

    /// Window-close gate: allow immediately if nothing is dirty, otherwise show
    /// the confirmation overlay and block the close.
    fn request_window_close(&mut self, cx: &mut Context<Self>) -> bool {
        if self.allow_window_close || self.dirty_tabs().is_empty() {
            return true;
        }
        self.pending_close = Some(CloseTarget::Window);
        cx.notify();
        false
    }

    fn close_confirm_cancel(&mut self, cx: &mut Context<Self>) {
        self.pending_close = None;
        cx.notify();
    }

    fn close_confirm_discard(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.pending_close.take() {
            Some(CloseTarget::Tab(i)) => self.force_close_tab(i, cx),
            Some(CloseTarget::Window) => {
                self.allow_window_close = true;
                window.remove_window();
            }
            None => {}
        }
    }

    fn close_confirm_save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        match self.pending_close.take() {
            Some(CloseTarget::Tab(i)) => {
                if i != self.active {
                    self.switch_to_tab(i, cx);
                }
                if self.save_active_sync() {
                    self.force_close_tab(self.active, cx);
                } else {
                    cx.notify();
                }
            }
            Some(CloseTarget::Window) => {
                if self.save_all_sync(cx) {
                    self.allow_window_close = true;
                    window.remove_window();
                } else {
                    cx.notify();
                }
            }
            None => {}
        }
    }

    /// Saves the active document synchronously (prompting for a path if untitled).
    /// Returns false if the user cancelled the save dialog.
    fn save_active_sync(&mut self) -> bool {
        let path = match self.document.path().map(Path::to_path_buf) {
            Some(p) => p,
            None => match crate::pick_save_file_path(&PathBuf::from("untitled.txt")) {
                Some(p) => p,
                None => return false,
            },
        };
        if self.document.save_to(path.clone()).is_ok() {
            self.file_meta = read_file_meta(&path);
            self.disk_changed = false;
            true
        } else {
            true
        }
    }

    /// Saves every dirty tab synchronously. Returns false if the user cancelled.
    fn save_all_sync(&mut self, cx: &mut Context<Self>) -> bool {
        for i in self.dirty_tabs() {
            if self.active != i {
                self.switch_to_tab(i, cx);
            }
            if !self.save_active_sync() {
                return false;
            }
        }
        true
    }

    /// True when the current tab is a pristine, empty, untitled buffer (so we can
    /// open a file into it instead of spawning a new tab).
    fn active_is_pristine(&self) -> bool {
        self.document.path().is_none()
            && !self.document.dirty()
            && self.document.buffer().len_chars() == 0
    }

    // ---- Recent files ----------------------------------------------------

    fn push_recent(&mut self, path: PathBuf) {
        self.recent.retain(|p| p != &path);
        self.recent.insert(0, path);
        self.recent.truncate(12);
    }

    fn toggle_recent(&mut self, cx: &mut Context<Self>) {
        self.show_recent = !self.show_recent;
        cx.notify();
    }

    // ---- External-modification detection ---------------------------------

    /// Starts a lightweight background poller that re-stats the active file every
    /// ~1.5s and flags external changes. Cheap (one `stat`); no per-file watcher
    /// threads, so it scales to many tabs.
    fn start_disk_watch(&mut self, cx: &mut Context<Self>) {
        if self.watch_started {
            return;
        }
        self.watch_started = true;
        cx.spawn(async move |this, cx| loop {
            cx.background_executor().timer(Duration::from_millis(1500)).await;
            let keep = this
                .update(cx, |this, cx| this.check_disk_change(cx))
                .unwrap_or(false);
            if !keep {
                break;
            }
        })
        .detach();
    }

    /// Returns `false` if the entity is gone (stop polling).
    fn check_disk_change(&mut self, cx: &mut Context<Self>) -> bool {
        if self.disk_changed {
            return true;
        }
        let Some(path) = self.document.path().map(Path::to_path_buf) else {
            return true;
        };
        if let (Some(now), Some(prev)) = (read_file_meta(&path), self.file_meta) {
            if now != prev {
                self.disk_changed = true;
                cx.notify();
            }
        }
        true
    }

    fn reload_from_disk(&mut self, cx: &mut Context<Self>) {
        self.disk_changed = false;
        if let Some(path) = self.document.path().map(Path::to_path_buf) {
            let caret = self.document.selections().primary().head;
            let scroll_y = self.scroll_y;
            let target = self.active;
            self.spawn_load(path, target, Some((caret, scroll_y)), cx);
        }
        cx.notify();
    }

    /// Reads and decodes `path` on a background thread, then installs the result
    /// into tab `target` on the main thread. `restore` optionally re-applies a
    /// caret offset and vertical scroll (used by reload-from-disk). Reading off
    /// the UI thread keeps large-file opens from freezing the window.
    fn spawn_load(
        &mut self,
        path: PathBuf,
        target: usize,
        restore: Option<(usize, Pixels)>,
        cx: &mut Context<Self>,
    ) {
        let read_path = path.clone();
        let read = cx
            .background_executor()
            .spawn(async move { load_file(&read_path).ok() });
        cx.spawn(async move |this, cx| {
            let loaded = read.await;
            let _ = this.update(cx, |this, cx| {
                if let Some(loaded) = loaded {
                    this.install_loaded(target, loaded, path, restore, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Installs a freshly-loaded file into tab `target` (live fields if it is the
    /// active tab, otherwise its parked slot). Runs on the main thread.
    fn install_loaded(
        &mut self,
        target: usize,
        loaded: cyberfiles_text_engine::LoadedFile,
        path: PathBuf,
        restore: Option<(usize, Pixels)>,
        _cx: &mut Context<Self>,
    ) {
        let language = language_for_path(Some(&path));
        let meta = read_file_meta(&path);
        self.push_recent(path.clone());
        let mut document = Document::from_loaded(loaded, Some(path), language);
        let len = document.buffer().len_chars();
        let caret = restore.map(|(c, _)| c.min(len)).unwrap_or(0);
        document.set_caret(caret);
        let scroll_y = restore.map(|(_, s)| s).unwrap_or(px(0.0));
        let syntax = SyntaxState::new(language);

        if target == self.active {
            self.document = document;
            self.syntax = syntax;
            self.parsed_revision = None;
            self.scroll_x = px(0.0);
            self.scroll_y = scroll_y;
            self.marked_range = None;
            self.file_meta = meta;
            self.disk_changed = false;
            self.needs_focus = true;
        } else if target < self.tabs.len() {
            let slot = &mut self.tabs[target];
            slot.document = document;
            slot.syntax = syntax;
            slot.parsed_revision = None;
            slot.scroll_x = px(0.0);
            slot.scroll_y = scroll_y;
            slot.file_meta = meta;
            slot.disk_changed = false;
        }
    }

    fn open_file(&mut self, cx: &mut Context<Self>) {
        let start = self.document.path().map(Path::to_path_buf);
        if let Some(path) = crate::pick_open_file_path(start.as_deref()) {
            self.open_path_in_tab(path, cx);
        }
    }

    /// Opens `path`: re-uses an existing tab already showing it, opens into the
    /// current tab if it's pristine, otherwise spawns a new tab.
    fn open_path_in_tab(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.document.path() == Some(path.as_path()) {
            cx.notify();
            return;
        }
        if let Some(existing) = (0..self.tabs.len())
            .find(|&i| i != self.active && self.tabs[i].document.path() == Some(path.as_path()))
        {
            self.switch_to_tab(existing, cx);
            return;
        }
        if !self.active_is_pristine() {
            self.park_active();
            self.tabs.push(TabSlot::placeholder());
            let index = self.tabs.len() - 1;
            self.activate(index);
        }
        let target = self.active;
        self.spawn_load(path, target, None, cx);
        cx.notify();
    }

    fn new_file(&mut self, cx: &mut Context<Self>) {
        self.document = Document::empty();
        self.syntax = SyntaxState::new("text");
        self.parsed_revision = None;
        self.scroll_y = px(0.0);
        self.scroll_x = px(0.0);
        self.file_meta = None;
        self.disk_changed = false;
        self.marked_range = None;
        self.document.set_caret(0);
        cx.notify();
    }

    fn save_file(&mut self, cx: &mut Context<Self>) {
        match self.document.path().map(Path::to_path_buf) {
            Some(path) => self.spawn_save(path, cx),
            None => self.save_file_as(cx),
        }
    }

    fn save_file_as(&mut self, cx: &mut Context<Self>) {
        let default = self
            .document
            .path()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("untitled.txt"));
        if let Some(path) = crate::pick_save_file_path(&default) {
            let language = language_for_path(Some(&path));
            self.document.set_language(language);
            self.syntax = SyntaxState::new(language);
            self.parsed_revision = None;
            self.push_recent(path.clone());
            self.spawn_save(path, cx);
        }
    }

    /// Encodes + writes `path` on a background thread, then records the save on
    /// the UI thread. Keeps large-file saves from freezing the window.
    fn spawn_save(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let target = self.active;
        let snapshot = self.document.save_snapshot();
        let snap_rev = snapshot.revision;
        let write_path = path.clone();
        let write = cx.background_executor().spawn(async move {
            let bytes = snapshot.encode();
            std::fs::write(&write_path, &bytes).is_ok()
        });
        cx.spawn(async move |this, cx| {
            let ok = write.await;
            let _ = this.update(cx, |this, cx| {
                if ok {
                    this.mark_tab_saved(target, path, snap_rev, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Records a completed background save into tab `target` (live or parked).
    fn mark_tab_saved(
        &mut self,
        target: usize,
        path: PathBuf,
        snap_rev: u64,
        _cx: &mut Context<Self>,
    ) {
        let meta = read_file_meta(&path);
        if target == self.active {
            self.document.mark_saved(path, snap_rev);
            self.file_meta = meta;
            self.disk_changed = false;
        } else if target < self.tabs.len() {
            let slot = &mut self.tabs[target];
            slot.document.mark_saved(path, snap_rev);
            slot.file_meta = meta;
            slot.disk_changed = false;
        }
    }

    // ---- Clipboard -------------------------------------------------------

    fn copy(&mut self, cx: &mut Context<Self>) {
        let primary = self.document.selections().primary();
        if primary.is_empty() {
            return;
        }
        let text = self.document.buffer().slice_text(primary.range());
        cx.write_to_clipboard(ClipboardItem::new_string(text));
    }

    fn cut(&mut self, cx: &mut Context<Self>) {
        let range = self.document.selections().primary().range();
        if range.is_empty() {
            return;
        }
        let text = self.document.buffer().slice_text(range.clone());
        cx.write_to_clipboard(ClipboardItem::new_string(text));
        self.document.replace_range(range, "");
        self.changed(cx);
    }

    fn paste(&mut self, cx: &mut Context<Self>) {
        if let Some(text) = cx.read_from_clipboard().and_then(|item| item.text()) {
            self.document.insert(&text);
            self.changed(cx);
        }
    }

    // ---- Selection / editing helpers -------------------------------------

    fn selected_line_range(&self) -> (usize, usize) {
        let primary = self.document.selections().primary();
        let buf = self.document.buffer();
        let first = buf.char_to_position(primary.start()).line;
        let end_pos = buf.char_to_position(primary.end());
        let last = if end_pos.column == 0 && primary.end() > primary.start() {
            end_pos.line.saturating_sub(1)
        } else {
            end_pos.line
        };
        (first, last)
    }

    fn select_line(&mut self, cx: &mut Context<Self>) {
        let buf = self.document.buffer();
        let line = buf.char_to_position(self.document.selections().primary().head).line;
        let start = buf.position_to_char(Position::new(line, 0));
        let line_count = buf.line_count();
        let end = if line + 1 < line_count {
            buf.position_to_char(Position::new(line + 1, 0))
        } else {
            buf.position_to_char(Position::new(line, buf.line_len_chars(line)))
        };
        self.document.set_selection(start, end);
        cx.notify();
    }

    fn indent(&mut self, cx: &mut Context<Self>) {
        let (first, last) = self.selected_line_range();
        for line in (first..=last).rev() {
            let start = self.document.buffer().position_to_char(Position::new(line, 0));
            self.document.replace_range(start..start, "    ");
        }
        self.changed(cx);
    }

    fn outdent(&mut self, cx: &mut Context<Self>) {
        let (first, last) = self.selected_line_range();
        for line in (first..=last).rev() {
            let text = self.document.buffer().line_text(line);
            let spaces = text.chars().take(4).take_while(|c| *c == ' ').count();
            if spaces > 0 {
                let start = self.document.buffer().position_to_char(Position::new(line, 0));
                self.document.replace_range(start..start + spaces, "");
            }
        }
        self.changed(cx);
    }

    fn toggle_comment(&mut self, cx: &mut Context<Self>) {
        let prefix = comment_prefix(self.document.language()).to_string();
        let (first, last) = self.selected_line_range();
        let all_commented = (first..=last).all(|line| {
            let t = self.document.buffer().line_text(line);
            let trimmed = t.trim_start();
            trimmed.is_empty() || trimmed.starts_with(&prefix)
        });
        let prefix_len = prefix.chars().count();
        for line in (first..=last).rev() {
            let text = self.document.buffer().line_text(line);
            if text.trim_start().is_empty() {
                continue;
            }
            let indent = text.chars().take_while(|c| c.is_whitespace()).count();
            let start = self.document.buffer().position_to_char(Position::new(line, 0));
            let at = start + indent;
            if all_commented {
                let rest: Vec<char> = text.chars().skip(indent).collect();
                let mut remove = prefix_len;
                if rest.get(prefix_len) == Some(&' ') {
                    remove += 1;
                }
                self.document.replace_range(at..at + remove, "");
            } else {
                self.document.replace_range(at..at, &format!("{prefix} "));
            }
        }
        self.changed(cx);
    }

    // ---- View ------------------------------------------------------------

    fn toggle_line_numbers(&mut self, cx: &mut Context<Self>) {
        self.show_line_numbers = !self.show_line_numbers;
        set_view_toggles(self.show_line_numbers, self.soft_wrap, cx);
        cx.notify();
    }

    fn zoom(&mut self, delta: f32, cx: &mut Context<Self>) {
        let size = (f32::from(self.font_size) + delta).clamp(8.0, 40.0);
        self.font_size = px(size);
        self.line_height = px((size * 1.45).round());
        cx.notify();
    }

    fn zoom_reset(&mut self, cx: &mut Context<Self>) {
        self.font_size = px(14.0);
        self.line_height = px(20.0);
        cx.notify();
    }

    fn toggle_shortcuts(&mut self, cx: &mut Context<Self>) {
        self.show_shortcuts = !self.show_shortcuts;
        cx.notify();
    }

    fn toggle_about(&mut self, cx: &mut Context<Self>) {
        self.show_about = !self.show_about;
        cx.notify();
    }

    // ---- Go to Line ------------------------------------------------------

    fn open_goto(&mut self, cx: &mut Context<Self>) {
        self.find = None;
        self.search_panel = None;
        self.goto = Some(String::new());
        self.input_target = InputTarget::GotoLine;
        cx.notify();
    }

    fn close_goto(&mut self, cx: &mut Context<Self>) {
        self.goto = None;
        self.input_target = InputTarget::Document;
        cx.notify();
    }

    fn goto_backspace(&mut self, cx: &mut Context<Self>) {
        if let Some(g) = self.goto.as_mut() {
            g.pop();
        }
        cx.notify();
    }

    fn do_goto(&mut self, cx: &mut Context<Self>) {
        if let Some(text) = self.goto.clone() {
            if let Ok(n) = text.trim().parse::<usize>() {
                let last = self.document.buffer().line_count().saturating_sub(1);
                let line = n.saturating_sub(1).min(last);
                let target = self
                    .document
                    .buffer()
                    .position_to_char(Position::new(line, 0));
                self.document.set_caret(target);
                self.ensure_caret_visible();
            }
        }
        self.close_goto(cx);
    }

    // ---- Find in Files (global search) -----------------------------------

    fn search_root(&self) -> PathBuf {
        self.document
            .path()
            .and_then(|p| p.parent())
            .map(Path::to_path_buf)
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."))
    }

    fn open_search_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.find = None;
        self.goto = None;
        let seed = {
            let primary = self.document.selections().primary();
            if !primary.is_empty() {
                let text = self.document.buffer().slice_text(primary.range());
                if text.contains('\n') {
                    String::new()
                } else {
                    text
                }
            } else {
                String::new()
            }
        };
        let root = self.search_root();
        match self.search_panel.as_mut() {
            Some(panel) => {
                if !seed.is_empty() {
                    let query = panel.query.clone();
                    query.update(cx, |s, cx| s.set_value(seed, window, cx));
                }
                let query = self.search_panel.as_ref().unwrap().query.clone();
                query.update(cx, |s, cx| s.focus(window, cx));
            }
            None => {
                let query = cx.new(|cx| {
                    InputState::new(window, cx)
                        .placeholder("Find in files")
                        .default_value(seed)
                });
                let mut subs = Vec::new();
                subs.push(cx.subscribe(&query, |this, _, ev: &InputEvent, cx| {
                    if let InputEvent::PressEnter { .. } = ev {
                        this.run_global_search(cx);
                    }
                }));
                query.update(cx, |s, cx| s.focus(window, cx));
                self.search_panel = Some(SearchPanelState {
                    query,
                    root,
                    results: Vec::new(),
                    rows: Vec::new(),
                    status: String::new(),
                    case_sensitive: false,
                    whole_word: false,
                    regex: false,
                    generation: 0,
                    scroll: VirtualListScrollHandle::new(),
                    _subs: subs,
                });
            }
        }
        cx.notify();
    }

    fn close_search_panel(&mut self, cx: &mut Context<Self>) {
        self.search_panel = None;
        self.input_target = InputTarget::Document;
        self.needs_focus = true;
        cx.notify();
    }

    /// Cancels any in-flight global search and points the panel at `root`'s
    /// directory, clearing stale results. Called when switching tabs so the
    /// "Find in Files" scope follows the active file.
    fn retarget_search_panel(&mut self) {
        let root = self.search_root();
        if let Some(panel) = self.search_panel.as_mut() {
            panel.generation += 1; // cancel any pending search
            panel.root = root;
            panel.results.clear();
            panel.rows.clear();
            panel.status.clear();
        }
    }

    /// Kicks off a directory search on a background thread; results are applied
    /// to the panel when ready (stale generations are dropped).
    fn run_global_search(&mut self, cx: &mut Context<Self>) {
        let query = match self.search_panel.as_ref() {
            Some(panel) => panel.query.read(cx).value().to_string(),
            None => return,
        };
        let Some(panel) = self.search_panel.as_mut() else {
            return;
        };
        if query.trim().is_empty() {
            panel.results.clear();
            panel.rows.clear();
            panel.status = String::new();
            cx.notify();
            return;
        }
        panel.generation += 1;
        let generation = panel.generation;
        let root = panel.root.clone();
        let options = GlobalSearchOptions {
            case_sensitive: panel.case_sensitive,
            whole_word: panel.whole_word,
            regex: panel.regex,
            ..Default::default()
        };
        panel.status = "Searching…".to_string();
        cx.notify();

        let task = cx
            .background_executor()
            .spawn(async move { search_directory(&root, &query, &options) });
        cx.spawn(async move |this, cx| {
            let outcome = task.await;
            this.update(cx, |this, cx| {
                let Some(panel) = this.search_panel.as_mut() else {
                    return;
                };
                if panel.generation != generation {
                    return; // a newer search superseded this one
                }
                match outcome {
                    Ok(results) => {
                        let files = results.len();
                        let hits: usize = results.iter().map(|f| f.matches.len()).sum();
                        panel.status = if hits == 0 {
                            "No results".to_string()
                        } else {
                            format!("{hits} matches in {files} files")
                        };
                        panel.results = results;
                        panel.rebuild_rows();
                    }
                    Err(err) => {
                        panel.results.clear();
                        panel.rows.clear();
                        panel.status = format!("Error: {err}");
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn open_search_result(&mut self, path: PathBuf, line_number: u64, cx: &mut Context<Self>) {
        let same = self.document.path() == Some(path.as_path());
        if !same {
            self.open_path_in_tab(path, cx);
        }
        let line = (line_number.saturating_sub(1)) as usize;
        let last = self.document.buffer().line_count().saturating_sub(1);
        let line = line.min(last);
        let target = self
            .document
            .buffer()
            .position_to_char(Position::new(line, 0));
        self.document.set_caret(target);
        self.input_target = InputTarget::Document;
        self.needs_focus = true;
        self.ensure_caret_visible();
        cx.notify();
    }

    // ---- Find / Replace --------------------------------------------------

    fn open_find(&mut self, replace_mode: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.search_panel = None;
        self.goto = None;
        let primary = self.document.selections().primary();
        let seed = if !primary.is_empty() {
            let text = self.document.buffer().slice_text(primary.range());
            if text.contains('\n') {
                None
            } else {
                Some(text)
            }
        } else {
            None
        };
        match self.find.as_mut() {
            Some(find) => {
                find.replace_mode = replace_mode || find.replace_mode;
                if let Some(seed) = seed {
                    let query = find.query.clone();
                    query.update(cx, |s, cx| s.set_value(seed, window, cx));
                }
                find.query.update(cx, |s, cx| s.focus(window, cx));
            }
            None => {
                let initial = seed.unwrap_or_default();
                let query = cx.new(|cx| {
                    InputState::new(window, cx)
                        .placeholder("Find")
                        .default_value(initial)
                });
                let replace =
                    cx.new(|cx| InputState::new(window, cx).placeholder("Replace with"));
                let mut subs = Vec::new();
                subs.push(cx.subscribe(&query, |this, _, ev: &InputEvent, cx| {
                    if let InputEvent::PressEnter { shift, .. } = ev {
                        this.do_find(!shift, cx);
                    }
                }));
                subs.push(cx.subscribe(&replace, |this, _, ev: &InputEvent, cx| {
                    if let InputEvent::PressEnter { .. } = ev {
                        this.do_replace(cx);
                    }
                }));
                query.update(cx, |s, cx| s.focus(window, cx));
                self.find = Some(FindState {
                    query,
                    replace,
                    replace_mode,
                    case_sensitive: false,
                    whole_word: false,
                    regex: false,
                    status: String::new(),
                    _subs: subs,
                });
            }
        }
        self.input_target = InputTarget::Document;
        self.update_find_status(cx);
        cx.notify();
    }

    fn close_find(&mut self, cx: &mut Context<Self>) {
        self.find = None;
        self.input_target = InputTarget::Document;
        self.needs_focus = true;
        cx.notify();
    }

    /// The current Find query text.
    fn find_query(&self, cx: &App) -> String {
        self.find
            .as_ref()
            .map(|f| f.query.read(cx).value().to_string())
            .unwrap_or_default()
    }

    /// The current Replace-with text.
    fn find_replace_text(&self, cx: &App) -> String {
        self.find
            .as_ref()
            .map(|f| f.replace.read(cx).value().to_string())
            .unwrap_or_default()
    }

    fn update_find_status(&mut self, cx: &mut Context<Self>) {
        let Some(options) = self.find.as_ref().map(|f| f.options()) else {
            return;
        };
        let query = self.find_query(cx);
        let status = if query.is_empty() {
            String::new()
        } else {
            match Searcher::new(&query, options) {
                Ok(searcher) => {
                    let head = self.document.selections().primary().start();
                    let (total, index) = searcher.count_and_index(self.document.buffer(), head);
                    if total == 0 {
                        "No matches".to_string()
                    } else {
                        match index {
                            Some(i) => format!("{i} of {total}"),
                            None => format!("{total} found"),
                        }
                    }
                }
                Err(_) => "Bad pattern".to_string(),
            }
        };
        if let Some(find) = self.find.as_mut() {
            find.status = status;
        }
    }

    fn do_find(&mut self, forward: bool, cx: &mut Context<Self>) {
        let Some(options) = self.find.as_ref().map(|f| f.options()) else {
            return;
        };
        let query = self.find_query(cx);
        if query.is_empty() {
            return;
        }
        let Ok(searcher) = Searcher::new(&query, options) else {
            if let Some(find) = self.find.as_mut() {
                find.status = "Bad pattern".to_string();
            }
            cx.notify();
            return;
        };
        let (start, end) = {
            let p = self.document.selections().primary();
            (p.start(), p.end())
        };
        let found = if forward {
            searcher.find_next(self.document.buffer(), end)
        } else {
            searcher.find_prev(self.document.buffer(), start)
        };
        if let Some(m) = found {
            self.document.set_selection(m.start, m.end);
            self.ensure_caret_visible();
        }
        self.update_find_status(cx);
        cx.notify();
    }

    fn current_match(&self, searcher: &Searcher) -> Option<Match> {
        let (start, end) = {
            let p = self.document.selections().primary();
            if p.is_empty() {
                return None;
            }
            (p.start(), p.end())
        };
        searcher
            .all_matches(self.document.buffer())
            .into_iter()
            .find(|cand| cand.start == start && cand.end == end)
    }

    fn do_replace(&mut self, cx: &mut Context<Self>) {
        let Some((options, regex)) = self.find.as_ref().map(|f| (f.options(), f.regex)) else {
            return;
        };
        let query = self.find_query(cx);
        if query.is_empty() {
            return;
        }
        let replace = self.find_replace_text(cx);
        let Ok(searcher) = Searcher::new(&query, options) else {
            return;
        };
        if let Some(m) = self.current_match(&searcher) {
            let replacement = searcher.replacement_for(self.document.buffer(), m, &replace, regex);
            self.document.replace_range(m.start..m.end, &replacement);
        }
        self.do_find(true, cx);
    }

    fn do_replace_all(&mut self, cx: &mut Context<Self>) {
        let Some((options, regex)) = self.find.as_ref().map(|f| (f.options(), f.regex)) else {
            return;
        };
        let query = self.find_query(cx);
        if query.is_empty() {
            return;
        }
        let replace = self.find_replace_text(cx);
        let Ok(searcher) = Searcher::new(&query, options) else {
            return;
        };
        // Single atomic transaction: O(matches) and one undo step even for tens
        // of thousands of matches.
        let count = self.document.replace_all(&searcher, &replace, regex);
        self.ensure_caret_visible();
        if let Some(find) = self.find.as_mut() {
            find.status = format!("Replaced {count}");
        }
        cx.notify();
    }

    // ---- Input -----------------------------------------------------------

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let ks = &event.keystroke;
        let shift = ks.modifiers.shift;
        let cmd = ks.modifiers.control || ks.modifiers.platform;

        // Global keys (work regardless of focus target).
        if ks.key == "escape" {
            if self.pending_close.is_some() {
                self.pending_close = None;
                cx.notify();
            } else if self.show_about {
                self.show_about = false;
                cx.notify();
            } else if self.show_shortcuts {
                self.show_shortcuts = false;
                cx.notify();
            } else if self.show_recent {
                self.show_recent = false;
                cx.notify();
            } else if self.goto.is_some() {
                self.close_goto(cx);
            } else if self.search_panel.is_some() {
                self.close_search_panel(cx);
            } else if self.find.is_some() {
                self.close_find(cx);
            } else {
                self.collapse_carets(cx);
            }
            return;
        }
        if ks.key == "f3" {
            self.do_find(!shift, cx);
            return;
        }
        if cmd {
            match ks.key.as_str() {
                "f" => {
                    if shift {
                        return self.open_search_panel(window, cx);
                    }
                    return self.open_find(false, window, cx);
                }
                "h" => return self.open_find(true, window, cx),
                "g" => return self.open_goto(cx),
                "o" => return self.open_file(cx),
                "n" | "t" => return self.new_tab(cx),
                "w" => return self.close_tab(self.active, cx),
                "e" => return self.toggle_recent(cx),
                "tab" => {
                    self.next_tab(if shift { -1 } else { 1 }, cx);
                    return;
                }
                "s" => {
                    if shift {
                        return self.save_file_as(cx);
                    }
                    return self.save_file(cx);
                }
                _ => {}
            }
        }

        // Typing into the Go to Line field (custom overlay; routes through the
        // editor focus). The Find / Find-in-Files panels own gpui-component
        // inputs, so their keys are handled by those inputs directly.
        if self.goto.is_some() && self.input_target == InputTarget::GotoLine {
            match ks.key.as_str() {
                "backspace" => self.goto_backspace(cx),
                "enter" => self.do_goto(cx),
                _ => {}
            }
            return;
        }

        // If a panel's text input holds focus, don't let editor keystrokes mutate
        // the document underneath it.
        if !self.focus_handle.is_focused(window) {
            return;
        }

        // Named (non-text) keys.
        match ks.key.as_str() {
            "backspace" => {
                self.document.delete_backward();
                return self.changed(cx);
            }
            "delete" => {
                self.document.delete_forward();
                return self.changed(cx);
            }
            "enter" if !cmd => {
                self.document.insert("\n");
                return self.changed(cx);
            }
            "tab" if !cmd => {
                self.document.insert("    ");
                return self.changed(cx);
            }
            "left" => {
                self.move_horizontal(-1, shift);
                return self.changed(cx);
            }
            "right" => {
                self.move_horizontal(1, shift);
                return self.changed(cx);
            }
            "up" => {
                self.move_vertical(-1, shift);
                return self.changed(cx);
            }
            "down" => {
                self.move_vertical(1, shift);
                return self.changed(cx);
            }
            "home" => {
                self.move_home(shift);
                return self.changed(cx);
            }
            "end" => {
                self.move_end(shift);
                return self.changed(cx);
            }
            _ => {}
        }

        if cmd {
            match ks.key.as_str() {
                "a" => {
                    self.document.select_all();
                    self.changed(cx);
                }
                "z" => {
                    self.document.undo();
                    self.changed(cx);
                }
                "y" => {
                    self.document.redo();
                    self.changed(cx);
                }
                "x" => self.cut(cx),
                "c" => self.copy(cx),
                "v" => self.paste(cx),
                "d" => self.add_next_occurrence(cx),
                "l" => self.select_line(cx),
                "=" | "+" => self.zoom(1.0, cx),
                "-" => self.zoom(-1.0, cx),
                "0" => self.zoom_reset(cx),
                _ => {}
            }
        }
        // Printable characters are inserted via EntityInputHandler (IME path).
    }

    // ---- Mouse / scroll --------------------------------------------------

    fn index_for_position(&self, pos: Point<Pixels>) -> usize {
        let Some(bounds) = self.last_bounds else {
            return 0;
        };
        if self.soft_wrap {
            return self.wrap_index_for_position(pos, bounds);
        }
        if self.visible.is_empty() {
            return 0;
        }
        let content_left = bounds.left() + self.gutter_width - self.scroll_x;
        let vl = self
            .visible
            .iter()
            .find(|vl| pos.y >= vl.top && pos.y < vl.top + self.line_height)
            .unwrap_or_else(|| {
                if pos.y < self.visible.first().unwrap().top {
                    self.visible.first().unwrap()
                } else {
                    self.visible.last().unwrap()
                }
            });
        let x = pos.x - content_left;
        let byte = if x <= px(0.0) {
            0
        } else {
            vl.shaped.closest_index_for_x(x)
        };
        let line_text = self.document.buffer().line_text(vl.line);
        let char_in_line = line_text[..byte.min(line_text.len())].chars().count();
        vl.start_char + char_in_line
    }

    /// Hit-tests a point against the wrapped visible lines (wrap mode).
    fn wrap_index_for_position(&self, pos: Point<Pixels>, bounds: Bounds<Pixels>) -> usize {
        if self.wrapped_visible.is_empty() {
            return 0;
        }
        let content_left = bounds.left() + self.gutter_width;
        let lh = self.line_height;
        let wl = self
            .wrapped_visible
            .iter()
            .find(|wl| {
                let block = lh * wrap_rows(&wl.wrapped) as f32;
                pos.y >= wl.top && pos.y < wl.top + block
            })
            .unwrap_or_else(|| {
                if pos.y < self.wrapped_visible.first().unwrap().top {
                    self.wrapped_visible.first().unwrap()
                } else {
                    self.wrapped_visible.last().unwrap()
                }
            });
        let local = point((pos.x - content_left).max(px(0.0)), pos.y - wl.top);
        let byte = match wl.wrapped.closest_index_for_position(local, lh) {
            Ok(b) => b,
            Err(b) => b,
        };
        let line_text = self.document.buffer().line_text(wl.line);
        let char_in_line = line_text[..byte.min(line_text.len())].chars().count();
        wl.start_char + char_in_line
    }

    fn on_mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        window.focus(&self.focus_handle, cx);
        self.input_target = InputTarget::Document;
        let idx = self.index_for_position(event.position);
        if event.click_count >= 3 {
            // Triple-click selects the whole line.
            self.document.set_caret(idx);
            self.select_line(cx);
            return;
        }
        if event.click_count == 2 {
            // Double-click selects the word under the cursor.
            self.document.set_caret(idx);
            self.select_word(cx);
            return;
        }
        if event.modifiers.alt {
            // Alt+Click drops an additional caret.
            self.add_caret(idx, cx);
        } else if event.modifiers.shift {
            let anchor = self.document.selections().primary().anchor;
            self.document.set_selection(anchor, idx);
        } else {
            self.document.set_caret(idx);
        }
        self.is_selecting = true;
        cx.notify();
    }

    fn on_mouse_move(&mut self, event: &MouseMoveEvent, _window: &mut Window, cx: &mut Context<Self>) {
        // If the button was released while the cursor was outside the window we
        // never saw a mouse-up; the next move with no button pressed ends any
        // in-progress drag/selection so it doesn't "stick" on re-entry.
        if event.pressed_button != Some(MouseButton::Left)
            && (self.scrollbar_drag.is_some()
                || self.hscrollbar_drag.is_some()
                || self.is_selecting)
        {
            self.scrollbar_drag = None;
            self.hscrollbar_drag = None;
            self.is_selecting = false;
            cx.notify();
            return;
        }
        if let Some((start_y, start_scroll)) = self.scrollbar_drag {
            if let Some(metrics) = self.scrollbar_metrics() {
                let denom = (metrics.viewport - metrics.thumb_h).max(px(1.0));
                let ratio = f32::from(metrics.max_scroll) / f32::from(denom);
                let dy = f32::from(event.position.y - start_y);
                let new = px(f32::from(start_scroll) + dy * ratio);
                self.apply_vertical_scroll(new, metrics.max_scroll);
                cx.notify();
            }
            return;
        }
        if let Some((start_x, start_scroll)) = self.hscrollbar_drag {
            if let Some(metrics) = self.hscrollbar_metrics() {
                let denom = (metrics.track - metrics.thumb_w).max(px(1.0));
                let ratio = f32::from(metrics.max_scroll) / f32::from(denom);
                let dx = f32::from(event.position.x - start_x);
                let new = f32::from(start_scroll) + dx * ratio;
                self.scroll_x = px(new.clamp(0.0, f32::from(metrics.max_scroll)));
                cx.notify();
            }
            return;
        }
        if self.is_selecting {
            let idx = self.index_for_position(event.position);
            let anchor = self.document.selections().primary().anchor;
            self.document.set_selection(anchor, idx);
            cx.notify();
        }
    }

    fn on_mouse_up(&mut self, _event: &MouseUpEvent, _window: &mut Window, _cx: &mut Context<Self>) {
        self.is_selecting = false;
        self.scrollbar_drag = None;
        self.hscrollbar_drag = None;
    }

    fn on_scroll(&mut self, event: &ScrollWheelEvent, window: &mut Window, cx: &mut Context<Self>) {
        let delta = event.delta.pixel_delta(self.line_height);
        if self.soft_wrap {
            // Vertical only; advance the wrapped anchor and re-normalize.
            self.wrap_top_off -= delta.y;
            self.normalize_wrap_scroll(window);
            cx.notify();
            return;
        }
        // Shift+wheel scrolls horizontally (the common convention).
        let (dx, dy) = if event.modifiers.shift {
            (delta.y, px(0.0))
        } else {
            (delta.x, delta.y)
        };
        self.scroll_y = (self.scroll_y - dy).max(px(0.0)).min(self.max_scroll());
        self.scroll_x = (self.scroll_x - dx).max(px(0.0)).min(self.max_scroll_x());
        cx.notify();
    }

    fn max_scroll_x(&self) -> Pixels {
        let Some(b) = self.last_bounds else {
            return px(0.0);
        };
        let view_w = (b.size.width - self.gutter_width - px(14.0)).max(px(0.0));
        (self.content_width - view_w).max(px(0.0))
    }

    fn scrollbar_metrics(&self) -> Option<ScrollbarMetrics> {
        let bounds = self.last_bounds?;
        let viewport = bounds.size.height;
        if viewport <= px(0.0) {
            return None;
        }
        // In wrap mode we approximate content height by document-line count
        // (each line ≥ 1 row) and express the scroll position in those virtual
        // pixels, so the thumb is proportional without an O(n) layout pass.
        let (content_h, pos) = if self.soft_wrap {
            let lines = self.document.buffer().line_count() as f32;
            let pos = self.line_height * self.wrap_top_line as f32 + self.wrap_top_off;
            (self.line_height * lines, pos)
        } else {
            (
                self.line_height * self.document.buffer().line_count() as f32,
                self.scroll_y,
            )
        };
        if content_h <= viewport {
            return None;
        }
        let thumb_h = (viewport * (f32::from(viewport) / f32::from(content_h)))
            .max(px(24.0))
            .min(viewport);
        let max_scroll = (content_h - viewport).max(px(0.0));
        let denom = (viewport - thumb_h).max(px(1.0));
        let thumb_top = denom * (f32::from(pos) / f32::from(max_scroll)).clamp(0.0, 1.0);
        Some(ScrollbarMetrics {
            viewport,
            thumb_top,
            thumb_h,
            max_scroll,
        })
    }

    /// Applies a vertical scrollbar position (`pos` in virtual pixels), routing
    /// to the wrap anchor or the pixel scroll depending on the mode.
    fn apply_vertical_scroll(&mut self, pos: Pixels, max_scroll: Pixels) {
        let pos = pos.max(px(0.0)).min(max_scroll);
        if self.soft_wrap {
            let lh = f32::from(self.line_height);
            let p = f32::from(pos);
            self.wrap_top_line = (p / lh).floor() as usize;
            self.wrap_top_off = px(p - (p / lh).floor() * lh);
        } else {
            self.scroll_y = pos;
        }
    }

    fn hscrollbar_metrics(&self) -> Option<HScrollbarMetrics> {
        if self.soft_wrap {
            return None;
        }
        let bounds = self.last_bounds?;
        let track = (bounds.size.width - self.gutter_width - px(14.0)).max(px(0.0));
        let content = self.content_width;
        if content <= track || track <= px(0.0) {
            return None;
        }
        let thumb_w = (track * (f32::from(track) / f32::from(content)))
            .max(px(24.0))
            .min(track);
        let max_scroll = (content - track).max(px(0.0));
        let denom = (track - thumb_w).max(px(1.0));
        let thumb_left = denom * (f32::from(self.scroll_x) / f32::from(max_scroll));
        Some(HScrollbarMetrics {
            track,
            thumb_left,
            thumb_w,
            max_scroll,
        })
    }

    fn render_hscrollbar(&self, cx: &mut Context<Self>) -> Option<Stateful<gpui::Div>> {
        let metrics = self.hscrollbar_metrics()?;
        let gutter = self.gutter_width;
        Some(
            div()
                .id("editor-hscrollbar")
                .absolute()
                .bottom_0()
                .left(gutter)
                .right(px(14.0))
                .h(px(12.0))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, event: &MouseDownEvent, _w, cx| {
                        let Some(metrics) = this.hscrollbar_metrics() else {
                            return;
                        };
                        let Some(bounds) = this.last_bounds else {
                            return;
                        };
                        let local_x = event.position.x - bounds.left() - this.gutter_width;
                        if local_x < metrics.thumb_left {
                            this.scroll_x = (this.scroll_x - metrics.track).max(px(0.0));
                        } else if local_x > metrics.thumb_left + metrics.thumb_w {
                            this.scroll_x = (this.scroll_x + metrics.track).min(metrics.max_scroll);
                        }
                        cx.stop_propagation();
                        cx.notify();
                    }),
                )
                .child(
                    div()
                        .id("editor-hscrollbar-thumb")
                        .absolute()
                        .bottom(px(2.0))
                        .left(metrics.thumb_left)
                        .h(px(8.0))
                        .w(metrics.thumb_w)
                        .rounded_full()
                        .bg(rgb(0x4a4a4a))
                        .hover(|s| s.bg(rgb(0x5a5a5a)))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, event: &MouseDownEvent, _w, cx| {
                                this.hscrollbar_drag = Some((event.position.x, this.scroll_x));
                                cx.stop_propagation();
                            }),
                        ),
                ),
        )
    }

    fn render_scrollbar(&self, cx: &mut Context<Self>) -> Option<Stateful<gpui::Div>> {
        let metrics = self.scrollbar_metrics()?;
        Some(
            div()
                .id("editor-scrollbar")
                .absolute()
                .top_0()
                .right_0()
                .h_full()
                .w(px(12.0))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, event: &MouseDownEvent, _w, cx| {
                        let Some(metrics) = this.scrollbar_metrics() else {
                            return;
                        };
                        let Some(bounds) = this.last_bounds else {
                            return;
                        };
                        let local_y = event.position.y - bounds.top();
                        let page = metrics.viewport;
                        let cur = this.vertical_scroll_pos();
                        if local_y < metrics.thumb_top {
                            this.apply_vertical_scroll(cur - page, metrics.max_scroll);
                        } else if local_y > metrics.thumb_top + metrics.thumb_h {
                            this.apply_vertical_scroll(cur + page, metrics.max_scroll);
                        }
                        cx.stop_propagation();
                        cx.notify();
                    }),
                )
                .child(
                    div()
                        .id("editor-scrollbar-thumb")
                        .absolute()
                        .right(px(2.0))
                        .top(metrics.thumb_top)
                        .w(px(8.0))
                        .h(metrics.thumb_h)
                        .rounded_full()
                        .bg(rgb(0x4a4a4a))
                        .hover(|s| s.bg(rgb(0x5a5a5a)))
                        .on_mouse_down(
                            MouseButton::Left,
                            cx.listener(|this, event: &MouseDownEvent, _w, cx| {
                                this.scrollbar_drag =
                                    Some((event.position.y, this.vertical_scroll_pos()));
                                cx.stop_propagation();
                            }),
                        ),
                ),
        )
    }

    /// Current vertical scroll position in the same virtual-pixel space the
    /// scrollbar uses (wrap anchor or pixel scroll).
    fn vertical_scroll_pos(&self) -> Pixels {
        if self.soft_wrap {
            self.line_height * self.wrap_top_line as f32 + self.wrap_top_off
        } else {
            self.scroll_y
        }
    }

    // ---- Header ----------------------------------------------------------

    fn render_header(&self) -> gpui::Div {
        let name = self
            .document
            .path()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("Untitled")
            .to_string();
        let dirty = if self.document.dirty() { " ●" } else { "" };
        let caret = self.document.selections().primary().head;
        let pos = self.document.buffer().char_to_position(caret);
        let info = format!(
            "{}   {}   {}   Ln {}, Col {}",
            self.document.language(),
            self.document.encoding().label(),
            self.document.line_ending().label(),
            pos.line + 1,
            pos.column + 1,
        );

        div()
            .flex()
            .flex_row()
            .items_center()
            .justify_between()
            .h(px(28.0))
            .px_3()
            .bg(rgb(0x252526))
            .text_color(rgb(0xcccccc))
            .text_size(px(12.0))
            .child(div().child(SharedString::from(format!("{name}{dirty}"))))
            .child(div().text_color(rgb(0x8a8a8a)).child(SharedString::from(info)))
    }

    /// The tab strip (one chip per open document + a "new tab" button).
    fn render_tab_bar(&self, cx: &mut Context<Self>) -> gpui::Div {
        let active = self.active;
        let mut strip = div()
            .flex()
            .flex_row()
            .items_center()
            .h(px(30.0))
            .bg(rgb(0x202020))
            .text_size(px(12.0))
            .border_b_1()
            .border_color(rgb(0x161616));

        for index in 0..self.tabs.len() {
            let is_active = index == active;
            let title = self.tab_title(index);
            let close = div()
                .id(("tab-close", index))
                .flex()
                .items_center()
                .justify_center()
                .w(px(16.0))
                .h(px(16.0))
                .rounded_sm()
                .text_color(rgb(0x9a9a9a))
                .hover(|s| s.bg(rgb(0x4a4a4a)).text_color(rgb(0xffffff)))
                .child(SharedString::from("\u{00d7}"))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _e: &MouseDownEvent, _w, cx| {
                        cx.stop_propagation();
                        this.close_tab(index, cx);
                    }),
                );

            let chip = div()
                .id(("tab", index))
                .flex()
                .flex_row()
                .items_center()
                .gap_1()
                .px_2()
                .h_full()
                .max_w(px(220.0))
                .border_r_1()
                .border_color(rgb(0x161616))
                .bg(if is_active {
                    rgb(0x1e1e1e)
                } else {
                    rgb(0x2a2a2a)
                })
                .text_color(if is_active {
                    rgb(0xffffff)
                } else {
                    rgb(0xb5b5b5)
                })
                .hover(|s| s.bg(rgb(0x333333)))
                .child(
                    div()
                        .overflow_hidden()
                        .child(SharedString::from(title)),
                )
                .child(close)
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(move |this, _e: &MouseDownEvent, _w, cx| {
                        this.switch_to_tab(index, cx);
                    }),
                );
            strip = strip.child(chip);
        }

        strip.child(
            div()
                .id("tab-new")
                .flex()
                .items_center()
                .justify_center()
                .w(px(28.0))
                .h_full()
                .text_color(rgb(0x9a9a9a))
                .hover(|s| s.bg(rgb(0x333333)).text_color(rgb(0xffffff)))
                .child(SharedString::from("+"))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _e: &MouseDownEvent, _w, cx| this.new_tab(cx)),
                ),
        )
    }

    /// A banner shown when the active file was modified on disk by another app.
    fn render_disk_banner(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        if !self.disk_changed {
            return None;
        }
        let reload = bar_button("disk-reload", "Reload", false).on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _e: &MouseDownEvent, _w, cx| this.reload_from_disk(cx)),
        );
        let ignore = bar_button("disk-ignore", "Ignore", false).on_mouse_down(
            MouseButton::Left,
            cx.listener(|this, _e: &MouseDownEvent, _w, cx| {
                this.disk_changed = false;
                cx.notify();
            }),
        );
        Some(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .h(px(28.0))
                .px_3()
                .bg(rgb(0x5a4a1a))
                .text_color(rgb(0xf0e0b0))
                .text_size(px(12.0))
                .child(SharedString::from(
                    "This file changed on disk.".to_string(),
                ))
                .child(reload)
                .child(ignore),
        )
    }

    /// The Recent Files dropdown (toggled with Ctrl+E).
    fn render_recent(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        if !self.show_recent {
            return None;
        }
        let mut list = div()
            .absolute()
            .top(px(4.0))
            .left(px(4.0))
            .w(px(420.0))
            .max_h(px(360.0))
            .overflow_hidden()
            .rounded_md()
            .bg(rgb(0x252526))
            .border_1()
            .border_color(rgb(0x3c3c3c))
            .text_size(px(12.0))
            .text_color(rgb(0xd4d4d4))
            .child(
                div()
                    .px_3()
                    .py_1()
                    .bg(rgb(0x2d2d2d))
                    .text_color(rgb(0x9a9a9a))
                    .child(SharedString::from("Recent Files")),
            );

        if self.recent.is_empty() {
            list = list.child(
                div()
                    .px_3()
                    .py_2()
                    .text_color(rgb(0x8a8a8a))
                    .child(SharedString::from("No recent files")),
            );
        }
        for (i, path) in self.recent.iter().enumerate() {
            let display = path.to_string_lossy().to_string();
            let target = path.clone();
            list = list.child(
                div()
                    .id(("recent", i))
                    .px_3()
                    .py_1()
                    .overflow_hidden()
                    .hover(|s| s.bg(rgb(0x094771)))
                    .child(SharedString::from(display))
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _e: &MouseDownEvent, _w, cx| {
                            this.show_recent = false;
                            this.open_path_in_tab(target.clone(), cx);
                        }),
                    ),
            );
        }
        Some(list)
    }

    fn render_title_bar(&self, cx: &mut Context<Self>) -> TitleBar {
        let menu_bar = editor_menu_bar(cx);
        TitleBar::new().child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .px_2()
                .size_full()
                .child(
                    div()
                        .flex_none()
                        .text_size(px(13.0))
                        .child(SharedString::from("CyberEditor")),
                )
                .child(div().flex_none().child(menu_bar)),
        )
    }

    fn render_about(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        if !self.show_about {
            return None;
        }
        let panel = div()
            .w(px(360.0))
            .flex()
            .flex_col()
            .gap_2()
            .p_4()
            .rounded_lg()
            .bg(rgb(0x252526))
            .border_1()
            .border_color(rgb(0x454545))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(div().text_size(px(16.0)).child(SharedString::from("CyberEditor")))
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(rgb(0x9a9a9a))
                    .child(SharedString::from("High-performance text & code editor")),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(rgb(0x9a9a9a))
                    .child(SharedString::from("Rust · GPUI · rope text engine")),
            )
            .child(
                bar_button("about-close", "Close", false).on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _e: &MouseDownEvent, _w, cx| {
                        this.show_about = false;
                        cx.stop_propagation();
                        cx.notify();
                    }),
                ),
            );

        Some(
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(gpui::rgba(0x00000080))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _e: &MouseDownEvent, _w, cx| {
                        this.show_about = false;
                        cx.notify();
                    }),
                )
                .child(panel),
        )
    }

    /// Keyboard-shortcuts reference overlay (Help → Keyboard Shortcuts).
    fn render_shortcuts(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        if !self.show_shortcuts {
            return None;
        }
        const SHORTCUTS: &[(&str, &str)] = &[
            ("File", ""),
            ("New tab", "Ctrl+N / Ctrl+T"),
            ("Open file", "Ctrl+O"),
            ("Save / Save As", "Ctrl+S / Ctrl+Shift+S"),
            ("Close tab", "Ctrl+W"),
            ("Next / Prev tab", "Ctrl+Tab / Ctrl+Shift+Tab"),
            ("Recent files", "Ctrl+E"),
            ("Edit", ""),
            ("Undo / Redo", "Ctrl+Z / Ctrl+Y"),
            ("Cut / Copy / Paste", "Ctrl+X / Ctrl+C / Ctrl+V"),
            ("Select all / line", "Ctrl+A / Ctrl+L"),
            ("Indent / Outdent", "Alt+] / Alt+["),
            ("Toggle comment", "Ctrl+/"),
            ("Zoom in / out / reset", "Ctrl+= / Ctrl+- / Ctrl+0"),
            ("Search & navigate", ""),
            ("Find / Replace", "Ctrl+F / Ctrl+H"),
            ("Find next / prev", "F3 / Shift+F3"),
            ("Find in files", "Ctrl+Shift+F"),
            ("Go to line", "Ctrl+G"),
            ("Add next occurrence", "Ctrl+D"),
            ("Add caret (mouse)", "Alt+Click"),
            ("Select word / line", "Double / Triple click"),
            ("View", ""),
            ("Word wrap", "Menu: View → Word Wrap"),
            ("Line numbers", "Menu: View → Line Numbers"),
        ];

        let mut list = div().flex().flex_col().gap_0p5();
        for (label, keys) in SHORTCUTS {
            if keys.is_empty() {
                // Section header.
                list = list.child(
                    div()
                        .mt_2()
                        .text_size(px(12.0))
                        .text_color(rgb(0x6fb3d2))
                        .child(SharedString::from(*label)),
                );
            } else {
                list = list.child(
                    div()
                        .flex()
                        .flex_row()
                        .justify_between()
                        .gap_4()
                        .text_size(px(12.0))
                        .child(
                            div()
                                .text_color(rgb(0xcccccc))
                                .child(SharedString::from(*label)),
                        )
                        .child(
                            div()
                                .text_color(rgb(0x9a9a9a))
                                .child(SharedString::from(*keys)),
                        ),
                );
            }
        }

        let panel = div()
            .w(px(440.0))
            .max_h(px(560.0))
            .flex()
            .flex_col()
            .gap_2()
            .p_4()
            .rounded_lg()
            .bg(rgb(0x252526))
            .border_1()
            .border_color(rgb(0x454545))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .text_size(px(16.0))
                    .child(SharedString::from("Keyboard Shortcuts")),
            )
            .child(div().overflow_hidden().child(list))
            .child(
                bar_button("shortcuts-close", "Close", false).on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _e: &MouseDownEvent, _w, cx| {
                        this.show_shortcuts = false;
                        cx.stop_propagation();
                        cx.notify();
                    }),
                ),
            );

        Some(
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(gpui::rgba(0x00000080))
                .on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _e: &MouseDownEvent, _w, cx| {
                        this.show_shortcuts = false;
                        cx.notify();
                    }),
                )
                .child(panel),
        )
    }

    /// Unsaved-changes confirmation overlay (closing a tab or the window).
    fn render_close_confirm(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let target = self.pending_close?;
        let message = match target {
            CloseTarget::Tab(i) => format!("Save changes to \u{201c}{}\u{201d} before closing?", self.tab_name(i)),
            CloseTarget::Window => {
                let n = self.dirty_tabs().len();
                if n <= 1 {
                    "You have unsaved changes. Save before closing?".to_string()
                } else {
                    format!("{n} files have unsaved changes. Save them before closing?")
                }
            }
        };
        let save_label = if target == CloseTarget::Window && self.dirty_tabs().len() > 1 {
            "Save All"
        } else {
            "Save"
        };

        let panel = div()
            .w(px(400.0))
            .flex()
            .flex_col()
            .gap_3()
            .p_4()
            .rounded_lg()
            .bg(rgb(0x252526))
            .border_1()
            .border_color(rgb(0x454545))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .text_size(px(15.0))
                    .child(SharedString::from("Unsaved Changes")),
            )
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(rgb(0xcccccc))
                    .child(SharedString::from(message)),
            )
            .child(
                div()
                    .flex()
                    .flex_row()
                    .justify_end()
                    .gap_2()
                    .child(bar_button("close-cancel", "Cancel", false).on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _e: &MouseDownEvent, _w, cx| {
                            cx.stop_propagation();
                            this.close_confirm_cancel(cx);
                        }),
                    ))
                    .child(bar_button("close-discard", "Don't Save", false).on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _e: &MouseDownEvent, window, cx| {
                            cx.stop_propagation();
                            this.close_confirm_discard(window, cx);
                        }),
                    ))
                    .child(bar_button("close-save", save_label, true).on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _e: &MouseDownEvent, window, cx| {
                            cx.stop_propagation();
                            this.close_confirm_save(window, cx);
                        }),
                    )),
            );

        Some(
            div()
                .absolute()
                .inset_0()
                .flex()
                .items_center()
                .justify_center()
                .bg(gpui::rgba(0x00000080))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(panel),
        )
    }

    fn render_search_panel(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let panel = self.search_panel.as_ref()?;

        let opt_btn = |id: &'static str, label: &str, active: bool, tip: &'static str| {
            Button::new(id)
                .ghost()
                .xsmall()
                .selected(active)
                .label(label.to_string())
                .tooltip(tip)
        };

        let controls = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_1()
            .child(div().flex_1().child(Input::new(&panel.query).small()))
            .child(
                Button::new("files-go")
                    .primary()
                    .xsmall()
                    .label("Search")
                    .on_click(
                        cx.listener(|this, _: &ClickEvent, _w, cx| this.run_global_search(cx)),
                    ),
            )
            .child(
                opt_btn("files-case", "Aa", panel.case_sensitive, "Match case").on_click(
                    cx.listener(|this, _: &ClickEvent, _w, cx| {
                        if let Some(p) = this.search_panel.as_mut() {
                            p.case_sensitive = !p.case_sensitive;
                        }
                        cx.notify();
                    }),
                ),
            )
            .child(
                opt_btn("files-word", "W", panel.whole_word, "Whole word").on_click(cx.listener(
                    |this, _: &ClickEvent, _w, cx| {
                        if let Some(p) = this.search_panel.as_mut() {
                            p.whole_word = !p.whole_word;
                        }
                        cx.notify();
                    },
                )),
            )
            .child(
                opt_btn("files-regex", ".*", panel.regex, "Regular expression").on_click(
                    cx.listener(|this, _: &ClickEvent, _w, cx| {
                        if let Some(p) = this.search_panel.as_mut() {
                            p.regex = !p.regex;
                        }
                        cx.notify();
                    }),
                ),
            )
            .child(
                Button::new("files-close")
                    .ghost()
                    .xsmall()
                    .label("\u{2715}")
                    .tooltip("Close (Esc)")
                    .on_click(
                        cx.listener(|this, _: &ClickEvent, _w, cx| this.close_search_panel(cx)),
                    ),
            );

        let root_label = panel.root.display().to_string();
        let header = div()
            .flex()
            .flex_col()
            .gap_1()
            .p_2()
            .border_b_1()
            .border_color(rgb(0x3a3a3a))
            .child(
                div()
                    .text_size(px(12.0))
                    .text_color(rgb(0xcccccc))
                    .child(SharedString::from("Find in Files")),
            )
            .child(controls)
            .child(
                div()
                    .text_size(px(10.0))
                    .text_color(rgb(0x6a6a6a))
                    .child(SharedString::from(format!("in {root_label}"))),
            )
            .child(
                div()
                    .text_size(px(11.0))
                    .text_color(rgb(0x9a9a9a))
                    .child(SharedString::from(panel.status.clone())),
            );

        // High-performance virtualized results list: only visible rows render.
        let row_count = panel.rows.len();
        let item_sizes: Rc<Vec<Size<Pixels>>> =
            Rc::new(vec![size(px(1.0), px(20.0)); row_count.max(1)]);
        let list = v_virtual_list(
            cx.entity().clone(),
            "files-virtual-list",
            item_sizes,
            move |this, range, _window, cx| {
                let Some(panel) = this.search_panel.as_ref() else {
                    return Vec::new();
                };
                let mut out = Vec::new();
                for index in range {
                    let Some(row) = panel.rows.get(index) else {
                        continue;
                    };
                    out.push(match row.clone() {
                        SearchRow::File { label, count } => div()
                            .id(("files-file-row", index))
                            .h(px(20.0))
                            .px_2()
                            .flex()
                            .items_center()
                            .text_size(px(11.0))
                            .text_color(rgb(0x7fb0e0))
                            .child(SharedString::from(format!("{label}  ({count})"))),
                        SearchRow::Match { path, line, text } => div()
                            .id(("files-match-row", index))
                            .h(px(20.0))
                            .px_2()
                            .pl_4()
                            .flex()
                            .items_center()
                            .text_size(px(12.0))
                            .text_color(rgb(0xd4d4d4))
                            .hover(|s| s.bg(rgb(0x094771)))
                            .child(SharedString::from(format!(
                                "{line:>5}: {text}"
                            )))
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _e: &MouseDownEvent, _w, cx| {
                                    this.open_search_result(path.clone(), line, cx);
                                    cx.stop_propagation();
                                }),
                            ),
                    });
                }
                out
            },
        )
        .track_scroll(&panel.scroll);

        let results_list = div()
            .id("files-results")
            .flex_1()
            .min_h_0()
            .child(list)
            .scrollbar(&panel.scroll, ScrollbarAxis::Vertical);

        Some(
            div()
                .absolute()
                .top_0()
                .right_0()
                .bottom_0()
                .w(px(380.0))
                .flex()
                .flex_col()
                .bg(rgb(0x252526))
                .border_l_1()
                .border_color(rgb(0x3a3a3a))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_scroll_wheel(|_, _, cx| cx.stop_propagation())
                .child(header)
                .child(results_list),
        )
    }

    fn render_goto(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let value = self.goto.as_ref()?;
        let last = self.document.buffer().line_count();
        let field = render_input_field(
            "goto-field",
            value,
            "Line",
            true,
            None,
            cx.listener(|_this, _e: &MouseDownEvent, _w, cx| cx.stop_propagation()),
        );
        Some(
            div()
                .absolute()
                .top_2()
                .left_4()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .p_2()
                .rounded_md()
                .bg(rgb(0x2d2d30))
                .border_1()
                .border_color(rgb(0x454545))
                .text_color(rgb(0xcccccc))
                .text_size(px(12.0))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .child(
                    div()
                        .text_color(rgb(0x9a9a9a))
                        .child(SharedString::from(format!("Go to line (1-{last}):"))),
                )
                .child(field)
                .child(bar_button("goto-go", "Go", false).on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|this, _e: &MouseDownEvent, _w, cx| {
                        this.do_goto(cx);
                        cx.stop_propagation();
                    }),
                )),
        )
    }

    fn render_find_bar(&self, cx: &mut Context<Self>) -> Option<gpui::Div> {
        let find = self.find.as_ref()?;
        let replace_mode = find.replace_mode;

        let query_field = div().w(px(200.0)).child(Input::new(&find.query).small());

        let status = div()
            .min_w(px(64.0))
            .text_size(px(11.0))
            .text_color(rgb(0x9a9a9a))
            .child(SharedString::from(find.status.clone()));

        let opt_btn = |id: &'static str, label: &str, active: bool, tip: &'static str| {
            Button::new(id)
                .ghost()
                .xsmall()
                .selected(active)
                .label(label.to_string())
                .tooltip(tip)
        };

        let find_row = div()
            .flex()
            .flex_row()
            .items_center()
            .gap_1()
            .child(query_field)
            .child(status)
            .child(
                Button::new("find-search")
                    .primary()
                    .xsmall()
                    .label("Find")
                    .on_click(cx.listener(|this, _: &ClickEvent, _w, cx| this.do_find(true, cx))),
            )
            .child(opt_btn("find-prev", "\u{2191}", false, "Find previous (Shift+F3)").on_click(
                cx.listener(|this, _: &ClickEvent, _w, cx| this.do_find(false, cx)),
            ))
            .child(opt_btn("find-next", "\u{2193}", false, "Find next (F3)").on_click(
                cx.listener(|this, _: &ClickEvent, _w, cx| this.do_find(true, cx)),
            ))
            .child(
                opt_btn("find-case", "Aa", find.case_sensitive, "Match case").on_click(
                    cx.listener(|this, _: &ClickEvent, _w, cx| {
                        if let Some(f) = this.find.as_mut() {
                            f.case_sensitive = !f.case_sensitive;
                        }
                        this.update_find_status(cx);
                        cx.notify();
                    }),
                ),
            )
            .child(
                opt_btn("find-word", "W", find.whole_word, "Whole word").on_click(cx.listener(
                    |this, _: &ClickEvent, _w, cx| {
                        if let Some(f) = this.find.as_mut() {
                            f.whole_word = !f.whole_word;
                        }
                        this.update_find_status(cx);
                        cx.notify();
                    },
                )),
            )
            .child(
                opt_btn("find-regex", ".*", find.regex, "Regular expression").on_click(cx.listener(
                    |this, _: &ClickEvent, _w, cx| {
                        if let Some(f) = this.find.as_mut() {
                            f.regex = !f.regex;
                        }
                        this.update_find_status(cx);
                        cx.notify();
                    },
                )),
            )
            .child(
                Button::new("find-close")
                    .ghost()
                    .xsmall()
                    .label("\u{2715}")
                    .tooltip("Close (Esc)")
                    .on_click(cx.listener(|this, _: &ClickEvent, _w, cx| this.close_find(cx))),
            );

        let mut bar = div()
            .absolute()
            .top_2()
            .right_4()
            .flex()
            .flex_col()
            .gap_1()
            .p_2()
            .rounded_md()
            .bg(rgb(0x2d2d30))
            .border_1()
            .border_color(rgb(0x454545))
            .text_color(rgb(0xcccccc))
            .text_size(px(12.0))
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(find_row);

        if replace_mode {
            let replace_field = div().w(px(200.0)).child(Input::new(&find.replace).small());
            let replace_row = div()
                .flex()
                .flex_row()
                .items_center()
                .gap_1()
                .child(replace_field)
                .child(
                    Button::new("replace-one")
                        .xsmall()
                        .label("Replace")
                        .on_click(
                            cx.listener(|this, _: &ClickEvent, _w, cx| this.do_replace(cx)),
                        ),
                )
                .child(
                    Button::new("replace-all")
                        .xsmall()
                        .label("Replace All")
                        .on_click(
                            cx.listener(|this, _: &ClickEvent, _w, cx| this.do_replace_all(cx)),
                        ),
                );
            bar = bar.child(replace_row);
        }

        Some(bar)
    }
}

/// A simple read-only text field showing the current value (with a caret bar
/// when active). Used only by the Go to Line overlay; editing happens through
/// the editor's key/IME handlers.
fn render_input_field(
    id: &'static str,
    value: &str,
    placeholder: &str,
    active: bool,
    caret: Option<usize>,
    on_down: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static,
) -> Stateful<gpui::Div> {
    let (text, color) = if value.is_empty() && !active {
        (placeholder.to_string(), rgb(0x6a6a6a))
    } else if active {
        // Insert a caret bar at the caret char position (defaults to end).
        let pos = caret.unwrap_or_else(|| value.chars().count());
        let byte = char_to_byte(value, pos);
        (
            format!("{}\u{2502}{}", &value[..byte], &value[byte..]),
            rgb(0xd4d4d4),
        )
    } else {
        (value.to_string(), rgb(0xd4d4d4))
    };
    div()
        .id(id)
        .w(px(180.0))
        .h(px(22.0))
        .px_2()
        .flex()
        .items_center()
        .rounded_sm()
        .bg(rgb(0x3c3c3c))
        .border_1()
        .border_color(if active { rgb(0x007acc) } else { rgb(0x3c3c3c) })
        .text_color(color)
        .child(SharedString::from(text))
        .on_mouse_down(MouseButton::Left, on_down)
}

/// A small clickable chip/button for the find bar.
fn bar_button(id: &'static str, label: &str, active: bool) -> Stateful<gpui::Div> {
    let mut el = div()
        .id(id)
        .h(px(22.0))
        .px_2()
        .flex()
        .items_center()
        .justify_center()
        .rounded_sm()
        .text_color(rgb(0xcccccc))
        .hover(|s| s.bg(rgb(0x3e3e42)))
        .child(SharedString::from(label.to_string()));
    if active {
        el = el.bg(rgb(0x094771));
    }
    el
}

impl Focusable for EngineEditor {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for EngineEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.needs_focus {
            window.focus(&self.focus_handle, cx);
            self.needs_focus = false;
        }
        self.start_disk_watch(cx);
        if !self.close_hooked {
            self.close_hooked = true;
            let weak = cx.entity().downgrade();
            window.on_window_should_close(cx, move |_window, cx| {
                weak.update(cx, |this, cx| this.request_window_close(cx))
                    .unwrap_or(true)
            });
        }
        let title_bar = self.render_title_bar(cx);
        let tab_bar = self.render_tab_bar(cx);
        let disk_banner = self.render_disk_banner(cx);
        let header = self.render_header();
        let focus = self.focus_handle.clone();
        let find_bar = self.render_find_bar(cx);
        let goto_bar = self.render_goto(cx);
        let search_panel = self.render_search_panel(cx);
        let about = self.render_about(cx);
        let shortcuts = self.render_shortcuts(cx);
        let close_confirm = self.render_close_confirm(cx);
        let recent = self.render_recent(cx);
        let scrollbar = self.render_scrollbar(cx);
        let hscrollbar = self.render_hscrollbar(cx);
        let canvas = EditorCanvas {
            editor: cx.entity(),
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .child(title_bar)
            .child(tab_bar)
            .children(disk_banner)
            .child(header)
            .child(
                div()
                    .track_focus(&focus)
                    .key_context("CyberEngineEditor")
                    .on_key_down(cx.listener(Self::on_key_down))
                    .on_action(cx.listener(|this, _: &NewFile, _w, cx| this.new_tab(cx)))
                    .on_action(cx.listener(|this, _: &OpenFile, _w, cx| this.open_file(cx)))
                    .on_action(cx.listener(|this, _: &SaveFile, _w, cx| this.save_file(cx)))
                    .on_action(cx.listener(|this, _: &SaveFileAs, _w, cx| this.save_file_as(cx)))
                    .on_action(cx.listener(|_, _: &ExitEditor, window, _| window.remove_window()))
                    .on_action(cx.listener(|this, _: &EditorUndo, _w, cx| {
                        this.document.undo();
                        this.changed(cx);
                    }))
                    .on_action(cx.listener(|this, _: &EditorRedo, _w, cx| {
                        this.document.redo();
                        this.changed(cx);
                    }))
                    .on_action(cx.listener(|this, _: &EditorCut, _w, cx| this.cut(cx)))
                    .on_action(cx.listener(|this, _: &EditorCopy, _w, cx| this.copy(cx)))
                    .on_action(cx.listener(|this, _: &EditorPaste, _w, cx| this.paste(cx)))
                    .on_action(cx.listener(|this, _: &SelectAll, _w, cx| {
                        this.document.select_all();
                        cx.notify();
                    }))
                    .on_action(cx.listener(|this, _: &FindText, window, cx| {
                        this.open_find(false, window, cx)
                    }))
                    .on_action(cx.listener(|this, _: &FindInFiles, window, cx| {
                        this.open_search_panel(window, cx)
                    }))
                    .on_action(cx.listener(|this, _: &ReplaceText, window, cx| {
                        this.open_find(true, window, cx)
                    }))
                    .on_action(cx.listener(|this, _: &ReplaceAllText, window, cx| {
                        if this.find.is_some() {
                            this.do_replace_all(cx);
                        } else {
                            this.open_find(true, window, cx);
                        }
                    }))
                    .on_action(cx.listener(|this, _: &FindNext, _w, cx| this.do_find(true, cx)))
                    .on_action(cx.listener(|this, _: &FindPrevious, _w, cx| this.do_find(false, cx)))
                    .on_action(cx.listener(|this, _: &IndentSelection, _w, cx| this.indent(cx)))
                    .on_action(cx.listener(|this, _: &OutdentSelection, _w, cx| this.outdent(cx)))
                    .on_action(cx.listener(|this, _: &ToggleComment, _w, cx| this.toggle_comment(cx)))
                    .on_action(cx.listener(|this, _: &ToggleLineNumbers, _w, cx| {
                        this.toggle_line_numbers(cx)
                    }))
                    .on_action(cx.listener(|this, _: &ToggleSoftWrap, _w, cx| {
                        this.toggle_soft_wrap(cx)
                    }))
                    .on_action(cx.listener(|this, _: &AboutEditor, _w, cx| this.toggle_about(cx)))
                    .on_action(cx.listener(|this, _: &KeyboardShortcuts, _w, cx| {
                        this.toggle_shortcuts(cx)
                    }))
                    .on_action(cx.listener(|this, _: &GoToLine, _w, cx| this.open_goto(cx)))
                    .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
                    .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
                    .on_mouse_move(cx.listener(Self::on_mouse_move))
                    .on_scroll_wheel(cx.listener(Self::on_scroll))
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .bg(rgb(0x1e1e1e))
                    .text_color(rgb(0xd4d4d4))
                    .text_size(self.font_size)
                    .line_height(self.line_height)
                    .font_family("Consolas")
                    .child(canvas)
                    .children(scrollbar)
                    .children(hscrollbar)
                    .children(find_bar)
                    .children(goto_bar)
                    .children(search_panel)
                    .children(about)
                    .children(shortcuts)
                    .children(close_confirm)
                    .children(recent),
            )
    }
}

// ---- EntityInputHandler (text + IME) -------------------------------------

impl EntityInputHandler for EngineEditor {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        actual_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let buf = self.document.buffer();
        let start = buf.utf16_to_char(range_utf16.start);
        let end = buf.utf16_to_char(range_utf16.end);
        actual_range.replace(buf.char_to_utf16(start)..buf.char_to_utf16(end));
        Some(buf.slice_text(start..end))
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let primary = self.document.selections().primary();
        let buf = self.document.buffer();
        Some(UTF16Selection {
            range: buf.char_to_utf16(primary.start())..buf.char_to_utf16(primary.end()),
            reversed: primary.head < primary.anchor,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        let r = self.marked_range.clone()?;
        let buf = self.document.buffer();
        Some(buf.char_to_utf16(r.start)..buf.char_to_utf16(r.end))
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.marked_range = None;
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.append_to_find_field(new_text) {
            cx.notify();
            return;
        }
        // Plain typing with multiple carets and no IME marking: insert at every
        // cursor (the engine handles the multi-span edit atomically).
        if range_utf16.is_none()
            && self.marked_range.is_none()
            && self.document.selections().len() > 1
        {
            self.document.insert(new_text);
            self.changed(cx);
            return;
        }
        let range_char = self.resolve_input_range(range_utf16);
        self.document.set_selection(range_char.start, range_char.end);
        self.document.insert(new_text);
        self.marked_range = None;
        self.changed(cx);
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        _new_selected_range_utf16: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.append_to_find_field(new_text) {
            cx.notify();
            return;
        }
        let range_char = self.resolve_input_range(range_utf16);
        self.document.set_selection(range_char.start, range_char.end);
        self.document.insert(new_text);
        if new_text.is_empty() {
            self.marked_range = None;
        } else {
            let inserted = new_text.chars().count();
            self.marked_range = Some(range_char.start..range_char.start + inserted);
        }
        self.changed(cx);
    }

    fn bounds_for_range(
        &mut self,
        range_utf16: Range<usize>,
        element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        let start_char = self.document.buffer().utf16_to_char(range_utf16.start);
        let pos = self.document.buffer().char_to_position(start_char);
        if self.soft_wrap {
            let wl = self.wrapped_visible.iter().find(|wl| wl.line == pos.line)?;
            let line_text = self.document.buffer().line_text(wl.line);
            let col = start_char.saturating_sub(wl.start_char);
            let byte = char_to_byte(&line_text, col);
            let p = wl.wrapped.position_for_index(byte, self.line_height)?;
            let x = element_bounds.left() + self.gutter_width + p.x;
            let top = wl.top + p.y;
            return Some(Bounds::from_corners(
                point(x, top),
                point(x, top + self.line_height),
            ));
        }
        let vl = self.visible.iter().find(|vl| vl.line == pos.line)?;
        let line_text = self.document.buffer().line_text(vl.line);
        let col = start_char.saturating_sub(vl.start_char);
        let byte = char_to_byte(&line_text, col);
        let x = element_bounds.left() + self.gutter_width - self.scroll_x
            + vl.shaped.x_for_index(byte);
        Some(Bounds::from_corners(
            point(x, vl.top),
            point(x, vl.top + self.line_height),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let idx = self.index_for_position(point);
        Some(self.document.buffer().char_to_utf16(idx))
    }
}

impl EngineEditor {
    /// If typing is currently directed at the Go to Line field, appends `text`
    /// to it and returns `true`. Returns `false` when text should edit the doc.
    /// (Find / Find-in-Files inputs are gpui-component widgets that handle their
    /// own typing, so they never reach here.)
    fn append_to_find_field(&mut self, text: &str) -> bool {
        match self.input_target {
            InputTarget::GotoLine => {
                if let Some(g) = self.goto.as_mut() {
                    g.extend(text.chars().filter(|c| c.is_ascii_digit()));
                }
                true
            }
            _ => false,
        }
    }

    /// Resolves the target char range for a text-input edit: explicit range,
    /// else the marked range, else the current selection.
    fn resolve_input_range(&self, range_utf16: Option<Range<usize>>) -> Range<usize> {
        if let Some(r) = range_utf16 {
            let buf = self.document.buffer();
            return buf.utf16_to_char(r.start)..buf.utf16_to_char(r.end);
        }
        if let Some(m) = self.marked_range.clone() {
            return m;
        }
        self.document.selections().primary().range()
    }
}

// ---- The custom text element ---------------------------------------------

struct EditorCanvas {
    editor: Entity<EngineEditor>,
}

struct CanvasPrepaint {
    rows: Vec<VisibleRow>,
    /// Populated instead of `rows` when soft wrap is on.
    wrapped_rows: Vec<WrappedRow>,
    gutter: Vec<(Pixels, ShapedLine)>,
    selections: Vec<PaintQuad>,
    carets: Vec<PaintQuad>,
    content_left: Pixels,
    gutter_left: Pixels,
}

struct VisibleRow {
    line: usize,
    start_char: usize,
    top: Pixels,
    shaped: ShapedLine,
}

struct WrappedRow {
    line: usize,
    start_char: usize,
    top: Pixels,
    wrapped: WrappedLine,
}

impl IntoElement for EditorCanvas {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for EditorCanvas {
    type RequestLayoutState = ();
    type PrepaintState = CanvasPrepaint;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.0).into();
        style.size.height = relative(1.0).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        // Keep syntax current and clamp scroll/gutter to this frame's bounds.
        self.editor.update(cx, |e, _| {
            e.refresh_syntax();
            e.last_bounds = Some(bounds);
            let line_count = e.document.buffer().line_count();
            let digits = line_count.to_string().len().max(3);
            e.gutter_width = if e.show_line_numbers {
                e.font_size * (digits as f32 + 2.0) * 0.6
            } else {
                px(8.0)
            };
            let total = e.line_height * line_count as f32;
            let max = (total - bounds.size.height).max(px(0.0));
            if e.scroll_y > max {
                e.scroll_y = max;
            }
        });

        let style = window.text_style();
        let font = style.font();
        let default_color = style.color;
        let font_size = style.font_size.to_pixels(window.rem_size());

        if self.editor.read(cx).soft_wrap {
            return self.prepaint_wrapped(bounds, &font, default_color, font_size, window, cx);
        }

        let editor = self.editor.read(cx);
        let line_height = editor.line_height;
        let scroll_y = editor.scroll_y;
        let gutter_width = editor.gutter_width;
        let show_line_numbers = editor.show_line_numbers;
        let focused = editor.focus_handle.is_focused(window);
        let buf = editor.document.buffer();
        let line_count = buf.line_count();
        let digits = line_count.to_string().len().max(3);

        let primary = editor.document.selections().primary();
        let cursors = editor.document.selections().cursors();

        // Resolve horizontal caret reveal up front (needs glyph metrics): shape
        // just the caret's line to find its x, then nudge `scroll_x`.
        let view_w = (bounds.size.width - gutter_width - px(14.0)).max(px(0.0));
        let mut scroll_x = editor.scroll_x;
        if editor.reveal_caret {
            let cpos = buf.char_to_position(primary.head);
            let cline = buf.line_text(cpos.line);
            let crun = TextRun {
                len: cline.len(),
                font: font.clone(),
                color: default_color,
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let cshaped = window.text_system().shape_line(
                SharedString::from(cline.clone()),
                font_size,
                &[crun],
                None,
            );
            let caret_x = cshaped.x_for_index(char_to_byte(&cline, cpos.column));
            let margin = px(24.0);
            if caret_x < scroll_x {
                scroll_x = caret_x;
            } else if caret_x > scroll_x + view_w - margin {
                scroll_x = caret_x - view_w + margin;
            }
            scroll_x = scroll_x.max(px(0.0));
        }

        let content_left = bounds.left() + gutter_width - scroll_x;
        let gutter_left = bounds.left() + px(4.0);

        let first_line = (f32::from(scroll_y) / f32::from(line_height)).floor() as usize;
        let visible_count = (f32::from(bounds.size.height) / f32::from(line_height)).ceil() as usize + 2;
        let last_line = (first_line + visible_count).min(line_count);

        let mut rows = Vec::new();
        let mut gutter = Vec::new();
        let mut selections = Vec::new();
        let mut carets: Vec<PaintQuad> = Vec::new();
        let mut content_w = px(0.0);
        let highlight_word = occurrence_word(&editor.document);

        for line in first_line..last_line {
            let top = bounds.top() + line_height * line as f32 - scroll_y;
            let line_start_char = buf.position_to_char(Position::new(line, 0));
            let line_text = buf.line_text(line);
            let line_char_len = buf.line_len_chars(line);
            let line_end_char = line_start_char + line_char_len;
            let line_start_byte = buf.char_to_byte(line_start_char);

            let runs = build_runs(
                &editor.syntax,
                buf,
                &line_text,
                line_start_byte,
                &font,
                default_color,
            );
            let shaped = window.text_system().shape_line(
                SharedString::from(line_text.clone()),
                font_size,
                &runs,
                None,
            );
            if shaped.width > content_w {
                content_w = shaped.width;
            }

            // Same-word occurrence highlights (skip the active selection itself).
            if let Some((word, sel_range)) = &highlight_word {
                for (scol, ecol) in word_occurrences(&line_text, word) {
                    let abs_s = line_start_char + scol;
                    let abs_e = line_start_char + ecol;
                    if abs_s == sel_range.start && abs_e == sel_range.end {
                        continue;
                    }
                    let x0 = content_left + shaped.x_for_index(char_to_byte(&line_text, scol));
                    let x1 = content_left + shaped.x_for_index(char_to_byte(&line_text, ecol));
                    selections.push(fill(
                        Bounds::from_corners(point(x0, top), point(x1, top + line_height)),
                        rgb(0x4c4a2f),
                    ));
                }
            }

            // Selection bands + carets for every cursor on this line.
            for cur in cursors {
                let range = cur.range();
                if !cur.is_empty() && range.start <= line_end_char && range.end > line_start_char {
                    let start_col = range.start.max(line_start_char) - line_start_char;
                    let end_col = range.end.min(line_end_char) - line_start_char;
                    let x0 = content_left + shaped.x_for_index(char_to_byte(&line_text, start_col));
                    let x1 = content_left + shaped.x_for_index(char_to_byte(&line_text, end_col));
                    selections.push(fill(
                        Bounds::from_corners(point(x0, top), point(x1, top + line_height)),
                        rgb(0x264f78),
                    ));
                }
                if focused && cur.head >= line_start_char && cur.head <= line_end_char {
                    let col = cur.head - line_start_char;
                    let cx_pos = content_left + shaped.x_for_index(char_to_byte(&line_text, col));
                    carets.push(fill(
                        Bounds::new(point(cx_pos, top), gpui::size(px(2.0), line_height)),
                        rgb(0xaeafad),
                    ));
                }
            }

            // Gutter line number.
            if show_line_numbers {
                let num = format!("{:>width$} ", line + 1, width = digits);
                let grun = TextRun {
                    len: num.len(),
                    font: font.clone(),
                    color: rgb(0x6e7681).into(),
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                };
                let gshaped =
                    window
                        .text_system()
                        .shape_line(SharedString::from(num), font_size, &[grun], None);
                gutter.push((top, gshaped));
            }

            rows.push(VisibleRow {
                line,
                start_char: line_start_char,
                top,
                shaped,
            });
        }

        // Add one character of right padding so the caret at line end is visible.
        let content_w = content_w + line_height * 0.6;
        // End the read borrow before mutating the entity.
        let _ = (buf, &editor);
        self.editor.update(cx, |e, _| {
            e.content_width = content_w;
            let max_x = (content_w - view_w).max(px(0.0));
            e.scroll_x = scroll_x.min(max_x).max(px(0.0));
            e.reveal_caret = false;
        });

        CanvasPrepaint {
            rows,
            wrapped_rows: Vec::new(),
            gutter,
            selections,
            carets,
            content_left,
            gutter_left,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let (focus_handle, line_height, gutter_width) = {
            let e = self.editor.read(cx);
            (e.focus_handle.clone(), e.line_height, e.gutter_width)
        };

        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.editor.clone()),
            cx,
        );

        // Clip the (horizontally scrollable) text + selections so they never
        // bleed over the fixed line-number gutter or the vertical scrollbar.
        let content_mask = gpui::ContentMask {
            bounds: Bounds::from_corners(
                point(bounds.left() + gutter_width, bounds.top()),
                point(bounds.right(), bounds.bottom()),
            ),
        };
        window.with_content_mask(Some(content_mask), |window| {
            for quad in prepaint.selections.drain(..) {
                window.paint_quad(quad);
            }
            for row in &prepaint.rows {
                let _ = row.shaped.paint(
                    point(prepaint.content_left, row.top),
                    line_height,
                    gpui::TextAlign::Left,
                    None,
                    window,
                    cx,
                );
            }
            for row in &prepaint.wrapped_rows {
                let _ = row.wrapped.paint(
                    point(prepaint.content_left, row.top),
                    line_height,
                    gpui::TextAlign::Left,
                    None,
                    window,
                    cx,
                );
            }
            for caret in prepaint.carets.drain(..) {
                window.paint_quad(caret);
            }
        });

        for (top, shaped) in &prepaint.gutter {
            let _ = shaped.paint(
                point(prepaint.gutter_left, *top),
                line_height,
                gpui::TextAlign::Left,
                None,
                window,
                cx,
            );
        }

        let visible: Vec<VisibleLine> = prepaint
            .rows
            .drain(..)
            .map(|r| VisibleLine {
                line: r.line,
                start_char: r.start_char,
                top: r.top,
                shaped: r.shaped,
            })
            .collect();
        let wrapped_visible: Vec<WrappedVisible> = prepaint
            .wrapped_rows
            .drain(..)
            .map(|r| WrappedVisible {
                line: r.line,
                start_char: r.start_char,
                top: r.top,
                wrapped: r.wrapped,
            })
            .collect();
        self.editor.update(cx, |e, _| {
            e.visible = visible;
            e.wrapped_visible = wrapped_visible;
        });
    }
}

impl EditorCanvas {
    /// Lays out the visible region in soft-wrap mode. The viewport is anchored at
    /// `wrap_top_line` + `wrap_top_off`, so layout cost is O(visible rows) — no
    /// full-document wrap pass, even for huge files.
    fn prepaint_wrapped(
        &mut self,
        bounds: Bounds<Pixels>,
        font: &Font,
        default_color: Hsla,
        font_size: Pixels,
        window: &mut Window,
        cx: &mut App,
    ) -> CanvasPrepaint {
        let editor = self.editor.read(cx);
        let line_height = editor.line_height;
        let lh = line_height;
        let gutter_width = editor.gutter_width;
        let show_line_numbers = editor.show_line_numbers;
        let focused = editor.focus_handle.is_focused(window);
        let buf = editor.document.buffer();
        let line_count = buf.line_count();
        let digits = line_count.to_string().len().max(3);
        let cursors = editor.document.selections().cursors();
        let syntax = &editor.syntax;

        let view_w = (bounds.size.width - gutter_width - px(14.0)).max(px(0.0));
        let content_left = bounds.left() + gutter_width;
        let gutter_left = bounds.left() + px(4.0);

        // Normalize the anchor so `off` lands within `top_line`'s block; only
        // measures lines adjacent to the viewport.
        let mut top_line = editor.wrap_top_line.min(line_count.saturating_sub(1));
        let mut off = editor.wrap_top_off;
        loop {
            if off < px(0.0) {
                if top_line == 0 {
                    off = px(0.0);
                    break;
                }
                top_line -= 1;
                let rows = measure_rows(window, &buf.line_text(top_line), font, font_size, view_w);
                off += lh * rows as f32;
                continue;
            }
            let rows = measure_rows(window, &buf.line_text(top_line), font, font_size, view_w);
            let block = lh * rows as f32;
            if off >= block {
                if top_line + 1 >= line_count {
                    off = (block - lh).max(px(0.0));
                    break;
                }
                off -= block;
                top_line += 1;
                continue;
            }
            break;
        }

        let mut wrapped_rows: Vec<WrappedRow> = Vec::new();
        let mut gutter: Vec<(Pixels, ShapedLine)> = Vec::new();
        let mut selections: Vec<PaintQuad> = Vec::new();
        let mut carets: Vec<PaintQuad> = Vec::new();
        let right = content_left + view_w;
        let highlight_word = occurrence_word(&editor.document);

        let mut y = bounds.top() - off;
        let mut line = top_line;
        let mut bottom_line = top_line;
        while y < bounds.bottom() && line < line_count {
            let line_start_char = buf.position_to_char(Position::new(line, 0));
            let line_text = buf.line_text(line);
            let line_char_len = buf.line_len_chars(line);
            let line_end_char = line_start_char + line_char_len;
            let line_start_byte = buf.char_to_byte(line_start_char);

            let runs = build_runs(syntax, buf, &line_text, line_start_byte, font, default_color);
            let Some(wrapped) = shape_one_wrapped(window, &line_text, &runs, font_size, view_w)
            else {
                line += 1;
                continue;
            };
            let rows = wrap_rows(&wrapped);
            let block = lh * rows as f32;

            if let Some((word, sel_range)) = &highlight_word {
                for (scol, ecol) in word_occurrences(&line_text, word) {
                    let abs_s = line_start_char + scol;
                    let abs_e = line_start_char + ecol;
                    if abs_s == sel_range.start && abs_e == sel_range.end {
                        continue;
                    }
                    let s_byte = char_to_byte(&line_text, scol);
                    let e_byte = char_to_byte(&line_text, ecol);
                    if let (Some(p0), Some(p1)) = (
                        wrapped.position_for_index(s_byte, lh),
                        wrapped.position_for_index(e_byte, lh),
                    ) {
                        if (f32::from(p0.y) - f32::from(p1.y)).abs() < 0.5 {
                            selections.push(fill(
                                Bounds::from_corners(
                                    point(content_left + p0.x, y + p0.y),
                                    point(content_left + p1.x, y + p0.y + lh),
                                ),
                                rgb(0x4c4a2f),
                            ));
                        }
                    }
                }
            }

            for cur in cursors {
                let range = cur.range();
                if !cur.is_empty() && range.start <= line_end_char && range.end > line_start_char {
                    let s_col = range.start.max(line_start_char) - line_start_char;
                    let e_col = range.end.min(line_end_char) - line_start_char;
                    let s_byte = char_to_byte(&line_text, s_col);
                    let e_byte = char_to_byte(&line_text, e_col);
                    let p0 = wrapped
                        .position_for_index(s_byte, lh)
                        .unwrap_or(point(px(0.0), px(0.0)));
                    let p1 = wrapped
                        .position_for_index(e_byte, lh)
                        .unwrap_or(point(view_w, lh * (rows.saturating_sub(1)) as f32));
                    let band = |x0: Pixels, x1: Pixels, top: Pixels| {
                        fill(
                            Bounds::from_corners(point(x0, top), point(x1, top + lh)),
                            rgb(0x264f78),
                        )
                    };
                    let row0 = (f32::from(p0.y) / f32::from(lh)).round() as i32;
                    let row1 = (f32::from(p1.y) / f32::from(lh)).round() as i32;
                    if row0 == row1 {
                        selections.push(band(content_left + p0.x, content_left + p1.x, y + p0.y));
                    } else {
                        selections.push(band(content_left + p0.x, right, y + p0.y));
                        for r in (row0 + 1)..row1 {
                            selections.push(band(content_left, right, y + lh * r as f32));
                        }
                        selections.push(band(content_left, content_left + p1.x, y + p1.y));
                    }
                }
                if focused && cur.head >= line_start_char && cur.head <= line_end_char {
                    let col = cur.head - line_start_char;
                    let b = char_to_byte(&line_text, col);
                    if let Some(p) = wrapped.position_for_index(b, lh) {
                        carets.push(fill(
                            Bounds::new(
                                point(content_left + p.x, y + p.y),
                                gpui::size(px(2.0), lh),
                            ),
                            rgb(0xaeafad),
                        ));
                    }
                }
            }

            if show_line_numbers {
                let num = format!("{:>width$} ", line + 1, width = digits);
                let grun = TextRun {
                    len: num.len(),
                    font: font.clone(),
                    color: rgb(0x6e7681).into(),
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                };
                let gshaped =
                    window
                        .text_system()
                        .shape_line(SharedString::from(num), font_size, &[grun], None);
                gutter.push((y, gshaped));
            }

            wrapped_rows.push(WrappedRow {
                line,
                start_char: line_start_char,
                top: y,
                wrapped,
            });
            bottom_line = line;
            y += block;
            line += 1;
        }

        let _ = (buf, &editor);
        self.editor.update(cx, |e, _| {
            e.wrap_top_line = top_line;
            e.wrap_top_off = off;
            e.wrap_bottom_line = bottom_line;
            e.content_width = px(0.0);
            e.scroll_x = px(0.0);
            e.reveal_caret = false;
        });

        CanvasPrepaint {
            rows: Vec::new(),
            wrapped_rows,
            gutter,
            selections,
            carets,
            content_left,
            gutter_left,
        }
    }
}

/// Shapes `text` wrapped to `width`, returning its single logical [`WrappedLine`].
fn shape_one_wrapped(
    window: &mut Window,
    text: &str,
    runs: &[TextRun],
    font_size: Pixels,
    width: Pixels,
) -> Option<WrappedLine> {
    window
        .text_system()
        .shape_text(
            SharedString::from(text.to_string()),
            font_size,
            runs,
            Some(width),
            None,
        )
        .ok()?
        .into_iter()
        .next()
}

/// Visual-row count of `text` wrapped to `width` (single-run; matches layout).
fn measure_rows(
    window: &mut Window,
    text: &str,
    font: &Font,
    font_size: Pixels,
    width: Pixels,
) -> usize {
    if width <= px(0.0) || text.is_empty() {
        return 1;
    }
    let run = TextRun {
        len: text.len(),
        font: font.clone(),
        color: rgb(0xffffff).into(),
        background_color: None,
        underline: None,
        strikethrough: None,
    };
    shape_one_wrapped(window, text, &[run], font_size, width)
        .map(|w| wrap_rows(&w))
        .unwrap_or(1)
}

/// Builds colored [`TextRun`]s for one line from the syntax highlights.
fn build_runs(
    syntax: &SyntaxState,
    buffer: &cyberfiles_text_engine::TextBuffer,
    line_text: &str,
    line_start_byte: usize,
    font: &Font,
    default_color: Hsla,
) -> Vec<TextRun> {
    let len = line_text.len();
    let mut runs: Vec<TextRun> = Vec::new();
    let mut pos = 0usize;

    let mk = |len: usize, color: Hsla| TextRun {
        len,
        font: font.clone(),
        color,
        background_color: None,
        underline: None,
        strikethrough: None,
    };

    for span in syntax.highlights_rope(buffer.rope(), line_start_byte..line_start_byte + len) {
        let start = span.start.saturating_sub(line_start_byte).max(pos).min(len);
        let end = span.end.saturating_sub(line_start_byte).min(len);
        if end <= start {
            continue;
        }
        if start > pos {
            runs.push(mk(start - pos, default_color));
        }
        runs.push(mk(end - start, kind_color(span.kind)));
        pos = end;
    }
    if pos < len {
        runs.push(mk(len - pos, default_color));
    }
    if runs.is_empty() && len > 0 {
        runs.push(mk(len, default_color));
    }
    runs
}

fn char_to_byte(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(b, _)| b)
        .unwrap_or(s.len())
}

/// Char-column ranges of whole-word, case-sensitive matches of `needle` in
/// `line_text` (used for same-word occurrence highlighting). O(line length).
fn word_occurrences(line_text: &str, needle: &str) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    if needle.is_empty() || needle.len() > line_text.len() {
        return out;
    }
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    let mut from = 0usize;
    while let Some(rel) = line_text[from..].find(needle) {
        let bstart = from + rel;
        let bend = bstart + needle.len();
        let before_ok = line_text[..bstart]
            .chars()
            .next_back()
            .map_or(true, |c| !is_word(c));
        let after_ok = line_text[bend..].chars().next().map_or(true, |c| !is_word(c));
        if before_ok && after_ok {
            let scol = line_text[..bstart].chars().count();
            let ecol = line_text[..bend].chars().count();
            out.push((scol, ecol));
        }
        from = bend.max(bstart + 1);
    }
    out
}

/// The selected text to highlight occurrences of: a single non-empty,
/// single-line, word-like selection. Returns `(text, char_range)`.
fn occurrence_word(document: &Document) -> Option<(String, Range<usize>)> {
    let sels = document.selections();
    if sels.len() != 1 {
        return None;
    }
    let p = sels.primary();
    if p.is_empty() {
        return None;
    }
    let text = document.buffer().slice_text(p.range());
    let count = text.chars().count();
    if count == 0 || count > 100 || !text.chars().all(|c| c.is_alphanumeric() || c == '_') {
        return None;
    }
    Some((text, p.range()))
}

fn comment_prefix(language: &str) -> &'static str {
    match language {
        "python" | "bash" => "#",
        _ => "//",
    }
}

fn kind_color(kind: HighlightKind) -> Hsla {
    let rgb_value = match kind {
        HighlightKind::Keyword => 0x569cd6,
        HighlightKind::Function => 0xdcdcaa,
        HighlightKind::Type => 0x4ec9b0,
        HighlightKind::String => 0xce9178,
        HighlightKind::Number => 0xb5cea8,
        HighlightKind::Comment => 0x6a9955,
        HighlightKind::Constant => 0x569cd6,
        HighlightKind::Variable => 0x9cdcfe,
        HighlightKind::Property => 0x9cdcfe,
        HighlightKind::Operator => 0xd4d4d4,
        HighlightKind::Punctuation => 0xd4d4d4,
        HighlightKind::Tag => 0x569cd6,
        HighlightKind::Attribute => 0x9cdcfe,
        HighlightKind::Label => 0xc8c8c8,
        HighlightKind::Constructor => 0x4ec9b0,
        HighlightKind::Other => 0xd4d4d4,
    };
    rgb(rgb_value).into()
}