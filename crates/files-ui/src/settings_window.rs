use gpui::{
    anchored, deferred, div, prelude::*, px, size, Anchor, AnyView, App, AppContext, Bounds,
    Context, DismissEvent, Entity, FocusHandle, Focusable, Global, IntoElement, MouseButton,
    MouseDownEvent, ParentElement, Point, Render, SharedString, Size, Styled, Subscription, Window,
    WindowBounds, WindowKind, WindowOptions,
};
use gpui_component::{v_flex, Root};
use rust_i18n::t;

use app_ui::popup_menu::PopupMenu;
use app_ui::title_bar::{title_bar_bottom_rule, TitleBar, TITLE_BAR_HEIGHT};

use crate::settings_view::build_settings;
use crate::shell::{append_dual_pane_popup_menu, dual_pane_menu_state, DualPanePopupProfile};

const SETTINGS_WINDOW_WIDTH: f32 = 1120.0;
const SETTINGS_WINDOW_HEIGHT: f32 = 720.0;
const SETTINGS_WINDOW_MIN_WIDTH: f32 = 640.0;
const SETTINGS_WINDOW_MIN_HEIGHT: f32 = 480.0;

#[derive(Default)]
pub struct FilesSettingsWindowState {
    handle: Option<gpui::AnyWindowHandle>,
}

impl Global for FilesSettingsWindowState {}

impl FilesSettingsWindowState {
    pub fn init(cx: &mut App) {
        if cx.try_global::<Self>().is_none() {
            cx.set_global(Self::default());
        }
    }

    pub fn open_files(cx: &mut App) {
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

        open_settings_window(cx);
    }
}

struct SettingsPopupMenuState {
    position: Point<gpui::Pixels>,
    menu: Entity<PopupMenu>,
    _subscription: Subscription,
}

struct FilesSettingsWindow {
    focus_handle: FocusHandle,
    popup_menu: Option<SettingsPopupMenuState>,
}

impl FilesSettingsWindow {
    fn new(cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            popup_menu: None,
        }
    }

    fn close_popup_menu(&mut self) {
        self.popup_menu = None;
    }

    fn open_dual_pane_context_menu(
        &mut self,
        position: Point<gpui::Pixels>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_popup_menu();
        let state = dual_pane_menu_state(cx);
        if !state.multi_pane_available && !state.dual {
            return;
        }

        let view = cx.entity();
        let menu = PopupMenu::build(window, cx, move |menu, window, cx| {
            append_dual_pane_popup_menu(menu, window, cx, state, DualPanePopupProfile::PageSurface)
        });

        let subscription = window.subscribe(&menu, cx, {
            let view = view.clone();
            move |_, _: &DismissEvent, window, cx| {
                let _ = view.update(cx, |view, cx| {
                    view.close_popup_menu();
                    cx.notify();
                });
                window.refresh();
            }
        });

        self.popup_menu = Some(SettingsPopupMenuState {
            position,
            menu,
            _subscription: subscription,
        });
        cx.notify();
    }
}

impl Focusable for FilesSettingsWindow {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for FilesSettingsWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let menu_overlay = self.popup_menu.as_ref().map(|state| {
            let position = state.position;
            let menu = state.menu.clone();
            deferred(
                anchored()
                    .position(position)
                    .anchor(Anchor::TopLeft)
                    .snap_to_window_with_margin(px(8.))
                    .child(menu),
            )
            .with_priority(1)
        });

        v_flex()
            .id("files-settings-window")
            .relative()
            .size_full()
            .min_h_0()
            .track_focus(&self.focus_handle)
            .when_some(menu_overlay, |page, overlay| page.child(overlay))
            .child(
                title_bar_bottom_rule(TitleBar::new().child(t!("nav.settings")), cx)
                    .h(TITLE_BAR_HEIGHT)
                    .flex_shrink_0(),
            )
            .child(
                div()
                    .id("files-settings-window-body")
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(|view, event: &MouseDownEvent, window, cx| {
                            if event.button != MouseButton::Right {
                                return;
                            }
                            view.open_dual_pane_context_menu(event.position, window, cx);
                            cx.stop_propagation();
                        }),
                    )
                    .child(build_settings(cx)),
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

fn open_settings_window(cx: &mut App) {
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
                    cx.global_mut::<FilesSettingsWindowState>().handle = None;
                    true
                });
                let view = cx.new(|cx| FilesSettingsWindow::new(cx));
                let shell = cx.new(|_| SettingsShell::new(view));
                cx.new(|cx| Root::new(shell, window, cx))
            })
            .expect("failed to open settings window");

        cx.update_global::<FilesSettingsWindowState, _>(|state, _| {
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
