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
    v_flex, ActiveTheme as _, Disableable, Sizable as _, StyledExt,
    WindowExt as _,
};

use crate::title_bar::TitleBar;


use gpui_component::input::InputEvent;
use rust_i18n::t;
use super::line_comment_prefix;

use super::{
    display_language, display_name, editor_menu_bar,
    load_document, AboutEditor, EditorCopy, EditorCut, EditorPaste, EditorRedo, EditorUndo,
    EditorHost, EditorSession, ExitEditor, FindNext, FindPrevious, FindText, GoToLine,
    IndentSelection, NewFile, OpenFile, OutdentSelection, ReplaceAllText, ReplaceText, SaveFile,
    SaveFileAs, SearchMatch, SelectAll, ToggleComment, ToggleLineNumbers, ToggleSoftWrap, APP_NAME,
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
        super::app_menus::set_view_toggles(session.line_numbers(), session.soft_wrap(), cx);

        let mut subscriptions = Vec::new();

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
            .map_err(|err| t!("editor.save.error", path = path.display(), err = err.to_string()).to_string())?;

        self.session.apply_save(path.clone(), text);
        self.editor.set_highlighter(
            self.session.language().clone(),
            self.session.file_path().map(PathBuf::as_path),
            cx,
        );
        window.push_notification(
            Notification::success(t!("editor.save.success", path = path.display()).to_string()),
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

    pub(crate) fn open_file_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let start_dir = self.session.file_path().cloned();
        let page = cx.entity().downgrade();
        let window = window.window_handle();
        cx.spawn(async move |_, cx| {
            let path = cx
                .background_spawn(async move {
                    super::file_dialog::pick_open_file_path(start_dir.as_deref())
                })
                .await;
            let Some(path) = path else {
                return;
            };
            let _ = window.update(cx, |_, window, cx| {
                let _ = page.update(cx, |page, cx| {
                    if let Err(message) = page.load_path_into_editor(path, window, cx) {
                        window.push_notification(Notification::error(message), cx);
                    }
                });
            });
        })
        .detach();
    }

    pub(crate) fn open_save_as_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let default_path = self.suggested_save_path();
        let page = cx.entity().downgrade();
        let window = window.window_handle();
        cx.spawn(async move |_, cx| {
            let path = cx
                .background_spawn(async move {
                    super::file_dialog::pick_save_file_path(&default_path)
                })
                .await;
            let Some(path) = path else {
                return;
            };
            let _ = window.update(cx, |_, window, cx| {
                let _ = page.update(cx, |page, cx| {
                    if let Err(message) = page.write_to_path(path, window, cx) {
                        window.push_notification(Notification::error(message), cx);
                    }
                });
            });
        })
        .detach();
    }

    fn new_document(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.session.dirty() {
            let page = cx.entity().downgrade();
            window.open_alert_dialog(cx, move |alert, _, _| {
                alert
                    .title(t!("editor.unsaved.title"))
                    .description(t!("editor.unsaved.discard_new"))
                    .show_cancel(true)
                    .on_ok({
                        let page = page.clone();
                        move |_, window, cx| {
                            page.update(cx, |page, cx| {
                                page.load_empty_document(window, cx);
                            })
                            .is_ok()
                        }
                    })
            });
            return;
        }
        self.load_empty_document(window, cx);
    }

    fn load_empty_document(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.editor.set_document(
            String::new(),
            SharedString::from("text"),
            None,
            window,
            cx,
        );
        self.session
            .apply_loaded_document(PathBuf::from("untitled.txt"), String::new());
        self.editor
            .set_highlighter(self.session.language().clone(), None, cx);
        self.editor.focus_deferred(window, cx);
        cx.notify();
    }

    pub(crate) fn run_editor_undo(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        // Handled by gpui-component InputState keybindings when the editor is focused.
    }

    pub(crate) fn run_editor_redo(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
    }

    pub(crate) fn run_editor_cut(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
    }

    pub(crate) fn run_editor_copy(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
    }

    pub(crate) fn run_editor_paste(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
    }

    pub(crate) fn run_select_all(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
    }

    fn show_about(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        window.open_alert_dialog(cx, move |alert, _, _| {
            alert
                .title(t!("editor.about.title"))
                .description(t!("editor.page.about.description"))
                .show_cancel(false)
                .on_ok(|_, _, _| true)
        });
    }

    fn go_to_line(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let cursor = self.editor.cursor_position();
        let default_target = format!("{}:{}", cursor.line + 1, cursor.character + 1);
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
            let default_target_for_submit = default_target.clone();
            focus_input_once(&focus_once, input_for_focus, window, cx);

            alert
                .title(t!("editor.goto.title"))
                .description(t!("editor.goto.description"))
                .show_cancel(true)
                .child(Input::new(&input).w_full())
                .on_ok(move |_, window, cx| {
                    let raw = input_for_submit.read(cx).value().trim().to_string();
                    let raw = if raw.is_empty() {
                        default_target_for_submit.clone()
                    } else {
                        raw
                    };
                    let Some(position) = parse_go_to_line_target(&raw) else {
                        window.push_notification(
                            Notification::warning(t!("editor.goto.invalid").to_string()),
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

    pub(crate) fn open_find_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(self.session.find_query().to_string())
                .placeholder(t!("editor.find.find_placeholder"))
        });
        let page = cx.entity().downgrade();
        let focus_once = Rc::new(Cell::new(false));

        window.open_alert_dialog(cx, move |alert, window, cx| {
            let input_for_focus = input.clone();
            let input_for_submit = input.clone();
            let page_for_submit = page.clone();
            focus_input_once(&focus_once, input_for_focus, window, cx);

            alert
                .title(t!("editor.find.title"))
                .description(t!("editor.find.description"))
                .show_cancel(true)
                .child(Input::new(&input).w_full())
                .on_ok(move |_, window, cx| {
                    let raw = input_for_submit.read(cx).value().trim().to_string();
                    if raw.is_empty() {
                        window.push_notification(
                            Notification::warning(t!("editor.find.enter").to_string()),
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
                .placeholder(t!("editor.find.placeholder"))
        });
        let replace_input = cx.new(|cx| {
            InputState::new(window, cx)
                .default_value(self.session.replace_query().to_string())
                .placeholder(t!("editor.find.replace_placeholder"))
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
                    t!("editor.find.replace_all")
                } else {
                    t!("editor.find.replace")
                })
                .description(if replace_all {
                    t!("editor.replace_all.description")
                } else {
                    t!("editor.replace.description")
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
                            Notification::warning(t!("editor.find.enter").to_string()),
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
                Notification::warning(t!("editor.find.no_match", query = query).to_string()),
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
                Notification::warning(t!("editor.find.no_match", query = query).to_string()),
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


        let current_text = self.editor.text(cx);
        let cursor = self.editor.cursor_position();
        let Some((new_text, replacement_match)) =
            replace_next_in_text(&current_text, cursor, query, replacement)
        else {
            window.push_notification(
                Notification::warning(t!("editor.find.no_match", query = query).to_string()),
                cx,
            );
            cx.notify();
            return false;
        };

        self.editor
            .set_document(
                new_text.clone(),
                self.session.language().clone(),
                self.session.file_path().map(PathBuf::as_path),
                window,
                cx,
            );
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


        let current_text = self.editor.text(cx);
        let Some((new_text, first_match, replacements)) =
            replace_all_in_text(&current_text, query, replacement)
        else {
            window.push_notification(
                Notification::warning(t!("editor.find.no_match", query = query).to_string()),
                cx,
            );
            cx.notify();
            return false;
        };

        self.editor
            .set_document(
                new_text.clone(),
                self.session.language().clone(),
                self.session.file_path().map(PathBuf::as_path),
                window,
                cx,
            );

        {
            self.session.update_dirty_from_text(&new_text);
            if let Some(search_match) = first_match {
                self.schedule_select_match(search_match, cx);
            } else {
                cx.notify();
            }
            window.push_notification(
                Notification::success(t!("editor.find.replaced", count = replacements).to_string()),
                cx,
            );
            true
        }
    }

    fn toggle_comment(&mut self, window: &mut Window, cx: &mut Context<Self>) {

        let Some(prefix) = line_comment_prefix(self.session.language().as_ref()) else {
            window.push_notification(
                Notification::warning(t!("editor.comment.unavailable").to_string()),
                cx,
            );
            return;
        };

        let current_text = self.editor.text(cx);
        let selected_range = self.editor.selected_range(cx);
        let Some((new_text, affected_span)) =
            toggle_line_comments_in_text(&current_text, selected_range, prefix)
        else {
            return;
        };

        self.editor
            .set_document(
                new_text.clone(),
                self.session.language().clone(),
                self.session.file_path().map(PathBuf::as_path),
                window,
                cx,
            );
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

        let current_text = self.editor.text(cx);
        let selected_range = self.editor.selected_range(cx);
        let indent_unit = self.session.preferred_indent_unit();
        let Some((new_text, affected_span)) =
            shift_indent_in_text(&current_text, selected_range, &indent_unit, indent)
        else {
            return;
        };

        self.editor.set_document(
            new_text.clone(),
            self.session.language().clone(),
            self.session.file_path().map(PathBuf::as_path),
            _window,
            cx,
        );
        {
            self.session.update_dirty_from_text(&new_text);
            self.schedule_select_match(affected_span, cx);
            cx.notify();
        }
    }

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
                .title(t!("editor.unsaved.title"))
                .description(t!("editor.unsaved.single"))
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
                    .when(self.session.dirty(), |row| {
                        row.child(
                            Label::new(t!("editor.status.unsaved"))
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
                            .label(t!("editor.toolbar.open"))
                            .on_click(cx.listener(Self::open_file)),
                    )
                    .child(
                        Button::new("save-as")
                            .small()
                            .ghost()
                            .label(t!("editor.menu.save_as"))
                            .on_click(cx.listener(Self::save_as)),
                    )
                    .child(
                        Button::new("go-to-line")
                            .small()
                            .ghost()
                            .label(t!("editor.menu.go_to_line"))
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.go_to_line(window, cx);
                            })),
                    )
                    .child(
                        Button::new("find-text")
                            .small()
                            .ghost()
                            .label(t!("editor.find.title"))
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.open_find_dialog(window, cx);
                            })),
                    )
                    .child(
                        Button::new("replace-text")
                            .small()
                            .ghost()
                            .label(t!("editor.find.replace"))
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.open_replace_dialog(false, window, cx);
                            })),
                    )
                    .child(
                        Button::new("replace-all-text")
                            .small()
                            .ghost()
                            .label(t!("editor.find.replace_all"))
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.open_replace_dialog(true, window, cx);
                            })),
                    )
                    .child(
                        Button::new("toggle-comment")
                            .small()
                            .ghost()
                            .label(t!("editor.toolbar.comment"))
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.toggle_comment(window, cx);
                            })),
                    )
                    .child(
                        Button::new("indent-selection")
                            .small()
                            .ghost()
                            .label(t!("editor.menu.indent"))
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.indent_selection(window, cx);
                            })),
                    )
                    .child(
                        Button::new("outdent-selection")
                            .small()
                            .ghost()
                            .label(t!("editor.menu.outdent"))
                            .on_click(cx.listener(|this, _: &ClickEvent, window, cx| {
                                this.outdent_selection(window, cx);
                            })),
                    )
                    .child(
                        Button::new("save")
                            .small()
                            .label(t!("editor.save"))
                            .disabled(!self.session.dirty() && self.session.file_path().is_some())
                            .on_click(cx.listener(Self::save)),
                    ),
            )
    }

    fn render_title_bar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let menu_bar = editor_menu_bar(cx);
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
                        .gap_2()
                        .child(Label::new(APP_NAME).text_sm().font_semibold())
                        .child(div().flex_none().child(menu_bar))
                )
                .child(
                    h_flex()
                        .items_center()
                        .gap_2()
                        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_mouse_down(gpui::MouseButton::Left, |_, _, cx| cx.stop_propagation()),
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
                    .min_w_0()
                    .items_center()
                    .gap_3()
                    .child(
                        Label::new(
                            self.session
                                .file_path()
                                .map(|p| p.display().to_string())
                                .unwrap_or_else(|| "untitled.txt".to_string()),
                        )
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .truncate(),
                    ),
            )
            .child(
                h_flex()
                    .items_center()
                    .gap_3()
                    .child(
                        Label::new(if self.session.dirty() {
                            t!("editor.status.modified")
                        } else {
                            t!("editor.status.saved")
                        })
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
                    )
                    .child(
                        Label::new(self.session.encoding_label().clone())
                            .text_xs()
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(
                        Label::new(t!("editor.status.lines", count = self.editor.line_count()))
                            .text_xs()
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(
                        Label::new(t!("editor.status.chars", count = self.editor.char_count()))
                            .text_xs()
                            .text_color(cx.theme().muted_foreground),
                    )
                    .when(self.editor.has_selection(cx), |row| {
                        row.child(
                            Label::new(t!(
                                "editor.status.sel",
                                count = self.editor.selected_char_count()
                            ))
                                .text_xs()
                                .text_color(cx.theme().muted_foreground),
                        )
                    })
                    .child(
                        Label::new(t!("editor.status.rev", count = self.editor.revision()))
                            .text_xs()
                            .text_color(cx.theme().muted_foreground),
                    )
                    .child(
                        Button::new("go-to-line-status")
                            .ghost()
                            .xsmall()
                            .label(t!(
                                "editor.status.ln_col",
                                line = self.editor.cursor_position().line + 1,
                                col = self.editor.cursor_position().character + 1
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
                                t!("editor.status.matches_zero").to_string()
                            } else if current == 0 {
                                t!("editor.status.matches_zero_total", total = total).to_string()
                            } else {
                                t!("editor.status.matches", current = current, total = total)
                                    .to_string()
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
                        Label::new(if self.session.soft_wrap() {
                            t!("editor.status.wrap_on")
                        } else {
                            t!("editor.status.wrap_off")
                        })
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
            .on_action(cx.listener(|this, _: &NewFile, window, cx| {
                this.new_document(window, cx);
            }))
            .on_action(cx.listener(|this, _: &SaveFile, window, cx| {
                this.save_current(window, cx);
            }))
            .on_action(cx.listener(|this, _: &OpenFile, window, cx| {
                this.open_file_dialog(window, cx);
            }))
            .on_action(cx.listener(|this, _: &SaveFileAs, window, cx| {
                this.open_save_as_dialog(window, cx);
            }))
            .on_action(cx.listener(|this, _: &ExitEditor, window, cx| {
                if this.request_close(window, cx) {
                    window.remove_window();
                }
            }))
            .on_action(cx.listener(|this, _: &EditorUndo, window, cx| {
                this.run_editor_undo(window, cx);
            }))
            .on_action(cx.listener(|this, _: &EditorRedo, window, cx| {
                this.run_editor_redo(window, cx);
            }))
            .on_action(cx.listener(|this, _: &EditorCut, window, cx| {
                this.run_editor_cut(window, cx);
            }))
            .on_action(cx.listener(|this, _: &EditorCopy, window, cx| {
                this.run_editor_copy(window, cx);
            }))
            .on_action(cx.listener(|this, _: &EditorPaste, window, cx| {
                this.run_editor_paste(window, cx);
            }))
            .on_action(cx.listener(|this, _: &SelectAll, window, cx| {
                this.run_select_all(window, cx);
            }))
            .on_action(cx.listener(|this, _: &ToggleLineNumbers, window, cx| {
                let line_numbers = this.session.toggle_line_numbers();
                this.editor.set_line_numbers(line_numbers, window, cx);
                super::app_menus::set_view_toggles(line_numbers, this.session.soft_wrap(), cx);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &ToggleSoftWrap, window, cx| {
                let soft_wrap = this.session.toggle_soft_wrap();
                this.editor.set_soft_wrap(soft_wrap, window, cx);
                super::app_menus::set_view_toggles(this.session.line_numbers(), soft_wrap, cx);
                cx.notify();
            }))
            .on_action(cx.listener(|this, _: &AboutEditor, window, cx| {
                this.show_about(window, cx);
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

fn leading_indent(line: &str) -> String {
    line.chars()
        .take_while(|ch| *ch == ' ' || *ch == '\t')
        .collect()
}

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

fn line_start_offset(text: &str, offset: usize) -> usize {
    text[..offset].rfind('\n').map(|idx| idx + 1).unwrap_or(0)
}

fn line_end_offset(text: &str, offset: usize) -> usize {
    text[offset..]
        .find('\n')
        .map(|idx| offset + idx)
        .unwrap_or(text.len())
}

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
