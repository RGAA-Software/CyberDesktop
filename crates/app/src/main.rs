use cyberfiles_ui::{init, open_main_window, Assets, MainPage};

fn main() {
    let app = gpui_platform::application().with_assets(Assets);

    app.run(move |cx| {
        init(cx);
        cx.activate(true);

        open_main_window(move |window, cx| MainPage::view(window, cx), cx);
    });
}
