//! Floating panel drag state.

use gpui::{Pixels, Point, px, point, size};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FloatingPanel {
    Find,
    Goto,
    SearchInFile,
}

/// In-progress title-bar drag: panel kind + cursor offset from panel origin.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PanelDrag {
    pub panel: FloatingPanel,
    pub offset: Point<Pixels>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PanelResizeEdge {
    Right,
    Bottom,
    BottomRight,
}

/// In-progress Find-in-File panel resize.
#[derive(Clone, Copy, Debug)]
pub(crate) struct PanelResize {
    pub edge: PanelResizeEdge,
    pub start_mouse: Point<Pixels>,
    pub start_origin: Point<Pixels>,
    pub start_size: gpui::Size<Pixels>,
}

pub(crate) const FIND_PANEL_WIDTH: Pixels = px(520.);
pub(crate) const GOTO_PANEL_WIDTH: Pixels = px(320.);
pub(crate) const SEARCH_PANEL_WIDTH: Pixels = px(400.);
pub(crate) const SEARCH_PANEL_HEIGHT: Pixels = px(480.);
pub(crate) const SEARCH_PANEL_MIN_WIDTH: Pixels = px(280.);
pub(crate) const SEARCH_PANEL_MIN_HEIGHT: Pixels = px(200.);
pub(crate) const PANEL_RESIZE_HANDLE: Pixels = px(5.);

pub(crate) fn default_find_panel_pos(viewport: gpui::Size<Pixels>) -> Point<Pixels> {
    point(
        (viewport.width - FIND_PANEL_WIDTH - px(16.)).max(px(0.)),
        px(8.),
    )
}

pub(crate) fn default_goto_panel_pos(_viewport: gpui::Size<Pixels>) -> Point<Pixels> {
    point(px(16.), px(8.))
}

pub(crate) fn default_search_panel_pos(viewport: gpui::Size<Pixels>, width: Pixels) -> Point<Pixels> {
    point(
        (viewport.width - width - px(8.)).max(px(0.)),
        px(8.),
    )
}
