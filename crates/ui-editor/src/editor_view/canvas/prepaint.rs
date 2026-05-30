//! Non-wrapped (horizontal scroll) viewport layout.

use cyberfiles_text_engine::Position;
use gpui::{
    fill, point, prelude::*, px, rgb, App, Bounds, Entity, Font, Hsla, PaintQuad, Pixels,
    SharedString, ShapedLine, TextRun, Window,
};

use super::element::{CanvasPrepaint, EditorCanvas, VisibleRow};
use super::super::editor::EngineEditor;
use super::super::ui::EditorColors;
use super::super::text_util::char_to_byte;
use super::syntax_paint::{build_runs, occurrence_word, word_occurrences};

pub(crate) fn prepaint_normal(
    canvas: &EditorCanvas,
    bounds: Bounds<Pixels>,
    font: &Font,
    colors: EditorColors,
    default_color: Hsla,
    font_size: Pixels,
    window: &mut Window,
    cx: &mut App,
) -> CanvasPrepaint {
        let editor = canvas.editor.read(cx);
        let line_height = editor.line_height;
        let scroll_y = editor.scroll_y;
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

        // Resolve horizontal caret reveal up front (needs glyph metrics): shape
        // just the caret's line to find its x, then nudge `scroll_x`.
        let view_w = (bounds.size.width - gutter_width - px(14.0)).max(px(0.0));
        let bottom_inset = if editor.needs_hscrollbar_lane(view_w) {
            EngineEditor::HSCROLLBAR_HEIGHT
        } else {
            px(0.0)
        };
        let viewport_h = (bounds.size.height - bottom_inset).max(px(0.0));
        let content_bottom = bounds.bottom() - bottom_inset;
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
        let visible_count = (f32::from(viewport_h) / f32::from(line_height)).ceil() as usize + 2;
        let last_line = (first_line + visible_count).min(line_count);

        let mut rows = Vec::new();
        let mut gutter = Vec::new();
        let mut selections = Vec::new();
        let mut carets: Vec<PaintQuad> = Vec::new();
        let mut content_w = px(0.0);
        let highlight_word = occurrence_word(&editor.document);

        for line in first_line..last_line {
            let top = bounds.top() + line_height * line as f32 - scroll_y;
            // Skip lines fully below the text lane (keep the line that ends at
            // `content_bottom`; strict `>` was dropping the last row at scroll end).
            if top >= content_bottom {
                break;
            }
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
                        colors.occurrence,
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
                        colors.selection,
                    ));
                }
                if focused
                    && caret_blink_visible
                    && cur.is_empty()
                    && cur.head >= line_start_char
                    && cur.head <= line_end_char
                {
                    let col = cur.head - line_start_char;
                    let cx_pos = content_left + shaped.x_for_index(char_to_byte(&line_text, col));
                    carets.push(fill(
                        Bounds::new(point(cx_pos, top), gpui::size(px(2.0), line_height)),
                        colors.caret,
                    ));
                }
            }

            // Gutter line number.
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
        canvas.editor.update(cx, |e, _| {
            e.content_width = content_w;
            e.reserve_hscrollbar_lane =
                !e.soft_wrap && (content_w > view_w || e.scroll_x > px(0.0));
            let lane_inset = if e.reserve_hscrollbar_lane {
                EngineEditor::HSCROLLBAR_HEIGHT
            } else {
                px(0.0)
            };
            e.scroll_y = EngineEditor::clamp_scroll_y_for_lane(
                e.scroll_y,
                e.line_height,
                line_count,
                bounds.size.height,
                lane_inset,
            );
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
