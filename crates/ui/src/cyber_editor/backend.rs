use std::{cell::RefCell, rc::Rc};

use gpui::{App, AppContext, Context, Entity, Focusable, Keystroke, Styled, Window};
use gpui_component::{
    input::{Input, InputState, TabSize},
    ActiveTheme as _,
};

use super::{EditorBufferModel, SearchMatch};

#[derive(Clone)]
pub(crate) struct ModelEditorBackend {
    input: Entity<InputState>,
    buffer: Rc<RefCell<EditorBufferModel>>,
}

impl ModelEditorBackend {
    pub(super) fn input_entity(&self) -> &Entity<InputState> {
        &self.input
    }

    pub(crate) fn new<T>(
        window: &mut Window,
        cx: &mut Context<T>,
        language: gpui::SharedString,
        initial_text: String,
        line_numbers: bool,
        soft_wrap: bool,
    ) -> Self {
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .code_editor(language.clone())
                .multi_line(true)
                .line_number(line_numbers)
                .folding(true)
                .indent_guides(true)
                .tab_size(TabSize {
                    tab_size: 4,
                    hard_tabs: false,
                })
                .soft_wrap(soft_wrap)
                .default_value(initial_text.clone())
                .placeholder("Open a UTF-8 text or code file")
        });

        let buffer = Rc::new(RefCell::new(EditorBufferModel::new(initial_text, language)));
        Self { input, buffer }
    }

    pub(super) fn focus_deferred<T>(&self, window: &mut Window, cx: &mut Context<T>) {
        let editor_focus = self.input.focus_handle(cx);
        window.defer(cx, move |window, cx| {
            editor_focus.focus(window, cx);
        });
    }

    pub(super) fn text(&self, cx: &App) -> String {
        self.input.read(cx).value().to_string()
    }

    pub(super) fn set_document(
        &self,
        text: String,
        language: gpui::SharedString,
        window: &mut Window,
        cx: &mut (impl AppContext + std::borrow::BorrowMut<App>),
    ) {
        self.buffer
            .borrow_mut()
            .set_document(text.clone(), language.clone());
        self.input.update(cx, |editor, cx| {
            editor.set_value(text, window, cx);
            editor.set_highlighter(language, cx);
        });
    }

    pub(super) fn set_highlighter<T>(&self, language: gpui::SharedString, cx: &mut Context<T>) {
        self.buffer.borrow_mut().set_language(language.clone());
        self.input.update(cx, |editor, cx| {
            editor.set_highlighter(language, cx);
        });
    }

    pub(super) fn set_line_numbers<T>(
        &self,
        line_numbers: bool,
        window: &mut Window,
        cx: &mut Context<T>,
    ) {
        self.input.update(cx, |editor, cx| {
            editor.set_line_number(line_numbers, window, cx);
        });
    }

    pub(super) fn set_soft_wrap<T>(
        &self,
        soft_wrap: bool,
        window: &mut Window,
        cx: &mut Context<T>,
    ) {
        self.input.update(cx, |editor, cx| {
            editor.set_soft_wrap(soft_wrap, window, cx);
        });
    }

    pub(super) fn sync_text_change(&self, text: &str) -> bool {
        self.buffer.borrow_mut().sync_text(text)
    }

    pub(super) fn sync_cursor_position(&self, cursor: gpui_component::input::Position) -> bool {
        self.buffer.borrow_mut().sync_cursor(cursor)
    }

    pub(super) fn sync_selection(
        &self,
        selected_range: std::ops::Range<usize>,
        selected_char_count: usize,
    ) -> bool {
        self.buffer
            .borrow_mut()
            .sync_selection(selected_range, selected_char_count)
    }

    pub(super) fn set_cursor_position(
        &self,
        position: gpui_component::input::Position,
        window: &mut Window,
        cx: &mut (impl AppContext + std::borrow::BorrowMut<App>),
    ) {
        self.buffer.borrow_mut().sync_cursor(position);
        self.buffer.borrow_mut().sync_selection(0..0, 0);
        self.input.update(cx, |editor, cx| {
            editor.set_cursor_position(position, window, cx);
        });
    }

    pub(super) fn line_count(&self) -> usize {
        self.buffer.borrow().line_count()
    }

    pub(super) fn char_count(&self) -> usize {
        self.buffer.borrow().char_count()
    }

    pub(super) fn revision(&self) -> u64 {
        self.buffer.borrow().revision()
    }

    pub(super) fn cursor_position(&self) -> gpui_component::input::Position {
        self.buffer.borrow().cursor()
    }

    pub(super) fn selected_char_count(&self) -> usize {
        self.buffer.borrow().selected_char_count()
    }

    pub(super) fn has_selection(&self) -> bool {
        self.buffer.borrow().has_selection()
    }

    pub(super) fn find_next(&self, query: &str, _cx: &App) -> Option<SearchMatch> {
        self.buffer.borrow().find_next(query)
    }

    pub(super) fn find_previous(&self, query: &str, _cx: &App) -> Option<SearchMatch> {
        self.buffer.borrow().find_previous(query)
    }

    pub(super) fn match_count(&self, query: &str, _cx: &App) -> usize {
        self.buffer.borrow().match_count(query)
    }

    pub(super) fn current_match_index(&self, query: &str, _cx: &App) -> usize {
        self.buffer.borrow().current_match_index(query)
    }

    pub(super) fn select_match(
        &self,
        search_match: SearchMatch,
        window: &mut Window,
        cx: &mut App,
    ) {
        self.buffer.borrow_mut().sync_cursor(search_match.start);
        self.buffer.borrow_mut().sync_selection(0..0, 0);
        self.input.update(cx, |editor, cx| {
            editor.set_cursor_position(search_match.start, window, cx);
        });
        for _ in 0..search_match.char_len {
            let _ = window.dispatch_keystroke(Keystroke::parse("shift-right").unwrap(), cx);
        }
        let editor = self.input.read(cx);
        self.buffer.borrow_mut().sync_selection(
            editor.selected_range(),
            editor.selected_value().chars().count(),
        );
    }

    pub(super) fn render<T>(&self, cx: &mut Context<T>) -> impl gpui::IntoElement {
        Input::new(&self.input)
            .bordered(false)
            .focus_bordered(false)
            .p_0()
            .h_full()
            .font_family(cx.theme().mono_font_family.clone())
            .text_size(cx.theme().mono_font_size)
    }
}
