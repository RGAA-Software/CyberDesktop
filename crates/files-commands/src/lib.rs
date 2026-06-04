mod action_specs;
mod keystroke_util;
mod shortcuts;

use gpui::{actions, App, KeyBinding};

pub use action_specs::{action_spec_by_id, action_specs, default_keystroke_for, ActionSpec};
pub use keystroke_util::{
    binding_conflict, is_customized, is_valid_binding_keystroke, keystroke_from_event,
    keystroke_to_display, resolved_keystroke_for, resolved_keystroke_raw,
};
pub use shortcuts::{shortcut_reference, ShortcutHelp};

actions!(
    files_commands,
    [
        NavigateBack,
        NavigateForward,
        NavigateUp,
        RefreshDirectory,
        OpenItem,
        SelectAll,
        RenameItem,
        CancelRename,
        DeleteItems,
        DeleteItemsPermanent,
        RestoreRecycleItems,
        RestoreAllRecycleItems,
        EmptyRecycleBin,
        UndoOperation,
        RedoOperation,
        NewFolder,
        NewFile,
        CopyPath,
        CopyItems,
        CutItems,
        PasteItems,
        NavigatePrevious,
        NavigateNext,
        FocusSearch,
        FocusOmnibar,
        ViewDetails,
        ViewList,
        ViewGrid,
        ViewCards,
        ViewColumns,
        ShellProperties,
        CompressItems,
        ExtractHere,
        ExtractToFolder,
        ReopenClosedTab,
        ToggleShowFileExtensions,
        ToggleDualPane,
        FocusOtherPane,
        CloseActivePane,
        OpenInNewPane,
        SplitPaneVertically,
        SplitPaneHorizontally,
        ArrangePanesVertically,
        ArrangePanesHorizontally,
    ]
);

/// GPUI key context for the file browser surface.
pub const FILE_BROWSER: &str = "FileBrowser";

pub fn init(cx: &mut App) {
    cx.bind_keys(resolve_key_bindings());
}

pub fn resolve_key_bindings() -> Vec<KeyBinding> {
    let overrides = files_core::keybinding_overrides();
    let mut bindings = Vec::new();
    for spec in action_specs() {
        let keystroke = overrides
            .get(spec.id)
            .map(String::as_str)
            .unwrap_or_else(|| default_keystroke_for(spec));
        if let Some(binding) = action_specs::key_binding_for(spec, keystroke) {
            bindings.push(binding);
        }
    }
    bindings.extend(action_specs::extra_file_browser_bindings());
    // Override gpui-component Input ctrl-f (in-field search) for omnibar/rename fields.
    if let Some(spec) = action_specs::action_spec_by_id("focus_search") {
        let keystroke = overrides
            .get(spec.id)
            .map(String::as_str)
            .unwrap_or_else(|| action_specs::default_keystroke_for(spec));
        bindings.push(KeyBinding::new(keystroke, FocusSearch, Some("Input")));
    }
    bindings
}

pub fn file_browser_key_bindings() -> Vec<KeyBinding> {
    resolve_key_bindings()
}
