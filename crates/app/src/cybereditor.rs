use std::path::PathBuf;

use cyberfiles_ui_editor::{init, open_window, Assets, CyberEditorPage};

fn main() {
    let path = std::env::args_os().nth(1).map(PathBuf::from);
    let app = gpui_platform::application().with_assets(Assets);

    app.run(move |cx| {
        init(cx);
        cx.activate(true);

        let path = path.clone();
        open_window("CyberEditor", move |window, cx| {
            CyberEditorPage::view(path.clone(), window, cx)
        }, cx);
    });
}
