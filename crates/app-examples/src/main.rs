mod mpv_demo;

use app_ui::{open_window, Assets};
use files_core::{init_tracing, set_config_app_id};

const EXAMPLES_CONFIG_APP_ID: &str = "cyber_examples";

fn main() {
    #[cfg(windows)]
    std::env::set_var("GPUI_DISABLE_DIRECT_COMPOSITION", "1");

    set_config_app_id(EXAMPLES_CONFIG_APP_ID);
    init_tracing(EXAMPLES_CONFIG_APP_ID);
    let app = gpui_platform::application().with_assets(Assets);

    app.run(|cx| {
        app_ui::init_editor_shell(cx);
        cx.activate(true);
        open_window(
            "CyberDesktop Examples",
            |window, cx| mpv_demo::MpvDemo::view(window, cx),
            cx,
        );
    });
}
