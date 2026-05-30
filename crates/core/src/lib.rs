pub mod config;
pub mod path_history;

pub const APP_NAME: &str = "CyberFiles";

/// Config directory / file namespace for the file manager binary.
pub const FILES_CONFIG_APP_ID: &str = "cyber_files";
/// Config directory / file namespace for the editor binary.
pub const EDITOR_CONFIG_APP_ID: &str = "cyber_editor";

pub const WINDOW_WIDTH: f32 = 1600.;
pub const WINDOW_HEIGHT: f32 = 900.;

pub use config::{
    context_menu_item_prefs, default_home_widget_order, file_sort_prefs_from_config,
    file_view_mode_from_config, flush_config, home_widget_prefs, load_config,
    normalize_home_widget_order, open_text_with_cybereditor_enabled, pinned_folder_paths,
    save_config, save_file_browser_prefs, save_home_widget_prefs, set_config_app_id,
    sidebar_is_compact, sidebar_is_offcanvas, window_size, AppConfig, ClosedTabSession,
    ContextMenuItemPrefs, FileTagConfig, HomeWidgetPrefs, SessionPaneLayout, VIEW_CARDS,
    VIEW_COLUMNS, VIEW_DETAILS, VIEW_GRID, VIEW_LIST,
};
pub use path_history::{path_history_list, record_path_history};
