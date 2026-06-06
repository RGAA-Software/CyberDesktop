use gpui::{prelude::*, *};
use gpui_component::{
    h_flex, label::Label, v_flex, ActiveTheme as _, ElementExt as _, InteractiveElementExt as _,
    StyledExt as _,
};

use crate::app_state::AppNavigation;
use crate::shell::navigation::NavigationTarget;
use crate::shell::pane_split::{
    ratio_from_pointer, secondary_too_narrow, PaneSplitDrag, PANE_MIN_SIZE,
    MULTI_PANE_WIDTH_THRESHOLD, SPLIT_HANDLE_SIZE, SPLIT_RATIO_MAX, SPLIT_RATIO_MIN,
};
use crate::shell::PaneShell;
use files_core::{load_config, SessionPaneLayout};
use files_fs::home_navigation_path;

const PANE_SHELL_RADIUS: Pixels = px(14.);
const PANE_TITLE_HEIGHT: Pixels = px(35.);
const SPLIT_BROWSER_PADDING: Pixels = px(10.);
const SPLIT_BROWSER_GAP: Pixels = px(10.);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneSide {
    Primary,
    Secondary,
}

/// Split direction between primary and secondary panes (Files `ShellPaneArrangement`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PaneArrangement {
    #[default]
    Vertical,
    Horizontal,
}

impl PaneArrangement {
    pub fn from_config(value: &str) -> Self {
        match value {
            "horizontal" => Self::Horizontal,
            _ => Self::Vertical,
        }
    }

    pub fn as_config_str(self) -> &'static str {
        match self {
            Self::Vertical => "vertical",
            Self::Horizontal => "horizontal",
        }
    }
}

pub struct ShellPanes {
    primary: Entity<PaneShell>,
    secondary: Entity<PaneShell>,
    dual_pane: bool,
    active: PaneSide,
    arrangement: PaneArrangement,
    split_ratio: f32,
    split_bounds: Bounds<Pixels>,
    /// Dual pane was hidden because the window is narrower than [`MULTI_PANE_WIDTH_THRESHOLD`].
    compact_suppressed_dual: bool,
    compact_secondary_tab: String,
}

impl ShellPanes {
    pub fn new(cx: &mut Context<Self>, target: NavigationTarget) -> Self {
        files_core::log_startup_step("shell_panes_new_begin");
        let secondary_path = match &target {
            NavigationTarget::Path(path) => path.clone(),
            _ => home_navigation_path(),
        };
        files_core::log_startup_step("shell_panes_primary_pane_begin");
        let primary = cx.new(|cx| PaneShell::new(cx, target));
        files_core::log_startup_step("shell_panes_secondary_pane_begin");
        let secondary = cx.new(|cx| PaneShell::new(cx, NavigationTarget::Path(secondary_path)));
        files_core::log_startup_step("shell_panes_panes_created");
        cx.observe(&primary, |this, _, cx| {
            this.primary_changed(cx);
        })
        .detach();
        cx.observe(&secondary, |this, _, cx| {
            this.secondary_changed(cx);
        })
        .detach();
        let arrangement = load_config()
            .map(|c| PaneArrangement::from_config(&c.shell_pane_arrangement))
            .unwrap_or_default();
        files_core::log_startup_step("shell_panes_new_done");
        Self {
            primary,
            secondary,
            dual_pane: false,
            active: PaneSide::Primary,
            arrangement,
            split_ratio: 0.5,
            split_bounds: Bounds::default(),
            compact_suppressed_dual: false,
            compact_secondary_tab: String::new(),
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

    pub fn arrangement(&self) -> PaneArrangement {
        self.arrangement
    }

    pub fn split_ratio(&self) -> f32 {
        self.split_ratio
    }

    pub fn primary(&self) -> Entity<PaneShell> {
        self.primary.clone()
    }

    pub fn set_arrangement(&mut self, arrangement: PaneArrangement, cx: &mut Context<Self>) {
        if self.arrangement != arrangement {
            self.arrangement = arrangement;
            cx.notify();
        }
    }

    pub fn split_pane(&mut self, arrangement: PaneArrangement, cx: &mut Context<Self>) {
        if self.dual_pane {
            return;
        }
        self.arrangement = arrangement;
        self.open_secondary_at_active(cx);
    }

    pub fn arrange_panes(&mut self, arrangement: PaneArrangement, cx: &mut Context<Self>) {
        if !self.dual_pane {
            return;
        }
        self.set_arrangement(arrangement, cx);
    }

    /// Toggle dual pane (Files `ToggleDualPaneAction`: off → open secondary at active location; on → close other).
    pub fn toggle_dual_pane(&mut self, cx: &mut Context<Self>) {
        if self.dual_pane {
            self.close_other_pane(cx);
        } else {
            self.open_secondary_at_active(cx);
        }
    }

    /// Show the secondary pane with the same navigation target as the active pane.
    pub fn open_secondary_at_active(&mut self, cx: &mut Context<Self>) {
        let target = self
            .active_pane()
            .read(cx)
            .current_navigation_target(cx);
        self.dual_pane = true;
        self.compact_suppressed_dual = false;
        self.active = PaneSide::Primary;
        self.secondary.update(cx, |pane, cx| {
            pane.navigate(target, cx);
        });
        cx.notify();
    }

    /// Hide the secondary pane (Files `CloseOtherPane`).
    pub fn close_other_pane(&mut self, cx: &mut Context<Self>) {
        if !self.dual_pane {
            return;
        }
        self.dual_pane = false;
        if self.active == PaneSide::Secondary {
            self.active = PaneSide::Primary;
        }
        cx.notify();
    }

    /// Close the active pane when dual; if only one pane remains, same as `close_other_pane`.
    pub fn close_active_pane(&mut self, cx: &mut Context<Self>) {
        if !self.dual_pane {
            return;
        }
        if self.active == PaneSide::Secondary {
            self.close_other_pane(cx);
        } else {
            let target = self.secondary.read(cx).current_navigation_target(cx);
            self.primary.update(cx, |pane, cx| {
                pane.navigate(target, cx);
            });
            self.close_other_pane(cx);
        }
    }

    pub fn focus_other_pane(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.dual_pane {
            return;
        }
        let other = match self.active {
            PaneSide::Primary => PaneSide::Secondary,
            PaneSide::Secondary => PaneSide::Primary,
        };
        self.activate_pane(other, window, cx);
    }

    pub fn adapt_viewport_width(
        &mut self,
        width: Pixels,
        encode_pane: impl Fn(&PaneShell, &App) -> String,
        cx: &mut Context<Self>,
    ) {
        let compact = width <= MULTI_PANE_WIDTH_THRESHOLD;
        if compact {
            if self.dual_pane {
                let secondary = self.secondary.read(cx);
                self.compact_secondary_tab = encode_pane(&secondary, cx);
                self.compact_suppressed_dual = true;
                self.close_other_pane(cx);
            }
            return;
        }
        if self.compact_suppressed_dual {
            self.compact_suppressed_dual = false;
            if !self.compact_secondary_tab.is_empty() {
                self.dual_pane = true;
                let tab = self.compact_secondary_tab.clone();
                self.secondary.update(cx, |pane, cx| {
                    pane.navigate(
                        NavigationTarget::decode_session_tab(tab.as_str()),
                        cx,
                    );
                });
                self.compact_secondary_tab.clear();
                cx.notify();
            }
        }
    }

    fn update_split_from_drag(&mut self, position: Point<Pixels>, cx: &mut Context<Self>) {
        let ratio = ratio_from_pointer(self.arrangement, self.split_bounds, position);
        if (self.split_ratio - ratio).abs() > f32::EPSILON {
            self.split_ratio = ratio;
            cx.notify();
        }
    }

    fn finish_split_drag(&mut self, cx: &mut Context<Self>) {
        if secondary_too_narrow(self.arrangement, self.split_bounds, self.split_ratio) {
            self.close_other_pane(cx);
        }
        if let Some(nav) = cx.try_global::<AppNavigation>() {
            nav.main_page().update(cx, |page, cx| page.persist_session(cx));
        }
    }

    fn reset_split_ratio(&mut self, cx: &mut Context<Self>) {
        self.split_ratio = 0.5;
        cx.notify();
        if let Some(nav) = cx.try_global::<AppNavigation>() {
            nav.main_page().update(cx, |page, cx| page.persist_session(cx));
        }
    }

    /// Restores dual-pane layout from a prior session.
    pub fn restore_layout(
        &mut self,
        layout: &SessionPaneLayout,
        decode_target: impl Fn(&str) -> NavigationTarget,
        cx: &mut Context<Self>,
    ) {
        self.arrangement = PaneArrangement::from_config(&layout.arrangement);
        self.split_ratio = layout.split_ratio.clamp(SPLIT_RATIO_MIN, SPLIT_RATIO_MAX);

        if !layout.primary_tab.is_empty() {
            let primary_target = decode_target(layout.primary_tab.as_str());
            self.primary.update(cx, |pane, cx| {
                pane.navigate(primary_target, cx);
            });
        }

        if !layout.dual_pane {
            return;
        }
        self.dual_pane = true;
        self.compact_suppressed_dual = false;
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

    fn clear_inactive_pane_selection(&self, active: PaneSide, cx: &mut Context<Self>) {
        let inactive = match active {
            PaneSide::Primary => self.secondary.clone(),
            PaneSide::Secondary => self.primary.clone(),
        };
        inactive.update(cx, |pane, cx| {
            pane.file_browser().update(cx, |browser, cx| {
                browser.clear_selection();
                cx.notify();
            });
        });
    }

    fn activate_pane(&mut self, side: PaneSide, window: &mut Window, cx: &mut Context<Self>) {
        let browser = match side {
            PaneSide::Primary => self.primary.read(cx).file_browser(),
            PaneSide::Secondary => self.secondary.read(cx).file_browser(),
        };
        window.focus(&browser.read(cx).focus_handle(cx), cx);
        if self.active != side {
            self.clear_inactive_pane_selection(side, cx);
            self.active = side;
            cx.notify();
        }
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

    fn render_split_handle(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let arrangement = self.arrangement;
        div()
            .id("shell-pane-splitter")
            .flex_none()
            .flex_shrink_0()
            .when(arrangement == PaneArrangement::Vertical, |this| {
                this.w(SPLIT_HANDLE_SIZE)
                    .h_full()
                    .cursor_col_resize()
            })
            .when(arrangement == PaneArrangement::Horizontal, |this| {
                this.h(SPLIT_HANDLE_SIZE)
                    .w_full()
                    .cursor_row_resize()
            })
            .bg(cx.theme().border)
            .rounded_full()
            .hover(|s| s.bg(cx.theme().drag_border))
            .on_double_click(cx.listener(|this, _, _, cx| {
                this.reset_split_ratio(cx);
            }))
            .on_drag(PaneSplitDrag, |_, _, _, cx| cx.new(|_| PaneSplitDrag))
            .on_drag_move::<PaneSplitDrag>(cx.listener(
                |this, event: &DragMoveEvent<PaneSplitDrag>, _, cx| {
                    this.update_split_from_drag(event.event.position, cx);
                },
            ))
    }

    /// Flex child for one pane: fixed share on the main axis, full cross-axis size.
    /// Uses `flex_shrink_0` so panes do not collapse (which emptied the file list).
    fn pane_split_leading(
        pane: impl IntoElement,
        share: f32,
        arrangement: PaneArrangement,
    ) -> impl IntoElement {
        let share = share.clamp(SPLIT_RATIO_MIN, SPLIT_RATIO_MAX);
        div()
            .flex_grow_0()
            .flex_shrink_0()
            .flex_basis(relative(share))
            .overflow_hidden()
            .when(arrangement == PaneArrangement::Vertical, |p| {
                p.h_full().min_h_0().min_w(PANE_MIN_SIZE).min_w_0()
            })
            .when(arrangement == PaneArrangement::Horizontal, |p| {
                p.w_full().min_w_0().min_h(PANE_MIN_SIZE).min_h_0()
            })
            .child(pane)
    }

    fn pane_split_trailing(pane: impl IntoElement, arrangement: PaneArrangement) -> impl IntoElement {
        div()
            .flex_1()
            .overflow_hidden()
            .when(arrangement == PaneArrangement::Vertical, |p| {
                p.h_full().min_h_0().min_w(PANE_MIN_SIZE).min_w_0()
            })
            .when(arrangement == PaneArrangement::Horizontal, |p| {
                p.w_full().min_w_0().min_h(PANE_MIN_SIZE).min_h_0()
            })
            .child(pane)
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
        let arrangement = self.arrangement;
        let primary = self.primary.clone();
        let secondary = self.secondary.clone();
        let primary_title = self.primary.read(cx).current_navigation_target(cx).tab_title();
        let secondary_title = self.secondary.read(cx).current_navigation_target(cx).tab_title();
        let primary_share = self.split_ratio;

        let pane_title = |title: SharedString, is_active: bool| {
            h_flex()
                .flex_none()
                .h(PANE_TITLE_HEIGHT)
                .px(px(12.))
                .items_center()
                .rounded_t(PANE_SHELL_RADIUS)
                .border_b_1()
                .border_color(if is_active {
                    cx.theme().primary
                } else {
                    cx.theme().border
                })
                .bg(if is_active {
                    cx.theme().accent
                } else {
                    cx.theme().background
                })
                .child(
                    Label::new(title)
                        .text_sm()
                        .when(is_active, |label| label.font_semibold())
                        .text_color(if is_active {
                            cx.theme().accent_foreground
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
                    .overflow_hidden()
                    .rounded(PANE_SHELL_RADIUS)
                    .border_1()
                    .border_color(if is_active {
                        cx.theme().primary
                    } else {
                        cx.theme().border
                    })
                    .bg(cx.theme().background)
                    // Capture phase: activate before FileBrowser/list stop_propagation.
                    .capture_any_mouse_down(cx.listener(move |this, _, window, cx| {
                        this.activate_pane(side, window, cx);
                    }))
                    .child(pane_title(title, is_active))
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .overflow_hidden()
                            .child(pane),
                    )
            };

        let primary_body = pane_wrapper(
            primary,
            primary_title,
            PaneSide::Primary,
            active == PaneSide::Primary,
        );
        let secondary_body = pane_wrapper(
            secondary,
            secondary_title,
            PaneSide::Secondary,
            active == PaneSide::Secondary,
        );
        let primary_pane = Self::pane_split_leading(primary_body, primary_share, arrangement);
        let secondary_pane = Self::pane_split_trailing(secondary_body, arrangement);

        let splitter = self.render_split_handle(cx);
        let weak = cx.weak_entity();
        let weak_h = weak.clone();
        let on_drop = cx.listener(|this, _: &PaneSplitDrag, _, cx| {
            this.finish_split_drag(cx);
        });

        let split_shell = |container: Div| {
            container
                .p(SPLIT_BROWSER_PADDING)
                .pb_0()
                .gap(SPLIT_BROWSER_GAP)
                .bg(cx.theme().background)
        };

        match arrangement {
            PaneArrangement::Vertical => split_shell(h_flex())
                .id("shell-panes")
                .size_full()
                .min_h_0()
                .on_prepaint(move |bounds, _, cx| {
                    let _ = weak.update(cx, |this, _| {
                        this.split_bounds = bounds;
                    });
                })
                .on_drop(on_drop)
                .child(primary_pane)
                .child(splitter)
                .child(secondary_pane)
                .into_any_element(),
            PaneArrangement::Horizontal => split_shell(v_flex())
                .id("shell-panes")
                .size_full()
                .min_h_0()
                .on_prepaint(move |bounds, _, cx| {
                    let _ = weak_h.update(cx, |this, _| {
                        this.split_bounds = bounds;
                    });
                })
                .on_drop(cx.listener(|this, _: &PaneSplitDrag, _, cx| {
                    this.finish_split_drag(cx);
                }))
                .child(primary_pane)
                .child(splitter)
                .child(secondary_pane)
                .into_any_element(),
        }
    }
}
