//! Go to Line overlay state.

use gpui::{Entity, Subscription};
use gpui_component::input::InputState;

pub(crate) struct GotoState {
    pub(crate) input: Entity<InputState>,
    pub(crate) _subs: Vec<Subscription>,
}
