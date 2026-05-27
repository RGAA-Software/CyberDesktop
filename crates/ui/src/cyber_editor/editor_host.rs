use std::path::Path;

use gpui::{App, AppContext, Context, Entity, Focusable, Window};
use gpui_component::input::Position;

use super::SearchMatch;

#[cfg(not(feature = "zed-engine"))]
use gpui_component::input::InputState;

#[cfg(not(feature = "zed-engine"))]
use super::ModelEditorBackend;

#[cfg(feature = "zed-engine")]
use super::ZedEditorBackend;

#[cfg(feature = "zed-engine")]
use editor::Editor;

#[cfg(feature = "zed-engine")]
#[derive(Clone)]
enum EditorBackendInner {
    Zed(ZedEditorBackend),
}

#[cfg(not(feature = "zed-engine"))]
#[derive(Clone)]
enum EditorBackendInner {
    Model(ModelEditorBackend),
}

/// Editor host boundary: `InputState` backend or vendored Zed [`editor::Editor`].
#[derive(Clone)]
pub(crate) struct EditorHost {
    backend: EditorBackendInner,
}

impl EditorHost {
    pub(crate) fn new<T>(
        window: &mut Window,
        cx: &mut Context<T>,
        language: gpui::SharedString,
        file_path: Option<&Path>,
        initial_text: String,
        line_numbers: bool,
        soft_wrap: bool,
    ) -> Self {
        #[cfg(feature = "zed-engine")]
        {
            Self {
                backend: EditorBackendInner::Zed(ZedEditorBackend::new(
                    window,
                    cx,
                    language,
                    file_path,
                    initial_text,
                    line_numbers,
                    soft_wrap,
                )),
            }
        }

        #[cfg(not(feature = "zed-engine"))]
        {
            let _ = file_path;
            Self {
                backend: EditorBackendInner::Model(ModelEditorBackend::new(
                    window,
                    cx,
                    language,
                    initial_text,
                    line_numbers,
                    soft_wrap,
                )),
            }
        }
    }

    #[cfg(not(feature = "zed-engine"))]
    pub(crate) fn input_entity(&self) -> &Entity<InputState> {
        match &self.backend {
            EditorBackendInner::Model(b) => b.input_entity(),
        }
    }

    #[cfg(feature = "zed-engine")]
    pub(crate) fn zed_entity(&self) -> &Entity<Editor> {
        match &self.backend {
            EditorBackendInner::Zed(b) => b.editor_entity(),
        }
    }

    #[cfg(feature = "zed-engine")]
    pub(crate) fn indent_label(&self, cx: &App) -> String {
        match &self.backend {
            EditorBackendInner::Zed(b) => b.indent_label(cx),
        }
    }

    #[cfg(feature = "zed-engine")]
    pub(crate) fn suspend_for_native_modal<C: gpui::AppContext>(&self, cx: &mut C) {
        match &self.backend {
            EditorBackendInner::Zed(b) => b.suspend_for_native_modal(cx),
        }
    }

    #[cfg(feature = "zed-engine")]
    pub(crate) fn resume_after_native_modal<C: gpui::AppContext>(
        &self,
        window: &mut Window,
        cx: &mut C,
    ) {
        match &self.backend {
            EditorBackendInner::Zed(b) => b.resume_after_native_modal(window, cx),
        }
    }

    pub(crate) fn focus_handle(&self, cx: &App) -> gpui::FocusHandle {
        #[cfg(feature = "zed-engine")]
        {
            return self
                .zed_entity()
                .read(cx)
                .focus_handle(cx);
        }
        #[cfg(not(feature = "zed-engine"))]
        {
            self.input_entity().read(cx).focus_handle(cx)
        }
    }

    pub(crate) fn focus_deferred<T>(&self, window: &mut Window, cx: &mut Context<T>) {
        match &self.backend {
            #[cfg(not(feature = "zed-engine"))]
            EditorBackendInner::Model(b) => b.focus_deferred(window, cx),
            #[cfg(feature = "zed-engine")]
            EditorBackendInner::Zed(b) => b.focus_deferred(window, cx),
        }
    }

    pub(crate) fn text(&self, cx: &App) -> String {
        match &self.backend {
            #[cfg(not(feature = "zed-engine"))]
            EditorBackendInner::Model(b) => b.text(cx),
            #[cfg(feature = "zed-engine")]
            EditorBackendInner::Zed(b) => b.text(cx),
        }
    }

    #[cfg(feature = "zed-engine")]
    pub(crate) fn is_dirty(&self, cx: &App) -> bool {
        match &self.backend {
            EditorBackendInner::Zed(b) => b.is_dirty(cx),
        }
    }

    pub(crate) fn set_document(
        &self,
        text: String,
        language: gpui::SharedString,
        file_path: Option<&Path>,
        window: &mut Window,
        cx: &mut (impl AppContext + std::borrow::BorrowMut<App>),
    ) {
        match &self.backend {
            #[cfg(not(feature = "zed-engine"))]
            EditorBackendInner::Model(b) => {
                let _ = file_path;
                b.set_document(text, language, window, cx)
            }
            #[cfg(feature = "zed-engine")]
            EditorBackendInner::Zed(b) => b.set_document(text, language, file_path, window, cx),
        }
    }

    pub(crate) fn set_highlighter<T>(
        &self,
        language: gpui::SharedString,
        file_path: Option<&Path>,
        cx: &mut Context<T>,
    ) {
        match &self.backend {
            #[cfg(not(feature = "zed-engine"))]
            EditorBackendInner::Model(b) => {
                let _ = file_path;
                b.set_highlighter(language, cx)
            }
            #[cfg(feature = "zed-engine")]
            EditorBackendInner::Zed(b) => b.set_highlighter(language, file_path, cx),
        }
    }

    #[cfg(feature = "zed-engine")]
    pub(crate) fn mark_saved<T>(&self, cx: &mut Context<T>) {
        match &self.backend {
            EditorBackendInner::Zed(b) => b.mark_saved(cx),
        }
    }

    pub(crate) fn set_line_numbers<T>(
        &self,
        line_numbers: bool,
        window: &mut Window,
        cx: &mut Context<T>,
    ) {
        match &self.backend {
            #[cfg(not(feature = "zed-engine"))]
            EditorBackendInner::Model(b) => b.set_line_numbers(line_numbers, window, cx),
            #[cfg(feature = "zed-engine")]
            EditorBackendInner::Zed(b) => b.set_line_numbers(line_numbers, window, cx),
        }
    }

    pub(crate) fn set_soft_wrap<T>(
        &self,
        soft_wrap: bool,
        window: &mut Window,
        cx: &mut Context<T>,
    ) {
        match &self.backend {
            #[cfg(not(feature = "zed-engine"))]
            EditorBackendInner::Model(b) => b.set_soft_wrap(soft_wrap, window, cx),
            #[cfg(feature = "zed-engine")]
            EditorBackendInner::Zed(b) => b.set_soft_wrap(soft_wrap, window, cx),
        }
    }

    pub(crate) fn render<T>(&self, cx: &mut Context<T>) -> impl gpui::IntoElement {
        match &self.backend {
            #[cfg(not(feature = "zed-engine"))]
            EditorBackendInner::Model(b) => b.render(cx),
            #[cfg(feature = "zed-engine")]
            EditorBackendInner::Zed(b) => b.render(cx),
        }
    }

    pub(crate) fn sync_text_change(&mut self, text: &str) -> bool {
        match &mut self.backend {
            #[cfg(not(feature = "zed-engine"))]
            EditorBackendInner::Model(b) => b.sync_text_change(text),
            #[cfg(feature = "zed-engine")]
            EditorBackendInner::Zed(b) => b.sync_text_change(text),
        }
    }

    pub(crate) fn sync_cursor_position(&mut self, cursor: Position) -> bool {
        match &mut self.backend {
            #[cfg(not(feature = "zed-engine"))]
            EditorBackendInner::Model(b) => b.sync_cursor_position(cursor),
            #[cfg(feature = "zed-engine")]
            EditorBackendInner::Zed(b) => b.sync_cursor_position(cursor),
        }
    }

    pub(crate) fn sync_selection(
        &mut self,
        selected_range: std::ops::Range<usize>,
        selected_char_count: usize,
    ) -> bool {
        match &mut self.backend {
            #[cfg(not(feature = "zed-engine"))]
            EditorBackendInner::Model(b) => {
                b.sync_selection(selected_range, selected_char_count)
            }
            #[cfg(feature = "zed-engine")]
            EditorBackendInner::Zed(b) => b.sync_selection(selected_range, selected_char_count),
        }
    }

    pub(crate) fn set_cursor_position(
        &self,
        cursor: Position,
        window: &mut Window,
        cx: &mut (impl AppContext + std::borrow::BorrowMut<App>),
    ) {
        match &self.backend {
            #[cfg(not(feature = "zed-engine"))]
            EditorBackendInner::Model(b) => b.set_cursor_position(cursor, window, cx),
            #[cfg(feature = "zed-engine")]
            EditorBackendInner::Zed(b) => b.set_cursor_position(cursor, window, cx),
        }
    }

    #[cfg(not(feature = "zed-engine"))]
    pub(crate) fn selected_range(&self, _cx: &App) -> std::ops::Range<usize> {
        match &self.backend {
            EditorBackendInner::Model(b) => b.input_entity().read(_cx).selected_range(),
        }
    }

    pub(crate) fn line_count(&self) -> usize {
        match &self.backend {
            #[cfg(not(feature = "zed-engine"))]
            EditorBackendInner::Model(b) => b.line_count(),
            #[cfg(feature = "zed-engine")]
            EditorBackendInner::Zed(b) => b.line_count(),
        }
    }

    pub(crate) fn char_count(&self) -> usize {
        match &self.backend {
            #[cfg(not(feature = "zed-engine"))]
            EditorBackendInner::Model(b) => b.char_count(),
            #[cfg(feature = "zed-engine")]
            EditorBackendInner::Zed(b) => b.char_count(),
        }
    }

    pub(crate) fn revision(&self) -> u64 {
        match &self.backend {
            #[cfg(not(feature = "zed-engine"))]
            EditorBackendInner::Model(b) => b.revision(),
            #[cfg(feature = "zed-engine")]
            EditorBackendInner::Zed(b) => b.revision(),
        }
    }

    pub(crate) fn cursor_position(&self) -> Position {
        match &self.backend {
            #[cfg(not(feature = "zed-engine"))]
            EditorBackendInner::Model(b) => b.cursor_position(),
            #[cfg(feature = "zed-engine")]
            EditorBackendInner::Zed(b) => b.cursor_position(),
        }
    }

    pub(crate) fn sync_cursor_selection_from_editor<T>(&self, cx: &mut Context<T>) {
        match &self.backend {
            #[cfg(not(feature = "zed-engine"))]
            EditorBackendInner::Model(_) => {}
            #[cfg(feature = "zed-engine")]
            EditorBackendInner::Zed(b) => b.sync_cursor_selection_from_editor(cx),
        }
    }

    pub(crate) fn selected_char_count(&self) -> usize {
        match &self.backend {
            #[cfg(not(feature = "zed-engine"))]
            EditorBackendInner::Model(b) => b.selected_char_count(),
            #[cfg(feature = "zed-engine")]
            EditorBackendInner::Zed(b) => b.selected_char_count(),
        }
    }

    pub(crate) fn has_selection(&self) -> bool {
        match &self.backend {
            #[cfg(not(feature = "zed-engine"))]
            EditorBackendInner::Model(b) => b.has_selection(),
            #[cfg(feature = "zed-engine")]
            EditorBackendInner::Zed(b) => b.has_selection(),
        }
    }

    pub(crate) fn find_next(&self, query: &str, cx: &App) -> Option<SearchMatch> {
        match &self.backend {
            #[cfg(not(feature = "zed-engine"))]
            EditorBackendInner::Model(b) => b.find_next(query, cx),
            #[cfg(feature = "zed-engine")]
            EditorBackendInner::Zed(b) => b.find_next(query, cx),
        }
    }

    pub(crate) fn find_previous(&self, query: &str, cx: &App) -> Option<SearchMatch> {
        match &self.backend {
            #[cfg(not(feature = "zed-engine"))]
            EditorBackendInner::Model(b) => b.find_previous(query, cx),
            #[cfg(feature = "zed-engine")]
            EditorBackendInner::Zed(b) => b.find_previous(query, cx),
        }
    }

    pub(crate) fn match_count(&self, query: &str, cx: &App) -> usize {
        match &self.backend {
            #[cfg(not(feature = "zed-engine"))]
            EditorBackendInner::Model(b) => b.match_count(query, cx),
            #[cfg(feature = "zed-engine")]
            EditorBackendInner::Zed(b) => b.match_count(query, cx),
        }
    }

    pub(crate) fn current_match_index(&self, query: &str, cx: &App) -> usize {
        match &self.backend {
            #[cfg(not(feature = "zed-engine"))]
            EditorBackendInner::Model(b) => b.current_match_index(query, cx),
            #[cfg(feature = "zed-engine")]
            EditorBackendInner::Zed(b) => b.current_match_index(query, cx),
        }
    }

    pub(crate) fn select_match(
        &self,
        search_match: SearchMatch,
        window: &mut Window,
        cx: &mut App,
    ) {
        match &self.backend {
            #[cfg(not(feature = "zed-engine"))]
            EditorBackendInner::Model(b) => b.select_match(search_match, window, cx),
            #[cfg(feature = "zed-engine")]
            EditorBackendInner::Zed(b) => b.select_match(search_match, window, cx),
        }
    }

    #[cfg(feature = "zed-engine")]
    pub(crate) fn replace_all<T>(
        &self,
        query: &str,
        replacement: &str,
        window: &mut Window,
        cx: &mut Context<T>,
    ) -> Option<usize> {
        match &self.backend {
            EditorBackendInner::Zed(b) => b.replace_all(query, replacement, window, cx),
        }
    }

    #[cfg(feature = "zed-engine")]
    pub(crate) fn replace_next<T>(
        &self,
        query: &str,
        replacement: &str,
        window: &mut Window,
        cx: &mut Context<T>,
    ) -> Option<()> {
        match &self.backend {
            EditorBackendInner::Zed(b) => b.replace_next(query, replacement, window, cx),
        }
    }

    #[cfg(feature = "zed-engine")]
    pub(crate) fn toggle_comments<T>(&self, window: &mut Window, cx: &mut Context<T>) {
        match &self.backend {
            EditorBackendInner::Zed(b) => b.toggle_comments(window, cx),
        }
    }

    #[cfg(feature = "zed-engine")]
    pub(crate) fn indent<T>(&self, window: &mut Window, cx: &mut Context<T>) {
        match &self.backend {
            EditorBackendInner::Zed(b) => b.indent(window, cx),
        }
    }

    #[cfg(feature = "zed-engine")]
    pub(crate) fn outdent<T>(&self, window: &mut Window, cx: &mut Context<T>) {
        match &self.backend {
            EditorBackendInner::Zed(b) => b.outdent(window, cx),
        }
    }

    #[cfg(feature = "zed-engine")]
    pub(crate) fn undo<T>(&self, window: &mut Window, cx: &mut Context<T>) {
        match &self.backend {
            EditorBackendInner::Zed(b) => b.undo(window, cx),
        }
    }

    #[cfg(feature = "zed-engine")]
    pub(crate) fn redo<T>(&self, window: &mut Window, cx: &mut Context<T>) {
        match &self.backend {
            EditorBackendInner::Zed(b) => b.redo(window, cx),
        }
    }

    #[cfg(feature = "zed-engine")]
    pub(crate) fn cut<T>(&self, window: &mut Window, cx: &mut Context<T>) {
        match &self.backend {
            EditorBackendInner::Zed(b) => b.cut(window, cx),
        }
    }

    #[cfg(feature = "zed-engine")]
    pub(crate) fn copy<T>(&self, window: &mut Window, cx: &mut Context<T>) {
        match &self.backend {
            EditorBackendInner::Zed(b) => b.copy(window, cx),
        }
    }

    #[cfg(feature = "zed-engine")]
    pub(crate) fn paste<T>(&self, window: &mut Window, cx: &mut Context<T>) {
        match &self.backend {
            EditorBackendInner::Zed(b) => b.paste(window, cx),
        }
    }

    #[cfg(feature = "zed-engine")]
    pub(crate) fn select_all<T>(&self, window: &mut Window, cx: &mut Context<T>) {
        match &self.backend {
            EditorBackendInner::Zed(b) => b.select_all(window, cx),
        }
    }

}
