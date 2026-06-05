use gpui::actions;
use schemars::JsonSchema;
use serde::Deserialize;

/// Reopen a specific entry from `session_closed_tabs` (0 = most recent).
#[derive(Clone, Debug, PartialEq, Deserialize, JsonSchema, gpui::Action)]
#[action(namespace = cyber_desktop_shell)]
#[serde(deny_unknown_fields)]
pub struct ReopenClosedTabAt {
    pub index: usize,
}
