//! Editor state types (no GPUI layout/paint).

mod find;
mod goto;
mod input_target;
mod panel;
mod scroll;
mod search_panel;
mod tab;

pub(crate) use find::FindState;
pub(crate) use goto::GotoState;
pub(crate) use input_target::InputTarget;
pub(crate) use panel::{
    default_find_panel_pos, default_goto_panel_pos, default_search_panel_pos, FloatingPanel,
    PanelDrag, PanelResize, PanelResizeEdge, FIND_PANEL_WIDTH, GOTO_PANEL_WIDTH,
    PANEL_RESIZE_HANDLE, SEARCH_PANEL_HEIGHT, SEARCH_PANEL_MIN_HEIGHT, SEARCH_PANEL_MIN_WIDTH,
    SEARCH_PANEL_WIDTH,
};
pub(crate) use scroll::{HScrollbarMetrics, ScrollbarMetrics, VisibleLine, WrappedVisible};
pub(crate) use search_panel::{SearchPanelState, SearchRow};
pub(crate) use tab::{read_file_meta, TabSlot};
