//! Draggable floating panel positioning.

use super::super::imports::*;
use super::super::state::{
    default_find_panel_pos, default_goto_panel_pos, default_search_panel_pos, FloatingPanel,
    PanelDrag, PanelResize, PanelResizeEdge, FIND_PANEL_WIDTH, GOTO_PANEL_WIDTH,
    SEARCH_PANEL_HEIGHT, SEARCH_PANEL_MIN_HEIGHT, SEARCH_PANEL_MIN_WIDTH, SEARCH_PANEL_WIDTH,
};

impl EngineEditor {
    pub(crate) fn editor_viewport(&self) -> Size<Pixels> {
        self.last_bounds
            .map(|b| b.size)
            .unwrap_or(size(px(800.), px(600.)))
    }

    pub(crate) fn panel_origin(&self, panel: FloatingPanel) -> Option<Point<Pixels>> {
        match panel {
            FloatingPanel::Find => self.find_panel_pos,
            FloatingPanel::Goto => self.goto_panel_pos,
            FloatingPanel::SearchInFile => self.search_panel_pos,
        }
    }

    pub(crate) fn set_panel_origin(&mut self, panel: FloatingPanel, pos: Point<Pixels>) {
        match panel {
            FloatingPanel::Find => self.find_panel_pos = Some(pos),
            FloatingPanel::Goto => self.goto_panel_pos = Some(pos),
            FloatingPanel::SearchInFile => self.search_panel_pos = Some(pos),
        }
    }

    pub(crate) fn clear_panel_origin(&mut self, panel: FloatingPanel) {
        match panel {
            FloatingPanel::Find => self.find_panel_pos = None,
            FloatingPanel::Goto => self.goto_panel_pos = None,
            FloatingPanel::SearchInFile => self.search_panel_pos = None,
        }
    }

    pub(crate) fn default_panel_origin(&self, panel: FloatingPanel) -> Point<Pixels> {
        let viewport = self.editor_viewport();
        match panel {
            FloatingPanel::Find => default_find_panel_pos(viewport),
            FloatingPanel::Goto => default_goto_panel_pos(viewport),
            FloatingPanel::SearchInFile => {
                default_search_panel_pos(viewport, self.resolved_search_panel_size().width)
            }
        }
    }

    pub(crate) fn resolved_search_panel_size(&self) -> Size<Pixels> {
        self.search_panel_size
            .unwrap_or(size(SEARCH_PANEL_WIDTH, SEARCH_PANEL_HEIGHT))
    }

    pub(crate) fn clamp_search_panel_size(&self, panel_size: Size<Pixels>) -> Size<Pixels> {
        let viewport = self.editor_viewport();
        let max_w = (viewport.width - px(16.)).max(SEARCH_PANEL_MIN_WIDTH);
        let max_h = (viewport.height - px(16.)).max(SEARCH_PANEL_MIN_HEIGHT);
        size(
            panel_size.width.clamp(SEARCH_PANEL_MIN_WIDTH, max_w),
            panel_size.height.clamp(SEARCH_PANEL_MIN_HEIGHT, max_h),
        )
    }

    pub(crate) fn resolved_panel_origin(&self, panel: FloatingPanel) -> Point<Pixels> {
        self.panel_origin(panel)
            .unwrap_or_else(|| self.default_panel_origin(panel))
    }

    pub(crate) fn panel_size(&self, panel: FloatingPanel) -> Size<Pixels> {
        match panel {
            FloatingPanel::Find => size(FIND_PANEL_WIDTH, px(0.)),
            FloatingPanel::Goto => size(GOTO_PANEL_WIDTH, px(0.)),
            FloatingPanel::SearchInFile => self.resolved_search_panel_size(),
        }
    }

    pub(crate) fn start_panel_drag(
        &mut self,
        panel: FloatingPanel,
        event: &MouseDownEvent,
        cx: &mut Context<Self>,
    ) {
        let Some(bounds) = self.last_bounds else {
            return;
        };
        let local = event.position - bounds.origin;
        let origin = self.resolved_panel_origin(panel);
        self.set_panel_origin(panel, origin);
        self.panel_drag = Some(PanelDrag {
            panel,
            offset: local - origin,
        });
        cx.notify();
    }

    pub(crate) fn drag_panel(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        let Some(drag) = self.panel_drag else {
            return;
        };
        let Some(bounds) = self.last_bounds else {
            return;
        };
        let local = event.position - bounds.origin;
        let panel_size = self.panel_size(drag.panel);
        let mut pos = local - drag.offset;
        let max_x = (bounds.size.width - panel_size.width).max(px(0.));
        let max_y = (bounds.size.height - panel_size.height).max(px(0.));
        pos.x = pos.x.clamp(px(0.), max_x);
        pos.y = pos.y.clamp(px(0.), max_y);
        self.set_panel_origin(drag.panel, pos);
        cx.notify();
    }

    pub(crate) fn end_panel_drag(&mut self) {
        self.panel_drag = None;
    }

    pub(crate) fn start_search_panel_resize(
        &mut self,
        edge: PanelResizeEdge,
        event: &MouseDownEvent,
        cx: &mut Context<Self>,
    ) {
        let Some(bounds) = self.last_bounds else {
            return;
        };
        let local = event.position - bounds.origin;
        let origin = self.resolved_panel_origin(FloatingPanel::SearchInFile);
        let panel_size = self.resolved_search_panel_size();
        self.set_panel_origin(FloatingPanel::SearchInFile, origin);
        self.search_panel_size = Some(panel_size);
        self.panel_resize = Some(PanelResize {
            edge,
            start_mouse: local,
            start_origin: origin,
            start_size: panel_size,
        });
        cx.notify();
    }

    pub(crate) fn resize_search_panel(&mut self, event: &MouseMoveEvent, cx: &mut Context<Self>) {
        let Some(resize) = self.panel_resize else {
            return;
        };
        let Some(bounds) = self.last_bounds else {
            return;
        };
        let local = event.position - bounds.origin;
        let dx = local.x - resize.start_mouse.x;
        let dy = local.y - resize.start_mouse.y;

        let mut width = resize.start_size.width;
        let mut height = resize.start_size.height;
        match resize.edge {
            PanelResizeEdge::Right | PanelResizeEdge::BottomRight => {
                width = px(f32::from(resize.start_size.width) + f32::from(dx));
            }
            _ => {}
        }
        match resize.edge {
            PanelResizeEdge::Bottom | PanelResizeEdge::BottomRight => {
                height = px(f32::from(resize.start_size.height) + f32::from(dy));
            }
            _ => {}
        }

        let size = self.clamp_search_panel_size(size(width, height));
        let max_x = (bounds.size.width - size.width).max(px(0.));
        let max_y = (bounds.size.height - size.height).max(px(0.));
        let origin = point(
            resize.start_origin.x.clamp(px(0.), max_x),
            resize.start_origin.y.clamp(px(0.), max_y),
        );

        self.search_panel_size = Some(size);
        self.set_panel_origin(FloatingPanel::SearchInFile, origin);
        cx.notify();
    }

    pub(crate) fn end_panel_resize(&mut self) {
        self.panel_resize = None;
    }
}
