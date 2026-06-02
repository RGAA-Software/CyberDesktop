use gpui::KeyBinding;

use crate::{
    CancelRename, CopyItems, CopyPath, CutItems, DeleteItems, DeleteItemsPermanent, FILE_BROWSER,
    FocusOmnibar, FocusSearch, NavigateBack, NavigateForward, NavigateNext, NavigatePrevious,
    NavigateUp, NewFile, NewFolder, OpenItem, PasteItems, RedoOperation, RefreshDirectory,
    ReopenClosedTab, RenameItem, SelectAll, UndoOperation, ViewCards, ViewColumns, ViewDetails,
    ViewGrid, ViewList,
};

pub struct ActionSpec {
    pub id: &'static str,
    pub default_keystroke: &'static str,
    pub default_keystroke_mac: Option<&'static str>,
    pub context: Option<&'static str>,
    pub i18n_key: &'static str,
}

pub fn action_specs() -> &'static [ActionSpec] {
    &[
        ActionSpec {
            id: "navigate_back",
            default_keystroke: "alt-left",
            default_keystroke_mac: None,
            context: Some(FILE_BROWSER),
            i18n_key: "settings.actions.navigate_back",
        },
        ActionSpec {
            id: "navigate_forward",
            default_keystroke: "alt-right",
            default_keystroke_mac: None,
            context: Some(FILE_BROWSER),
            i18n_key: "settings.actions.navigate_forward",
        },
        ActionSpec {
            id: "navigate_up",
            default_keystroke: "backspace",
            default_keystroke_mac: None,
            context: Some(FILE_BROWSER),
            i18n_key: "settings.actions.navigate_up",
        },
        ActionSpec {
            id: "refresh_directory",
            default_keystroke: "f5",
            default_keystroke_mac: None,
            context: None,
            i18n_key: "settings.actions.refresh",
        },
        ActionSpec {
            id: "open_item",
            default_keystroke: "enter",
            default_keystroke_mac: None,
            context: Some(FILE_BROWSER),
            i18n_key: "settings.actions.open",
        },
        ActionSpec {
            id: "rename_item",
            default_keystroke: "f2",
            default_keystroke_mac: None,
            context: Some(FILE_BROWSER),
            i18n_key: "settings.actions.rename",
        },
        ActionSpec {
            id: "cancel_rename",
            default_keystroke: "escape",
            default_keystroke_mac: None,
            context: Some(FILE_BROWSER),
            i18n_key: "settings.actions.cancel_rename",
        },
        ActionSpec {
            id: "delete_items",
            default_keystroke: "delete",
            default_keystroke_mac: Some("cmd-backspace"),
            context: Some(FILE_BROWSER),
            i18n_key: "settings.actions.delete",
        },
        ActionSpec {
            id: "delete_items_permanent",
            default_keystroke: "shift-delete",
            default_keystroke_mac: None,
            context: Some(FILE_BROWSER),
            i18n_key: "settings.actions.delete_permanent",
        },
        ActionSpec {
            id: "navigate_previous",
            default_keystroke: "up",
            default_keystroke_mac: None,
            context: Some(FILE_BROWSER),
            i18n_key: "settings.actions.navigate_previous",
        },
        ActionSpec {
            id: "navigate_next",
            default_keystroke: "down",
            default_keystroke_mac: None,
            context: Some(FILE_BROWSER),
            i18n_key: "settings.actions.navigate_next",
        },
        ActionSpec {
            id: "new_folder",
            default_keystroke: "ctrl-shift-n",
            default_keystroke_mac: Some("cmd-shift-n"),
            context: Some(FILE_BROWSER),
            i18n_key: "settings.actions.new_folder",
        },
        ActionSpec {
            id: "new_file",
            default_keystroke: "ctrl-shift-m",
            default_keystroke_mac: Some("cmd-shift-m"),
            context: Some(FILE_BROWSER),
            i18n_key: "settings.actions.new_file",
        },
        ActionSpec {
            id: "focus_search",
            default_keystroke: "ctrl-f",
            default_keystroke_mac: Some("cmd-f"),
            context: None,
            i18n_key: "settings.actions.focus_search",
        },
        ActionSpec {
            id: "focus_omnibar",
            default_keystroke: "ctrl-l",
            default_keystroke_mac: Some("cmd-l"),
            context: None,
            i18n_key: "settings.actions.focus_omnibar",
        },
        ActionSpec {
            id: "reopen_closed_tab",
            default_keystroke: "ctrl-shift-t",
            default_keystroke_mac: Some("cmd-shift-t"),
            context: None,
            i18n_key: "settings.actions.reopen_tab",
        },
        ActionSpec {
            id: "view_details",
            default_keystroke: "ctrl-1",
            default_keystroke_mac: None,
            context: Some(FILE_BROWSER),
            i18n_key: "settings.actions.view_details",
        },
        ActionSpec {
            id: "view_list",
            default_keystroke: "ctrl-2",
            default_keystroke_mac: None,
            context: Some(FILE_BROWSER),
            i18n_key: "settings.actions.view_list",
        },
        ActionSpec {
            id: "view_grid",
            default_keystroke: "ctrl-3",
            default_keystroke_mac: None,
            context: Some(FILE_BROWSER),
            i18n_key: "settings.actions.view_grid",
        },
        ActionSpec {
            id: "view_cards",
            default_keystroke: "ctrl-shift-4",
            default_keystroke_mac: None,
            context: Some(FILE_BROWSER),
            i18n_key: "settings.actions.view_cards",
        },
        ActionSpec {
            id: "view_columns",
            default_keystroke: "ctrl-4",
            default_keystroke_mac: None,
            context: Some(FILE_BROWSER),
            i18n_key: "settings.actions.view_columns",
        },
        ActionSpec {
            id: "copy_path",
            default_keystroke: "ctrl-shift-c",
            default_keystroke_mac: Some("cmd-shift-c"),
            context: Some(FILE_BROWSER),
            i18n_key: "settings.actions.copy_path",
        },
        ActionSpec {
            id: "copy_items",
            default_keystroke: "ctrl-c",
            default_keystroke_mac: Some("cmd-c"),
            context: Some(FILE_BROWSER),
            i18n_key: "settings.actions.copy",
        },
        ActionSpec {
            id: "cut_items",
            default_keystroke: "ctrl-x",
            default_keystroke_mac: Some("cmd-x"),
            context: Some(FILE_BROWSER),
            i18n_key: "settings.actions.cut",
        },
        ActionSpec {
            id: "paste_items",
            default_keystroke: "ctrl-v",
            default_keystroke_mac: Some("cmd-v"),
            context: Some(FILE_BROWSER),
            i18n_key: "settings.actions.paste",
        },
        ActionSpec {
            id: "select_all",
            default_keystroke: "ctrl-a",
            default_keystroke_mac: Some("cmd-a"),
            context: Some(FILE_BROWSER),
            i18n_key: "settings.actions.select_all",
        },
        ActionSpec {
            id: "undo_operation",
            default_keystroke: "ctrl-z",
            default_keystroke_mac: Some("cmd-z"),
            context: None,
            i18n_key: "settings.actions.undo",
        },
        ActionSpec {
            id: "redo_operation",
            default_keystroke: "ctrl-y",
            default_keystroke_mac: Some("cmd-shift-z"),
            context: None,
            i18n_key: "settings.actions.redo",
        },
    ]
}

pub fn action_spec_by_id(id: &str) -> Option<&'static ActionSpec> {
    action_specs().iter().find(|spec| spec.id == id)
}

pub fn default_keystroke_for(spec: &ActionSpec) -> &str {
    #[cfg(target_os = "macos")]
    {
        if let Some(mac) = spec.default_keystroke_mac {
            return mac;
        }
    }
    spec.default_keystroke
}

pub fn key_binding_for(spec: &ActionSpec, keystroke: &str) -> Option<KeyBinding> {
    Some(match spec.id {
        "navigate_back" => KeyBinding::new(keystroke, NavigateBack, spec.context),
        "navigate_forward" => KeyBinding::new(keystroke, NavigateForward, spec.context),
        "navigate_up" => KeyBinding::new(keystroke, NavigateUp, spec.context),
        "refresh_directory" => KeyBinding::new(keystroke, RefreshDirectory, spec.context),
        "open_item" => KeyBinding::new(keystroke, OpenItem, spec.context),
        "rename_item" => KeyBinding::new(keystroke, RenameItem, spec.context),
        "cancel_rename" => KeyBinding::new(keystroke, CancelRename, spec.context),
        "delete_items" => KeyBinding::new(keystroke, DeleteItems, spec.context),
        "delete_items_permanent" => KeyBinding::new(keystroke, DeleteItemsPermanent, spec.context),
        "navigate_previous" => KeyBinding::new(keystroke, NavigatePrevious, spec.context),
        "navigate_next" => KeyBinding::new(keystroke, NavigateNext, spec.context),
        "new_folder" => KeyBinding::new(keystroke, NewFolder, spec.context),
        "new_file" => KeyBinding::new(keystroke, NewFile, spec.context),
        "focus_search" => KeyBinding::new(keystroke, FocusSearch, spec.context),
        "focus_omnibar" => KeyBinding::new(keystroke, FocusOmnibar, spec.context),
        "reopen_closed_tab" => KeyBinding::new(keystroke, ReopenClosedTab, spec.context),
        "view_details" => KeyBinding::new(keystroke, ViewDetails, spec.context),
        "view_list" => KeyBinding::new(keystroke, ViewList, spec.context),
        "view_grid" => KeyBinding::new(keystroke, ViewGrid, spec.context),
        "view_cards" => KeyBinding::new(keystroke, ViewCards, spec.context),
        "view_columns" => KeyBinding::new(keystroke, ViewColumns, spec.context),
        "copy_path" => KeyBinding::new(keystroke, CopyPath, spec.context),
        "copy_items" => KeyBinding::new(keystroke, CopyItems, spec.context),
        "cut_items" => KeyBinding::new(keystroke, CutItems, spec.context),
        "paste_items" => KeyBinding::new(keystroke, PasteItems, spec.context),
        "select_all" => KeyBinding::new(keystroke, SelectAll, spec.context),
        "undo_operation" => KeyBinding::new(keystroke, UndoOperation, spec.context),
        "redo_operation" => KeyBinding::new(keystroke, RedoOperation, spec.context),
        _ => return None,
    })
}

/// Non-customizable alternate bindings kept for parity with Explorer / macOS habits.
pub fn extra_file_browser_bindings() -> Vec<KeyBinding> {
    let extras = vec![KeyBinding::new(
        "secondary-backspace",
        DeleteItems,
        Some(FILE_BROWSER),
    )];
    #[cfg(target_os = "macos")]
    {
        extras.push(KeyBinding::new(
            "delete",
            DeleteItems,
            Some(FILE_BROWSER),
        ));
    }
    extras
}
