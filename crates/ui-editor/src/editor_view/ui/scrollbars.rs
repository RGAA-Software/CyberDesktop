//! UI fragment: `ui/scrollbars.rs`.

use super::super::imports::*;

impl EngineEditor {
    /// Height reserved at the bottom when the horizontal scrollbar is shown.
    pub(crate) const HSCROLLBAR_HEIGHT: Pixels = px(12.0);

    /// True when a horizontal scrollbar is painted (non-wrap mode, content wider than viewport).
    pub(crate) fn shows_hscrollbar(&self) -> bool {
        if self.soft_wrap {
            return false;
        }
        let Some(bounds) = self.last_bounds else {
            return false;
        };
        let track = (bounds.size.width - self.gutter_width - px(14.0)).max(px(0.0));
        self.content_width > track
    }

    /// Whether the bottom lane for the horizontal scrollbar should be reserved this frame.
    pub(crate) fn needs_hscrollbar_lane(&self, view_w: Pixels) -> bool {
        if self.soft_wrap {
            return false;
        }
        self.reserve_hscrollbar_lane
            || self.scroll_x > px(0.0)
            || self.content_width > view_w
    }

    /// Bottom padding so the last line is not covered by the horizontal scrollbar.
    pub(crate) fn editor_bottom_inset(&self) -> Pixels {
        let view_w = self.view_width();
        if self.needs_hscrollbar_lane(view_w) {
            Self::HSCROLLBAR_HEIGHT
        } else {
            px(0.0)
        }
    }

    /// Clamps vertical scroll; when the horizontal scrollbar lane appears, nudges
    /// scroll to the new bottom so the last document line stays visible.
    pub(crate) fn clamp_scroll_y_for_lane(
        scroll_y: Pixels,
        line_height: Pixels,
        line_count: usize,
        bounds_height: Pixels,
        lane_inset: Pixels,
    ) -> Pixels {
        let total = line_height * line_count as f32;
        let viewport_h = (bounds_height - lane_inset).max(px(0.0));
        let max_scroll = (total - viewport_h).max(px(0.0));
        let full_max = (total - bounds_height).max(px(0.0));
        let mut y = scroll_y;
        if lane_inset > px(0.0) && y >= full_max - px(1.0) {
            y = max_scroll;
        } else if y > max_scroll {
            y = max_scroll;
        }
        y
    }

    pub(crate) fn max_scroll_x(&self) -> Pixels {
        let Some(b) = self.last_bounds else {
            return px(0.0);
        };
        let view_w = (b.size.width - self.gutter_width - px(14.0)).max(px(0.0));
        (self.content_width - view_w).max(px(0.0))
    }

    pub(crate) fn scrollbar_metrics(&self) -> Option<ScrollbarMetrics> {
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
    pub(crate) fn apply_vertical_scroll(&mut self, pos: Pixels, max_scroll: Pixels) {
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

    pub(crate) fn hscrollbar_metrics(&self) -> Option<HScrollbarMetrics> {
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

    pub(crate) fn render_hscrollbar(&self, cx: &mut Context<Self>) -> Option<Stateful<gpui::Div>> {
        let thumb = cx.theme().scrollbar_thumb;
        let thumb_hover = cx.theme().scrollbar_thumb_hover;
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
                        .bg(thumb)
                        .hover(|s| s.bg(thumb_hover))
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

    pub(crate) fn render_scrollbar(&self, cx: &mut Context<Self>) -> Option<Stateful<gpui::Div>> {
        let thumb = cx.theme().scrollbar_thumb;
        let thumb_hover = cx.theme().scrollbar_thumb_hover;
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
                        .bg(thumb)
                        .hover(|s| s.bg(thumb_hover))
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
    pub(crate) fn vertical_scroll_pos(&self) -> Pixels {
        if self.soft_wrap {
            self.line_height * self.wrap_top_line as f32 + self.wrap_top_off
        } else {
            self.scroll_y
        }
    }
}
