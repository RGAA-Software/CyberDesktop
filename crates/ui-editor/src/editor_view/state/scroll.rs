use gpui::{Pixels, ShapedLine, WrappedLine};

/// Geometry of the vertical scrollbar for the current frame.
pub(crate) struct ScrollbarMetrics {
    pub(crate) viewport: Pixels,
    pub(crate) thumb_top: Pixels,
    pub(crate) thumb_h: Pixels,
    pub(crate) max_scroll: Pixels,
}

/// Geometry of the horizontal scrollbar for the current frame.
pub(crate) struct HScrollbarMetrics {
    /// Track length (the scrollable gutter-to-edge span).
    pub(crate) track: Pixels,
    pub(crate) thumb_left: Pixels,
    pub(crate) thumb_w: Pixels,
    pub(crate) max_scroll: Pixels,
}

/// A shaped, currently-visible line retained for hit-testing.
pub(crate) struct VisibleLine {
    pub(crate) line: usize,
    pub(crate) start_char: usize,
    pub(crate) top: Pixels,
    pub(crate) shaped: ShapedLine,
}

/// A wrapped, currently-visible logical line retained for hit-testing (wrap mode).
pub(crate) struct WrappedVisible {
    pub(crate) line: usize,
    pub(crate) start_char: usize,
    pub(crate) top: Pixels,
    pub(crate) wrapped: WrappedLine,
}
