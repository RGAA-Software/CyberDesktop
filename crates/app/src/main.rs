use cyber_desktop_core::{set_config_app_id, FILES_CONFIG_APP_ID};
use cyber_desktop_ui::{init, open_main_window, Assets, MainPage};

fn main() {
    set_config_app_id(FILES_CONFIG_APP_ID);
    let app = gpui_platform::application().with_assets(Assets);

    app.run(move |cx| {
        init(cx);
        cx.activate(true);

        open_main_window(move |window, cx| MainPage::view(window, cx), cx);
    });
}
