use std::collections::BTreeMap;

use gpui::{KeybindingKeystroke, Keystroke};

use crate::action_specs::{action_spec_by_id, action_specs, default_keystroke_for};

pub fn keystroke_to_display(raw: &str) -> String {
    Keystroke::parse(raw)
        .map(|keystroke| KeybindingKeystroke::from_keystroke(keystroke).to_string())
        .unwrap_or_else(|_| raw.to_string())
}

pub fn is_valid_binding_keystroke(raw: &str) -> bool {
    let Ok(keystroke) = Keystroke::parse(raw) else {
        return false;
    };
    if keystroke.key.is_empty() {
        return false;
    }
    !matches!(
        keystroke.key.as_str(),
        "ctrl" | "control" | "alt" | "shift" | "cmd" | "platform" | "fn" | "function"
    )
}

pub fn keystroke_from_event(keystroke: &Keystroke) -> Option<String> {
    if keystroke.key.is_empty() {
        return None;
    }
    if matches!(
        keystroke.key.as_str(),
        "ctrl" | "control" | "alt" | "shift" | "cmd" | "platform" | "fn" | "function"
    ) {
        return None;
    }
    Some(keystroke.unparse())
}

pub fn resolved_keystroke_for(action_id: &str, overrides: &BTreeMap<String, String>) -> String {
    let Some(spec) = action_spec_by_id(action_id) else {
        return String::new();
    };
    let raw = overrides
        .get(action_id)
        .map(String::as_str)
        .unwrap_or_else(|| default_keystroke_for(spec));
    keystroke_to_display(raw)
}

pub fn resolved_keystroke_raw(action_id: &str, overrides: &BTreeMap<String, String>) -> String {
    let Some(spec) = action_spec_by_id(action_id) else {
        return String::new();
    };
    overrides
        .get(action_id)
        .cloned()
        .unwrap_or_else(|| default_keystroke_for(spec).to_string())
}

fn contexts_conflict(left: Option<&str>, right: Option<&str>) -> bool {
    left == right || left.is_none() || right.is_none()
}

pub fn binding_conflict(
    action_id: &str,
    keystroke: &str,
    overrides: &BTreeMap<String, String>,
) -> Option<&'static str> {
    let spec = action_spec_by_id(action_id)?;
    for other in action_specs() {
        if other.id == action_id {
            continue;
        }
        let other_raw = overrides
            .get(other.id)
            .map(String::as_str)
            .unwrap_or_else(|| default_keystroke_for(other));
        if other_raw == keystroke && contexts_conflict(spec.context, other.context) {
            return Some(other.id);
        }
    }
    None
}

pub fn is_customized(action_id: &str, overrides: &BTreeMap<String, String>) -> bool {
    overrides.contains_key(action_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action_specs::default_keystroke_for;

    #[test]
    fn keystroke_to_display_formats_ctrl_shift_key() {
        let display = keystroke_to_display("ctrl-shift-c");
        assert!(display.to_ascii_lowercase().contains("ctrl"));
        assert!(display.to_ascii_lowercase().contains("c"));
    }

    #[test]
    fn binding_conflict_detects_same_keystroke_in_file_browser() {
        let mut overrides = BTreeMap::new();
        overrides.insert("copy_items".into(), "ctrl-x".into());
        let conflict = binding_conflict("cut_items", "ctrl-x", &overrides);
        assert_eq!(conflict, Some("copy_items"));
    }

    #[test]
    fn resolved_keystroke_uses_default_without_override() {
        let overrides = BTreeMap::new();
        let spec = action_spec_by_id("copy_items").unwrap();
        assert_eq!(
            resolved_keystroke_raw("copy_items", &overrides),
            default_keystroke_for(spec)
        );
    }
}
