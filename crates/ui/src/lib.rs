mod app_state;
mod color_icon;
mod cyber_editor;
mod drag;
mod file_browser;
mod file_ops;
mod home;
mod i18n;
mod icons;
mod info_pane;
mod list_icon_cache;
mod main_page;
mod omnibar;
mod popup_menu;
mod resizable;
mod settings_view;
mod shell;
mod status_center;
mod shell_icon;
mod sidebar;
mod tab;
mod theme;
mod title_bar;
mod toolbar_button;

rust_i18n::i18n!("locales", fallback = "en");

use gpui::App;

pub use cyberfiles_assets::Assets;
pub use cyber_editor::CyberEditorPage;
pub use i18n::{init_locale, locale, set_locale};
pub use main_page::MainPage;
pub use popup_menu::{ContextMenuExt, DropdownMenu, PopupMenu, PopupMenuItem};
pub use shell::{open_main_window, open_window, open_window_with_close_handler};

pub fn init(cx: &mut App) {
    cyberfiles_commands::init(cx);
    cyber_editor::init(cx);

    let config = cyberfiles_core::load_config();
    if let Some(ref cfg) = config {
        set_locale(&cfg.locale);
    } else {
        init_locale();
    }
    gpui_component::init(cx);
    #[cfg(feature = "zed-engine")]
    cyber_editor::init_editor_menus(cx);
    popup_menu::init(cx);
    theme::install(cx);
    cx.set_global(crate::app_state::AppFileClipboard::default());
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
