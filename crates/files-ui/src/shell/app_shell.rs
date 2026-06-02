use std::time::Duration;

use gpui::{
    div, prelude::FluentBuilder, AnyView, App, AppContext, Context, FocusHandle, Focusable,
    InteractiveElement, IntoElement, MouseButton, ParentElement, Render, Styled, Window,
};
use gpui_component::{v_flex, Root};

use super::preferences::{persist_window_bounds, window_size_for_persist};
use crate::app_state::AppNavigation;
use crate::drag::DraggedFilePaths;
use app_platform_windows as platform;

const WINDOW_BOUNDS_DEBOUNCE_MS: u64 = 400;

/// Window chrome: main content (integrated title bar + tabs) + overlay layers.
pub struct AppShell {
    focus_handle: FocusHandle,
    view: AnyView,
    last_seen_window_size: Option<(f32, f32)>,
    bounds_persist_generation: u64,
}

impl AppShell {
    pub fn new(view: impl Into<AnyView>, _window: &mut Window, cx: &mut Context<Self>) -> Self {
        Self {
            focus_handle: cx.focus_handle(),
            view: view.into(),
            last_seen_window_size: None,
            bounds_persist_generation: 0,
        }
    }

    fn schedule_bounds_persist_if_changed(&mut self, cx: &mut Context<Self>) {
        let Some(size) = window_size_for_persist(cx) else {
            return;
        };
        if self.last_seen_window_size == Some(size) {
            return;
        }
        self.last_seen_window_size = Some(size);
        self.bounds_persist_generation = self.bounds_persist_generation.wrapping_add(1);
        let generation = self.bounds_persist_generation;
        cx.spawn(async move |shell, cx| {
            cx.background_spawn(async move {
                std::thread::sleep(Duration::from_millis(WINDOW_BOUNDS_DEBOUNCE_MS));
            })
            .await;
            let _ = shell.update(cx, |shell, cx| {
                if shell.bounds_persist_generation != generation {
                    return;
                }
                persist_window_bounds(cx);
            });
        })
        .detach();
    }
}

impl Focusable for AppShell {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for AppShell {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if platform::take_native_drag_leave_pending() {
            if let Some(nav) = cx.try_global::<AppNavigation>() {
                nav.main_page().update(cx, |page, cx| {
                    page.maybe_begin_native_drag_out(window.mouse_position(), window, cx);
                });
            }
        }
        self.schedule_bounds_persist_if_changed(cx);
        let sheet_layer = Root::render_sheet_layer(window, cx);
        let dialog_layer = Root::render_dialog_layer(window, cx);
        let notification_layer = Root::render_notification_layer(window, cx);
        let path_edit_active = cx
            .try_global::<AppNavigation>()
            .is_some_and(|nav| nav.main_page().read(cx).omnibar_path_edit_active());

        div()
            .id("app-shell")
            .size_full()
            .on_drag_move::<DraggedFilePaths>(cx.listener(
                |_, event: &gpui::DragMoveEvent<DraggedFilePaths>, window, cx| {
                    if let Some(nav) = cx.try_global::<AppNavigation>() {
                        nav.main_page().update(cx, |page, cx| {
                            page.maybe_begin_native_drag_out(event.event.position, window, cx);
                        });
                    }
                },
            ))
            .when(path_edit_active, |shell| {
                shell.on_mouse_down(
                    MouseButton::Left,
                    cx.listener(|_, _, _, cx| {
                        if let Some(nav) = cx.try_global::<AppNavigation>() {
                            nav.main_page().update(cx, |page, cx| {
                                page.dismiss_omnibar_path_edit(cx);
                            });
                        }
                    }),
                )
            })
            .child(
                v_flex()
                    .size_full()
                    .child(
                        div()
                            .id("app-shell-main")
                            .flex_1()
                            .min_h_0()
                            .min_w_0()
                            .w_full()
                            .overflow_hidden()
                            .child(self.view.clone()),
                    )
                    .children(sheet_layer)
                    .children(dialog_layer)
                    .children(notification_layer),
            )
    }
}
