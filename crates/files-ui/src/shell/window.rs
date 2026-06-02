use files_core::window_size;
#[cfg(feature = "full-app")]
use files_core::APP_NAME;
use gpui::{
    px, size, App, AppContext, Bounds, Focusable, SharedString, Size, Window, WindowBounds, WindowKind,
    WindowOptions,
};
use gpui_component::Root;

#[cfg(feature = "full-app")]
use super::app_shell::AppShell;
#[cfg(feature = "full-app")]
use crate::app_state::AppNavigation;
use app_ui::title_bar::TitleBar;

#[cfg(feature = "full-app")]
pub fn open_main_window<F, E>(crate_view_fn: F, cx: &mut App)
where
    E: Into<gpui::AnyView>,
    F: FnOnce(&mut Window, &mut App) -> E + Send + 'static,
{
    open_main_window_with_close_handler(APP_NAME, crate_view_fn, |_, _| true, cx);
}

#[cfg(feature = "full-app")]
fn open_main_window_with_close_handler<F, E, C>(
    title: impl Into<SharedString>,
    crate_view_fn: F,
    on_should_close: C,
    cx: &mut App,
) where
    E: Into<gpui::AnyView>,
    F: FnOnce(&mut Window, &mut App) -> E + Send + 'static,
    C: Fn(&mut Window, &mut App) -> bool + Send + 'static,
{
    let (width, height) = window_size();
    let mut window_size = size(px(width), px(height));
    if let Some(display) = cx.primary_display() {
        let display_size = display.bounds().size;
        window_size.width = window_size.width.min(display_size.width * 0.85);
        window_size.height = window_size.height.min(display_size.height * 0.85);
    }
    let window_bounds = Bounds::centered(None, window_size, cx);
    let title = title.into();

    cx.spawn(async move |cx| {
        let mut on_should_close = Some(on_should_close);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(window_bounds)),
            titlebar: Some(TitleBar::title_bar_options()),
            window_min_size: Some(Size {
                width: px(480.),
                height: px(320.),
            }),
            kind: WindowKind::Normal,
            #[cfg(target_os = "linux")]
            window_background: gpui::WindowBackgroundAppearance::Transparent,
            #[cfg(target_os = "linux")]
            window_decorations: Some(gpui::WindowDecorations::Client),
            ..Default::default()
        };

        let window = cx
            .open_window(options, |window, cx| {
                if let Some(on_should_close) = on_should_close.take() {
                    window.on_window_should_close(cx, on_should_close);
                }
                let view = crate_view_fn(window, cx);

                let shell = cx.new(|cx| AppShell::new(view, window, cx));

                window.defer(cx, move |window, cx| {
                    if window.focused(cx).is_none() {
                        if let Some(nav) = cx.try_global::<AppNavigation>() {
                            nav.main_page().read(cx).focus_handle(cx).focus(window, cx);
                        }
                    }
                });

                cx.new(|cx| Root::new(shell, window, cx))
            })
            .expect("failed to open window");

            let main_handle = window.into();
            let _ = cx.update(|cx| {
                crate::app_state::MainWindowState::set(main_handle, cx);
            });

        window.update(cx, |_, window, _| {
            window.activate_window();
            window.set_window_title(&title);
        })?;

        Ok::<_, anyhow::Error>(())
    })
    .detach();
}
