//! Non-wrapped (horizontal scroll) viewport layout.

use cyberfiles_text_engine::Position;
use gpui::{
    fill, point, prelude::*, px, App, Bounds, Font, Hsla, PaintQuad, Pixels, SharedString,
    TextRun, Window,
};

use super::element::{CanvasPrepaint, EditorCanvas, VisibleRow};
use super::horizontal_viewport::{
    caret_x_from_col, estimated_line_width, measure_avg_char_width, viewport_col_range,
    LONG_LINE_COL_THRESHOLD,
};
use super::super::editor::EngineEditor;
use super::super::r#impl::FOLD_GUTTER_WIDTH;
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

    let view_w = (bounds.size.width - gutter_width - px(14.0)).max(px(0.0));
    let bottom_inset = if editor.needs_hscrollbar_lane(view_w) {
        EngineEditor::HSCROLLBAR_HEIGHT
    } else {
        px(0.0)
    };
    let viewport_h = (bounds.size.height - bottom_inset).max(px(0.0));
    let content_bottom = bounds.bottom() - bottom_inset;
    let mut scroll_x = editor.scroll_x;
    let char_width = measure_avg_char_width(window, font, font_size);

    if editor.reveal_caret {
        let cpos = buf.char_to_position(primary.head);
        let line_len = buf.line_len_chars(cpos.line);
        let caret_x = if line_len > LONG_LINE_COL_THRESHOLD {
            caret_x_from_col(char_width, cpos.column)
        } else {
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
            cshaped.x_for_index(char_to_byte(&cline, cpos.column))
        };
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
    let fold_left = bounds.left() + gutter_width - FOLD_GUTTER_WIDTH;

    let display_lines = if editor.display_lines.is_empty() {
        (0..line_count).collect::<Vec<_>>()
    } else {
        editor.display_lines.clone()
    };
    let display_count = display_lines.len().max(1);
    let first_display = (f32::from(scroll_y) / f32::from(line_height)).floor() as usize;
    let visible_count = (f32::from(viewport_h) / f32::from(line_height)).ceil() as usize + 2;
    let last_display = (first_display + visible_count).min(display_count);

    let mut rows = Vec::new();
    let mut gutter = Vec::new();
    let mut fold_gutter = Vec::new();
    let mut selections = Vec::new();
    let mut carets: Vec<PaintQuad> = Vec::new();
    let mut content_w = px(0.0);
    let highlight_word = occurrence_word(&editor.document);

    for dix in first_display..last_display {
        let Some(line) = display_lines.get(dix).copied() else {
            break;
        };
        let top = bounds.top()
            + line_height * (dix - first_display) as f32
            - (scroll_y - line_height * first_display as f32);
        if top >= content_bottom {
            break;
        }
        let line_start_char = buf.position_to_char(Position::new(line, 0));
        let line_char_len = buf.line_len_chars(line);
        let line_end_char = line_start_char + line_char_len;
        let is_long = line_char_len > LONG_LINE_COL_THRESHOLD;
        let collapsed = editor.is_folded_header(line);

        let (col_start, line_text) = if is_long {
            let (cs, ce) = viewport_col_range(scroll_x, view_w, char_width, line_char_len);
            let mut frag = buf.line_chars_slice(line, cs, ce);
            if collapsed && ce >= line_char_len {
                frag.push_str("  \u{2026}");
            }
            (cs, frag)
        } else {
            let mut lt = buf.line_text(line);
            if collapsed {
                lt.push_str("  \u{2026}");
            }
            (0, lt)
        };
        let fragment_left = content_left + caret_x_from_col(char_width, col_start);
        let fragment_start_byte = buf.char_to_byte(line_start_char + col_start);
        let frag_char_len = line_text.chars().count();
        let col_end = col_start + frag_char_len;

        let runs = build_runs(
            &editor.syntax,
            buf,
            &line_text,
            fragment_start_byte,
            &font,
            default_color,
        );
        let shaped = window.text_system().shape_line(
            SharedString::from(line_text.clone()),
            font_size,
            &runs,
            None,
        );
        let line_width = if is_long {
            estimated_line_width(char_width, line_char_len)
        } else {
            shaped.width
        };
        if line_width > content_w {
            content_w = line_width;
        }

        if let Some((word, sel_range)) = &highlight_word {
            for (scol, ecol) in word_occurrences(&line_text, word) {
                let abs_s = line_start_char + col_start + scol;
                let abs_e = line_start_char + col_start + ecol;
                if abs_s == sel_range.start && abs_e == sel_range.end {
                    continue;
                }
                let x0 = fragment_left + shaped.x_for_index(char_to_byte(&line_text, scol));
                let x1 = fragment_left + shaped.x_for_index(char_to_byte(&line_text, ecol));
                selections.push(fill(
                    Bounds::from_corners(point(x0, top), point(x1, top + line_height)),
                    colors.occurrence,
                ));
            }
        }

        for cur in cursors {
            let range = cur.range();
            if !cur.is_empty() && range.start <= line_end_char && range.end > line_start_char {
                let start_col = range.start.max(line_start_char) - line_start_char;
                let end_col = range.end.min(line_end_char) - line_start_char;
                if end_col > col_start && start_col < col_end {
                    let local_start = start_col.saturating_sub(col_start);
                    let local_end = (end_col - col_start).min(frag_char_len);
                    let x0 =
                        fragment_left + shaped.x_for_index(char_to_byte(&line_text, local_start));
                    let x1 =
                        fragment_left + shaped.x_for_index(char_to_byte(&line_text, local_end));
                    selections.push(fill(
                        Bounds::from_corners(point(x0, top), point(x1, top + line_height)),
                        colors.selection,
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
                if col >= col_start && col <= col_end {
                    let local_col = col - col_start;
                    let cx_pos =
                        fragment_left + shaped.x_for_index(char_to_byte(&line_text, local_col));
                    carets.push(fill(
                        Bounds::new(point(cx_pos, top), gpui::size(px(2.0), line_height)),
                        colors.caret,
                    ));
                }
            }
        }

        if collapsed || editor.crease_at(line).is_some() {
            fold_gutter.push((top, collapsed));
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
            gutter.push((top, gshaped));
        }

        rows.push(VisibleRow {
            line,
            start_char: line_start_char,
            start_col: col_start,
            fragment_text: line_text,
            fragment_left,
            top,
            shaped,
        });
    }

    let content_w = content_w + line_height * 0.6;

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
            e.display_line_count(),
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
        fold_gutter,
        selections,
        carets,
        content_left,
        gutter_left,
        fold_left,
    }
}
