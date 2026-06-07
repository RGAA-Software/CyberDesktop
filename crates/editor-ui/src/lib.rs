pub mod editor_view;

pub use app_ui::{
    open_editor_window, open_window, pick_open_file_path, pick_save_file_path, Assets,
};
pub use editor_view::EngineEditor;

use gpui::App;

rust_i18n::i18n!("../app-ui/locales", fallback = "en");

pub fn init(cx: &mut App) {
    app_ui::init_editor_shell(cx);
}
