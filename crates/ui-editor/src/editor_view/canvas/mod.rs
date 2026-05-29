//! Custom GPUI element: virtualized text surface with syntax highlighting.

use cyberfiles_text_engine::Position;
use gpui::{
    fill, point, prelude::*, px, relative, rgb, App, Bounds, Element, ElementId,
    ElementInputHandler, Entity, Font, GlobalElementId, Hsla, LayoutId, PaintQuad, Pixels,
    ShapedLine, SharedString, Style, TextRun, Window, WrappedLine,
};

use super::state::{VisibleLine, WrappedVisible};
use super::EngineEditor;
use super::text_util::{char_to_byte, wrap_rows};
use syntax_paint::{build_runs, measure_rows, occurrence_word, shape_one_wrapped, word_occurrences};

mod syntax_paint;

pub(crate) struct EditorCanvas {
    pub(crate) editor: Entity<EngineEditor>,
}

pub(crate) struct CanvasPrepaint {
    rows: Vec<VisibleRow>,
    /// Populated instead of `rows` when soft wrap is on.
    wrapped_rows: Vec<WrappedRow>,
    gutter: Vec<(Pixels, ShapedLine)>,
    selections: Vec<PaintQuad>,
    carets: Vec<PaintQuad>,
    content_left: Pixels,
    gutter_left: Pixels,
}

pub(crate) struct VisibleRow {
    line: usize,
    start_char: usize,
    top: Pixels,
    shaped: ShapedLine,
}

pub(crate) struct WrappedRow {
    line: usize,
    start_char: usize,
    top: Pixels,
    wrapped: WrappedLine,
}

impl IntoElement for EditorCanvas {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for EditorCanvas {
    type RequestLayoutState = ();
    type PrepaintState = CanvasPrepaint;

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.size.width = relative(1.0).into();
        style.size.height = relative(1.0).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        window: &mut Window,
        cx: &mut App,
    ) -> Self::PrepaintState {
        // Keep syntax current and clamp scroll/gutter to this frame's bounds.
        self.editor.update(cx, |e, _| {
            e.refresh_syntax();
            e.last_bounds = Some(bounds);
            let line_count = e.document.buffer().line_count();
            let digits = line_count.to_string().len().max(3);
            e.gutter_width = if e.show_line_numbers {
                e.font_size * (digits as f32 + 2.0) * 0.6
            } else {
                px(8.0)
            };
            let total = e.line_height * line_count as f32;
            let max = (total - bounds.size.height).max(px(0.0));
            if e.scroll_y > max {
                e.scroll_y = max;
            }
        });

        let style = window.text_style();
        let font = style.font();
        let default_color = style.color;
        let font_size = style.font_size.to_pixels(window.rem_size());

        if self.editor.read(cx).soft_wrap {
            return self.prepaint_wrapped(bounds, &font, default_color, font_size, window, cx);
        }

        let editor = self.editor.read(cx);
        let line_height = editor.line_height;
        let scroll_y = editor.scroll_y;
        let gutter_width = editor.gutter_width;
        let show_line_numbers = editor.show_line_numbers;
        let focused = editor.focus_handle.is_focused(window);
        let buf = editor.document.buffer();
        let line_count = buf.line_count();
        let digits = line_count.to_string().len().max(3);

        let primary = editor.document.selections().primary();
        let cursors = editor.document.selections().cursors();

        // Resolve horizontal caret reveal up front (needs glyph metrics): shape
        // just the caret's line to find its x, then nudge `scroll_x`.
        let view_w = (bounds.size.width - gutter_width - px(14.0)).max(px(0.0));
        let mut scroll_x = editor.scroll_x;
        if editor.reveal_caret {
            let cpos = buf.char_to_position(primary.head);
            let cline = buf.line_text(cpos.line);
            let crun = TextRun {
                len: cline.len(),
                font: font.clone(),
                color: default_color,
                background_color: None,
                underline: None,
                strikethrough: None,
            };
            let cshaped = window.text_system().shape_line(
                SharedString::from(cline.clone()),
                font_size,
                &[crun],
                None,
            );
            let caret_x = cshaped.x_for_index(char_to_byte(&cline, cpos.column));
            let margin = px(24.0);
            if caret_x < scroll_x {
                scroll_x = caret_x;
            } else if caret_x > scroll_x + view_w - margin {
                scroll_x = caret_x - view_w + margin;
            }
            scroll_x = scroll_x.max(px(0.0));
        }

        let content_left = bounds.left() + gutter_width - scroll_x;
        let gutter_left = bounds.left() + px(4.0);

        let first_line = (f32::from(scroll_y) / f32::from(line_height)).floor() as usize;
        let visible_count = (f32::from(bounds.size.height) / f32::from(line_height)).ceil() as usize + 2;
        let last_line = (first_line + visible_count).min(line_count);

        let mut rows = Vec::new();
        let mut gutter = Vec::new();
        let mut selections = Vec::new();
        let mut carets: Vec<PaintQuad> = Vec::new();
        let mut content_w = px(0.0);
        let highlight_word = occurrence_word(&editor.document);

        for line in first_line..last_line {
            let top = bounds.top() + line_height * line as f32 - scroll_y;
            let line_start_char = buf.position_to_char(Position::new(line, 0));
            let line_text = buf.line_text(line);
            let line_char_len = buf.line_len_chars(line);
            let line_end_char = line_start_char + line_char_len;
            let line_start_byte = buf.char_to_byte(line_start_char);

            let runs = build_runs(
                &editor.syntax,
                buf,
                &line_text,
                line_start_byte,
                &font,
                default_color,
            );
            let shaped = window.text_system().shape_line(
                SharedString::from(line_text.clone()),
                font_size,
                &runs,
                None,
            );
            if shaped.width > content_w {
                content_w = shaped.width;
            }

            // Same-word occurrence highlights (skip the active selection itself).
            if let Some((word, sel_range)) = &highlight_word {
                for (scol, ecol) in word_occurrences(&line_text, word) {
                    let abs_s = line_start_char + scol;
                    let abs_e = line_start_char + ecol;
                    if abs_s == sel_range.start && abs_e == sel_range.end {
                        continue;
                    }
                    let x0 = content_left + shaped.x_for_index(char_to_byte(&line_text, scol));
                    let x1 = content_left + shaped.x_for_index(char_to_byte(&line_text, ecol));
                    selections.push(fill(
                        Bounds::from_corners(point(x0, top), point(x1, top + line_height)),
                        rgb(0x4c4a2f),
                    ));
                }
            }

            // Selection bands + carets for every cursor on this line.
            for cur in cursors {
                let range = cur.range();
                if !cur.is_empty() && range.start <= line_end_char && range.end > line_start_char {
                    let start_col = range.start.max(line_start_char) - line_start_char;
                    let end_col = range.end.min(line_end_char) - line_start_char;
                    let x0 = content_left + shaped.x_for_index(char_to_byte(&line_text, start_col));
                    let x1 = content_left + shaped.x_for_index(char_to_byte(&line_text, end_col));
                    selections.push(fill(
                        Bounds::from_corners(point(x0, top), point(x1, top + line_height)),
                        rgb(0x264f78),
                    ));
                }
                if focused && cur.head >= line_start_char && cur.head <= line_end_char {
                    let col = cur.head - line_start_char;
                    let cx_pos = content_left + shaped.x_for_index(char_to_byte(&line_text, col));
                    carets.push(fill(
                        Bounds::new(point(cx_pos, top), gpui::size(px(2.0), line_height)),
                        rgb(0xaeafad),
                    ));
                }
            }

            // Gutter line number.
            if show_line_numbers {
                let num = format!("{:>width$} ", line + 1, width = digits);
                let grun = TextRun {
                    len: num.len(),
                    font: font.clone(),
                    color: rgb(0x6e7681).into(),
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                };
                let gshaped =
                    window
                        .text_system()
                        .shape_line(SharedString::from(num), font_size, &[grun], None);
                gutter.push((top, gshaped));
            }

            rows.push(VisibleRow {
                line,
                start_char: line_start_char,
                top,
                shaped,
            });
        }

        // Add one character of right padding so the caret at line end is visible.
        let content_w = content_w + line_height * 0.6;
        // End the read borrow before mutating the entity.
        let _ = (buf, &editor);
        self.editor.update(cx, |e, _| {
            e.content_width = content_w;
            let max_x = (content_w - view_w).max(px(0.0));
            e.scroll_x = scroll_x.min(max_x).max(px(0.0));
            e.reveal_caret = false;
        });

        CanvasPrepaint {
            rows,
            wrapped_rows: Vec::new(),
            gutter,
            selections,
            carets,
            content_left,
            gutter_left,
        }
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        _request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        let (focus_handle, line_height, gutter_width) = {
            let e = self.editor.read(cx);
            (e.focus_handle.clone(), e.line_height, e.gutter_width)
        };

        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, self.editor.clone()),
            cx,
        );

        // Clip the (horizontally scrollable) text + selections so they never
        // bleed over the fixed line-number gutter or the vertical scrollbar.
        let content_mask = gpui::ContentMask {
            bounds: Bounds::from_corners(
                point(bounds.left() + gutter_width, bounds.top()),
                point(bounds.right(), bounds.bottom()),
            ),
        };
        window.with_content_mask(Some(content_mask), |window| {
            for quad in prepaint.selections.drain(..) {
                window.paint_quad(quad);
            }
            for row in &prepaint.rows {
                let _ = row.shaped.paint(
                    point(prepaint.content_left, row.top),
                    line_height,
                    gpui::TextAlign::Left,
                    None,
                    window,
                    cx,
                );
            }
            for row in &prepaint.wrapped_rows {
                let _ = row.wrapped.paint(
                    point(prepaint.content_left, row.top),
                    line_height,
                    gpui::TextAlign::Left,
                    None,
                    window,
                    cx,
                );
            }
            for caret in prepaint.carets.drain(..) {
                window.paint_quad(caret);
            }
        });

        for (top, shaped) in &prepaint.gutter {
            let _ = shaped.paint(
                point(prepaint.gutter_left, *top),
                line_height,
                gpui::TextAlign::Left,
                None,
                window,
                cx,
            );
        }

        let visible: Vec<VisibleLine> = prepaint
            .rows
            .drain(..)
            .map(|r| VisibleLine {
                line: r.line,
                start_char: r.start_char,
                top: r.top,
                shaped: r.shaped,
            })
            .collect();
        let wrapped_visible: Vec<WrappedVisible> = prepaint
            .wrapped_rows
            .drain(..)
            .map(|r| WrappedVisible {
                line: r.line,
                start_char: r.start_char,
                top: r.top,
                wrapped: r.wrapped,
            })
            .collect();
        self.editor.update(cx, |e, _| {
            e.visible = visible;
            e.wrapped_visible = wrapped_visible;
        });
    }
}

impl EditorCanvas {
    /// Lays out the visible region in soft-wrap mode. The viewport is anchored at
    /// `wrap_top_line` + `wrap_top_off`, so layout cost is O(visible rows) — no
    /// full-document wrap pass, even for huge files.
    fn prepaint_wrapped(
        &mut self,
        bounds: Bounds<Pixels>,
        font: &Font,
        default_color: Hsla,
        font_size: Pixels,
        window: &mut Window,
        cx: &mut App,
    ) -> CanvasPrepaint {
        let editor = self.editor.read(cx);
        let line_height = editor.line_height;
        let lh = line_height;
        let gutter_width = editor.gutter_width;
        let show_line_numbers = editor.show_line_numbers;
        let focused = editor.focus_handle.is_focused(window);
        let buf = editor.document.buffer();
        let line_count = buf.line_count();
        let digits = line_count.to_string().len().max(3);
        let cursors = editor.document.selections().cursors();
        let syntax = &editor.syntax;

        let view_w = (bounds.size.width - gutter_width - px(14.0)).max(px(0.0));
        let content_left = bounds.left() + gutter_width;
        let gutter_left = bounds.left() + px(4.0);

        // Normalize the anchor so `off` lands within `top_line`'s block; only
        // measures lines adjacent to the viewport.
        let mut top_line = editor.wrap_top_line.min(line_count.saturating_sub(1));
        let mut off = editor.wrap_top_off;
        loop {
            if off < px(0.0) {
                if top_line == 0 {
                    off = px(0.0);
                    break;
                }
                top_line -= 1;
                let rows = measure_rows(window, &buf.line_text(top_line), font, font_size, view_w);
                off += lh * rows as f32;
                continue;
            }
            let rows = measure_rows(window, &buf.line_text(top_line), font, font_size, view_w);
            let block = lh * rows as f32;
            if off >= block {
                if top_line + 1 >= line_count {
                    off = (block - lh).max(px(0.0));
                    break;
                }
                off -= block;
                top_line += 1;
                continue;
            }
            break;
        }

        let mut wrapped_rows: Vec<WrappedRow> = Vec::new();
        let mut gutter: Vec<(Pixels, ShapedLine)> = Vec::new();
        let mut selections: Vec<PaintQuad> = Vec::new();
        let mut carets: Vec<PaintQuad> = Vec::new();
        let right = content_left + view_w;
        let highlight_word = occurrence_word(&editor.document);

        let mut y = bounds.top() - off;
        let mut line = top_line;
        let mut bottom_line = top_line;
        while y < bounds.bottom() && line < line_count {
            let line_start_char = buf.position_to_char(Position::new(line, 0));
            let line_text = buf.line_text(line);
            let line_char_len = buf.line_len_chars(line);
            let line_end_char = line_start_char + line_char_len;
            let line_start_byte = buf.char_to_byte(line_start_char);

            let runs = build_runs(syntax, buf, &line_text, line_start_byte, font, default_color);
            let Some(wrapped) = shape_one_wrapped(window, &line_text, &runs, font_size, view_w)
            else {
                line += 1;
                continue;
            };
            let rows = wrap_rows(&wrapped);
            let block = lh * rows as f32;

            if let Some((word, sel_range)) = &highlight_word {
                for (scol, ecol) in word_occurrences(&line_text, word) {
                    let abs_s = line_start_char + scol;
                    let abs_e = line_start_char + ecol;
                    if abs_s == sel_range.start && abs_e == sel_range.end {
                        continue;
                    }
                    let s_byte = char_to_byte(&line_text, scol);
                    let e_byte = char_to_byte(&line_text, ecol);
                    if let (Some(p0), Some(p1)) = (
                        wrapped.position_for_index(s_byte, lh),
                        wrapped.position_for_index(e_byte, lh),
                    ) {
                        if (f32::from(p0.y) - f32::from(p1.y)).abs() < 0.5 {
                            selections.push(fill(
                                Bounds::from_corners(
                                    point(content_left + p0.x, y + p0.y),
                                    point(content_left + p1.x, y + p0.y + lh),
                                ),
                                rgb(0x4c4a2f),
                            ));
                        }
                    }
                }
            }

            for cur in cursors {
                let range = cur.range();
                if !cur.is_empty() && range.start <= line_end_char && range.end > line_start_char {
                    let s_col = range.start.max(line_start_char) - line_start_char;
                    let e_col = range.end.min(line_end_char) - line_start_char;
                    let s_byte = char_to_byte(&line_text, s_col);
                    let e_byte = char_to_byte(&line_text, e_col);
                    let p0 = wrapped
                        .position_for_index(s_byte, lh)
                        .unwrap_or(point(px(0.0), px(0.0)));
                    let p1 = wrapped
                        .position_for_index(e_byte, lh)
                        .unwrap_or(point(view_w, lh * (rows.saturating_sub(1)) as f32));
                    let band = |x0: Pixels, x1: Pixels, top: Pixels| {
                        fill(
                            Bounds::from_corners(point(x0, top), point(x1, top + lh)),
                            rgb(0x264f78),
                        )
                    };
                    let row0 = (f32::from(p0.y) / f32::from(lh)).round() as i32;
                    let row1 = (f32::from(p1.y) / f32::from(lh)).round() as i32;
                    if row0 == row1 {
                        selections.push(band(content_left + p0.x, content_left + p1.x, y + p0.y));
                    } else {
                        selections.push(band(content_left + p0.x, right, y + p0.y));
                        for r in (row0 + 1)..row1 {
                            selections.push(band(content_left, right, y + lh * r as f32));
                        }
                        selections.push(band(content_left, content_left + p1.x, y + p1.y));
                    }
                }
                if focused && cur.head >= line_start_char && cur.head <= line_end_char {
                    let col = cur.head - line_start_char;
                    let b = char_to_byte(&line_text, col);
                    if let Some(p) = wrapped.position_for_index(b, lh) {
                        carets.push(fill(
                            Bounds::new(
                                point(content_left + p.x, y + p.y),
                                gpui::size(px(2.0), lh),
                            ),
                            rgb(0xaeafad),
                        ));
                    }
                }
            }

            if show_line_numbers {
                let num = format!("{:>width$} ", line + 1, width = digits);
                let grun = TextRun {
                    len: num.len(),
                    font: font.clone(),
                    color: rgb(0x6e7681).into(),
                    background_color: None,
                    underline: None,
                    strikethrough: None,
                };
                let gshaped =
                    window
                        .text_system()
                        .shape_line(SharedString::from(num), font_size, &[grun], None);
                gutter.push((y, gshaped));
            }

            wrapped_rows.push(WrappedRow {
                line,
                start_char: line_start_char,
                top: y,
                wrapped,
            });
            bottom_line = line;
            y += block;
            line += 1;
        }

        let _ = (buf, &editor);
        self.editor.update(cx, |e, _| {
            e.wrap_top_line = top_line;
            e.wrap_top_off = off;
            e.wrap_bottom_line = bottom_line;
            e.content_width = px(0.0);
            e.scroll_x = px(0.0);
            e.reveal_caret = false;
        });

        CanvasPrepaint {
            rows: Vec::new(),
            wrapped_rows,
            gutter,
            selections,
            carets,
            content_left,
            gutter_left,
        }
    }
}
