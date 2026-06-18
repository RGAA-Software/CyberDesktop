//! Shared imports for `editor_view` submodules.

pub use std::ops::Range;
pub use std::path::{Path, PathBuf};
pub use std::rc::Rc;
pub use std::time::Duration;

pub use app_ui::{
    editor_menu_bar, set_view_toggles, AboutEditor, EditorCopy, EditorCut, EditorPaste, EditorRedo,
    EditorUndo, ExitEditor, FindInFiles, FindNext, FindPrevious, FindText, FoldAll, GoToLine,
    IndentSelection, KeyboardShortcuts, NewFile, OpenFile, OutdentSelection, ReplaceAllText,
    ReplaceText, SaveFile, SaveFileAs, SelectAll, TitleBar, ToggleComment, ToggleFold,
    ToggleFullMarkdownPreview, ToggleLineNumbers, ToggleMarkdownPreview, ToggleSoftWrap, UnfoldAll,
    EDITOR_CONTEXT,
};
pub use editor_text_engine::{
    Cursor, Document, FileMatches, Match, Position, SearchOptions, Searcher, SelectionSet,
    SyntaxState, TextBuffer,
};

pub use gpui::{
    div, point, prelude::*, px, rgb, size, App, Bounds, ClickEvent, ClipboardItem, Context,
    CursorStyle, DragMoveEvent, Entity, EntityInputHandler, ExternalPaths, FocusHandle, Focusable,
    IntoElement, KeyDownEvent, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels,
    Point, Render, ScrollWheelEvent, SharedString, Size, Stateful, TextRun, UTF16Selection, Window,
};
pub use gpui_component::{
    button::{Button, ButtonVariants as _},
    dialog::{Dialog, DialogFooter},
    h_flex,
    input::{Input, InputEvent, InputState},
    label::Label,
    scroll::{ScrollableElement as _, ScrollbarAxis},
    separator::Separator,
    v_flex, v_virtual_list, ActiveTheme as _, Selectable as _, Sizable as _,
    VirtualListScrollHandle,
};
pub use rust_i18n::t;

pub(crate) use super::canvas::EditorCanvas;
pub(crate) use super::editor::{CloseTarget, EngineEditor, PreviewResizeDrag};
pub(crate) use super::language::{comment_prefix, language_for_path};
pub(crate) use super::state::*;
pub(crate) use super::text_util::wrap_rows;
