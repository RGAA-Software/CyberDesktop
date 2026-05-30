//! Paint shaped lines, selections, carets, and gutter.

use gpui::{
    point, App, Bounds, CursorStyle, Element, ElementInputHandler, GlobalElementId, Pixels, Window,
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
        set_editor_cursors(prepaint, window, content_cursor);
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
        window.with_content_mask(Some(gutter_mask), |window| {
            for (top, collapsed) in &canvas_prepaint.fold_gutter {
                fold_icon::paint_fold_chevron(
                    window,
                    cx,
                    fold_left,
                    *top,
                    line_height,
                    *collapsed,
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
) {
    window.set_cursor_style(CursorStyle::Arrow, &prepaint.gutter_hitbox);
    window.set_cursor_style(content_cursor, &prepaint.content_hitbox);
}
