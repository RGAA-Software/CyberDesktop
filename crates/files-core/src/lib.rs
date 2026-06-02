pub mod config;
pub mod history_store;
pub mod logging;
pub mod path_history;
pub mod search_history;
pub mod session_store;

pub const APP_NAME: &str = "CyberFiles";

/// Config directory / file namespace for the file manager binary.
pub const FILES_CONFIG_APP_ID: &str = "cyber_files";
/// Config directory / file namespace for the editor binary.
pub const EDITOR_CONFIG_APP_ID: &str = "cyber_editor";

pub const WINDOW_WIDTH: f32 = 1600.;
pub const WINDOW_HEIGHT: f32 = 900.;

pub const GITHUB_REPO_URL: &str = "https://github.com/RGAA-Software/CyberDesktop";

pub use config::{
    context_menu_item_prefs, default_home_widget_order, file_sort_prefs_from_config,
    file_view_mode_from_config, flush_config, home_widget_prefs, load_config,
    normalize_home_widget_order, open_text_with_cybereditor_enabled, pinned_folder_paths,
    save_config, save_file_browser_prefs, save_home_widget_prefs, save_keybinding_override,
    reset_all_keybinding_overrides, reset_keybinding_override, keybinding_overrides,
    set_config_app_id,
    sidebar_is_compact, sidebar_is_offcanvas, window_size, AppConfig, ClosedTabSession,
    ContextMenuItemPrefs, FileTagConfig, HomeWidgetPrefs, SessionPaneLayout, VIEW_CARDS,
    VIEW_COLUMNS, VIEW_DETAILS, VIEW_GRID, VIEW_LIST, GROUP_CREATED, GROUP_DATE_DAY,
    GROUP_DATE_MONTH, GROUP_DATE_YEAR, GROUP_MODIFIED, GROUP_NAME, GROUP_NONE, GROUP_SIZE,
    GROUP_TAG, GROUP_TYPE, file_group_date_unit_from_config, file_group_from_config,
};
pub use logging::init_tracing;
pub use path_history::{path_history_list, record_path_history};
pub use search_history::{record_search_history, search_history_list};
pub use session_store::{load_session_tabs, save_session_tabs};
