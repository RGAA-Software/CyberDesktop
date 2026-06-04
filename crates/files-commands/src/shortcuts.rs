//! Human-readable shortcut reference for the Actions settings page.
//!
//! Derived from `action_specs()` — the single source of truth for default key bindings.

pub struct ShortcutHelp {
    pub action_id: &'static str,
    /// Full locale key, e.g. `settings.actions.copy`.
    pub message_key: &'static str,
}

/// Action ids shown under Settings → Tabs → Dual pane (subset of [`action_specs`]).
pub const DUAL_PANE_SHORTCUT_ACTION_IDS: &[&str] = &[
    "toggle_dual_pane",
    "focus_other_pane",
    "close_active_pane",
    "open_in_new_pane",
    "split_pane_vertically",
    "split_pane_horizontally",
];

pub fn shortcut_reference() -> Vec<ShortcutHelp> {
    crate::action_specs::action_specs()
        .iter()
        .map(|spec| ShortcutHelp {
            action_id: spec.id,
            message_key: spec.i18n_key,
        })
        .collect()
}

pub fn dual_pane_shortcut_reference() -> Vec<ShortcutHelp> {
    DUAL_PANE_SHORTCUT_ACTION_IDS
        .iter()
        .filter_map(|id| {
            crate::action_specs::action_spec_by_id(id).map(|spec| ShortcutHelp {
                action_id: spec.id,
                message_key: spec.i18n_key,
            })
        })
        .collect()
}
