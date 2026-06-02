//! Human-readable shortcut reference for the Actions settings page.
//!
//! Derived from `action_specs()` — the single source of truth for default key bindings.

pub struct ShortcutHelp {
    pub action_id: &'static str,
    /// Full locale key, e.g. `settings.actions.copy`.
    pub message_key: &'static str,
}

pub fn shortcut_reference() -> Vec<ShortcutHelp> {
    crate::action_specs::action_specs()
        .iter()
        .map(|spec| ShortcutHelp {
            action_id: spec.id,
            message_key: spec.i18n_key,
        })
        .collect()
}
