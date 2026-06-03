use files_core::{
    disable_direct_composition_enabled, init_tracing, set_config_app_id, FILES_CONFIG_APP_ID,
};
use files_ui::{init, open_main_window, Assets, MainPage};

fn main() {
    set_config_app_id(FILES_CONFIG_APP_ID);

    #[cfg(windows)]
    let disable_direct_composition = disable_direct_composition_enabled();
    #[cfg(windows)]
    unsafe {
        if disable_direct_composition {
            std::env::set_var("GPUI_DISABLE_DIRECT_COMPOSITION", "1");
        } else {
            std::env::remove_var("GPUI_DISABLE_DIRECT_COMPOSITION");
        }
    }

    init_tracing(FILES_CONFIG_APP_ID);
    #[cfg(windows)]
    eprintln!(
        "GPUI DirectComposition disabled: {}",
        disable_direct_composition
    );
    let app = gpui_platform::application().with_assets(Assets);

    app.run(move |cx| {
        init(cx);
        cx.activate(true);

        open_main_window(move |window, cx| MainPage::view(window, cx), cx);
    });
}
