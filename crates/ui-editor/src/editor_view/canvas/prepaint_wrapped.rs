//! Soft-wrap viewport layout.

use cyberfiles_text_engine::Position;
use gpui::{
    fill, point, prelude::*, px, rgb, App, Bounds, Entity, Font, Hsla, PaintQuad, Pixels,
    SharedString, ShapedLine, TextRun, Window,
};

use super::element::{CanvasPrepaint, EditorCanvas, WrappedRow};
use super::super::text_util::{char_to_byte, wrap_rows};
use super::syntax_paint::{build_runs, measure_rows, occurrence_word, shape_one_wrapped, word_occurrences};

pub(crate) fn prepaint_wrapped(
    canvas: &EditorCanvas,
    bounds: Bounds<Pixels>,
    font: &Font,
    default_color: Hsla,
    font_size: Pixels,
    window: &mut Window,
    cx: &mut App,
) -> CanvasPrepaint {
        let editor = canvas.editor.read(cx);
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
        canvas.editor.update(cx, |e, _| {
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
