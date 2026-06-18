//! Application menu bar (File / Edit / Selection / View / Help) for CyberEditor.

use gpui::{App, BorrowAppContext, Entity, Global, Menu, MenuItem, OsAction, SharedString};
use gpui_component::{menu::AppMenuBar, GlobalState};
use rust_i18n::t;

use super::{
    AboutEditor, EditorCopy, EditorCut, EditorPaste, EditorRedo, EditorUndo, ExitEditor,
    FindInFiles, FindNext, FindPrevious, FindText, GoToLine, IndentSelection, KeyboardShortcuts,
    NewFile, OpenFile, OutdentSelection, ReplaceAllText, ReplaceText, SaveFile, SaveFileAs,
    SelectAll, ToggleComment, ToggleFullMarkdownPreview, ToggleLineNumbers, ToggleMarkdownPreview,
    ToggleSoftWrap,
};

struct EditorMenuState {
    menu_bar: Entity<AppMenuBar>,
    line_numbers_checked: bool,
    soft_wrap_checked: bool,
    markdown_preview_checked: bool,
    full_markdown_preview_checked: bool,
}

impl Global for EditorMenuState {}

pub fn menu_bar(cx: &App) -> Entity<AppMenuBar> {
    cx.global::<EditorMenuState>().menu_bar.clone()
}

pub fn init_editor_menus(cx: &mut App) -> Entity<AppMenuBar> {
    let menu_bar = AppMenuBar::new(cx);
    cx.set_global(EditorMenuState {
        menu_bar: menu_bar.clone(),
        line_numbers_checked: true,
        soft_wrap_checked: false,
        markdown_preview_checked: false,
        full_markdown_preview_checked: false,
    });
    reload(cx);
    menu_bar
}

pub fn set_view_toggles(
    line_numbers_checked: bool,
    soft_wrap_checked: bool,
    markdown_preview_checked: bool,
    full_markdown_preview_checked: bool,
    cx: &mut App,
) {
    if !cx.has_global::<EditorMenuState>() {
        return;
    }
    cx.update_global::<EditorMenuState, _>(|state, _| {
        state.line_numbers_checked = line_numbers_checked;
        state.soft_wrap_checked = soft_wrap_checked;
        state.markdown_preview_checked = markdown_preview_checked;
        state.full_markdown_preview_checked = full_markdown_preview_checked;
    });
    reload(cx);
}

pub fn reload(cx: &mut App) {
    if !cx.has_global::<EditorMenuState>() {
        return;
    }
    let menu_bar = cx.global::<EditorMenuState>().menu_bar.clone();
    let line_numbers_checked = cx.global::<EditorMenuState>().line_numbers_checked;
    let soft_wrap_checked = cx.global::<EditorMenuState>().soft_wrap_checked;
    let markdown_preview_checked = cx.global::<EditorMenuState>().markdown_preview_checked;
    let full_markdown_preview_checked =
        cx.global::<EditorMenuState>().full_markdown_preview_checked;
    cx.set_menus(build_menus(
        line_numbers_checked,
        soft_wrap_checked,
        markdown_preview_checked,
        full_markdown_preview_checked,
    ));
    let owned = build_menus(
        line_numbers_checked,
        soft_wrap_checked,
        markdown_preview_checked,
        full_markdown_preview_checked,
    )
    .into_iter()
    .map(|menu| menu.owned())
    .collect();
    if cx.has_global::<GlobalState>() {
        GlobalState::global_mut(cx).set_app_menus(owned);
    }
    menu_bar.update(cx, |bar, cx| bar.reload(cx));
}

/// Top-level menu title with access-key hint, e.g. `File(F)`.
fn menu_title(label: impl Into<SharedString>, access_key: char) -> SharedString {
    let label = label.into();
    SharedString::from(format!("{}({access_key})", label.as_ref()))
}

fn build_menus(
    line_numbers_checked: bool,
    soft_wrap_checked: bool,
    markdown_preview_checked: bool,
    full_markdown_preview_checked: bool,
) -> Vec<Menu> {
    vec![
        Menu {
            name: menu_title(SharedString::from(t!("editor.menu.file")), 'F'),
            items: vec![
                MenuItem::action(SharedString::from(t!("editor.menu.new")), NewFile),
                MenuItem::action(SharedString::from(t!("editor.menu.open")), OpenFile),
                MenuItem::separator(),
                MenuItem::action(SharedString::from(t!("editor.menu.save")), SaveFile),
                MenuItem::action(SharedString::from(t!("editor.menu.save_as")), SaveFileAs),
                MenuItem::separator(),
                MenuItem::action(SharedString::from(t!("editor.menu.exit")), ExitEditor),
            ],
            disabled: false,
        },
        Menu {
            name: menu_title(SharedString::from(t!("editor.menu.edit")), 'E'),
            items: vec![
                MenuItem::os_action(
                    SharedString::from(t!("editor.menu.undo")),
                    EditorUndo,
                    OsAction::Undo,
                ),
                MenuItem::os_action(
                    SharedString::from(t!("editor.menu.redo")),
                    EditorRedo,
                    OsAction::Redo,
                ),
                MenuItem::separator(),
                MenuItem::os_action(
                    SharedString::from(t!("editor.menu.cut")),
                    EditorCut,
                    OsAction::Cut,
                ),
                MenuItem::os_action(
                    SharedString::from(t!("editor.menu.copy")),
                    EditorCopy,
                    OsAction::Copy,
                ),
                MenuItem::os_action(
                    SharedString::from(t!("editor.menu.paste")),
                    EditorPaste,
                    OsAction::Paste,
                ),
                MenuItem::separator(),
                MenuItem::action(SharedString::from(t!("editor.menu.find")), FindText),
                MenuItem::action(
                    SharedString::from(t!("editor.menu.find_in_file")),
                    FindInFiles,
                ),
                MenuItem::action(SharedString::from(t!("editor.menu.replace")), ReplaceText),
                MenuItem::action(
                    SharedString::from(t!("editor.menu.replace_all")),
                    ReplaceAllText,
                ),
                MenuItem::separator(),
                MenuItem::action(SharedString::from(t!("editor.menu.find_next")), FindNext),
                MenuItem::action(
                    SharedString::from(t!("editor.menu.find_previous")),
                    FindPrevious,
                ),
            ],
            disabled: false,
        },
        Menu {
            name: menu_title(SharedString::from(t!("editor.menu.selection")), 'S'),
            items: vec![
                MenuItem::os_action(
                    SharedString::from(t!("editor.menu.select_all")),
                    SelectAll,
                    OsAction::SelectAll,
                ),
                MenuItem::separator(),
                MenuItem::action(
                    SharedString::from(t!("editor.menu.toggle_comment")),
                    ToggleComment,
                ),
                MenuItem::action(
                    SharedString::from(t!("editor.menu.indent")),
                    IndentSelection,
                ),
                MenuItem::action(
                    SharedString::from(t!("editor.menu.outdent")),
                    OutdentSelection,
                ),
            ],
            disabled: false,
        },
        Menu {
            name: menu_title(SharedString::from(t!("editor.menu.view")), 'V'),
            items: vec![
                MenuItem::action(SharedString::from(t!("editor.menu.go_to_line")), GoToLine),
                MenuItem::separator(),
                MenuItem::action(
                    SharedString::from(t!("editor.menu.line_numbers")),
                    ToggleLineNumbers,
                )
                .checked(line_numbers_checked),
                MenuItem::action(
                    SharedString::from(t!("editor.menu.word_wrap")),
                    ToggleSoftWrap,
                )
                .checked(soft_wrap_checked),
                MenuItem::action(
                    SharedString::from(t!("editor.menu.markdown_preview")),
                    ToggleMarkdownPreview,
                )
                .checked(markdown_preview_checked),
                MenuItem::action(
                    SharedString::from(t!("editor.menu.full_markdown_preview")),
                    ToggleFullMarkdownPreview,
                )
                .checked(full_markdown_preview_checked),
            ],
            disabled: false,
        },
        Menu {
            name: menu_title(SharedString::from(t!("editor.menu.help")), 'H'),
            items: vec![
                MenuItem::action(
                    SharedString::from(t!("editor.menu.keyboard_shortcuts")),
                    KeyboardShortcuts,
                ),
                MenuItem::separator(),
                MenuItem::action(SharedString::from(t!("editor.menu.about")), AboutEditor),
            ],
            disabled: false,
        },
    ]
}
