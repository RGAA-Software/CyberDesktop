#[cfg(feature = "full-app")]
mod app_state;
#[cfg(feature = "full-app")]
mod color_icon;
mod cyber_editor;
#[cfg(feature = "full-app")]
mod drag;
#[cfg(feature = "full-app")]
mod file_browser;
#[cfg(feature = "full-app")]
mod file_ops;
#[cfg(feature = "full-app")]
mod home;
mod i18n;
#[cfg(feature = "full-app")]
mod icons;
#[cfg(feature = "full-app")]
mod info_pane;
#[cfg(feature = "full-app")]
mod list_icon_cache;
#[cfg(feature = "full-app")]
mod main_page;
#[cfg(feature = "full-app")]
mod omnibar;
mod popup_menu;
#[cfg(feature = "full-app")]
mod resizable;
#[cfg(feature = "full-app")]
mod settings_view;
mod shell;
#[cfg(feature = "full-app")]
mod status_center;
#[cfg(feature = "full-app")]
mod shell_icon;
#[cfg(feature = "full-app")]
mod sidebar;
#[cfg(feature = "full-app")]
mod tab;
mod theme;
mod title_bar;
#[cfg(feature = "full-app")]
mod toolbar_button;

rust_i18n::i18n!("locales", fallback = "en");

use gpui::App;

pub use cyberfiles_assets::Assets;
pub use cyber_editor::{
    editor_menu_bar, pick_open_file_path, pick_save_file_path, set_view_toggles, AboutEditor,
    CyberEditorPage, EditorCopy, EditorCut, EditorPaste, EditorRedo, EditorUndo, ExitEditor,
    FindInFiles, FindNext, FindPrevious, FindText, GoToLine, IndentSelection, KeyboardShortcuts,
    NewFile, OpenFile, OutdentSelection, ReplaceAllText, ReplaceText, SaveFile, SaveFileAs,
    SelectAll, ToggleComment, ToggleLineNumbers, ToggleSoftWrap,
};
pub use i18n::{init_locale, locale, set_locale};
#[cfg(feature = "full-app")]
pub use main_page::MainPage;
pub use popup_menu::{ContextMenuExt, DropdownMenu, PopupMenu, PopupMenuItem};
pub use title_bar::TitleBar;
#[cfg(feature = "full-app")]
pub use shell::open_main_window;
pub use shell::{open_editor_window, open_window, open_window_with_close_handler};

pub fn init_editor_shell(cx: &mut App) {
    cyber_editor::init(cx);

    let config = cyberfiles_core::load_config();
    if let Some(ref cfg) = config {
        set_locale(&cfg.locale);
    } else {
        init_locale();
    }
    gpui_component::init(cx);
    popup_menu::init(cx);
    cyber_editor::init_editor_menus(cx);
    theme::install(cx);
    #[cfg(feature = "full-app")]
    cx.set_global(crate::app_state::AppFileClipboard::default());
}

#[cfg(feature = "full-app")]
pub fn init(cx: &mut App) {
    init_editor_shell(cx);
    cyberfiles_commands::init(cx);
    popup_menu::init(cx);

    let config = cyberfiles_core::load_config();
    if let Some(ref cfg) = config {
        shell::preferences::apply_config(cfg, cx);
    }

    #[cfg(windows)]
    cyberfiles_platform_windows::warm_up_query_context_menu();

    cx.on_action(|_: &shell::Quit, cx| {
        shell::preferences::persist_window_bounds(cx);
        cyberfiles_core::flush_config();
        cx.quit();
    });
}
