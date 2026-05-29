use cyberfiles_core::window_size;
#[cfg(feature = "full-app")]
use cyberfiles_core::APP_NAME;
use gpui::{
    div, px, size, AnyView, App, AppContext, Bounds, Context, Focusable, IntoElement,
    InteractiveElement, ParentElement, Render, SharedString, Size, Styled, Window, WindowBounds,
    WindowKind, WindowOptions,
};
use gpui_component::Root;

#[cfg(feature = "full-app")]
use super::app_shell::AppShell;
#[cfg(feature = "full-app")]
use crate::app_state::AppNavigation;
use crate::title_bar::TitleBar;

/// Minimal window chrome for the standalone editor: a draggable
/// gpui-component title bar (window controls + drag region) above the editor
/// view, plus overlay layers. Always available so `cybereditor` gets it even
/// when the `full-app` feature is unified in by the workspace build.
struct EditorShell {
    view: AnyView,
}

impl EditorShell {
    fn new(view: impl Into<AnyView>) -> Self {
        Self { view: view.into() }
    }
}

impl Render for EditorShell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // The editor view renders its own title bar (drag region + menu bar +
        // window controls), so the shell only hosts the view and overlays.
        let sheet_layer = Root::render_sheet_layer(window, cx);
        let dialog_layer = Root::render_dialog_layer(window, cx);
        let notification_layer = Root::render_notification_layer(window, cx);

        div()
            .id("editor-shell")
            .size_full()
            .child(
                div()
                    .id("editor-shell-main")
                    .size_full()
                    .child(self.view.clone()),
            )
            .children(sheet_layer)
            .children(dialog_layer)
            .children(notification_layer)
    }
}

#[cfg(feature = "full-app")]
pub fn open_main_window<F, E>(crate_view_fn: F, cx: &mut App)
where
    E: Into<gpui::AnyView>,
    F: FnOnce(&mut Window, &mut App) -> E + Send + 'static,
{
    open_window(APP_NAME, crate_view_fn, cx);
}

pub fn open_window<F, E>(title: impl Into<SharedString>, crate_view_fn: F, cx: &mut App)
where
    E: Into<gpui::AnyView>,
    F: FnOnce(&mut Window, &mut App) -> E + Send + 'static,
{
    open_window_with_close_handler(title, crate_view_fn, |_, _| true, cx);
}

/// Opens a standalone editor window wrapped in [`EditorShell`] (draggable
/// title bar + window controls). Used by the `cybereditor` binary regardless
/// of which crate features the workspace build happens to unify in.
pub fn open_editor_window<F, E>(title: impl Into<SharedString>, crate_view_fn: F, cx: &mut App)
where
    E: Into<gpui::AnyView>,
    F: FnOnce(&mut Window, &mut App) -> E + Send + 'static,
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
                let view = crate_view_fn(window, cx);
                let shell = cx.new(|_| EditorShell::new(view));
                cx.new(|cx| Root::new(shell, window, cx))
            })
            .expect("failed to open window");

        window.update(cx, |_, window, _| {
            window.activate_window();
            window.set_window_title(&title);
        })?;

        Ok::<_, anyhow::Error>(())
    })
    .detach();
}

pub fn open_window_with_close_handler<F, E, C>(
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

                #[cfg(feature = "full-app")]
                {
                let shell = cx.new(|cx| AppShell::new(view, window, cx));

                window.defer(cx, move |window, cx| {
                    if window.focused(cx).is_none() {
                        if let Some(nav) = cx.try_global::<AppNavigation>() {
                            nav.main_page().read(cx).focus_handle(cx).focus(window, cx);
                        }
                    }
                });

                cx.new(|cx| Root::new(shell, window, cx))
                }
                #[cfg(not(feature = "full-app"))]
                {
                    let shell = cx.new(|_| EditorShell::new(view));
                    cx.new(|cx| Root::new(shell, window, cx))
                }
            })
            .expect("failed to open window");

        window.update(cx, |_, window, _| {
            window.activate_window();
            window.set_window_title(&title);
        })?;

        Ok::<_, anyhow::Error>(())
    })
    .detach();
}
