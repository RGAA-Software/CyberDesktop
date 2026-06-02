//! Paint shaped lines, selections, carets, and gutter.

use gpui::{
    fill, point, App, Bounds, CursorStyle, Element, ElementInputHandler, GlobalElementId, Pixels,
    Window,
};

use super::element::{EditorCanvas, EditorCanvasPrepaint};
use super::fold_icon;
use super::super::state::{VisibleLine, WrappedVisible};

pub(crate) fn paint(
    canvas: &mut EditorCanvas,
    _id: Option<&GlobalElementId>,
    _inspector_id: Option<&gpui::InspectorElementId>,
    bounds: Bounds<Pixels>,
    _request_layout: &mut <EditorCanvas as Element>::RequestLayoutState,
    prepaint: &mut EditorCanvasPrepaint,
    window: &mut Window,
    cx: &mut App,
) {
        let content_cursor = if canvas.editor.read(cx).external_file_drop_hover {
            CursorStyle::PointingHand
        } else {
            CursorStyle::IBeam
        };
        let fold_hover_line = canvas.editor.read(cx).fold_gutter_hover_line;
        set_editor_cursors(prepaint, window, content_cursor, fold_hover_line.is_some());
        let fold_left = prepaint.canvas.fold_left;
        let canvas_prepaint = &mut prepaint.canvas;
        let (focus_handle, line_height, gutter_width, bottom_inset) = {
            let e = canvas.editor.read(cx);
            (
                e.focus_handle.clone(),
                e.line_height,
                e.gutter_width,
                e.editor_bottom_inset(),
            )
        };
        let gutter_hover = canvas.colors.gutter_hover;

        window.handle_input(
            &focus_handle,
            ElementInputHandler::new(bounds, canvas.editor.clone()),
            cx,
        );

        // Clip the (horizontally scrollable) text + selections so they never
        // bleed over the fixed line-number gutter or the vertical scrollbar.
        let content_mask = gpui::ContentMask {
            bounds: Bounds::from_corners(
                point(bounds.left() + gutter_width, bounds.top()),
                point(bounds.right(), bounds.bottom() - bottom_inset),
            ),
        };
        window.with_content_mask(Some(content_mask), |window| {
            for quad in canvas_prepaint.selections.drain(..) {
                window.paint_quad(quad);
            }
            for row in &canvas_prepaint.rows {
                let _ = row.shaped.paint(
                    point(row.fragment_left, row.top),
                    line_height,
                    gpui::TextAlign::Left,
                    None,
                    window,
                    cx,
                );
            }
            for row in &canvas_prepaint.wrapped_rows {
                let _ = row.wrapped.paint(
                    point(canvas_prepaint.content_left, row.top),
                    line_height,
                    gpui::TextAlign::Left,
                    None,
                    window,
                    cx,
                );
            }
            for caret in canvas_prepaint.carets.drain(..) {
                window.paint_quad(caret);
            }
        });

        let gutter_mask = gpui::ContentMask {
            bounds: Bounds::from_corners(
                point(bounds.left(), bounds.top()),
                point(bounds.left() + gutter_width, bounds.bottom() - bottom_inset),
            ),
        };
        let fold_color = canvas.colors.line_number;
        window.with_content_mask(Some(gutter_mask), |window| {
            for (top, collapsed, line) in &canvas_prepaint.fold_gutter {
                if Some(*line) == fold_hover_line {
                    window.paint_quad(fill(
                        fold_icon::fold_hit_bounds(fold_left, *top, line_height),
                        gutter_hover,
                    ));
                }
                fold_icon::paint_fold_chevron(
                    window,
                    cx,
                    fold_left,
                    *top,
                    line_height,
                    *collapsed,
                    fold_color,
                );
            }
            for (top, shaped) in &canvas_prepaint.gutter {
                let _ = shaped.paint(
                    point(canvas_prepaint.gutter_left, *top),
                    line_height,
                    gpui::TextAlign::Left,
                    None,
                    window,
                    cx,
                );
            }
        });

        let visible: Vec<VisibleLine> = canvas_prepaint
            .rows
            .drain(..)
            .map(|r| VisibleLine {
                line: r.line,
                start_char: r.start_char,
                start_col: r.start_col,
                fragment_text: r.fragment_text,
                fragment_left: r.fragment_left,
                top: r.top,
                shaped: r.shaped,
            })
            .collect();
        let wrapped_visible: Vec<WrappedVisible> = canvas_prepaint
            .wrapped_rows
            .drain(..)
            .map(|r| WrappedVisible {
                line: r.line,
                start_char: r.start_char,
                block_top: r.block_top,
                top: r.top,
                start_col: r.start_col,
                fragment_text: r.fragment_text,
                wrap_row_count: r.wrap_row_count,
                wrapped: r.wrapped,
            })
            .collect();
        canvas.editor.update(cx, |e, _| {
            e.visible = visible;
            e.wrapped_visible = wrapped_visible;
        });

}

pub(crate) fn set_editor_cursors(
    prepaint: &EditorCanvasPrepaint,
    window: &mut Window,
    content_cursor: CursorStyle,
    fold_gutter_hover: bool,
) {
    let gutter_cursor = if fold_gutter_hover {
        CursorStyle::PointingHand
    } else {
        CursorStyle::Arrow
    };
    window.set_cursor_style(gutter_cursor, &prepaint.gutter_hitbox);
    window.set_cursor_style(content_cursor, &prepaint.content_hitbox);
}
