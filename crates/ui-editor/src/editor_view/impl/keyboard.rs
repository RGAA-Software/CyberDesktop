//! `EngineEditor` — `keyboard`.

use super::super::imports::*;

impl EngineEditor {
    pub(crate) fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
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

}
