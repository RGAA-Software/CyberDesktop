pub mod color_icon;
pub mod cyber_editor;
pub mod i18n;
pub mod popup_menu;
mod settings_window;
pub mod tab;
pub mod theme;
pub mod title_bar;
pub mod toolbar_button;
mod window;

rust_i18n::i18n!("locales", fallback = "en");

use gpui::App;

pub use app_assets::Assets;
pub use color_icon::{color_icon, color_icon_box};
pub use files_core::GITHUB_REPO_URL;
pub use cyber_editor::{
    apply_theme_mode, build_editor_settings, editor_menu_bar, pick_open_file_path, pick_save_file_path,
    set_view_toggles, AboutEditor, EditorCopy, EditorCut, EditorPaste, EditorRedo,
    EditorUndo, ExitEditor, FindInFiles, FindNext, FindPrevious, FindText, FoldAll, GoToLine,
    IndentSelection, KeyboardShortcuts, NewFile, OpenFile, OutdentSelection, ReplaceAllText,
    ReplaceText, SaveFile, SaveFileAs, SelectAll, ToggleComment, ToggleFold, ToggleLineNumbers,
    ToggleMarkdownPreview, ToggleSoftWrap, UnfoldAll, EDITOR_CONTEXT,
};
pub use i18n::{init_locale, locale, set_locale};
pub use popup_menu::{ContextMenuExt, DropdownMenu, PopupMenu, PopupMenuItem};
pub use settings_window::{SettingsWindowState};
pub use tab::{Tab, TabBar};
pub use title_bar::{title_bar_bottom_rule, TitleBar, TITLE_BAR_HEIGHT};
pub use toolbar_button::{toolbar_icon, toolbar_icon_button, TOOLBAR_BUTTON_PX, TOOLBAR_ICON_BUTTON_SIZE};
pub use window::{open_editor_window, open_window, open_window_with_close_handler};

pub fn init_editor_shell(cx: &mut App) {
    let _ = app_assets::Assets.load_fonts(cx);
    let config = files_core::load_config();
    if let Some(ref cfg) = config {
        set_locale(&cfg.locale);
    } else {
        init_locale();
    }
    gpui_component::init(cx);
    popup_menu::init(cx);
    cyber_editor::init(cx);
    cyber_editor::init_editor_menus(cx);
    theme::install(cx);
    if let Some(ref cfg) = config {
        theme::apply_from_config(cfg, cx);
    } else {
        theme::apply_set("CyberEditor", gpui_component::ThemeMode::Light, cx);
    }
    SettingsWindowState::init(cx);
    use crate::cyber_editor::ExitEditor;
    cx.on_action(|_: &ExitEditor, _cx| {
        files_core::flush_config();
    });
}
