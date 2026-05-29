//! Application menu bar (File / Edit / Selection / View / Help) for CyberEditor.

use gpui::{App, BorrowAppContext, Entity, Global, Menu, MenuItem, SharedString};
use gpui_component::{menu::AppMenuBar, GlobalState};

use super::{
    AboutEditor, EditorCopy, EditorCut, EditorPaste, EditorRedo, EditorUndo, ExitEditor,
    FindInFiles, FindNext, FindPrevious, FindText, GoToLine, IndentSelection, KeyboardShortcuts,
    NewFile, OpenFile, OutdentSelection, ReplaceAllText, ReplaceText, SaveFile, SaveFileAs,
    SelectAll, ToggleComment, ToggleLineNumbers, ToggleSoftWrap,
};

struct EditorMenuState {
    menu_bar: Entity<AppMenuBar>,
    line_numbers_checked: bool,
    soft_wrap_checked: bool,
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
    });
    reload(cx);
    menu_bar
}

pub fn set_view_toggles(line_numbers_checked: bool, soft_wrap_checked: bool, cx: &mut App) {
    if !cx.has_global::<EditorMenuState>() {
        return;
    }
    cx.update_global::<EditorMenuState, _>(|state, _| {
        state.line_numbers_checked = line_numbers_checked;
        state.soft_wrap_checked = soft_wrap_checked;
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
    cx.set_menus(build_menus(line_numbers_checked, soft_wrap_checked));
    let owned = build_menus(line_numbers_checked, soft_wrap_checked)
        .into_iter()
        .map(|menu| menu.owned())
        .collect();
    if cx.has_global::<GlobalState>() {
        GlobalState::global_mut(cx).set_app_menus(owned);
    }
    menu_bar.update(cx, |bar, cx| bar.reload(cx));
}

fn build_menus(line_numbers_checked: bool, soft_wrap_checked: bool) -> Vec<Menu> {
    vec![
        Menu {
            name: SharedString::from("File"),
            items: vec![
                MenuItem::action("New", NewFile),
                MenuItem::action("Open...", OpenFile),
                MenuItem::separator(),
                MenuItem::action("Save", SaveFile),
                MenuItem::action("Save As...", SaveFileAs),
                MenuItem::separator(),
                MenuItem::action("Exit", ExitEditor),
            ],
            disabled: false,
        },
        Menu {
            name: SharedString::from("Edit"),
            items: vec![
                MenuItem::action("Undo", EditorUndo),
                MenuItem::action("Redo", EditorRedo),
                MenuItem::separator(),
                MenuItem::action("Cut", EditorCut),
                MenuItem::action("Copy", EditorCopy),
                MenuItem::action("Paste", EditorPaste),
                MenuItem::separator(),
                MenuItem::action("Find...", FindText),
                MenuItem::action("Find in Files...", FindInFiles),
                MenuItem::action("Replace...", ReplaceText),
                MenuItem::action("Replace All...", ReplaceAllText),
                MenuItem::separator(),
                MenuItem::action("Find Next", FindNext),
                MenuItem::action("Find Previous", FindPrevious),
            ],
            disabled: false,
        },
        Menu {
            name: SharedString::from("Selection"),
            items: vec![
                MenuItem::action("Select All", SelectAll),
                MenuItem::separator(),
                MenuItem::action("Toggle Comment", ToggleComment),
                MenuItem::action("Indent", IndentSelection),
                MenuItem::action("Outdent", OutdentSelection),
            ],
            disabled: false,
        },
        Menu {
            name: SharedString::from("View"),
            items: vec![
                MenuItem::action("Go to Line...", GoToLine),
                MenuItem::separator(),
                MenuItem::action("Line Numbers", ToggleLineNumbers).checked(line_numbers_checked),
                MenuItem::action("Word Wrap", ToggleSoftWrap).checked(soft_wrap_checked),
            ],
            disabled: false,
        },
        Menu {
            name: SharedString::from("Help"),
            items: vec![
                MenuItem::action("Keyboard Shortcuts", KeyboardShortcuts),
                MenuItem::separator(),
                MenuItem::action("About CyberEditor", AboutEditor),
            ],
            disabled: false,
        },
    ]
}
