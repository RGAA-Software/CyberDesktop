use gpui::{App, BorrowAppContext, Global, KeyDownEvent};

use files_commands::{
    binding_conflict, is_valid_binding_keystroke, keystroke_from_event, resolved_keystroke_for,
};
use files_core::keybinding_overrides;

/// Re-register all application key bindings after the user changes shortcuts.
pub fn rebind_all_keybindings(cx: &mut App) {
    cx.clear_key_bindings();
    gpui_component::init(cx);
    app_ui::popup_menu::init(cx);
    app_ui::cyber_editor::init(cx);
    files_commands::init(cx);
}

/// Save a keybinding override and apply it immediately.
pub fn apply_keybinding_override(
    action_id: &str,
    keystroke: &str,
    cx: &mut App,
) -> anyhow::Result<()> {
    files_core::save_keybinding_override(action_id, keystroke)?;
    rebind_all_keybindings(cx);
    Ok(())
}

pub fn reset_keybinding(action_id: &str, cx: &mut App) -> anyhow::Result<()> {
    files_core::reset_keybinding_override(action_id)?;
    rebind_all_keybindings(cx);
    Ok(())
}

pub fn reset_all_keybindings(cx: &mut App) -> anyhow::Result<()> {
    files_core::reset_all_keybinding_overrides()?;
    rebind_all_keybindings(cx);
    Ok(())
}

#[derive(Default, Clone)]
pub struct KeybindingCaptureGlobal {
    pub recording_action_id: Option<String>,
    pub conflict_action_id: Option<String>,
}

impl Global for KeybindingCaptureGlobal {}

pub fn init_keybinding_capture(cx: &mut App) {
    cx.set_global(KeybindingCaptureGlobal::default());
}

pub fn start_recording(action_id: String, cx: &mut App) {
    cx.update_global::<KeybindingCaptureGlobal, _>(|state, _| {
        state.recording_action_id = Some(action_id);
        state.conflict_action_id = None;
    });
}

pub fn stop_recording(cx: &mut App) {
    cx.update_global::<KeybindingCaptureGlobal, _>(|state, _| {
        state.recording_action_id = None;
    });
}

pub fn clear_conflict(cx: &mut App) {
    cx.update_global::<KeybindingCaptureGlobal, _>(|state, _| {
        state.conflict_action_id = None;
    });
}

pub fn recording_action_id(cx: &App) -> Option<String> {
    cx.try_global::<KeybindingCaptureGlobal>()
        .and_then(|state| state.recording_action_id.clone())
}

pub fn conflict_action_id(cx: &App) -> Option<String> {
    cx.try_global::<KeybindingCaptureGlobal>()
        .and_then(|state| state.conflict_action_id.clone())
}

pub fn handle_recording_key(event: &KeyDownEvent, cx: &mut App) -> bool {
    let Some(action_id) = recording_action_id(cx) else {
        return false;
    };

    if event.keystroke.key == "escape" {
        stop_recording(cx);
        return true;
    }

    let Some(raw) = keystroke_from_event(&event.keystroke) else {
        return true;
    };
    if !is_valid_binding_keystroke(&raw) {
        return true;
    }

    let overrides = keybinding_overrides();
    if let Some(conflict) = binding_conflict(&action_id, &raw, &overrides) {
        cx.update_global::<KeybindingCaptureGlobal, _>(|state, _| {
            state.conflict_action_id = Some(conflict.to_string());
        });
        stop_recording(cx);
        return true;
    }

    let _ = apply_keybinding_override(&action_id, &raw, cx);
    stop_recording(cx);
    true
}

pub fn display_keystroke_for(action_id: &str) -> String {
    resolved_keystroke_for(action_id, &keybinding_overrides())
}
