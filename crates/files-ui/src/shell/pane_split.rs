use gpui::{prelude::*, *};

/// Minimum width/height of either pane (Files grid min ~100px).
pub(crate) const PANE_MIN_SIZE: Pixels = px(100.);
pub(crate) const SPLIT_HANDLE_SIZE: Pixels = px(4.);
pub(crate) const SPLIT_RATIO_MIN: f32 = 0.15;
pub(crate) const SPLIT_RATIO_MAX: f32 = 0.85;
/// Files `Constants.UI.MultiplePaneWidthThreshold`
pub(crate) const MULTI_PANE_WIDTH_THRESHOLD: Pixels = px(750.);

#[derive(Clone, Copy, Debug)]
pub(crate) struct PaneSplitDrag;

impl Render for PaneSplitDrag {
    fn render(&mut self, _: &mut Window, _: &mut Context<'_, Self>) -> impl IntoElement {
        Empty
    }
}

pub(crate) fn ratio_from_pointer(
    arrangement: super::PaneArrangement,
    bounds: Bounds<Pixels>,
    position: Point<Pixels>,
) -> f32 {
    if bounds.size.width.is_zero() && bounds.size.height.is_zero() {
        return 0.5;
    }
    let raw = match arrangement {
        super::PaneArrangement::Vertical => {
            let span = bounds.size.width - SPLIT_HANDLE_SIZE;
            if span <= px(0.) {
                0.5
            } else {
                (position.x - bounds.left()).as_f32() / span.as_f32()
            }
        }
        super::PaneArrangement::Horizontal => {
            let span = bounds.size.height - SPLIT_HANDLE_SIZE;
            if span <= px(0.) {
                0.5
            } else {
                (position.y - bounds.top()).as_f32() / span.as_f32()
            }
        }
    };
    raw.clamp(SPLIT_RATIO_MIN, SPLIT_RATIO_MAX)
}

pub(crate) fn secondary_too_narrow(
    arrangement: super::PaneArrangement,
    bounds: Bounds<Pixels>,
    split_ratio: f32,
) -> bool {
    let secondary_px = match arrangement {
        super::PaneArrangement::Vertical => {
            let span = (bounds.size.width - SPLIT_HANDLE_SIZE).as_f32();
            span * (1.0 - split_ratio)
        }
        super::PaneArrangement::Horizontal => {
            let span = (bounds.size.height - SPLIT_HANDLE_SIZE).as_f32();
            span * (1.0 - split_ratio)
        }
    };
    secondary_px < PANE_MIN_SIZE.as_f32()
}
