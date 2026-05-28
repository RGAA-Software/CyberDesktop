mod app_menus;
mod buffer_model;
#[cfg(not(feature = "zed-engine"))]
mod backend;
mod context_menu;
mod document;
mod editor_host;
mod file_dialog;
#[cfg(feature = "zed-engine")]
mod zed_backend;
#[cfg(feature = "zed-engine")]
pub(crate) use zed_backend::ZedEditorBackend;
mod language;
mod metadata;
mod page;
mod session;

pub use app_menus::{init_editor_menus, menu_bar as editor_menu_bar};

use gpui::{actions, App, KeyBinding};

pub use page::CyberEditorPage;

pub(crate) use buffer_model::{EditorBufferModel, SearchMatch};
#[cfg(not(feature = "zed-engine"))]
pub(crate) use backend::ModelEditorBackend;
pub(crate) use document::{display_language, display_name, display_path, load_document};
pub(crate) use editor_host::EditorHost;
pub(crate) use language::language_for_path;
#[cfg(not(feature = "zed-engine"))]
pub(crate) use language::line_comment_prefix;
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
    #[cfg(feature = "zed-engine")]
    cyber_editor_engine::init(cx);

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
        // While the Zed editor surface is focused, `key_context` is `Editor`, not `CyberEditor`.
        #[cfg(feature = "zed-engine")]
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-o", OpenFile, Some("Editor")),
        #[cfg(feature = "zed-engine")]
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-s", SaveFile, Some("Editor")),
        #[cfg(feature = "zed-engine")]
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-shift-s", SaveFileAs, Some("Editor")),
        #[cfg(feature = "zed-engine")]
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-g", GoToLine, Some("Editor")),
        #[cfg(feature = "zed-engine")]
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-f", FindText, Some("Editor")),
        #[cfg(feature = "zed-engine")]
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-h", ReplaceText, Some("Editor")),
        #[cfg(feature = "zed-engine")]
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-shift-h", ReplaceAllText, Some("Editor")),
        #[cfg(feature = "zed-engine")]
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-/", ToggleComment, Some("Editor")),
        #[cfg(feature = "zed-engine")]
        KeyBinding::new("f3", FindNext, Some("Editor")),
        #[cfg(feature = "zed-engine")]
        KeyBinding::new("shift-f3", FindPrevious, Some("Editor")),
        #[cfg(feature = "zed-engine")]
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-n", NewFile, Some("Editor")),
        #[cfg(feature = "zed-engine")]
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-z", EditorUndo, Some("Editor")),
        #[cfg(feature = "zed-engine")]
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-y", EditorRedo, Some("Editor")),
        #[cfg(feature = "zed-engine")]
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-x", EditorCut, Some("Editor")),
        #[cfg(feature = "zed-engine")]
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-c", EditorCopy, Some("Editor")),
        #[cfg(feature = "zed-engine")]
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-v", EditorPaste, Some("Editor")),
        #[cfg(feature = "zed-engine")]
        #[cfg(not(target_os = "macos"))]
        KeyBinding::new("ctrl-a", SelectAll, Some("Editor")),
    ]);
}
