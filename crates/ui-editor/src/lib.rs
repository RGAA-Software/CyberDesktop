pub mod editor_view;

pub use cyberfiles_ui::{
    open_editor_window, open_window, pick_open_file_path, pick_save_file_path, Assets,
    CyberEditorPage,
};
pub use editor_view::EngineEditor;

use gpui::App;

pub fn init(cx: &mut App) {
    cyberfiles_ui::init_editor_shell(cx);
}
