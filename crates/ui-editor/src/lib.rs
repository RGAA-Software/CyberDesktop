pub use cyberfiles_ui::{open_window, Assets, CyberEditorPage};

use gpui::App;

pub fn init(cx: &mut App) {
    cyberfiles_ui::init_editor_shell(cx);
}
