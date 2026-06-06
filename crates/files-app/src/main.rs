use files_core::{
    disable_direct_composition_enabled, init_tracing, log_startup_step, mark_process_start,
    set_config_app_id, FILES_CONFIG_APP_ID,
};
use files_ui::{init, open_main_window, Assets, MainPage};

fn main() {
    mark_process_start();
    set_config_app_id(FILES_CONFIG_APP_ID);
    log_startup_step("set_config_app_id");

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
    log_startup_step("init_tracing");
    #[cfg(windows)]
    eprintln!(
        "GPUI DirectComposition disabled: {}",
        disable_direct_composition
    );
    let app = gpui_platform::application().with_assets(Assets);

    app.run(move |cx| {
        log_startup_step("gpui_app_run_callback");
        init(cx);
        log_startup_step("files_ui_init");
        cx.activate(true);
        log_startup_step("cx_activate");

        open_main_window(move |window, cx| MainPage::view(window, cx), cx);
        log_startup_step("open_main_window_scheduled");
    });
}
