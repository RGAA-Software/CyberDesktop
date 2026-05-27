//! Menu keyboard actions for CyberFiles [`PopupMenu`] (separate namespace from gpui-component `ui::`).

use gpui::{actions, Action};
use serde::Deserialize;

#[derive(Clone, Action, PartialEq, Eq, Deserialize)]
#[action(namespace = cyberfiles_popup, no_json)]
pub struct Confirm {
    pub secondary: bool,
}

actions!(
    cyberfiles_popup,
    [
        Cancel,
        SelectUp,
        SelectDown,
        SelectLeft,
        SelectRight,
        SelectFirst,
        SelectLast,
        SelectPrevColumn,
        SelectNextColumn,
        SelectPageUp,
        SelectPageDown
    ]
);
