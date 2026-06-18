use gpui::{
    div, prelude::*, px, size, AnyView, App, AppContext, Bounds, Context, FocusHandle, Focusable,
    Global, IntoElement, ParentElement, Render, SharedString, Size, Styled, Window, WindowBounds,
    WindowKind, WindowOptions,
};
use gpui_component::{v_flex, Root};
use rust_i18n::t;

use crate::cyber_editor::build_editor_settings;
use crate::title_bar::{title_bar_bottom_rule, TitleBar, TITLE_BAR_HEIGHT};

const SETTINGS_WINDOW_WIDTH: f32 = 1120.0;
const SETTINGS_WINDOW_HEIGHT: f32 = 720.0;
const SETTINGS_WINDOW_MIN_WIDTH: f32 = 640.0;
const SETTINGS_WINDOW_MIN_HEIGHT: f32 = 480.0;

#[derive(Default)]
pub struct SettingsWindowState {
    handle: Option<gpui::AnyWindowHandle>,
}

impl Global for SettingsWindowState {}

impl SettingsWindowState {
    pub fn init(cx: &mut App) {
        if cx.try_global::<Self>().is_none() {
            cx.set_global(Self::default());
        }
    }

    pub fn open_editor(cx: &mut App) {
        Self::open_with_title_bar_height(cx, None);
    }

    pub fn open_media_player_settings(cx: &mut App) {
        Self::open_with_title_bar_height(cx, Some(px(35.)));
    }

    fn open_with_title_bar_height(cx: &mut App, title_bar_height: Option<gpui::Pixels>) {
        Self::init(cx);

        if let Some(handle) = cx.global::<Self>().handle {
            if handle
                .update(cx, |_, window, _| {
                    window.activate_window();
                })
                .is_ok()
            {
                return;
            }
            cx.global_mut::<Self>().handle = None;
        }

        open_settings_window(cx, title_bar_height);
    }
}

struct SettingsWindow {
    focus_handle: FocusHandle,
    title_bar_height: Option<gpui::Pixels>,
}

impl SettingsWindow {
    fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            title_bar_height: None,
        }
    }
}

impl Focusable for SettingsWindow {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for SettingsWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .id("settings-window")
            .size_full()
            .min_h_0()
            .track_focus(&self.focus_handle)
            .child(
                title_bar_bottom_rule(TitleBar::new().child(t!("nav.settings")), cx)
                    .h(self.title_bar_height.unwrap_or(TITLE_BAR_HEIGHT))
                    .flex_shrink_0(),
            )
            .child(
                div()
                    .id("settings-window-body")
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .child(build_editor_settings(cx)),
            )
    }
}

struct SettingsShell {
    view: AnyView,
}

impl SettingsShell {
    fn new(view: impl Into<AnyView>) -> Self {
        Self { view: view.into() }
    }
}

impl Render for SettingsShell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let sheet_layer = Root::render_sheet_layer(window, cx);
        let dialog_layer = Root::render_dialog_layer(window, cx);
        let notification_layer = Root::render_notification_layer(window, cx);

        div()
            .id("settings-shell")
            .size_full()
            .child(
                div()
                    .id("settings-shell-main")
                    .size_full()
                    .child(self.view.clone()),
            )
            .children(sheet_layer)
            .children(dialog_layer)
            .children(notification_layer)
    }
}

fn open_settings_window(cx: &mut App, title_bar_height: Option<gpui::Pixels>) {
    let mut window_size = size(px(SETTINGS_WINDOW_WIDTH), px(SETTINGS_WINDOW_HEIGHT));
    if let Some(display) = cx.primary_display() {
        let display_size = display.bounds().size;
        window_size.width = window_size.width.min(display_size.width * 0.9);
        window_size.height = window_size.height.min(display_size.height * 0.9);
    }
    let window_bounds = Bounds::centered(None, window_size, cx);
    let title = SharedString::from(t!("nav.settings"));

    cx.spawn(async move |cx| {
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(window_bounds)),
            titlebar: Some(TitleBar::title_bar_options()),
            window_min_size: Some(Size {
                width: px(SETTINGS_WINDOW_MIN_WIDTH),
                height: px(SETTINGS_WINDOW_MIN_HEIGHT),
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
                window.on_window_should_close(cx, |_, cx| {
                    cx.global_mut::<SettingsWindowState>().handle = None;
                    true
                });
                let view = cx.new(|cx| {
                    let mut w = SettingsWindow::new(cx);
                    if let Some(h) = title_bar_height {
                        w.title_bar_height = Some(h);
                    }
                    w
                });
                let shell = cx.new(|_| SettingsShell::new(view));
                cx.new(|cx| Root::new(shell, window, cx))
            })
            .expect("failed to open window");

        cx.update_global::<SettingsWindowState, _>(|state, _| {
            state.handle = Some(window.into());
        });

        window.update(cx, |_, window, _| {
            window.activate_window();
            window.set_window_title(&title);
        })?;

        Ok::<_, anyhow::Error>(())
    })
    .detach();
}
