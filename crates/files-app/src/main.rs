use files_core::{
    init_tracing, log_startup_step, mark_process_start, set_config_app_id, FILES_CONFIG_APP_ID,
};
use files_ui::{init, open_main_window, Assets, MainPage};

#[cfg(windows)]
mod shell_menu_test;

fn main() {
    set_config_app_id(FILES_CONFIG_APP_ID);

    #[cfg(windows)]
    {
        let args: Vec<String> = std::env::args().skip(1).collect();
        if args.iter().any(|arg| arg == "--shell-menu-test") {
            init_tracing(FILES_CONFIG_APP_ID);
            let code = shell_menu_test::run(&args);
            // Hard exit: a wedged Shell extension thread can hold the loader lock, which
            // deadlocks the normal ExitProcess path and leaves a zombie process behind.
            app_platform_windows::hard_exit_process(code as u32);
        }
    }

    init_tracing(FILES_CONFIG_APP_ID);
    log_startup_step("init_tracing");
    mark_process_start();
    log_startup_step("set_config_app_id");

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
