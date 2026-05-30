//! `EngineEditor` — `mouse_scroll`.

use super::super::imports::*;

impl EngineEditor {
    pub(crate) fn index_for_position(&self, pos: Point<Pixels>) -> usize {
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
    pub(crate) fn wrap_index_for_position(&self, pos: Point<Pixels>, bounds: Bounds<Pixels>) -> usize {
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

    pub(crate) fn on_mouse_down(&mut self, event: &MouseDownEvent, window: &mut Window, cx: &mut Context<Self>) {
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

    pub(crate) fn on_mouse_move(&mut self, event: &MouseMoveEvent, _window: &mut Window, cx: &mut Context<Self>) {
        // If the button was released while the cursor was outside the window we
        // never saw a mouse-up; the next move with no button pressed ends any
        // in-progress drag/selection so it doesn't "stick" on re-entry.
        if event.pressed_button != Some(MouseButton::Left)
            && (self.scrollbar_drag.is_some()
                || self.hscrollbar_drag.is_some()
                || self.panel_drag.is_some()
                || self.panel_resize.is_some()
                || self.is_selecting)
        {
            self.scrollbar_drag = None;
            self.hscrollbar_drag = None;
            self.end_panel_drag();
            self.end_panel_resize();
            self.is_selecting = false;
            cx.notify();
            return;
        }
        if self.panel_resize.is_some() {
            if event.pressed_button == Some(MouseButton::Left) {
                self.resize_search_panel(event, cx);
            }
            return;
        }
        if self.panel_drag.is_some() {
            if event.pressed_button == Some(MouseButton::Left) {
                self.drag_panel(event, cx);
            }
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

    pub(crate) fn on_mouse_up(&mut self, _event: &MouseUpEvent, _window: &mut Window, _cx: &mut Context<Self>) {
        self.is_selecting = false;
        self.scrollbar_drag = None;
        self.hscrollbar_drag = None;
        self.end_panel_drag();
        self.end_panel_resize();
    }

    pub(crate) fn on_scroll(&mut self, event: &ScrollWheelEvent, window: &mut Window, cx: &mut Context<Self>) {
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

}
