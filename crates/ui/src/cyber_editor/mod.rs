mod app_menus;
mod backend;
mod buffer_model;
mod document;
mod editor_host;
mod file_dialog;
mod language;
mod metadata;
mod page;
mod session;

pub use app_menus::{init_editor_menus, menu_bar as editor_menu_bar};

use gpui::{actions, App, KeyBinding};

pub use page::CyberEditorPage;

pub(crate) use backend::ModelEditorBackend;
pub(crate) use buffer_model::{EditorBufferModel, SearchMatch};
pub(crate) use document::{display_language, display_name, display_path, load_document};
pub(crate) use editor_host::EditorHost;
pub(crate) use language::{language_for_path, line_comment_prefix};
pub(crate) use metadata::{detect_indent_style, detect_line_ending, IndentStyle, LineEndingKind};
pub(crate) use session::EditorSession;

pub(crate) const APP_NAME: &str = "CyberEditor";
pub(crate) const EDITOR_CONTEXT: &str = "CyberEditor";

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
        ReplaceText,
        ReplaceAllText,
        ToggleComment,
        IndentSelection,
        OutdentSelection,
        ToggleLineNumbers,
        ToggleSoftWrap,
        FindNext,
        FindPrevious,
        AboutEditor
    ]
);

pub fn init(cx: &mut App) {
    cx.bind_keys([
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-o", OpenFile, Some(EDITOR_CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-s", SaveFile, Some(EDITOR_CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-shift-s", SaveFileAs, Some(EDITOR_CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-g", GoToLine, Some(EDITOR_CONTEXT)),
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-f", FindText, Some(EDITOR_CONTEXT)),
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
        KeyBinding::new("f3", FindNext, Some(EDITOR_CONTEXT)),
        KeyBinding::new("shift-f3", FindPrevious, Some(EDITOR_CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-o", OpenFile, Some(EDITOR_CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-s", SaveFile, Some(EDITOR_CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-shift-s", SaveFileAs, Some(EDITOR_CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-g", GoToLine, Some(EDITOR_CONTEXT)),
        #[cfg(target_os = "macos")]
        KeyBinding::new("cmd-f", FindText, Some(EDITOR_CONTEXT)),
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
        KeyBinding::new("f3", FindNext, Some(EDITOR_CONTEXT)),
        KeyBinding::new("shift-f3", FindPrevious, Some(EDITOR_CONTEXT)),
    ]);
}
