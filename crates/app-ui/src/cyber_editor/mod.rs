mod app_menus;
mod editor_settings;
mod file_dialog;
mod preferences;

pub use preferences::apply_theme_mode;
pub use app_menus::{init_editor_menus, menu_bar as editor_menu_bar, set_view_toggles};
pub use editor_settings::build_editor_settings;
pub use file_dialog::{pick_open_file_path, pick_save_file_path};

use gpui::{actions, App, KeyBinding};

pub const APP_NAME: &str = "CyberEditor";
pub const EDITOR_CONTEXT: &str = "CyberEditor";

actions!(
    cybereditor,
    [
        NewFile,
        OpenFile,
        SaveFile,
        SaveFileAs,
        ExitEditor,
        EditorUndo,
        EditorRedo,
        EditorCut,
        EditorCopy,
        EditorPaste,
        SelectAll,
        GoToLine,
        FindText,
        FindInFiles,
        ReplaceText,
        ReplaceAllText,
        ToggleComment,
        IndentSelection,
        OutdentSelection,
        ToggleLineNumbers,
        ToggleSoftWrap,
        FindNext,
        FindPrevious,
        AboutEditor,
        KeyboardShortcuts,
        ToggleFold,
        FoldAll,
        UnfoldAll,
    ]
);

pub fn init(cx: &mut App) {
    cx.bind_keys([
        // File
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-n", NewFile, Some(EDITOR_CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-t", NewFile, Some(EDITOR_CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-o", OpenFile, Some(EDITOR_CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-s", SaveFile, Some(EDITOR_CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-shift-s", SaveFileAs, Some(EDITOR_CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("alt-f4", ExitEditor, Some(EDITOR_CONTEXT)),
        // Edit
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-z", EditorUndo, Some(EDITOR_CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-y", EditorRedo, Some(EDITOR_CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-x", EditorCut, Some(EDITOR_CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-c", EditorCopy, Some(EDITOR_CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-v", EditorPaste, Some(EDITOR_CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-a", SelectAll, Some(EDITOR_CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-g", GoToLine, Some(EDITOR_CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-f", FindText, Some(EDITOR_CONTEXT)),
        // Override gpui-component Input ctrl-f when find/goto panel inputs are focused.
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-f", FindText, Some("Input")),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-shift-f", FindInFiles, Some(EDITOR_CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-h", ReplaceText, Some(EDITOR_CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-shift-h", ReplaceAllText, Some(EDITOR_CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-/", ToggleComment, Some(EDITOR_CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("alt-]", IndentSelection, Some(EDITOR_CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("alt-[", OutdentSelection, Some(EDITOR_CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("alt-l", ToggleLineNumbers, Some(EDITOR_CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("alt-z", ToggleSoftWrap, Some(EDITOR_CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("alt-shift-f", ToggleFold, Some(EDITOR_CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("alt-shift-k", FoldAll, Some(EDITOR_CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("alt-shift-j", UnfoldAll, Some(EDITOR_CONTEXT)),
        KeyBinding::new("f3", FindNext, Some(EDITOR_CONTEXT)),
        KeyBinding::new("shift-f3", FindPrevious, Some(EDITOR_CONTEXT)),
        KeyBinding::new("f1", KeyboardShortcuts, Some(EDITOR_CONTEXT)),
        // macOS
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-n", NewFile, Some(EDITOR_CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-t", NewFile, Some(EDITOR_CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-o", OpenFile, Some(EDITOR_CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-s", SaveFile, Some(EDITOR_CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-shift-s", SaveFileAs, Some(EDITOR_CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-q", ExitEditor, Some(EDITOR_CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-z", EditorUndo, Some(EDITOR_CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-shift-z", EditorRedo, Some(EDITOR_CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-x", EditorCut, Some(EDITOR_CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-c", EditorCopy, Some(EDITOR_CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-v", EditorPaste, Some(EDITOR_CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-a", SelectAll, Some(EDITOR_CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-g", GoToLine, Some(EDITOR_CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-f", FindText, Some(EDITOR_CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-f", FindText, Some("Input")),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-shift-f", FindInFiles, Some(EDITOR_CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-h", ReplaceText, Some(EDITOR_CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-shift-h", ReplaceAllText, Some(EDITOR_CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-/", ToggleComment, Some(EDITOR_CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("alt-]", IndentSelection, Some(EDITOR_CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("alt-[", OutdentSelection, Some(EDITOR_CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("alt-l", ToggleLineNumbers, Some(EDITOR_CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("alt-z", ToggleSoftWrap, Some(EDITOR_CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("alt-shift-f", ToggleFold, Some(EDITOR_CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("alt-shift-k", FoldAll, Some(EDITOR_CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("alt-shift-j", UnfoldAll, Some(EDITOR_CONTEXT)),
    ]);
}
