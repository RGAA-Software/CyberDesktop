//! Human-readable shortcut reference for the Actions settings page.

pub struct ShortcutHelp {
    /// Full locale key, e.g. `settings.actions.copy`.
    pub message_key: &'static str,
    pub keystroke: &'static str,
}

pub fn shortcut_reference() -> &'static [ShortcutHelp] {
    &[
        ShortcutHelp {
            message_key: "settings.actions.navigate_back",
            keystroke: "Alt+Left",
        },
        ShortcutHelp {
            message_key: "settings.actions.navigate_forward",
            keystroke: "Alt+Right",
        },
        ShortcutHelp {
            message_key: "settings.actions.navigate_up",
            keystroke: "Backspace",
        },
        ShortcutHelp {
            message_key: "settings.actions.refresh",
            keystroke: "F5",
        },
        ShortcutHelp {
            message_key: "settings.actions.open",
            keystroke: "Enter",
        },
        ShortcutHelp {
            message_key: "settings.actions.rename",
            keystroke: "F2",
        },
        ShortcutHelp {
            message_key: "settings.actions.delete",
            keystroke: "Delete",
        },
        ShortcutHelp {
            message_key: "settings.actions.delete_permanent",
            keystroke: "Shift+Delete",
        },
        ShortcutHelp {
            message_key: "settings.actions.select_all",
            keystroke: "Ctrl+A",
        },
        ShortcutHelp {
            message_key: "settings.actions.copy",
            keystroke: "Ctrl+C",
        },
        ShortcutHelp {
            message_key: "settings.actions.cut",
            keystroke: "Ctrl+X",
        },
        ShortcutHelp {
            message_key: "settings.actions.paste",
            keystroke: "Ctrl+V",
        },
        ShortcutHelp {
            message_key: "settings.actions.copy_path",
            keystroke: "Ctrl+Shift+C",
        },
        ShortcutHelp {
            message_key: "settings.actions.new_folder",
            keystroke: "Ctrl+Shift+N",
        },
        ShortcutHelp {
            message_key: "settings.actions.new_file",
            keystroke: "Ctrl+Shift+M",
        },
        ShortcutHelp {
            message_key: "settings.actions.focus_search",
            keystroke: "Ctrl+F",
        },
        ShortcutHelp {
            message_key: "settings.actions.focus_omnibar",
            keystroke: "Ctrl+L",
        },
        ShortcutHelp {
            message_key: "settings.actions.reopen_tab",
            keystroke: "Ctrl+Shift+T",
        },
        ShortcutHelp {
            message_key: "settings.actions.view_details",
            keystroke: "Ctrl+1",
        },
        ShortcutHelp {
            message_key: "settings.actions.view_grid",
            keystroke: "Ctrl+2",
        },
        ShortcutHelp {
            message_key: "settings.actions.view_columns",
            keystroke: "Ctrl+3",
        },
    ]
}
