use std::{cell::Cell, path::PathBuf, rc::Rc};

use gpui::{
    div, prelude::FluentBuilder, App, AppContext, ClickEvent, Context, Entity, FocusHandle,
    Focusable, InteractiveElement, IntoElement, ParentElement, Render, SharedString, Styled,
    Subscription, Window,
};
use gpui_component::{
    button::{Button, ButtonVariants as _},
    h_flex,
    input::{Input, InputState},
    label::Label,
    notification::Notification,
    v_flex, ActiveTheme as _, Disableable, Selectable, Sizable as _, StyledExt,
    WindowExt as _,
};

use crate::title_bar::TitleBar;

#[cfg(feature = "zed-engine")]
use editor::EditorEvent;

#[cfg(not(feature = "zed-engine"))]
use gpui_component::input::InputEvent;
#[cfg(not(feature = "zed-engine"))]
use super::line_comment_prefix;

use super::{
    display_language, display_name, display_path, load_document, EditorHost, EditorSession,
    FindNext, FindPrevious, FindText, GoToLine, IndentSelection, OpenFile, OutdentSelection,
    ReplaceAllText, ReplaceText, SaveFile, SaveFileAs, SearchMatch, ToggleComment, APP_NAME,
    EDITOR_CONTEXT,
};

pub struct CyberEditorPage {
    focus_handle: FocusHandle,
    editor: EditorHost,
    session: EditorSession,
    _subscriptions: Vec<Subscription>,
}

impl CyberEditorPage {
    pub fn view(path: Option<PathBuf>, window: &mut Window, cx: &mut App) -> Entity<Self> {
        let page = cx.new(|cx| Self::new(path, window, cx));
        let weak = page.downgrade();
        window.on_window_should_close(cx, move |window, cx| {
            weak.update(cx, |page, cx| page.request_close(window, cx))
                .unwrap_or(true)
        });
        page
    }

    pub fn new(path: Option<PathBuf>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let document = load_document(path.as_deref());
        let initial_text = document.text;
        let session = EditorSession::new(path, initial_text.clone());

        let editor = EditorHost::new(
            window,
            cx,
            session.language().clone(),
            session.file_path().map(PathBuf::as_path),
            initial_text.clone(),
            session.line_numbers(),
            session.soft_wrap(),
        );
        editor.focus_deferred(window, cx);

        let mut subscriptions = Vec::new();

        #[cfg(not(feature = "zed-engine"))]
        {
            let editor_for_observation = editor.input_entity().clone();
            let editor_for_handler = editor_for_observation.clone();
            let observe_subscription = cx.observe(&editor_for_observation, move |this, _, cx| {
                let editor_state = editor_for_handler.read(cx);
                let current_text = editor_state.value().to_string();
                let text_changed = this.editor.sync_text_change(&current_text);
                let cursor_changed =
                    this.editor.sync_cursor_position(editor_state.cursor_position());
                let selection_changed = this.editor.sync_selection(
                    editor_state.selected_range(),
                    editor_state.selected_value().chars().count(),
                );
                let dirty_changed = this.session.update_dirty_from_text(&current_text);

                if text_changed || cursor_changed || selection_changed || dirty_changed {
                    cx.notify();
                }
            });
            let enter_editor = editor.input_entity().clone();
            let enter_editor_for_handler = enter_editor.clone();
            let enter_subscription =
                cx.subscribe(&enter_editor, move |this, _, event: &InputEvent, cx| {
                    if let InputEvent::PressEnter {
                        secondary: false,
                        ..
                    } = event
                    {
                        let editor_state = enter_editor_for_handler.read(cx);
                        let current_text = editor_state.value().to_string();
                        let cursor = editor_state.cursor_position();
                        this.maybe_auto_indent_after_enter(&current_text, cursor, cx);
                    }
                });
            subscriptions.push(observe_subscription);
            subscriptions.push(enter_subscription);
        }

        #[cfg(feature = "zed-engine")]
        {
            let zed = editor.zed_entity().clone();
            editor.sync_cursor_selection_from_editor(cx);
            let event_subscription =
                cx.subscribe(&zed, move |this, _, event: &EditorEvent, cx| match event {
                    EditorEvent::Edited { .. } => {
                        let current_text = this.editor.text(cx);
                        let text_changed = this.editor.sync_text_change(&current_text);
                        let metadata_changed = this.session.refresh_metadata_from_text(&current_text);
                        if text_changed {
                            // After Zed applies Enter, text changes and cursor moves. We sync cursor
                            // from the engine and try the same lightweight auto-indent as the
                            // InputState backend.
                            this.editor.sync_cursor_selection_from_editor(cx);
                        }
                        if text_changed || metadata_changed {
                            cx.notify();
                        }
                    }
                    EditorEvent::DirtyChanged => {
                        if this.session.set_dirty(this.editor.is_dirty(cx)) {
                            cx.notify();
                        }
                    }
                    EditorEvent::Saved => {
                        if this.session.set_dirty(false) {
                            cx.notify();
                        }
                    }
                    EditorEvent::SelectionsChanged { local: true } => {
                        this.editor.sync_cursor_selection_from_editor(cx);
                        cx.notify();
                    }
                    _ => {}
                });
            subscriptions.push(event_subscription);

        }

        if let Some(error) = document.load_error {
            window.push_notification(Notification::error(error), cx);
        }

        Self {
            focus_handle: cx.focus_handle(),
            editor,
            session,
            _subscriptions: subscriptions,
        }
    }

    fn save(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.save_current(window, cx);
    }

    fn open_file(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.open_file_dialog(window, cx);
    }

    fn save_as(&mut self, _: &ClickEvent, window: &mut Window, cx: &mut Context<Self>) {
        self.open_save_as_dialog(window, cx);
    }

    fn save_current(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(path) = self.session.file_path().cloned() {
            let _ = self.write_to_path(path, window, cx);
        } else {
            self.open_save_as_dialog(window, cx);
        }
    }

    fn write_to_path(
        &mut self,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let text = self.editor.text(cx);
        std::fs::write(&path, text.as_bytes())
            .map_err(|err| format!("Failed to save {}: {err}", path.display()))?;

        self.session.apply_save(path.clone(), text);
        self.editor.set_highlighter(
            self.session.language().clone(),
            self.session.file_path().map(PathBuf::as_path),
            cx,
        );
        #[cfg(feature = "zed-engine")]
        self.editor.mark_saved(cx);
        window.push_notification(
            Notification::success(format!("Saved {}", path.display())),
            cx,
        );
        cx.notify();
        Ok(())
    }

    fn load_path_into_editor(
        &mut self,
        path: PathBuf,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        let document = load_document(Some(&path));
        if let Some(error) = document.load_error {
            return Err(error);
        }

        let text = document.text;
        self.editor.set_document(
            text.clone(),
            SharedString::from(super::language_for_path(Some(&path))),
            Some(path.as_path()),
            window,
            cx,
        );
        self.session.apply_loaded_document(path, text);
        cx.notify();
        Ok(())
    }

    fn open_file_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let default_path = self
            .session
            .file_path()
            .cloned()
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(default_path.to_string_lossy().to_string())
                .placeholder("Enter the full file path to open")
        });
        let page = cx.entity().downgrade();
        let focus_once = Rc::new(Cell::new(false));

        window.open_alert_dialog(cx, move |alert, window, cx| {
            let input_for_focus = input.clone();
            let input_for_submit = input.clone();
            let page_for_submit = page.clone();
            focus_input_once(&focus_once, input_for_focus, window, cx);

            alert
                .title("Open File")
                .description("Enter the full path of the UTF-8 text or code file to open.")
                .show_cancel(true)
                .child(Input::new(&input).w_full())
                .on_ok(move |_, window, cx| {
                    let raw = input_for_submit.read(cx).value().trim().to_string();
                    if raw.is_empty() {
                        window.push_notification(
                            Notification::warning("A file path is required."),
                            cx,
                        );
                        return false;
                    }
                    let path = PathBuf::from(raw);
                    match page_for_submit.update(cx, |page, cx| {
                        page.load_path_into_editor(path, window, cx)
                    }) {
                        Ok(Ok(())) => true,
                        Ok(Err(message)) => {
                            window.push_notification(Notification::error(message), cx);
                            false
                        }
                        Err(_) => true,
                    }
                })
        });
    }

    fn open_save_as_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let default_path = self.suggested_save_path();
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(default_path.to_string_lossy().to_string())
                .placeholder("Enter the destination file path")
        });
        let page = cx.entity().downgrade();
        let focus_once = Rc::new(Cell::new(false));

        window.open_alert_dialog(cx, move |alert, window, cx| {
            let input_for_focus = input.clone();
            let input_for_submit = input.clone();
            let page_for_submit = page.clone();
            focus_input_once(&focus_once, input_for_focus, window, cx);

            alert
                .title("Save As")
                .description("Enter the full destination path for this document.")
                .show_cancel(true)
                .child(Input::new(&input).w_full())
                .on_ok(move |_, window, cx| {
                    let raw = input_for_submit.read(cx).value().trim().to_string();
                    if raw.is_empty() {
                        window.push_notification(
                            Notification::warning("A destination path is required."),
                            cx,
                        );
                        return false;
                    }
                    let path = PathBuf::from(raw);
                    match page_for_submit.update(cx, |page, cx| {
                        page.write_to_path(path, window, cx)
                    }) {
                        Ok(Ok(())) => true,
                        Ok(Err(message)) => {
                            window.push_notification(Notification::error(message), cx);
                            false
                        }
                        Err(_) => true,
                    }
                })
        });
    }

    fn go_to_line(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let cursor = self.editor.cursor_position();
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(format!("{}:{}", cursor.line + 1, cursor.character + 1))
        });
        let page = cx.entity().downgrade();
        let focus_once = Rc::new(Cell::new(false));

        window.open_alert_dialog(cx, move |alert, window, cx| {
            let input_for_focus = input.clone();
            let input_for_submit = input.clone();
            let page_for_submit = page.clone();
            focus_input_once(&focus_once, input_for_focus, window, cx);

            alert
                .title("Go to Line")
                .description("Enter a 1-based line or line:column target.")
                .show_cancel(true)
                .child(Input::new(&input).w_full())
                .on_ok(move |_, window, cx| {
                    let raw = input_for_submit.read(cx).value().trim().to_string();
                    let Some(position) = parse_go_to_line_target(&raw) else {
                        window.push_notification(
                            Notification::warning("Enter a line number or line:column."),
                            cx,
                        );
                        return false;
                    };
                    match page_for_submit.update(cx, |page, cx| {
                        page.editor.set_cursor_position(position, window, cx);
                        cx.notify();
                    }) {
                        Ok(_) => true,
                        Err(_) => true,
                    }
                })
        });
    }

    fn open_find_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(self.session.find_query().to_string())
                .placeholder("Enter text to find")
        });
        let page = cx.entity().downgrade();
        let focus_once = Rc::new(Cell::new(false));

        window.open_alert_dialog(cx, move |alert, window, cx| {
            let input_for_focus = input.clone();
            let input_for_submit = input.clone();
            let page_for_submit = page.clone();
            focus_input_once(&focus_once, input_for_focus, window, cx);

            alert
                .title("Find")
                .description("Enter text to find from the current cursor position.")
                .show_cancel(true)
                .child(Input::new(&input).w_full())
                .on_ok(move |_, window, cx| {
                    let raw = input_for_submit.read(cx).value().trim().to_string();
                    if raw.is_empty() {
                        window.push_notification(
                            Notification::warning("Enter text to find."),
                            cx,
                        );
                        return false;
                    }
                    match page_for_submit.update(cx, |page, cx| {
                        page.find_next(&raw, window, cx)
                    }) {
                        Ok(found) => found,
                        Err(_) => true,
                    }
                })
        });
    }

    fn open_replace_dialog(
        &mut self,
        replace_all: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let find_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(self.session.find_query().to_string())
                .placeholder("Find")
        });
        let replace_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(self.session.replace_query().to_string())
                .placeholder("Replace with")
        });
        let page = cx.entity().downgrade();
        let focus_once = Rc::new(Cell::new(false));

        window.open_alert_dialog(cx, move |alert, window, cx| {
            let find_input_for_focus = find_input.clone();
            let find_input_for_submit = find_input.clone();
            let replace_input_for_submit = replace_input.clone();
            let page_for_submit = page.clone();
            focus_input_once(&focus_once, find_input_for_focus, window, cx);

            alert
                .title(if replace_all {
                    "Replace All"
                } else {
                    "Replace"
                })
                .description(if replace_all {
                    "Replace every match in the current document."
                } else {
                    "Replace the next match from the current cursor position."
                })
                .show_cancel(true)
                .child(
                    v_flex()
                        .w_full()
                        .gap_2()
                        .child(Input::new(&find_input).w_full())
                        .child(Input::new(&replace_input).w_full()),
                )
                .on_ok(move |_, window, cx| {
                    let find = find_input_for_submit.read(cx).value().trim().to_string();
                    let replace_with = replace_input_for_submit.read(cx).value().to_string();
                    if find.is_empty() {
                        window.push_notification(
                            Notification::warning("Enter text to find."),
                            cx,
                        );
                        return false;
                    }
                    match page_for_submit.update(cx, |page, cx| {
                        if replace_all {
                            page.replace_all(&find, &replace_with, window, cx)
                        } else {
                            page.replace_next(&find, &replace_with, window, cx)
                        }
                    }) {
                        Ok(replaced) => replaced,
                        Err(_) => true,
                    }
                })
        });
    }

    fn find_next(&mut self, query: &str, window: &mut Window, cx: &mut Context<Self>) -> bool {
        self.session.set_find_query(query.to_string());
        let Some(search_match) = self.editor.find_next(query, cx) else {
            window.push_notification(
                Notification::warning(format!("No match found for \"{query}\".")),
                cx,
            );
            cx.notify();
            return false;
        };

        self.schedule_select_match(search_match, cx);
        true
    }

    fn find_next_from_session(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let query = self.session.find_query().trim().to_string();
        if query.is_empty() {
            self.open_find_dialog(window, cx);
            return;
        }

        let _ = self.find_next(&query, window, cx);
    }

    fn find_previous(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let query = self.session.find_query().trim().to_string();
        if query.is_empty() {
            self.open_find_dialog(window, cx);
            return;
        }

        let Some(search_match) = self.editor.find_previous(&query, cx) else {
            window.push_notification(
                Notification::warning(format!("No match found for \"{query}\".")),
                cx,
            );
            cx.notify();
            return;
        };

        self.schedule_select_match(search_match, cx);
    }

    fn replace_next(
        &mut self,
        query: &str,
        replacement: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.session.set_find_query(query.to_string());
        self.session.set_replace_query(replacement.to_string());

        #[cfg(feature = "zed-engine")]
        {
            if self
                .editor
                .replace_next(query, replacement, window, cx)
                .is_some()
            {
                return true;
            }

            window.push_notification(
                Notification::warning(format!("No match found for \"{query}\".")),
                cx,
            );
            cx.notify();
            return false;
        }

        #[cfg(not(feature = "zed-engine"))]
        let current_text = self.editor.text(cx);
        #[cfg(not(feature = "zed-engine"))]
        let cursor = self.editor.cursor_position();
        #[cfg(not(feature = "zed-engine"))]
        let Some((new_text, replacement_match)) =
            replace_next_in_text(&current_text, cursor, query, replacement)
        else {
            window.push_notification(
                Notification::warning(format!("No match found for \"{query}\".")),
                cx,
            );
            cx.notify();
            return false;
        };

        #[cfg(not(feature = "zed-engine"))]
        self.editor
            .set_document(
                new_text.clone(),
                self.session.language().clone(),
                self.session.file_path().map(PathBuf::as_path),
                window,
                cx,
            );
        #[cfg(not(feature = "zed-engine"))]
        {
            self.session.update_dirty_from_text(&new_text);
            self.schedule_select_match(replacement_match, cx);
            cx.notify();
            true
        }
    }

    fn replace_all(
        &mut self,
        query: &str,
        replacement: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        self.session.set_find_query(query.to_string());
        self.session.set_replace_query(replacement.to_string());

        #[cfg(feature = "zed-engine")]
        {
            let Some(replacements) = self.editor.replace_all(query, replacement, window, cx) else {
                window.push_notification(
                    Notification::warning(format!("No match found for \"{query}\".")),
                    cx,
                );
                cx.notify();
                return false;
            };

            window.push_notification(
                Notification::success(format!("Replaced {replacements} match(es).")),
                cx,
            );
            return true;
        }

        #[cfg(not(feature = "zed-engine"))]
        let current_text = self.editor.text(cx);
        #[cfg(not(feature = "zed-engine"))]
        let Some((new_text, first_match, replacements)) =
            replace_all_in_text(&current_text, query, replacement)
        else {
            window.push_notification(
                Notification::warning(format!("No match found for \"{query}\".")),
                cx,
            );
            cx.notify();
            return false;
        };

        #[cfg(not(feature = "zed-engine"))]
        self.editor
            .set_document(
                new_text.clone(),
                self.session.language().clone(),
                self.session.file_path().map(PathBuf::as_path),
                window,
                cx,
            );

        #[cfg(not(feature = "zed-engine"))]
        {
            self.session.update_dirty_from_text(&new_text);
            if let Some(search_match) = first_match {
                self.schedule_select_match(search_match, cx);
            } else {
                cx.notify();
            }
            window.push_notification(
                Notification::success(format!("Replaced {replacements} match(es).")),
                cx,
            );
            true
        }
    }

    fn toggle_comment(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        #[cfg(feature = "zed-engine")]
        {
            self.editor.toggle_comments(window, cx);
            return;
        }

        #[cfg(not(feature = "zed-engine"))]
        let Some(prefix) = line_comment_prefix(self.session.language().as_ref()) else {
            window.push_notification(
                Notification::warning("Line comment toggle is not available for this language."),
                cx,
            );
            return;
        };

        #[cfg(not(feature = "zed-engine"))]
        let current_text = self.editor.text(cx);
        #[cfg(not(feature = "zed-engine"))]
        let selected_range = self.editor.selected_range(cx);
        #[cfg(not(feature = "zed-engine"))]
        let Some((new_text, affected_span)) =
            toggle_line_comments_in_text(&current_text, selected_range, prefix)
        else {
            return;
        };

        #[cfg(not(feature = "zed-engine"))]
        self.editor
            .set_document(
                new_text.clone(),
                self.session.language().clone(),
                self.session.file_path().map(PathBuf::as_path),
                window,
                cx,
            );
        #[cfg(not(feature = "zed-engine"))]
        {
            self.session.update_dirty_from_text(&new_text);
            self.schedule_select_match(affected_span, cx);
            cx.notify();
        }
    }

    fn indent_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.shift_indent(true, window, cx);
    }

    fn outdent_selection(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.shift_indent(false, window, cx);
    }

    fn shift_indent(&mut self, indent: bool, _window: &mut Window, cx: &mut Context<Self>) {
        #[cfg(feature = "zed-engine")]
        {
            if indent {
                self.editor.indent(_window, cx);
            } else {
                self.editor.outdent(_window, cx);
            }
            return;
        }

        #[cfg(not(feature = "zed-engine"))]
        let current_text = self.editor.text(cx);
        #[cfg(not(feature = "zed-engine"))]
        let selected_range = self.editor.selected_range(cx);
        #[cfg(not(feature = "zed-engine"))]
        let indent_unit = self.session.preferred_indent_unit();
        #[cfg(not(feature = "zed-engine"))]
        let Some((new_text, affected_span)) =
            shift_indent_in_text(&current_text, selected_range, &indent_unit, indent)
        else {
            return;
        };

        #[cfg(not(feature = "zed-engine"))]
        self.editor.set_document(
            new_text.clone(),
            self.session.language().clone(),
            self.session.file_path().map(PathBuf::as_path),
            _window,
            cx,
        );
        #[cfg(not(feature = "zed-engine"))]
        {
            self.session.update_dirty_from_text(&new_text);
            self.schedule_select_match(affected_span, cx);
            cx.notify();
        }
    }

    #[cfg(not(feature = "zed-engine"))]
    fn maybe_auto_indent_after_enter(
        &mut self,
        text: &str,
        cursor: gpui_component::input::Position,
        cx: &mut Context<Self>,
    ) {
        let indent_unit = self.session.preferred_indent_unit();
        let Some((new_text, new_cursor)) =
            auto_indent_after_enter(text, cursor, self.session.language().as_ref(), &indent_unit)
        else {
            return;
        };

        self.apply_text_with_cursor(new_text, new_cursor, cx);
    }

    #[cfg(not(feature = "zed-engine"))]
    fn apply_text_with_cursor(
        &mut self,
        new_text: String,
        cursor: gpui_component::input::Position,
        cx: &mut Context<Self>,
    ) {
        let language = self.session.language().clone();
        let editor = self.editor.clone();
        cx.defer(move |cx| {
            let Some(window) = cx.active_window() else {
                return;
            };
            let _ = window.update(cx, |_, window, cx| {
                editor.set_document(new_text, language, None, window, cx);
                editor.set_cursor_position(cursor, window, cx);
            });
        });
    }

    fn schedule_select_match(&mut self, search_match: SearchMatch, cx: &mut Context<Self>) {
        let editor = self.editor.clone();
        cx.defer(move |cx| {
            let Some(window) = cx.active_window() else {
                return;
            };
            let _ = window.update(cx, |_, window, cx| {
                editor.select_match(search_match, window, cx);
            });
        });
    }

    fn request_close(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        if !self.session.dirty() {
            return true;
        }
        if window.has_active_dialog(cx) {
            return false;
        }

        let page = cx.entity().downgrade();
        window.open_alert_dialog(cx, move |alert, _, _| {
            alert
                .title("Unsaved Changes")
                .description("This file has unsaved changes. Save before closing?")
                .show_cancel(true)
                .on_ok({
                    let page = page.clone();
                    move |_, window, cx| match page.update(cx, |page, cx| {
                        page.save_current(window, cx);
                        !page.session.dirty()
                    }) {
                        Ok(true) => {
                            window.remove_window();
                            true
                        }
                        Ok(false) => false,
                        Err(_) => true,
                    }
                })
                .on_cancel({
                    move |_, window, _cx| {
                        window.remove_window();
                        true
                    }
                })
        });
        false
    }

    fn suggested_save_path(&self) -> PathBuf {
        self.session.suggested_save_path()
    }

    fn toggle_line_numbers(
        &mut self,
        _: &ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let line_numbers = self.session.toggle_line_numbers();
        self.editor.set_line_numbers(line_numbers, window, cx);
        cx.notify();
    }

    fn toggle_soft_wrap(
        &mut self,
        _: &ClickEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let soft_wrap = self.session.toggle_soft_wrap();
        self.editor.set_soft_wrap(soft_wrap, window, cx);
        cx.notify();
    }

    fn render_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .w_full()
            .items_center()
            .justify_between()
            .gap_3()
            .px_4()
            .py_2()
            .border_b_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .child(
                h_flex()
                    .min_w_0()
                    .items_center()
                    .gap_3()
                    .child(
                        Label::new(display_name(self.session.file_path().map(PathBuf::as_path)))
                            .text_sm()
                            .font_semibold()
                            .truncate(),
                    )
                    .child(
                        Label::new(display_language(self.session.language()))
                            .text_xs()
                            .text_color(cx.theme().muted_foreground),
                    )
                    .when(self.session.dirty(), |row| {
                        row.child(
                            Label::new("Unsaved")
                                .text_xs()
                                .text_color(cx.theme().warning),
                        )
                    }),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_2()
                    .child(
                        Button::new("open-file")
                            .small()
                            .ghost()
                            .label("Open File")
                            .on_click(cx.listener(Self::open_file)),
                    )
                    .child(
                        Button::new("toggle-line-numbers")
                            .small()
                            .ghost()
                            .selected(self.session.line_numbers())
                            .label("Line Numbers")
                            .on_click(cx.listener(Self::toggle_line_numbers)),
                    )
                    .child(
                        Button::new("toggle-soft-wrap")
                            .small()
                            .ghost()
                            .selected(self.session.soft_wrap())
                            .label("Wrap")
                            .on_click(cx.listener(Self::toggle_soft_wrap)),
                    )
                    .child(
                        Button::new("save-as")
                            .small()
                            .ghost()
                            .label("Save As")
                            .on_click(cx.listener(Self::save_as)),
                    )
                    .child(
                        Button::new("go-to-line")
                            .small()
                            .ghost()
                            .label("Go to Line")
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.go_to_line(window, cx);
                            })),
                    )
                    .child(
                        Button::new("find-text")
                            .small()
                            .ghost()
                            .label("Find")
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.open_find_dialog(window, cx);
                            })),
                    )
                    .child(
                        Button::new("replace-text")
                            .small()
                            .ghost()
                            .label("Replace")
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.open_replace_dialog(false, window, cx);
                            })),
                    )
                    .child(
                        Button::new("replace-all-text")
                            .small()
                            .ghost()
                            .label("Replace All")
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.open_replace_dialog(true, window, cx);
                            })),
                    )
                    .child(
                        Button::new("toggle-comment")
                            .small()
                            .ghost()
                            .label("Comment")
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.toggle_comment(window, cx);
                            })),
                    )
                    .child(
                        Button::new("indent-selection")
                            .small()
                            .ghost()
                            .label("Indent")
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.indent_selection(window, cx);
                            })),
                    )
                    .child(
                        Button::new("outdent-selection")
                            .small()
                            .ghost()
                            .label("Outdent")
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.outdent_selection(window, cx);
                            })),
                    )
                    .child(
                        Button::new("save")
                            .small()
                            .label("Save")
                            .disabled(!self.session.dirty() && self.session.file_path().is_some())
                            .on_click(cx.listener(Self::save)),
                    ),
            )
    }

    fn render_title_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        TitleBar::new().child(
            h_flex()
                .id("cybereditor-title-bar")
                .h_full()
                .w_full()
                .min_w_0()
                .items_center()
                .justify_between()
                .gap_3()
                .px_3()
                .child(
                    h_flex()
                        .min_w_0()
                        .items_center()
                        .gap_3()
                        .child(Label::new(APP_NAME).text_sm().font_semibold())
                        .child(
                            Label::new(display_path(self.session.file_path().map(PathBuf::as_path)))
                                .text_xs()
                                .text_color(cx.theme().muted_foreground)
                                .truncate(),
                        ),
                )
                .child(
                    h_flex()
                        .items_center()
                        .gap_2()
                        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .child(
                            Label::new(if self.session.dirty() { "Unsaved" } else { "Saved" })
                                .text_xs()
                                .text_color(if self.session.dirty() {
                                    cx.theme().warning
                                } else {
                                    cx.theme().muted_foreground
                                }),
                        ),
                ),
        )
    }

    fn window_title(&self) -> SharedString {
        let prefix = if self.session.dirty() { "* " } else { "" };
        SharedString::from(format!(
            "{prefix}{} - {APP_NAME}",
            display_name(self.session.file_path().map(PathBuf::as_path))
        ))
    }

    fn render_status_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .w_full()
            .items_center()
            .justify_between()
            .gap_3()
            .px_4()
            .py_1()
            .border_t_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().title_bar)
            .child(
                h_flex()
                    .items_center()
                    .gap_3()
                    .child(
                        Label::new(if self.session.dirty() { "Modified" } else { "Saved" })
                            .text_xs()
                            .text_color(if self.session.dirty() {
                                cx.theme().warning
                            } else {
                                cx.theme().muted_foreground
                            }),
                    )
                    .child(
                        Label::new(display_language(self.session.language()))
                            .text_xs()
                            .text_color(cx.theme().muted_foreground),
                    ),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_3()
                    .child(
                        Label::new(self.session.encoding_label().clone())
                            .text_xs()
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(
                        Label::new(format!("Lines: {}", self.editor.line_count()))
                            .text_xs()
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(
                        Label::new(format!("Chars: {}", self.editor.char_count()))
                            .text_xs()
                            .text_color(cx.theme().muted_foreground),
                    )
                    .when(self.editor.has_selection(), |row| {
                        row.child(
                            Label::new(format!("Sel: {}", self.editor.selected_char_count()))
                                .text_xs()
                                .text_color(cx.theme().muted_foreground),
                        )
                    })
                    .child(
                        Label::new(format!("Rev: {}", self.editor.revision()))
                            .text_xs()
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(
                        Button::new("go-to-line-status")
                            .ghost()
                            .xsmall()
                            .label(format!(
                                "Ln {}, Col {}",
                                self.editor.cursor_position().line + 1,
                                self.editor.cursor_position().character + 1
                            ))
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.go_to_line(window, cx);
                            })),
                    )
                    .when(!self.session.find_query().is_empty(), |row| {
                        let total = self.editor.match_count(self.session.find_query(), cx);
                        let current = self.editor.current_match_index(self.session.find_query(), cx);
                        row.child(
                            Label::new(if total == 0 {
                                "Matches: 0".to_string()
                            } else if current == 0 {
                                format!("Matches: 0/{total}")
                            } else {
                                format!("Matches: {current}/{total}")
                            })
                            .text_xs()
                            .text_color(cx.theme().muted_foreground),
                        )
                    })
                    .child(
                        Label::new(self.session.line_ending_label())
                            .text_xs()
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(
                        Label::new(self.session.indent_label())
                            .text_xs()
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(
                        Label::new(if self.session.soft_wrap() { "Wrap On" } else { "Wrap Off" })
                            .text_xs()
                            .text_color(cx.theme().muted_foreground),
                    ),
            )
    }
}

impl Focusable for CyberEditorPage {
    fn focus_handle(&self, cx: &App) -> FocusHandle {
        self.editor.focus_handle(cx)
    }
}

impl Render for CyberEditorPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let title = self.window_title();
        window.set_window_title(&title);

        let editor_focus = self.editor.focus_handle(cx);

        v_flex()
            .id("cyber-editor-page")
            .size_full()
            .min_h_0()
            .min_w_0()
            .on_action(cx.listener(|this, _: &SaveFile, window, cx| {
                this.save_current(window, cx);
            }))
            .on_action(cx.listener(|this, _: &OpenFile, window, cx| {
                this.open_file_dialog(window, cx);
            }))
            .on_action(cx.listener(|this, _: &SaveFileAs, window, cx| {
                this.open_save_as_dialog(window, cx);
            }))
            .on_action(cx.listener(|this, _: &GoToLine, window, cx| {
                this.go_to_line(window, cx);
            }))
            .on_action(cx.listener(|this, _: &FindText, window, cx| {
                this.open_find_dialog(window, cx);
            }))
            .on_action(cx.listener(|this, _: &ReplaceText, window, cx| {
                this.open_replace_dialog(false, window, cx);
            }))
            .on_action(cx.listener(|this, _: &ReplaceAllText, window, cx| {
                this.open_replace_dialog(true, window, cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleComment, window, cx| {
                this.toggle_comment(window, cx);
            }))
            .on_action(cx.listener(|this, _: &IndentSelection, window, cx| {
                this.indent_selection(window, cx);
            }))
            .on_action(cx.listener(|this, _: &OutdentSelection, window, cx| {
                this.outdent_selection(window, cx);
            }))
            .on_action(cx.listener(|this, _: &FindNext, window, cx| {
                this.find_next_from_session(window, cx);
            }))
            .on_action(cx.listener(|this, _: &FindPrevious, window, cx| {
                this.find_previous(window, cx);
            }))
            .child(self.render_title_bar(cx))
            .child(
                div()
                    .track_focus(&self.focus_handle)
                    .key_context(EDITOR_CONTEXT)
                    .child(self.render_toolbar(cx)),
            )
            .child(
                div()
                    .id("cyber-editor-surface")
                    .flex_1()
                    .min_h_0()
                    .min_w_0()
                    .track_focus(&editor_focus)
                    .on_mouse_down(gpui::MouseButton::Left, {
                        let editor_focus = editor_focus.clone();
                        move |_, window, cx| {
                            editor_focus.focus(window, cx);
                        }
                    })
                    .child(self.editor.render(cx)),
            )
            .child(self.render_status_bar(cx))
    }
}

fn parse_go_to_line_target(raw: &str) -> Option<gpui_component::input::Position> {
    let mut parts = raw.split(':');
    let line = parts.next()?.trim().parse::<u32>().ok()?;
    if line == 0 {
        return None;
    }

    let column = match parts.next() {
        Some(value) if !value.trim().is_empty() => value.trim().parse::<u32>().ok()?,
        Some(_) | None => 1,
    };
    if column == 0 || parts.next().is_some() {
        return None;
    }

    Some(gpui_component::input::Position::new(line - 1, column - 1))
}

#[cfg(not(feature = "zed-engine"))]
fn single_span_edit(old_text: &str, new_text: &str) -> Option<(std::ops::Range<usize>, String)> {
    if old_text == new_text {
        return None;
    }

    let old_bytes = old_text.as_bytes();
    let new_bytes = new_text.as_bytes();

    let mut start = 0usize;
    while start < old_bytes.len() && start < new_bytes.len() && old_bytes[start] == new_bytes[start]
    {
        start += 1;
    }

    let mut old_end = old_bytes.len();
    let mut new_end = new_bytes.len();
    while old_end > start && new_end > start && old_bytes[old_end - 1] == new_bytes[new_end - 1] {
        old_end -= 1;
        new_end -= 1;
    }

    Some((start..old_end, new_text[start..new_end].to_string()))
}

#[cfg(not(feature = "zed-engine"))]
fn search_match_to_byte_range(text: &str, search_match: SearchMatch) -> std::ops::Range<usize> {
    let start = position_to_byte_offset(text, search_match.start);
    let end = advance_by_chars_bytes(text, start, search_match.char_len);
    start..end
}

#[cfg(not(feature = "zed-engine"))]
fn build_replace_all_edits(
    text: &str,
    query: &str,
    replacement: &str,
) -> Vec<(std::ops::Range<usize>, String)> {
    if query.is_empty() {
        return Vec::new();
    }

    let mut edits = Vec::new();
    let mut start = 0usize;
    while let Some(offset) = text[start..].find(query) {
        let match_start = start + offset;
        let match_end = match_start + query.len();
        edits.push((match_start..match_end, replacement.to_string()));
        start = match_end;
        if start > text.len() {
            break;
        }
    }
    edits
}

#[cfg(not(feature = "zed-engine"))]
fn replace_next_in_text(
    text: &str,
    cursor: gpui_component::input::Position,
    query: &str,
    replacement: &str,
) -> Option<(String, SearchMatch)> {
    if query.is_empty() {
        return None;
    }

    let start = position_to_byte_offset(text, cursor);
    let match_offset = text[start..]
        .find(query)
        .map(|offset| start + offset)
        .or_else(|| text[..start].find(query))?;

    let match_end = match_offset + query.len();
    let mut new_text =
        String::with_capacity(text.len() + replacement.len().saturating_sub(query.len()));
    new_text.push_str(&text[..match_offset]);
    new_text.push_str(replacement);
    new_text.push_str(&text[match_end..]);

    let replacement_match = SearchMatch {
        start: byte_offset_to_position(&new_text, match_offset),
        char_len: replacement.chars().count() as u32,
    };

    Some((new_text, replacement_match))
}

#[cfg(not(feature = "zed-engine"))]
fn replace_all_in_text(
    text: &str,
    query: &str,
    replacement: &str,
) -> Option<(String, Option<SearchMatch>, usize)> {
    if query.is_empty() {
        return None;
    }

    let replacements = text.matches(query).count();
    if replacements == 0 {
        return None;
    }

    let new_text = text.replace(query, replacement);
    let first_match = if replacement.is_empty() {
        None
    } else {
        new_text.find(replacement).map(|offset| SearchMatch {
            start: byte_offset_to_position(&new_text, offset),
            char_len: replacement.chars().count() as u32,
        })
    };
    Some((new_text, first_match, replacements))
}

#[cfg(not(feature = "zed-engine"))]
fn auto_indent_after_enter(
    text: &str,
    cursor: gpui_component::input::Position,
    language: &str,
    indent_unit: &str,
) -> Option<(String, gpui_component::input::Position)> {
    let cursor_offset = position_to_byte_offset(text, cursor);
    if cursor_offset == 0 || !text[..cursor_offset].ends_with('\n') {
        return None;
    }

    let current_line_start = line_start_offset(text, cursor_offset);
    let previous_line_end = current_line_start.saturating_sub(1);
    let previous_line_start = line_start_offset(text, previous_line_end);
    let previous_line = &text[previous_line_start..previous_line_end];
    let current_line_end = line_end_offset(text, current_line_start);
    let current_line = &text[current_line_start..current_line_end];

    if !should_increase_indent(previous_line, language) {
        return None;
    }

    let previous_indent = leading_indent(previous_line);
    let current_indent = leading_indent(current_line);
    let expected_indent = format!("{previous_indent}{indent_unit}");
    if current_indent == expected_indent {
        return None;
    }

    let trimmed_current = current_line.trim_start();
    let mut new_text = String::with_capacity(text.len() + expected_indent.len().saturating_sub(current_indent.len()));
    new_text.push_str(&text[..current_line_start]);
    new_text.push_str(&expected_indent);
    new_text.push_str(trimmed_current);
    new_text.push_str(&text[current_line_end..]);

    let new_cursor = gpui_component::input::Position::new(
        cursor.line,
        expected_indent.chars().count() as u32,
    );
    Some((new_text, new_cursor))
}

#[cfg(not(feature = "zed-engine"))]
fn should_increase_indent(previous_line: &str, language: &str) -> bool {
    let trimmed = previous_line.trim_end();
    if trimmed.is_empty() {
        return false;
    }

    if matches!(trimmed.chars().last(), Some('{') | Some('[') | Some('(')) {
        return true;
    }

    if matches!(language, "python" | "yaml") && trimmed.ends_with(':') {
        return true;
    }

    false
}

#[cfg(not(feature = "zed-engine"))]
fn leading_indent(line: &str) -> String {
    line.chars()
        .take_while(|ch| *ch == ' ' || *ch == '\t')
        .collect()
}

#[cfg(not(feature = "zed-engine"))]
fn toggle_line_comments_in_text(
    text: &str,
    selected_range: std::ops::Range<usize>,
    prefix: &str,
) -> Option<(String, SearchMatch)> {
    let line_start = line_start_offset(text, selected_range.start.min(text.len()));
    let normalized_end = normalize_selection_end(text, &selected_range);
    let line_end = line_end_offset(text, normalized_end);

    let block = &text[line_start..line_end];
    let lines: Vec<&str> = block.split('\n').collect();
    if lines.is_empty() {
        return None;
    }

    let non_empty_lines: Vec<&str> = lines
        .iter()
        .copied()
        .filter(|line| !line.trim().is_empty())
        .collect();
    if non_empty_lines.is_empty() {
        return None;
    }

    let should_uncomment = non_empty_lines
        .iter()
        .all(|line| trimmed_comment_prefix(line, prefix).is_some());

    let new_block = lines
        .into_iter()
        .map(|line| {
            if line.trim().is_empty() {
                return line.to_string();
            }

            let indent_len = line
                .char_indices()
                .find(|(_, ch)| !ch.is_whitespace())
                .map(|(idx, _)| idx)
                .unwrap_or(line.len());
            let (indent, rest) = line.split_at(indent_len);

            if should_uncomment {
                let uncommented = trimmed_comment_prefix(line, prefix).unwrap_or(rest);
                format!("{indent}{uncommented}")
            } else {
                format!("{indent}{prefix} {rest}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    let mut new_text = String::with_capacity(text.len() + new_block.len().saturating_sub(block.len()));
    new_text.push_str(&text[..line_start]);
    new_text.push_str(&new_block);
    new_text.push_str(&text[line_end..]);

    let affected_span = SearchMatch {
        start: byte_offset_to_position(&new_text, line_start),
        char_len: new_block.chars().count() as u32,
    };

    Some((new_text, affected_span))
}

#[cfg(not(feature = "zed-engine"))]
fn shift_indent_in_text(
    text: &str,
    selected_range: std::ops::Range<usize>,
    indent_unit: &str,
    indent: bool,
) -> Option<(String, SearchMatch)> {
    let line_start = line_start_offset(text, selected_range.start.min(text.len()));
    let normalized_end = normalize_selection_end(text, &selected_range);
    let line_end = line_end_offset(text, normalized_end);

    let block = &text[line_start..line_end];
    let lines: Vec<&str> = block.split('\n').collect();
    if lines.is_empty() {
        return None;
    }

    let new_block = lines
        .into_iter()
        .map(|line| {
            if indent {
                format!("{indent_unit}{line}")
            } else {
                outdent_line(line, indent_unit)
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    if new_block == block {
        return None;
    }

    let mut new_text = String::with_capacity(text.len() + new_block.len().saturating_sub(block.len()));
    new_text.push_str(&text[..line_start]);
    new_text.push_str(&new_block);
    new_text.push_str(&text[line_end..]);

    Some((
        new_text,
        SearchMatch {
            start: byte_offset_to_position(text, line_start),
            char_len: new_block.chars().count() as u32,
        },
    ))
}

#[cfg(not(feature = "zed-engine"))]
fn outdent_line(line: &str, indent_unit: &str) -> String {
    if line.is_empty() {
        return String::new();
    }
    if let Some(rest) = line.strip_prefix(indent_unit) {
        return rest.to_string();
    }

    let leading_whitespace = line
        .chars()
        .take_while(|ch| *ch == ' ' || *ch == '\t')
        .collect::<String>();
    if leading_whitespace.is_empty() {
        return line.to_string();
    }

    let remove_len = if indent_unit == "\t" {
        leading_whitespace
            .chars()
            .next()
            .map(|ch| ch.len_utf8())
            .unwrap_or(0)
    } else {
        leading_whitespace
            .chars()
            .take(indent_unit.chars().count())
            .map(char::len_utf8)
            .sum()
    };

    line[remove_len.min(line.len())..].to_string()
}

#[cfg(not(feature = "zed-engine"))]
fn trimmed_comment_prefix<'a>(line: &'a str, prefix: &str) -> Option<&'a str> {
    let indent_len = line
        .char_indices()
        .find(|(_, ch)| !ch.is_whitespace())
        .map(|(idx, _)| idx)
        .unwrap_or(line.len());
    let (_, rest) = line.split_at(indent_len);
    let rest = rest.strip_prefix(prefix)?;
    Some(rest.strip_prefix(' ').unwrap_or(rest))
}

#[cfg(not(feature = "zed-engine"))]
fn line_start_offset(text: &str, offset: usize) -> usize {
    text[..offset].rfind('\n').map(|idx| idx + 1).unwrap_or(0)
}

#[cfg(not(feature = "zed-engine"))]
fn line_end_offset(text: &str, offset: usize) -> usize {
    text[offset..]
        .find('\n')
        .map(|idx| offset + idx)
        .unwrap_or(text.len())
}

#[cfg(not(feature = "zed-engine"))]
fn normalize_selection_end(text: &str, selected_range: &std::ops::Range<usize>) -> usize {
    if selected_range.end > selected_range.start
        && selected_range.end <= text.len()
        && text.as_bytes()[selected_range.end - 1] == b'\n'
    {
        selected_range.end - 1
    } else {
        selected_range.end.min(text.len())
    }
}

#[cfg(not(feature = "zed-engine"))]
fn position_to_byte_offset(text: &str, position: gpui_component::input::Position) -> usize {
    let mut line = 0u32;
    let mut column = 0u32;

    for (offset, ch) in text.char_indices() {
        if line == position.line && column == position.character {
            return offset;
        }

        if ch == '\n' {
            line += 1;
            column = 0;
        } else {
            column += 1;
        }
    }

    text.len()
}

#[cfg(not(feature = "zed-engine"))]
fn byte_offset_to_position(text: &str, byte_offset: usize) -> gpui_component::input::Position {
    let mut line = 0u32;
    let mut column = 0u32;

    for (offset, ch) in text.char_indices() {
        if offset >= byte_offset {
            break;
        }

        if ch == '\n' {
            line += 1;
            column = 0;
        } else {
            column += 1;
        }
    }

    gpui_component::input::Position::new(line, column)
}

#[cfg(not(feature = "zed-engine"))]
fn advance_by_chars_bytes(text: &str, start_byte: usize, char_len: u32) -> usize {
    if char_len == 0 {
        return start_byte;
    }

    let mut count = 0u32;
    let mut last_end = start_byte;
    for (offset, ch) in text[start_byte..].char_indices() {
        if count >= char_len {
            break;
        }
        count += 1;
        last_end = start_byte + offset + ch.len_utf8();
        if count == char_len {
            break;
        }
    }

    if count < char_len {
        text.len()
    } else {
        last_end
    }
}

fn focus_input_once(
    armed: &Rc<Cell<bool>>,
    input: Entity<InputState>,
    window: &mut Window,
    cx: &mut App,
) {
    if armed.replace(true) {
        return;
    }

    window.defer(cx, move |window, cx| {
        let _ = input.update(cx, |input, cx| {
            input.focus(window, cx);
        });
    });
}
