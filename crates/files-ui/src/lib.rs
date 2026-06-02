#[cfg(feature = "full-app")]
mod app_state;
#[cfg(feature = "full-app")]
mod color_icon;
#[cfg(feature = "full-app")]
mod drag;
#[cfg(feature = "full-app")]
mod file_browser;
#[cfg(feature = "full-app")]
mod file_ops;
mod file_ops_history;
#[cfg(feature = "full-app")]
mod home;
#[cfg(feature = "full-app")]
mod icons;
#[cfg(feature = "full-app")]
mod audio_log;
mod audio_player;
mod info_pane;
mod keybindings;
#[cfg(feature = "full-app")]
mod list_icon_cache;
#[cfg(feature = "full-app")]
mod main_page;
#[cfg(feature = "full-app")]
mod omnibar;
#[cfg(feature = "full-app")]
mod resizable;
#[cfg(feature = "full-app")]
mod settings_view;
mod settings_window;
mod shell;
#[cfg(feature = "full-app")]
mod status_center;
#[cfg(feature = "full-app")]
mod shell_icon;
#[cfg(feature = "full-app")]
mod sidebar;

rust_i18n::i18n!("../app-ui/locales", fallback = "en");

use gpui::App;

pub use app_assets::Assets;
#[cfg(feature = "full-app")]
pub use main_page::MainPage;
#[cfg(feature = "full-app")]
pub use shell::open_main_window;

#[cfg(feature = "full-app")]
pub fn init(cx: &mut App) {
    files_fs::log_extract_environment();
    app_ui::init_editor_shell(cx);
    files_commands::init(cx);
    crate::keybindings::init_keybinding_capture(cx);

    use files_commands::FocusSearch;
    cx.on_action(|_: &FocusSearch, cx| {
        crate::app_state::AppNavigation::focus_search_on_main_window(cx);
    });

    let config = files_core::load_config();
    if let Some(ref cfg) = config {
        shell::preferences::apply_config(cfg, cx);
    }

    settings_window::FilesSettingsWindowState::init(cx);

    #[cfg(windows)]
    app_platform_windows::warm_up_query_context_menu();

    cx.on_action(|_: &shell::Quit, cx| {
        shell::preferences::persist_window_bounds(cx);
        files_core::flush_config();
        cx.quit();
    });
}
