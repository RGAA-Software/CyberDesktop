//! `EditorCanvas` types and GPUI [`Element`] wiring.

use gpui::{
    point, prelude::*, px, relative, App, Bounds, Element, ElementId, GlobalElementId, Hitbox,
    HitboxBehavior, LayoutId, PaintQuad, Pixels, ShapedLine, Style, Window, WrappedLine,
};

use gpui::Entity;

use super::super::r#impl::FOLD_GUTTER_WIDTH;
use super::super::ui::EditorColors;
use super::super::EngineEditor;
use super::{paint, prepaint, prepaint_wrapped};

pub(crate) struct EditorCanvas {
    pub(crate) editor: Entity<EngineEditor>,
    pub(crate) colors: EditorColors,
}

pub(crate) struct CanvasPrepaint {
    pub(crate) rows: Vec<VisibleRow>,
    /// Populated instead of `rows` when soft wrap is on.
    pub(crate) wrapped_rows: Vec<WrappedRow>,
    pub(crate) gutter: Vec<(Pixels, ShapedLine)>,
    pub(crate) fold_gutter: Vec<(Pixels, bool, usize)>,
    pub(crate) selections: Vec<PaintQuad>,
    pub(crate) carets: Vec<PaintQuad>,
    pub(crate) content_left: Pixels,
    pub(crate) gutter_left: Pixels,
    pub(crate) fold_left: Pixels,
}

pub(crate) struct EditorCanvasPrepaint {
    pub(crate) canvas: CanvasPrepaint,
    pub(crate) content_hitbox: Hitbox,
    pub(crate) gutter_hitbox: Hitbox,
}

pub(crate) fn editor_hitboxes(
    bounds: Bounds<Pixels>,
    gutter_width: Pixels,
    bottom_inset: Pixels,
    window: &mut Window,
) -> (Hitbox, Hitbox) {
    let content_bounds = Bounds::from_corners(
        point(bounds.left() + gutter_width, bounds.top()),
        point(bounds.right(), bounds.bottom() - bottom_inset),
    );
    let gutter_bounds = Bounds::from_corners(
        point(bounds.left(), bounds.top()),
        point(bounds.left() + gutter_width, bounds.bottom() - bottom_inset),
    );
    (
        window.insert_hitbox(content_bounds, HitboxBehavior::Normal),
        window.insert_hitbox(gutter_bounds, HitboxBehavior::Normal),
    )
}

pub(crate) struct VisibleRow {
    pub(crate) line: usize,
    pub(crate) start_char: usize,
    /// Column within the line where the shaped fragment begins.
    pub(crate) start_col: usize,
    /// Text that was shaped (full line or viewport slice).
    pub(crate) fragment_text: String,
    /// Left edge of the fragment in window coordinates.
    pub(crate) fragment_left: Pixels,
    pub(crate) top: Pixels,
    pub(crate) shaped: ShapedLine,
}

pub(crate) struct WrappedRow {
    pub(crate) line: usize,
    pub(crate) start_char: usize,
    /// Top of the full document-line block (for hit-testing).
    pub(crate) block_top: Pixels,
    /// Top of the shaped fragment (for painting).
    pub(crate) top: Pixels,
    /// Column within the line where the shaped fragment begins (long lines).
    pub(crate) start_col: usize,
    /// Shaped fragment text (empty for short lines).
    pub(crate) fragment_text: String,
    /// Total virtual wrap rows for the line (`0` = use shaped row count).
    pub(crate) wrap_row_count: usize,
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
    type PrepaintState = EditorCanvasPrepaint;

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
        self.editor.update(cx, |e, cx| {
            e.refresh_syntax(cx);
            e.last_bounds = Some(bounds);
            let line_count = if e.soft_wrap {
                e.document.buffer().line_count()
            } else {
                e.display_line_count()
            };
            let digits = line_count.to_string().len().max(3);
            e.gutter_width = if e.show_line_numbers {
                e.font_size * (digits as f32 + 2.0) * 0.6 + FOLD_GUTTER_WIDTH + px(4.0)
            } else {
                FOLD_GUTTER_WIDTH + px(4.0)
            };
            let lane_inset = e.editor_bottom_inset();
            e.scroll_y = super::super::EngineEditor::clamp_scroll_y_for_lane(
                e.scroll_y,
                e.line_height,
                line_count,
                bounds.size.height,
                lane_inset,
            );
        });

        let style = window.text_style();
        let font = style.font();
        let colors = self.colors;
        let default_color = colors.foreground;
        let font_size = style.font_size.to_pixels(window.rem_size());

        let canvas = if self.editor.read(cx).soft_wrap {
            prepaint_wrapped::prepaint_wrapped(
                self,
                bounds,
                &font,
                colors,
                default_color,
                font_size,
                window,
                cx,
            )
        } else {
            let mut out = None;
            self.editor.update(cx, |editor, _| {
                out = Some(prepaint::prepaint_normal(
                    editor,
                    bounds,
                    &font,
                    colors,
                    default_color,
                    font_size,
                    window,
                ));
            });
            out.expect("prepaint_normal")
        };

        let (gutter_width, bottom_inset) = {
            let e = self.editor.read(cx);
            (e.gutter_width, e.editor_bottom_inset())
        };
        let (content_hitbox, gutter_hitbox) =
            editor_hitboxes(bounds, gutter_width, bottom_inset, window);
        EditorCanvasPrepaint {
            canvas,
            content_hitbox,
            gutter_hitbox,
        }
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
        paint::paint(
            self,
            id,
            inspector_id,
            bounds,
            request_layout,
            prepaint,
            window,
            cx,
        );
    }
}
