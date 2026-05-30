use std::path::PathBuf;

use cyber_desktop_core::{set_config_app_id, EDITOR_CONFIG_APP_ID};
use cyber_desktop_ui_editor::{init, open_editor_window, Assets, EngineEditor};

fn main() {
    set_config_app_id(EDITOR_CONFIG_APP_ID);
    let path = std::env::args_os().nth(1).map(PathBuf::from);
    let app = gpui_platform::application().with_assets(Assets);

    app.run(move |cx| {
        init(cx);
        cx.activate(true);

        let path = path.clone();
        open_editor_window(
            "CyberEditor",
            move |window, cx| EngineEditor::view(path.clone(), window, cx),
            cx,
        );
    });
}
