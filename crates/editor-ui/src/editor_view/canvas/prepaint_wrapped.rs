//! Soft-wrap viewport layout.

use editor_text_engine::Position;
use gpui::{
    fill, point, px, Bounds, Font, Hsla, PaintQuad, Pixels, SharedString, ShapedLine,
    TextRun, Window,
};

use super::element::{CanvasPrepaint, EditorCanvas, WrappedRow};
use super::horizontal_viewport::measure_avg_char_width;
use super::super::state::LONG_LINE_COL_THRESHOLD;
use super::super::ui::EditorColors;
use super::super::text_util::{expand_tabs, wrap_rows, EDITOR_TAB_SIZE};
use super::syntax_paint::{build_runs, measure_rows, occurrence_word, shape_one_wrapped, word_occurrences};
use super::wrap_virtual::{
    char_range_for_wrap_subrows, cols_per_row, estimated_wrap_rows, visible_subrow_range,
};

pub(crate) fn prepaint_wrapped(
    canvas: &EditorCanvas,
    bounds: Bounds<Pixels>,
    font: &Font,
    colors: EditorColors,
    default_color: Hsla,
    font_size: Pixels,
    window: &mut Window,
    cx: &mut gpui::App,
) -> CanvasPrepaint {
    let editor = canvas.editor.read(cx);
    let line_height = editor.line_height;
    let lh = line_height;
    let gutter_width = editor.gutter_width;
    let show_line_numbers = editor.show_line_numbers;
    let focused = editor.focus_handle.is_focused(window);
    let caret_blink_visible = editor.caret_blink_visible;
    let buf = editor.document.buffer();
    let line_count = buf.line_count();
    let digits = line_count.to_string().len().max(3);
    let primary = editor.document.selections().primary();
    let caret_line = buf.char_to_position(primary.head).line;
    let cursors = editor.document.selections().cursors();
    let syntax = &editor.syntax;

    let view_w = (bounds.size.width - gutter_width - px(14.0)).max(px(0.0));
    let content_bottom = bounds.bottom();
    let content_top = bounds.top();
    let content_left = bounds.left() + gutter_width;
    let gutter_left = bounds.left() + px(4.0);
    let char_width = measure_avg_char_width(window, font, font_size);

    let mut top_line = editor.wrap_top_line.min(line_count.saturating_sub(1));
    let mut off = editor.wrap_top_off;
    loop {
        if off < px(0.0) {
            if top_line == 0 {
                off = px(0.0);
                break;
            }
            top_line -= 1;
            let rows = wrap_rows_for_line(buf, top_line, window, font, font_size, view_w, char_width);
            off += lh * rows as f32;
            continue;
        }
        let rows = wrap_rows_for_line(buf, top_line, window, font, font_size, view_w, char_width);
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
    while y < content_bottom && line < line_count {
        let line_start_char = buf.position_to_char(Position::new(line, 0));
        let line_char_len = buf.line_len_chars(line);
        let line_end_char = line_start_char + line_char_len;
        let is_long = line_char_len > LONG_LINE_COL_THRESHOLD;
        let cols = cols_per_row(char_width, view_w);
        let rows = if is_long {
            estimated_wrap_rows(line_char_len, cols)
        } else {
            measure_rows(window, &buf.line_text(line), font, font_size, view_w)
        };
        let block = lh * rows as f32;

        if y + block <= content_top {
            bottom_line = line;
            y += block;
            line += 1;
            continue;
        }

        let (line_text, wrapped, paint_top, start_col, wrap_row_count) = if is_long {
            let (first_sub, last_sub) =
                visible_subrow_range(y, content_top, content_bottom, lh, rows);
            let (col_start, col_end) =
                char_range_for_wrap_subrows(first_sub, last_sub, cols, line_char_len);
            let fragment = buf.line_chars_slice(line, col_start, col_end);
            let frag_byte = buf.char_to_byte(line_start_char + col_start);
            let expanded = expand_tabs(&fragment, EDITOR_TAB_SIZE);
            let runs = build_runs(syntax, buf, &fragment, Some(&expanded), frag_byte, font, default_color);
            let Some(wrapped) = shape_one_wrapped(window, &expanded.text, &runs, font_size, view_w) else {
                bottom_line = line;
                y += block;
                line += 1;
                continue;
            };
            let paint_top = y + lh * first_sub as f32;
            (
                fragment,
                wrapped,
                paint_top,
                col_start,
                rows,
            )
        } else {
            let line_text = buf.line_text(line);
            let line_start_byte = buf.char_to_byte(line_start_char);
            let expanded = expand_tabs(&line_text, EDITOR_TAB_SIZE);
            let runs = build_runs(syntax, buf, &line_text, Some(&expanded), line_start_byte, font, default_color);
            let Some(wrapped) = shape_one_wrapped(window, &expanded.text, &runs, font_size, view_w) else {
                bottom_line = line;
                y += block;
                line += 1;
                continue;
            };
            (line_text, wrapped, y, 0usize, 0usize)
        };

        let shaped_rows = wrap_rows(&wrapped);
        let text_base = paint_top;

        if let Some((word, sel_range)) = &highlight_word {
            let expanded = expand_tabs(&line_text, EDITOR_TAB_SIZE);
            for (scol, ecol) in word_occurrences(&line_text, word) {
                let abs_s = line_start_char + start_col + scol;
                let abs_e = line_start_char + start_col + ecol;
                if abs_s == sel_range.start && abs_e == sel_range.end {
                    continue;
                }
                let s_byte = expanded.original_char_to_expanded_byte(scol);
                let e_byte = expanded.original_char_to_expanded_byte(ecol);
                if let (Some(p0), Some(p1)) = (
                    wrapped.position_for_index(s_byte, lh),
                    wrapped.position_for_index(e_byte, lh),
                ) {
                    if (f32::from(p0.y) - f32::from(p1.y)).abs() < 0.5 {
                        selections.push(fill(
                            Bounds::from_corners(
                                point(content_left + p0.x, text_base + p0.y),
                                point(content_left + p1.x, text_base + p0.y + lh),
                            ),
                            colors.occurrence,
                        ));
                    }
                }
            }
        }

        for cur in cursors {
            let expanded = expand_tabs(&line_text, EDITOR_TAB_SIZE);
            let range = cur.range();
            if !cur.is_empty() && range.start <= line_end_char && range.end > line_start_char {
                let s_col = range.start.max(line_start_char) - line_start_char;
                let e_col = range.end.min(line_end_char) - line_start_char;
                if is_long && (e_col <= start_col || s_col >= start_col + line_text.chars().count())
                {
                    continue;
                }
                let local_s = if is_long { s_col.saturating_sub(start_col) } else { s_col };
                let local_e = if is_long {
                    (e_col - start_col).min(line_text.chars().count())
                } else {
                    e_col
                };
                let s_byte = expanded.original_char_to_expanded_byte(local_s);
                let e_byte = expanded.original_char_to_expanded_byte(local_e);
                let p0 = wrapped
                    .position_for_index(s_byte, lh)
                    .unwrap_or(point(px(0.0), px(0.0)));
                let p1 = wrapped
                    .position_for_index(e_byte, lh)
                    .unwrap_or(point(view_w, lh * (shaped_rows.saturating_sub(1)) as f32));
                let band = |x0: Pixels, x1: Pixels, top: Pixels| {
                    fill(
                        Bounds::from_corners(point(x0, top), point(x1, top + lh)),
                        colors.selection,
                    )
                };
                let row0 = (f32::from(p0.y) / f32::from(lh)).round() as i32;
                let row1 = (f32::from(p1.y) / f32::from(lh)).round() as i32;
                if row0 == row1 {
                    selections.push(band(
                        content_left + p0.x,
                        content_left + p1.x,
                        text_base + p0.y,
                    ));
                } else {
                    selections.push(band(content_left + p0.x, right, text_base + p0.y));
                    for r in (row0 + 1)..row1 {
                        selections.push(band(content_left, right, text_base + lh * r as f32));
                    }
                    selections.push(band(
                        content_left,
                        content_left + p1.x,
                        text_base + p1.y,
                    ));
                }
            }
            if focused
                && caret_blink_visible
                && cur.is_empty()
                && cur.head >= line_start_char
                && cur.head <= line_end_char
            {
                let col = cur.head - line_start_char;
                if is_long && (col < start_col || col >= start_col + line_text.chars().count()) {
                    continue;
                }
                let local_col = if is_long { col - start_col } else { col };
                let b = expanded.original_char_to_expanded_byte(local_col);
                if let Some(p) = wrapped.position_for_index(b, lh) {
                    carets.push(fill(
                        Bounds::new(
                            point(content_left + p.x, text_base + p.y),
                            gpui::size(px(2.0), lh),
                        ),
                        colors.caret,
                    ));
                }
            }
        }

        if show_line_numbers {
            let num = format!("{:>width$} ", line + 1, width = digits);
            let line_num_color = if line == caret_line {
                colors.active_line_number
            } else {
                colors.line_number
            };
            let grun = TextRun {
                len: num.len(),
                font: font.clone(),
                color: line_num_color,
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
            block_top: y,
            top: paint_top,
            start_col,
            fragment_text: if is_long { line_text.clone() } else { String::new() },
            wrap_row_count,
            wrapped,
        });
        bottom_line = line;
        y += block;
        line += 1;
    }

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
        fold_gutter: Vec::new(),
        selections,
        carets,
        content_left,
        gutter_left,
        fold_left: bounds.left() + gutter_width - super::super::r#impl::FOLD_GUTTER_WIDTH,
    }
}

fn wrap_rows_for_line(
    buf: &editor_text_engine::TextBuffer,
    line: usize,
    window: &mut Window,
    font: &Font,
    font_size: Pixels,
    view_w: Pixels,
    char_width: Pixels,
) -> usize {
    let len = buf.line_len_chars(line);
    if len > LONG_LINE_COL_THRESHOLD {
        estimated_wrap_rows(len, cols_per_row(char_width, view_w))
    } else {
        let expanded = expand_tabs(&buf.line_text(line), EDITOR_TAB_SIZE);
        measure_rows(window, &expanded.text, font, font_size, view_w)
    }
}
