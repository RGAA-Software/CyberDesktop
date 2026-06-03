use std::path::PathBuf;

use files_core::{init_tracing, set_config_app_id, MEDIA_PLAYER_CONFIG_APP_ID};
use media_player_ui::{init, open_main_window, Assets, PlayerPage};

fn main() {
    set_config_app_id(MEDIA_PLAYER_CONFIG_APP_ID);

    #[cfg(windows)]
    unsafe {
        std::env::set_var("GPUI_DISABLE_DIRECT_COMPOSITION", "1");
    }

    init_tracing(MEDIA_PLAYER_CONFIG_APP_ID);

    let paths: Vec<PathBuf> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    let app = gpui_platform::application().with_assets(Assets);

    app.run(move |cx| {
        init(cx);
        cx.activate(true);

        let paths = paths.clone();
        open_main_window(
            "Cyber Media Player",
            move |window, cx| PlayerPage::view(paths.clone(), window, cx),
            cx,
        );
    });
}
