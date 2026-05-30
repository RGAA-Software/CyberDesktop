//! Shared imports for `editor_view` submodules.

pub use std::ops::Range;
pub use std::path::{Path, PathBuf};
pub use std::rc::Rc;
pub use std::time::{Duration, SystemTime};

pub use cyberfiles_text_engine::{
    load_file, search_directory, Cursor, Document, FileMatches, GlobalSearchOptions, Match,
    Position, SearchOptions, Searcher, SelectionSet, SyntaxState, TextBuffer,
};
pub use cyberfiles_ui::{
    build_editor_settings, editor_menu_bar, set_view_toggles, AboutEditor, EditorCopy, EditorCut, EditorPaste, EditorRedo,
    EditorUndo, ExitEditor, FindInFiles, FindNext, FindPrevious, FindText, GoToLine,
    IndentSelection, KeyboardShortcuts, NewFile, OpenFile, OutdentSelection, ReplaceAllText,
    ReplaceText, SaveFile, SaveFileAs, SelectAll, TitleBar, ToggleComment, ToggleLineNumbers,
    ToggleSoftWrap, EDITOR_CONTEXT,
};
pub use rust_i18n::t;
pub use gpui::{
    div, point, prelude::*, px, rgb, size, App, Bounds, ClickEvent, ClipboardItem, Context, Entity,
    EntityInputHandler, FocusHandle, Focusable, GlobalElementId, Hsla, IntoElement, KeyDownEvent,
    LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, PaintQuad, Pixels, Point,
    Render, ScrollWheelEvent, SharedString, ShapedLine, Size, Stateful, Style, Subscription,
    TextRun, UTF16Selection, Window, WrappedLine, relative,
};
pub use gpui_component::{
    button::{Button, ButtonVariants as _},
    dialog::{Dialog, DialogFooter},
    h_flex, v_flex,
    input::{Input, InputEvent, InputState},
    label::Label,
    scroll::{ScrollableElement as _, ScrollbarAxis},
    separator::Separator,
    v_virtual_list, ActiveTheme as _, Selectable as _, Sizable as _,
    VirtualListScrollHandle,
};

pub(crate) use super::canvas::EditorCanvas;
pub(crate) use super::editor::{CloseTarget, EngineEditor};
pub(crate) use super::language::{comment_prefix, language_for_path};
pub(crate) use super::state::*;
pub(crate) use super::text_util::{char_to_byte, wrap_rows};
