//! Paint shaped lines, selections, carets, and gutter.

use gpui::{
    point, prelude::*, App, Bounds, Element, ElementInputHandler, GlobalElementId, Pixels,
    rgb, Window,
};

use super::element::{CanvasPrepaint, EditorCanvas};
use super::super::state::{VisibleLine, WrappedVisible};

pub(crate) fn paint(
    canvas: &mut EditorCanvas,
    _id: Option<&GlobalElementId>,
    _inspector_id: Option<&gpui::InspectorElementId>,
    bounds: Bounds<Pixels>,
    _request_layout: &mut <EditorCanvas as Element>::RequestLayoutState,
    prepaint: &mut CanvasPrepaint,
    window: &mut Window,
    cx: &mut App,
) {
        let (focus_handle, line_height, gutter_width) = {
            let e = canvas.editor.read(cx);
            (e.focus_handle.clone(), e.line_height, e.gutter_width)
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
        canvas.editor.update(cx, |e, _| {
            e.visible = visible;
            e.wrapped_visible = wrapped_visible;
        });

}
