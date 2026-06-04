use gpui::{prelude::*, *};
use gpui_component::{h_flex, label::Label, v_flex, ActiveTheme as _};

use crate::shell::navigation::NavigationTarget;
use crate::shell::PaneShell;
use files_core::SessionPaneLayout;
use files_fs::home_navigation_path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneSide {
    Primary,
    Secondary,
}

pub struct ShellPanes {
    primary: Entity<PaneShell>,
    secondary: Entity<PaneShell>,
    dual_pane: bool,
    active: PaneSide,
}

impl ShellPanes {
    pub fn new(cx: &mut Context<Self>, target: NavigationTarget) -> Self {
        let secondary_path = match &target {
            NavigationTarget::Path(path) => path.clone(),
            _ => home_navigation_path(),
        };
        let primary = cx.new(|cx| PaneShell::new(cx, target));
        let secondary = cx.new(|cx| PaneShell::new(cx, NavigationTarget::Path(secondary_path)));
        cx.observe(&primary, |this, _, cx| {
            this.primary_changed(cx);
        })
        .detach();
        cx.observe(&secondary, |this, _, cx| {
            this.secondary_changed(cx);
        })
        .detach();
        Self {
            primary,
            secondary,
            dual_pane: false,
            active: PaneSide::Primary,
        }
    }

    fn primary_changed(&mut self, cx: &mut Context<Self>) {
        cx.notify();
    }

    fn secondary_changed(&mut self, cx: &mut Context<Self>) {
        cx.notify();
    }

    pub fn dual_pane(&self) -> bool {
        self.dual_pane
    }

    pub fn toggle_dual_pane(&mut self, cx: &mut Context<Self>) {
        self.dual_pane = !self.dual_pane;
        if self.dual_pane {
            self.active = PaneSide::Primary;
        }
        cx.notify();
    }

    /// Restores dual-pane layout from a prior session.
    pub fn restore_layout(
        &mut self,
        layout: &SessionPaneLayout,
        decode_target: impl Fn(&str) -> NavigationTarget,
        cx: &mut Context<Self>,
    ) {
        if !layout.dual_pane {
            return;
        }
        self.dual_pane = true;
        let secondary_target = decode_target(if layout.secondary_tab.is_empty() {
            "home"
        } else {
            layout.secondary_tab.as_str()
        });
        self.secondary.update(cx, |pane, cx| {
            pane.navigate(secondary_target, cx);
        });
        self.active = if layout.active_side == "secondary" {
            PaneSide::Secondary
        } else {
            PaneSide::Primary
        };
        cx.notify();
    }

    pub fn active_side(&self) -> PaneSide {
        self.active
    }

    pub fn set_active(&mut self, side: PaneSide, cx: &mut Context<Self>) {
        if self.active != side {
            self.active = side;
            cx.notify();
        }
    }

    fn activate_pane(&mut self, side: PaneSide, window: &mut Window, cx: &mut Context<Self>) {
        let browser = match side {
            PaneSide::Primary => self.primary.read(cx).file_browser(),
            PaneSide::Secondary => self.secondary.read(cx).file_browser(),
        };
        window.focus(&browser.read(cx).focus_handle(cx), cx);
        self.set_active(side, cx);
    }

    pub fn active_pane(&self) -> Entity<PaneShell> {
        match (self.dual_pane, self.active) {
            (true, PaneSide::Secondary) => self.secondary.clone(),
            _ => self.primary.clone(),
        }
    }

    pub fn secondary(&self) -> Entity<PaneShell> {
        self.secondary.clone()
    }

    pub fn for_each_pane<F>(&self, mut visit: F)
    where
        F: FnMut(Entity<PaneShell>),
    {
        visit(self.primary.clone());
        if self.dual_pane {
            visit(self.secondary.clone());
        }
    }

    pub fn navigate_active(&mut self, target: NavigationTarget, cx: &mut Context<Self>) {
        self.active_pane().update(cx, |shell, cx| {
            shell.navigate(target, cx);
        });
        cx.notify();
    }
}

impl Render for ShellPanes {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if !self.dual_pane {
            return div()
                .id("shell-pane-single")
                .size_full()
                .min_h_0()
                .child(self.active_pane())
                .into_any_element();
        }

        let active = self.active;
        let primary = self.primary.clone();
        let secondary = self.secondary.clone();
        let primary_title = self.primary.read(cx).current_navigation_target(cx).tab_title();
        let secondary_title = self.secondary.read(cx).current_navigation_target(cx).tab_title();

        let pane_title = |title: SharedString, is_active: bool| {
            h_flex()
                .h_8()
                .px_3()
                .items_center()
                .bg(if is_active {
                    cx.theme().primary
                } else {
                    cx.theme().background
                })
                .child(
                    Label::new(title).text_color(if is_active {
                        cx.theme().primary_foreground
                    } else {
                        cx.theme().foreground
                    }),
                )
        };

        let pane_wrapper =
            |pane: Entity<PaneShell>,
             title: SharedString,
             side: PaneSide,
             is_active: bool| {
                v_flex()
                    .size_full()
                    .min_h_0()
                    .child(pane_title(title, is_active))
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .border_2()
                            .border_color(if is_active {
                                cx.theme().primary
                            } else {
                                cx.theme().border
                            })
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, _, window, cx| {
                                    this.activate_pane(side, window, cx);
                                }),
                            )
                            .child(pane),
                    )
            };

        // Equal flex split — avoid nested gpui_component::h_resizable here; a third
        // resizable group inside main-layout + main-with-info-pane caused stack overflow
        // when toggling dual pane (synchronous notify/prepaint loops).
        h_flex()
            .id("shell-panes")
            .size_full()
            .min_h_0()
            .child(
                pane_wrapper(
                    primary,
                    primary_title,
                    PaneSide::Primary,
                    active == PaneSide::Primary,
                )
                .flex_1()
                .min_w_0(),
            )
            .child(
                pane_wrapper(
                    secondary,
                    secondary_title,
                    PaneSide::Secondary,
                    active == PaneSide::Secondary,
                )
                .flex_1()
                .min_w_0(),
            )
            .into_any_element()
    }
}
