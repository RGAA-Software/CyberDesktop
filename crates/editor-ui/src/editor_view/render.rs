//! Root `Render` implementation for the editor view.

use super::imports::*;

impl Focusable for EngineEditor {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}


impl Render for EngineEditor {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.external_file_drop_hover && !cx.has_active_drag() {
            self.external_file_drop_hover = false;
        }
        if self.needs_focus {
            window.focus(&self.focus_handle, cx);
            self.needs_focus = false;
        }
        self.start_disk_watch(cx);
        self.start_caret_blink(cx);
        if !self.close_hooked {
            self.close_hooked = true;
            let weak = cx.entity().downgrade();
            window.on_window_should_close(cx, move |_window, cx| {
                weak.update(cx, |this, cx| this.request_window_close(cx))
                    .unwrap_or(true)
            });
        }
        let colors = super::ui::EditorColors::from_app(cx);
        let title_bar = self.render_title_bar(cx);
        if let Some(ix) = self.pending_tab_scroll_to_ix.take() {
            self.tab_bar_scroll_handle.scroll_to_item(ix);
        }
        let tab_bar = self.render_tab_bar(cx);
        let disk_banner = self.render_disk_banner(cx);
        let header = self.render_header(cx);
        let focus = self.focus_handle.clone();
        let find_bar = self.render_find_bar(cx);
        let goto_bar = self.render_goto(cx);
        let search_panel = self.render_search_panel(cx);
        let about = self.render_about(cx);
        let shortcuts = self.render_shortcuts(cx);
        let close_confirm = self.render_close_confirm(cx);
        let recent = self.render_recent(cx);
        let context_menu = self.render_context_menu_overlay(window);
        let file_load_bar = self.render_file_load_bar(cx);
        let scrollbar = self.render_scrollbar(cx);
        let hscrollbar = self.render_hscrollbar(cx);
        let drop_highlight = colors.selection.alpha(0.25);
        let canvas = EditorCanvas {
            editor: cx.entity(),
            colors,
        };

        div()
            .flex()
            .flex_col()
            .size_full()
            .child(title_bar)
            .child(tab_bar)
            .children(disk_banner)
            .child(
                div()
                    .track_focus(&focus)
                    .key_context(EDITOR_CONTEXT)
                    .on_key_down(cx.listener(Self::on_key_down))
                    .on_action(cx.listener(|this, _: &NewFile, _w, cx| this.new_tab(cx)))
                    .on_action(cx.listener(|this, _: &OpenFile, _w, cx| this.open_file(cx)))
                    .on_action(cx.listener(|this, _: &SaveFile, _w, cx| this.save_file(cx)))
                    .on_action(cx.listener(|this, _: &SaveFileAs, _w, cx| this.save_file_as(cx)))
                    .on_action(cx.listener(|this, _: &ExitEditor, window, cx| {
                        if this.request_window_close(cx) {
                            window.remove_window();
                        }
                    }))
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
                        this.open_find(true, window, cx);
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
                    .on_action(cx.listener(|this, _: &GoToLine, window, cx| {
                        this.open_goto(window, cx)
                    }))
                    .on_action(cx.listener(|this, _: &ToggleFold, _w, cx| {
                        this.toggle_fold_at_caret(cx)
                    }))
                    .on_action(cx.listener(|this, _: &FoldAll, _w, cx| this.fold_all(cx)))
                    .on_action(cx.listener(|this, _: &UnfoldAll, _w, cx| this.unfold_all(cx)))
                    .on_mouse_down(MouseButton::Left, cx.listener(Self::on_mouse_down))
                    .on_mouse_down(MouseButton::Right, cx.listener(Self::on_mouse_right))
                    .on_mouse_up(MouseButton::Left, cx.listener(Self::on_mouse_up))
                    .on_mouse_move(cx.listener(Self::on_mouse_move))
                    .on_scroll_wheel(cx.listener(Self::on_scroll))
                    .on_drag_move::<ExternalPaths>(cx.listener(
                        |this, event: &DragMoveEvent<ExternalPaths>, window, cx| {
                            let droppable = super::r#impl::external_paths_are_droppable(event.drag(cx));
                            this.set_external_file_drop_hover(droppable, window, cx);
                        },
                    ))
                    .can_drop(|payload, _, _| {
                        payload
                            .downcast_ref::<ExternalPaths>()
                            .is_some_and(super::r#impl::external_paths_are_droppable)
                    })
                    .drag_over::<ExternalPaths>(move |style, paths, _, _| {
                        if super::r#impl::external_paths_are_droppable(paths) {
                            style.cursor(CursorStyle::PointingHand).bg(drop_highlight)
                        } else {
                            style.cursor(CursorStyle::OperationNotAllowed)
                        }
                    })
                    .on_drop(cx.listener(|this, paths: &ExternalPaths, window, cx| {
                        this.handle_external_file_drop(paths, cx);
                        this.set_external_file_drop_hover(false, window, cx);
                    }))
                    .relative()
                    .flex_1()
                    .min_h_0()
                    .bg(colors.background)
                    .text_color(colors.foreground)
                    .text_size(self.font_size)
                    .line_height(self.line_height)
                    .font_family(cx.theme().mono_font_family.clone())
                    .child(canvas)
                    .children(file_load_bar)
                    .children(scrollbar)
                    .children(hscrollbar)
                    .children(find_bar)
                    .children(goto_bar)
                    .children(search_panel)
                    .children(about)
                    .children(shortcuts)
                    .children(close_confirm)
                    .children(recent)
                    .child(context_menu),
            )
            .child(header)
    }
}
