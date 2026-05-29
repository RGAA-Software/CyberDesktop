//! Root `Render` implementation for the editor view.

use super::imports::*;

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
            .child(header)
    }
}
