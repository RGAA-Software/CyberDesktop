//! `EditorCanvas` types and GPUI [`Element`] wiring.

use gpui::{
    prelude::*, relative, App, Bounds, Element, ElementId, GlobalElementId, LayoutId, PaintQuad,
    Pixels, ShapedLine, Style, Window, WrappedLine,
};

use gpui::Entity;

use super::super::EngineEditor;
use super::{paint, prepaint, prepaint_wrapped};

pub(crate) struct EditorCanvas {
    pub(crate) editor: Entity<EngineEditor>,
}

pub(crate) struct CanvasPrepaint {
    pub(crate) rows: Vec<VisibleRow>,
    /// Populated instead of `rows` when soft wrap is on.
    pub(crate) wrapped_rows: Vec<WrappedRow>,
    pub(crate) gutter: Vec<(Pixels, ShapedLine)>,
    pub(crate) selections: Vec<PaintQuad>,
    pub(crate) carets: Vec<PaintQuad>,
    pub(crate) content_left: Pixels,
    pub(crate) gutter_left: Pixels,
}

pub(crate) struct VisibleRow {
    pub(crate) line: usize,
    pub(crate) start_char: usize,
    pub(crate) top: Pixels,
    pub(crate) shaped: ShapedLine,
}

pub(crate) struct WrappedRow {
    pub(crate) line: usize,
    pub(crate) start_char: usize,
    pub(crate) top: Pixels,
    pub(crate) wrapped: WrappedLine,
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
        self.editor.update(cx, |e, _| {
            e.refresh_syntax();
            e.last_bounds = Some(bounds);
            let line_count = e.document.buffer().line_count();
            let digits = line_count.to_string().len().max(3);
            e.gutter_width = if e.show_line_numbers {
                e.font_size * (digits as f32 + 2.0) * 0.6
            } else {
                gpui::px(8.0)
            };
            let total = e.line_height * line_count as f32;
            let max = (total - bounds.size.height).max(gpui::px(0.0));
            if e.scroll_y > max {
                e.scroll_y = max;
            }
        });

        let style = window.text_style();
        let font = style.font();
        let default_color = style.color;
        let font_size = style.font_size.to_pixels(window.rem_size());

        if self.editor.read(cx).soft_wrap {
            return prepaint_wrapped::prepaint_wrapped(
                self,
                bounds,
                &font,
                default_color,
                font_size,
                window,
                cx,
            );
        }

        prepaint::prepaint_normal(
            self,
            bounds,
            &font,
            default_color,
            font_size,
            window,
            cx,
        )
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        inspector_id: Option<&gpui::InspectorElementId>,
        bounds: Bounds<Pixels>,
        request_layout: &mut Self::RequestLayoutState,
        prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        paint::paint(self, id, inspector_id, bounds, request_layout, prepaint, window, cx);
    }
}
