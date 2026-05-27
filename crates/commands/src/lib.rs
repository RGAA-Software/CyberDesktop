mod shortcuts;

use gpui::{actions, App, KeyBinding};

pub use shortcuts::{shortcut_reference, ShortcutHelp};

actions!(
    cyberfiles_commands,
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
        ReopenClosedTab,
        ToggleShowFileExtensions,
    ]
);

/// GPUI key context for the file browser surface.
pub const FILE_BROWSER: &str = "FileBrowser";

pub fn init(cx: &mut App) {
    cx.bind_keys(file_browser_key_bindings());
}

pub fn file_browser_key_bindings() -> Vec<KeyBinding> {
    let mut bindings = vec![
        KeyBinding::new("backspace", NavigateUp, Some(FILE_BROWSER)),
        KeyBinding::new("enter", OpenItem, Some(FILE_BROWSER)),
        KeyBinding::new("f2", RenameItem, Some(FILE_BROWSER)),
        KeyBinding::new("escape", CancelRename, Some(FILE_BROWSER)),
        KeyBinding::new("delete", DeleteItems, Some(FILE_BROWSER)),
        KeyBinding::new("up", NavigatePrevious, Some(FILE_BROWSER)),
        KeyBinding::new("down", NavigateNext, Some(FILE_BROWSER)),
        KeyBinding::new("ctrl-shift-n", NewFolder, Some(FILE_BROWSER)),
        KeyBinding::new("ctrl-shift-m", NewFile, Some(FILE_BROWSER)),
        KeyBinding::new("ctrl-f", FocusSearch, Some(FILE_BROWSER)),
        KeyBinding::new("ctrl-l", FocusOmnibar, None),
        KeyBinding::new("ctrl-shift-t", ReopenClosedTab, None),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-l", FocusOmnibar, None),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-shift-t", ReopenClosedTab, None),
        KeyBinding::new("ctrl-1", ViewDetails, Some(FILE_BROWSER)),
        KeyBinding::new("ctrl-2", ViewList, Some(FILE_BROWSER)),
        KeyBinding::new("ctrl-3", ViewGrid, Some(FILE_BROWSER)),
        KeyBinding::new("ctrl-shift-4", ViewCards, Some(FILE_BROWSER)),
        KeyBinding::new("ctrl-4", ViewColumns, Some(FILE_BROWSER)),
        KeyBinding::new("ctrl-shift-c", CopyPath, Some(FILE_BROWSER)),
        KeyBinding::new("ctrl-c", CopyItems, Some(FILE_BROWSER)),
        KeyBinding::new("ctrl-x", CutItems, Some(FILE_BROWSER)),
        KeyBinding::new("ctrl-v", PasteItems, Some(FILE_BROWSER)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-shift-n", NewFolder, Some(FILE_BROWSER)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-shift-m", NewFile, Some(FILE_BROWSER)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-f", FocusSearch, Some(FILE_BROWSER)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-shift-c", CopyPath, Some(FILE_BROWSER)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-c", CopyItems, Some(FILE_BROWSER)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-x", CutItems, Some(FILE_BROWSER)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-v", PasteItems, Some(FILE_BROWSER)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-backspace", DeleteItems, Some(FILE_BROWSER)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-a", SelectAll, Some(FILE_BROWSER)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-a", SelectAll, Some(FILE_BROWSER)),
        KeyBinding::new("alt-left", NavigateBack, Some(FILE_BROWSER)),
        KeyBinding::new("alt-right", NavigateForward, Some(FILE_BROWSER)),
        KeyBinding::new("f5", RefreshDirectory, Some(FILE_BROWSER)),
    ];

    bindings.extend([
        KeyBinding::new("secondary-backspace", DeleteItems, Some(FILE_BROWSER)),
        KeyBinding::new("shift-delete", DeleteItemsPermanent, Some(FILE_BROWSER)),
    ]);

    bindings
}
