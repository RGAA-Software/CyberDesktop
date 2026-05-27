use std::{borrow::Cow, cell::RefCell, ops::Range, rc::Rc};

use cyber_editor_engine::{self as engine, create_editor, editor_text, focus_editor_deferred, render_editor};
use editor::{
    actions::{Indent, Outdent, ToggleComments},
    items::active_match_index, Editor, MultiBufferOffset, SelectionEffects,
};
use futures_lite::future::block_on;
use gpui::{App, AppContext, Context, Entity, Window};
use language::{language_settings::SoftWrap, BufferSnapshot};
use multi_buffer::{Anchor, MBTextSummary, MultiBufferSnapshot};
use project::search::SearchQuery;

use super::{EditorBufferModel, SearchMatch};
use gpui_component::input::Position;
use text::{Point, TextSummary};
use util::paths::PathMatcher;
use workspace::searchable::Direction;

#[derive(Clone)]
pub(crate) struct ZedEditorBackend {
    editor: Entity<Editor>,
    buffer: Rc<RefCell<EditorBufferModel>>,
    search_cache: Rc<RefCell<Option<SearchCache>>>,
    revision: u64,
}

#[derive(Clone)]
struct SearchCache {
    query: String,
    revision: u64,
    matches: Vec<Range<usize>>,
}

impl ZedEditorBackend {
    pub(crate) fn editor_entity(&self) -> &Entity<Editor> {
        &self.editor
    }

    pub(crate) fn new<T>(
        window: &mut Window,
        cx: &mut Context<T>,
        language: gpui::SharedString,
        initial_text: String,
        line_numbers: bool,
        soft_wrap: bool,
    ) -> Self {
        let editor = create_editor(initial_text.clone(), window, cx);
        editor.update(cx, |editor, cx| {
            editor.hide_minimap_by_default(window, cx);
            editor.set_show_gutter(line_numbers, cx);
            editor.set_show_line_numbers(line_numbers, cx);
            editor.set_show_git_diff_gutter(false, cx);
            editor.set_show_code_actions(false, cx);
            editor.set_show_runnables(false, cx);
            editor.set_show_wrap_guides(false, cx);
            editor.set_show_indent_guides(true, cx);
            editor.set_soft_wrap_mode(
                if soft_wrap {
                    SoftWrap::EditorWidth
                } else {
                    SoftWrap::None
                },
                cx,
            );
        });
        let buffer = Rc::new(RefCell::new(EditorBufferModel::new(initial_text, language)));
        Self {
            editor,
            buffer,
            search_cache: Rc::new(RefCell::new(None)),
            revision: 0,
        }
    }

    pub(crate) fn focus_deferred<T>(&self, window: &mut Window, cx: &mut Context<T>) {
        focus_editor_deferred(&self.editor, window, cx);
    }

    pub(crate) fn text(&self, cx: &App) -> String {
        editor_text(&self.editor, cx)
    }

    pub(crate) fn is_dirty(&self, cx: &App) -> bool {
        self.editor.read(cx).buffer().read(cx).is_dirty(cx)
    }

    pub(crate) fn set_document(
        &self,
        text: String,
        language: gpui::SharedString,
        window: &mut Window,
        cx: &mut (impl AppContext + std::borrow::BorrowMut<App>),
    ) {
        self.buffer
            .borrow_mut()
            .set_document(text.clone(), language);
        self.search_cache.borrow_mut().take();
        engine::set_editor_text(&self.editor, &text, window, cx);
    }

    pub(crate) fn set_highlighter<T>(
        &self,
        language: gpui::SharedString,
        cx: &mut Context<T>,
    ) {
        self.buffer.borrow_mut().set_language(language);
        let _ = cx;
    }

    pub(crate) fn set_line_numbers<T>(
        &self,
        line_numbers: bool,
        _window: &mut Window,
        cx: &mut Context<T>,
    ) {
        self.editor.update(cx, |editor, cx| {
            editor.set_show_gutter(line_numbers, cx);
            editor.set_show_line_numbers(line_numbers, cx);
        });
    }

    pub(crate) fn set_soft_wrap<T>(
        &self,
        soft_wrap: bool,
        _window: &mut Window,
        cx: &mut Context<T>,
    ) {
        self.editor.update(cx, |editor, cx| {
            editor.set_soft_wrap_mode(
                if soft_wrap {
                    SoftWrap::EditorWidth
                } else {
                    SoftWrap::None
                },
                cx,
            );
        });
    }

    pub(crate) fn render<T>(&self, cx: &mut Context<T>) -> impl gpui::IntoElement {
        render_editor(&self.editor, cx)
    }


    pub(crate) fn sync_text_change(&mut self, text: &str) -> bool {
        let changed = self.buffer.borrow_mut().sync_text(text);
        if changed {
            self.revision = self.revision.wrapping_add(1);
            self.search_cache.borrow_mut().take();
        }
        changed
    }

    pub(crate) fn sync_cursor_position(
        &mut self,
        _cursor: gpui_component::input::Position,
    ) -> bool {
        false
    }

    pub(crate) fn sync_selection(
        &mut self,
        _selected_range: std::ops::Range<usize>,
        _selected_char_count: usize,
    ) -> bool {
        false
    }

    pub(crate) fn set_cursor_position(
        &self,
        cursor: gpui_component::input::Position,
        window: &mut Window,
        cx: &mut (impl AppContext + std::borrow::BorrowMut<App>),
    ) {
        // Update lightweight model immediately so string-based commands work right away.
        self.buffer.borrow_mut().sync_cursor(cursor);
        self.buffer.borrow_mut().sync_selection(0..0, 0);

        self.editor.update(cx, |editor, cx| {
            let snapshot = editor.buffer().read(cx).snapshot(cx);
            let start_byte = snapshot
                .point_to_offset(position_to_point(cursor))
                .0;
            editor.change_selections(SelectionEffects::no_scroll(), window, cx, |s| {
                s.select_ranges([MultiBufferOffset(start_byte)..MultiBufferOffset(start_byte)]);
            });
            cx.notify();
        });
    }

    pub(crate) fn line_count(&self) -> usize {
        self.buffer.borrow().line_count()
    }

    pub(crate) fn char_count(&self) -> usize {
        self.buffer.borrow().char_count()
    }

    pub(crate) fn revision(&self) -> u64 {
        self.revision
    }

    pub(crate) fn cursor_position(&self) -> gpui_component::input::Position {
        self.buffer.borrow().cursor()
    }

    pub(crate) fn selected_char_count(&self) -> usize {
        self.buffer.borrow().selected_char_count()
    }

    pub(crate) fn has_selection(&self) -> bool {
        self.buffer.borrow().has_selection()
    }

    pub(crate) fn sync_cursor_selection_from_editor<T>(&self, cx: &mut Context<T>) {
        let buffer = self.buffer.clone();
        self.editor.update(cx, |editor, cx| {
            let display_snapshot = editor.display_snapshot(cx);
            let selection = editor.selections.newest::<MultiBufferOffset>(&display_snapshot);
            let buffer_snapshot = editor.buffer().read(cx).snapshot(cx);

            let selected_start = selection.start.0.min(selection.end.0);
            let selected_end = selection.start.0.max(selection.end.0);
            let cursor_byte = selection.head().0;
            let cursor = point_to_position(buffer_snapshot.offset_to_point(MultiBufferOffset(cursor_byte)));
            let selected_char_count = if selected_start == selected_end {
                0
            } else {
                buffer_snapshot
                    .text_summary_for_range::<MBTextSummary, _>(
                        MultiBufferOffset(selected_start)..MultiBufferOffset(selected_end),
                    )
                    .chars
            };

            let mut buf = buffer.borrow_mut();
            buf.sync_cursor(cursor);
            buf.sync_selection(selected_start..selected_end, selected_char_count);
        });
    }

    pub(crate) fn find_next(&self, query: &str, cx: &App) -> Option<SearchMatch> {
        self.search_match(query, Direction::Next, cx)
    }

    pub(crate) fn find_previous(&self, query: &str, cx: &App) -> Option<SearchMatch> {
        self.search_match(query, Direction::Prev, cx)
    }

    pub(crate) fn match_count(&self, query: &str, cx: &App) -> usize {
        self.search_matches(query, cx)
            .map(|matches| matches.len())
            .unwrap_or(0)
    }

    pub(crate) fn current_match_index(&self, query: &str, cx: &App) -> usize {
        let snapshot = self.editor.read(cx).buffer().read(cx).snapshot(cx);
        let Some(matches) = self.search_matches(query, cx) else {
            return 0;
        };
        if matches.is_empty() {
            return 0;
        }

        let anchor_ranges = anchor_ranges_for_matches(&snapshot, &matches);
        let cursor = self.cursor_anchor(&snapshot);
        active_match_index(Direction::Next, &anchor_ranges, &cursor, &snapshot)
            .map(|index| index + 1)
            .unwrap_or(0)
    }

    pub(crate) fn select_match(
        &self,
        search_match: SearchMatch,
        window: &mut Window,
        cx: &mut App,
    ) {
        // Keep our lightweight model in sync so string-based commands work immediately.
        self.buffer
            .borrow_mut()
            .sync_cursor(search_match.start);

        self.editor.update(cx, |editor, cx| {
            let snapshot = editor.buffer().read(cx).snapshot(cx);
            let start_byte = snapshot
                .point_to_offset(position_to_point(search_match.start))
                .0;
            let end_byte = snapshot
                .as_singleton()
                .map(|buffer| advance_by_chars_snapshot(buffer, start_byte, search_match.char_len))
                .unwrap_or(start_byte);
            self.buffer.borrow_mut().sync_selection(
                start_byte..end_byte,
                search_match.char_len as usize,
            );
            editor.change_selections(SelectionEffects::no_scroll(), window, cx, |s| {
                s.select_ranges([MultiBufferOffset(start_byte)..MultiBufferOffset(end_byte)]);
            });
            cx.notify();
        });
    }

    pub(crate) fn apply_edit(
        &self,
        edit_range: Range<usize>,
        replacement: String,
        selection_after: Range<usize>,
        window: &mut Window,
        cx: &mut (impl AppContext + std::borrow::BorrowMut<App>),
    ) {
        self.editor.update(cx, |editor, cx| {
            editor.transact(window, cx, |editor, window, cx| {
                editor.edit(
                    [(
                        MultiBufferOffset(edit_range.start)..MultiBufferOffset(edit_range.end),
                        replacement.clone(),
                    )],
                    cx,
                );
                editor.change_selections(SelectionEffects::no_scroll(), window, cx, |s| {
                    s.select_ranges([MultiBufferOffset(selection_after.start)
                        ..MultiBufferOffset(selection_after.end)]);
                });
            });
            cx.notify();
        });
    }

    pub(crate) fn replace_next<T>(
        &self,
        query: &str,
        replacement: &str,
        window: &mut Window,
        cx: &mut Context<T>,
    ) -> Option<()> {
        let app: &App = &*cx;
        let snapshot = self.editor.read(app).buffer().read(app).snapshot(app);
        let matches = self.search_matches(query, app)?;
        let match_range = self.match_range_for_direction(&snapshot, &matches, Direction::Next)?;
        let search_query = build_search_query(query)
            .ok()?
            .with_replacement(replacement.to_string());
        let matched_text = text_for_range(snapshot.as_singleton()?, match_range.clone());
        let replacement_text = search_query.replacement_for(&matched_text)?.into_owned();
        let selection_after = match_range.start..match_range.start + replacement_text.len();
        self.apply_edit(match_range, replacement_text, selection_after, window, cx);
        Some(())
    }

    pub(crate) fn replace_all<T>(
        &self,
        query: &str,
        replacement: &str,
        window: &mut Window,
        cx: &mut Context<T>,
    ) -> Option<usize> {
        if query.is_empty() {
            return None;
        }

        self.editor.update(cx, |editor, cx| {
            let snapshot = editor.buffer().read(cx).snapshot(cx);
            let buffer = snapshot.as_singleton()?;
            let search_query = build_search_query(query).ok()?.with_replacement(replacement.to_string());
            let edits = build_replace_all_edits(buffer, &search_query);
            let replacement_count = edits.len();
            if replacement_count == 0 {
                return None;
            }

            let first_match_start = edits.first().map(|(range, _)| range.start)?;
            let selection_after = (!replacement.is_empty())
                .then_some(first_match_start..first_match_start + replacement.len());

            editor.transact(window, cx, |editor, window, cx| {
                editor.edit(
                    edits.iter().map(|(range, text)| {
                        (
                            MultiBufferOffset(range.start)..MultiBufferOffset(range.end),
                            text.clone(),
                        )
                    }),
                    cx,
                );
                if let Some(selection_after) = selection_after.clone() {
                    editor.change_selections(SelectionEffects::no_scroll(), window, cx, |s| {
                        s.select_ranges([MultiBufferOffset(selection_after.start)
                            ..MultiBufferOffset(selection_after.end)]);
                    });
                }
            });
            cx.notify();
            Some(replacement_count)
        })
    }

    pub(crate) fn toggle_comments<T>(&self, window: &mut Window, cx: &mut Context<T>) {
        self.editor.update(cx, |editor, cx| {
            editor.toggle_comments(&ToggleComments::default(), window, cx);
            cx.notify();
        });
    }

    pub(crate) fn indent<T>(&self, window: &mut Window, cx: &mut Context<T>) {
        self.editor.update(cx, |editor, cx| {
            editor.indent(&Indent, window, cx);
            cx.notify();
        });
    }

    pub(crate) fn outdent<T>(&self, window: &mut Window, cx: &mut Context<T>) {
        self.editor.update(cx, |editor, cx| {
            editor.outdent(&Outdent, window, cx);
            cx.notify();
        });
    }

    fn search_match(&self, query: &str, direction: Direction, cx: &App) -> Option<SearchMatch> {
        let snapshot = self.editor.read(cx).buffer().read(cx).snapshot(cx);
        let matches = self.search_matches(query, cx)?;
        let range = self.match_range_for_direction(&snapshot, &matches, direction)?;
        Some(search_match_from_range(&snapshot, &range))
    }

    fn search_matches(&self, query: &str, cx: &App) -> Option<Vec<Range<usize>>> {
        if query.is_empty() {
            return None;
        }

        if let Some(cache) = self.search_cache.borrow().as_ref() {
            if cache.query == query && cache.revision == self.revision {
                return Some(cache.matches.clone());
            }
        }

        let snapshot = self.editor.read(cx).buffer().read(cx).snapshot(cx);
        let buffer = snapshot.as_singleton()?;
        let search_query = build_search_query(query).ok()?;
        let matches = block_on(search_query.search(buffer, None));
        *self.search_cache.borrow_mut() = Some(SearchCache {
            query: query.to_string(),
            revision: self.revision,
            matches: matches.clone(),
        });
        Some(matches)
    }

    fn cursor_anchor(&self, snapshot: &MultiBufferSnapshot) -> Anchor {
        let cursor = self.buffer.borrow().cursor();
        snapshot.anchor_after(MultiBufferOffset(
            snapshot.point_to_offset(position_to_point(cursor)).0,
        ))
    }

    fn match_range_for_direction(
        &self,
        snapshot: &MultiBufferSnapshot,
        matches: &[Range<usize>],
        direction: Direction,
    ) -> Option<Range<usize>> {
        let anchor_ranges = anchor_ranges_for_matches(snapshot, matches);
        let cursor = self.cursor_anchor(snapshot);
        let index = match direction {
            Direction::Next => anchor_ranges
                .iter()
                .position(|range| range.start.cmp(&cursor, snapshot).is_gt())
                .unwrap_or(0),
            Direction::Prev => anchor_ranges
                .iter()
                .rposition(|range| range.end.cmp(&cursor, snapshot).is_lt())
                .unwrap_or(anchor_ranges.len().saturating_sub(1)),
        };
        matches.get(index).cloned()
    }
}

fn position_to_point(position: Position) -> Point {
    Point::new(position.line, position.character)
}

fn point_to_position(point: Point) -> Position {
    Position::new(point.row, point.column)
}

fn advance_by_chars_snapshot(snapshot: &BufferSnapshot, start_byte: usize, char_len: u32) -> usize {
    if char_len == 0 {
        return start_byte;
    }

    let mut count = 0u32;
    let mut last_end = start_byte;
    let mut chunk_start = start_byte;
    for chunk in snapshot.text_for_range(start_byte..snapshot.len()) {
        for (offset, ch) in chunk.char_indices() {
            if count >= char_len {
                break;
            }
            count += 1;
            last_end = chunk_start + offset + ch.len_utf8();
            if count == char_len {
                break;
            }
        }
        if count >= char_len {
            break;
        }
        chunk_start += chunk.len();
    }

    if count < char_len {
        snapshot.len()
    } else {
        last_end
    }
}

fn build_replace_all_edits(
    buffer: &BufferSnapshot,
    query: &SearchQuery,
) -> Vec<(Range<usize>, String)> {
    let num_cpus = std::thread::available_parallelism()
        .map(|parallelism| parallelism.get() as u32)
        .unwrap_or(1);
    let mut edits = Vec::new();
    for search_range in chunk_search_range(buffer, query, num_cpus, 0..buffer.len()) {
        for relative_range in block_on(query.search(buffer, Some(search_range.clone()))) {
            let absolute_range =
                search_range.start + relative_range.start..search_range.start + relative_range.end;
            let matched_text = text_for_range(buffer, absolute_range.clone());
            if let Some(replacement) = query.replacement_for(&matched_text) {
                edits.push((absolute_range, replacement.into_owned()));
            }
        }
    }
    edits
}

fn chunk_search_range<'a>(
    buffer: &'a BufferSnapshot,
    query: &'a SearchQuery,
    num_cpus: u32,
    range: Range<usize>,
) -> Box<dyn Iterator<Item = Range<usize>> + 'a> {
    if range.is_empty() {
        return Box::new(std::iter::empty());
    }

    let summary = buffer.text_summary_for_range::<TextSummary, _>(range.clone());
    let num_chunks = if !query.is_regex() && !query.as_str().contains('\n') {
        std::num::NonZeroU32::new(summary.lines.row.saturating_add(1).min(num_cpus.max(1)))
    } else {
        std::num::NonZeroU32::new(1)
    };

    let Some(num_chunks) = num_chunks else {
        return Box::new(std::iter::empty());
    };

    let mut chunk_start = range.start;
    let rope = buffer.as_rope().clone();
    let range_end = range.end;
    let average_chunk_length = summary.len.div_ceil(num_chunks.get() as usize);
    Box::new(std::iter::from_fn(move || {
        if chunk_start >= range_end {
            return None;
        }

        let candidate_position = chunk_start + average_chunk_length;
        let adjusted = rope.ceil_char_boundary(candidate_position);
        let mut as_point = rope.offset_to_point(adjusted);
        as_point.row += 1;
        as_point.column = 0;
        let end_offset = buffer.point_to_offset(as_point).min(range_end);
        let next = chunk_start..end_offset;
        chunk_start = end_offset;
        Some(next)
    }))
}

fn build_search_query(query: &str) -> anyhow::Result<SearchQuery> {
    SearchQuery::text(
        query,
        false,
        true,
        false,
        PathMatcher::default(),
        PathMatcher::default(),
        false,
        None,
    )
}

fn search_match_from_range(snapshot: &MultiBufferSnapshot, range: &Range<usize>) -> SearchMatch {
    SearchMatch {
        start: point_to_position(snapshot.offset_to_point(MultiBufferOffset(range.start))),
        char_len: snapshot
            .text_summary_for_range::<MBTextSummary, _>(
                MultiBufferOffset(range.start)..MultiBufferOffset(range.end),
            )
            .chars as u32,
    }
}

fn anchor_ranges_for_matches(
    snapshot: &MultiBufferSnapshot,
    matches: &[Range<usize>],
) -> Vec<Range<Anchor>> {
    matches
        .iter()
        .map(|range| {
            snapshot.anchor_after(MultiBufferOffset(range.start))
                ..snapshot.anchor_before(MultiBufferOffset(range.end))
        })
        .collect()
}

fn text_for_range(buffer: &BufferSnapshot, range: Range<usize>) -> Cow<'_, str> {
    let chunks = buffer.text_for_range(range).collect::<Vec<_>>();
    if chunks.len() == 1 {
        chunks.first().copied().unwrap_or_default().into()
    } else {
        chunks.join("").into()
    }
}
