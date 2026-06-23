use std::rc::Rc;

use gpui::prelude::FluentBuilder as _;
use gpui::{
    div, px, rgb, AnyElement, App, ClickEvent, Context, Decorations, Hsla, InteractiveElement,
    IntoElement, MouseButton, ParentElement, Pixels, Render, RenderOnce,
    StatefulInteractiveElement as _, StyleRefinement, Styled, TitlebarOptions, Window,
    WindowControlArea,
};
use gpui_component::{
    h_flex, ActiveTheme, Icon, IconName, InteractiveElementExt as _, Sizable as _, StyledExt,
};
use smallvec::SmallVec;

pub const TITLE_BAR_HEIGHT: Pixels = px(46.);
#[cfg(target_os = "macos")]
const TITLE_BAR_LEFT_PADDING: Pixels = px(80.);
#[cfg(not(target_os = "macos"))]
const TITLE_BAR_LEFT_PADDING: Pixels = px(13.);

/// 1px bottom rule shared by [`TitleBar`] and [`crate::tab::TabBar::bottom_border`].
pub fn title_bar_bottom_rule<T: Styled>(this: T, cx: &App) -> T {
    this.border_b_1().border_color(cx.theme().title_bar_border)
}

/// Vertical rule between adjacent unselected tabs (see [`crate::tab::TabBar::inactive_separators`]).
pub const TAB_INACTIVE_SEPARATOR_HEIGHT: Pixels = px(12.);
pub const TAB_INACTIVE_SEPARATOR_WIDTH: Pixels = px(2.);

pub fn tab_inactive_separator(cx: &App, visible: bool) -> gpui::Div {
    div()
        .flex_shrink_0()
        .flex()
        .items_center()
        .h_full()
        .w(TAB_INACTIVE_SEPARATOR_WIDTH)
        .child(
            div()
                .w(TAB_INACTIVE_SEPARATOR_WIDTH)
                .h(TAB_INACTIVE_SEPARATOR_HEIGHT)
                .bg(if visible {
                    cx.theme().title_bar_border
                } else {
                    cx.theme().transparent
                }),
        )
}

#[derive(IntoElement)]
pub struct TitleBar {
    style: StyleRefinement,
    children: SmallVec<[AnyElement; 1]>,
    trailing_before_controls: SmallVec<[AnyElement; 2]>,
    on_close_window: Option<Rc<Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>>>,
    design_window_controls: bool,
}

impl TitleBar {
    pub fn new() -> Self {
        Self {
            style: StyleRefinement::default(),
            children: SmallVec::new(),
            trailing_before_controls: SmallVec::new(),
            on_close_window: None,
            design_window_controls: false,
        }
    }

    /// Use bordered custom minimize / maximize / close buttons (CyberMonitor design).
    pub fn design_window_controls(mut self, enabled: bool) -> Self {
        self.design_window_controls = enabled;
        self
    }

    /// Icon buttons placed immediately left of the window controls (minimize / maximize / close).
    pub fn trailing_before_controls(mut self, element: impl IntoElement) -> Self {
        self.trailing_before_controls
            .push(element.into_any_element());
        self
    }

    pub fn title_bar_options() -> TitlebarOptions {
        TitlebarOptions {
            title: None,
            appears_transparent: true,
            traffic_light_position: Some(gpui::point(px(9.0), px(9.0))),
        }
    }

    #[allow(dead_code)]
    pub fn on_close_window(
        mut self,
        f: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    ) -> Self {
        if cfg!(target_os = "linux") {
            self.on_close_window = Some(Rc::new(Box::new(f)));
        }
        self
    }
}

#[derive(Clone)]
enum ControlIcon {
    Minimize,
    Restore,
    Maximize,
    Close {
        on_close_window: Option<Rc<Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>>>,
    },
}

impl ControlIcon {
    fn minimize() -> Self {
        Self::Minimize
    }

    fn restore() -> Self {
        Self::Restore
    }

    fn maximize() -> Self {
        Self::Maximize
    }

    fn close(on_close_window: Option<Rc<Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>>>) -> Self {
        Self::Close { on_close_window }
    }

    fn id(&self) -> &'static str {
        match self {
            Self::Minimize => "minimize",
            Self::Restore => "restore",
            Self::Maximize => "maximize",
            Self::Close { .. } => "close",
        }
    }

    fn icon(&self) -> IconName {
        match self {
            Self::Minimize => IconName::WindowMinimize,
            Self::Restore => IconName::WindowRestore,
            Self::Maximize => IconName::WindowMaximize,
            Self::Close { .. } => IconName::WindowClose,
        }
    }

    fn window_control_area(&self) -> WindowControlArea {
        match self {
            Self::Minimize => WindowControlArea::Min,
            Self::Restore | Self::Maximize => WindowControlArea::Max,
            Self::Close { .. } => WindowControlArea::Close,
        }
    }

    fn is_close(&self) -> bool {
        matches!(self, Self::Close { .. })
    }

    fn hover_fg(&self, cx: &App) -> Hsla {
        if self.is_close() {
            cx.theme().danger_foreground
        } else {
            cx.theme().secondary_foreground
        }
    }

    fn hover_bg(&self, cx: &App) -> Hsla {
        if self.is_close() {
            cx.theme().danger
        } else {
            cx.theme().secondary_hover
        }
    }

    fn active_bg(&self, cx: &mut App) -> Hsla {
        if self.is_close() {
            cx.theme().danger_active
        } else {
            cx.theme().secondary_active
        }
    }
}

fn design_border_strong(cx: &App) -> Hsla {
    if cx.theme().mode.is_dark() {
        rgb(0x33405c).into()
    } else {
        rgb(0xc9d2e4).into()
    }
}

fn design_topbar_hover_bg(cx: &App) -> Hsla {
    if cx.theme().mode.is_dark() {
        rgb(0x171d2c).into()
    } else {
        rgb(0xeef3fb).into()
    }
}

const DESIGN_WIN_BTN: Pixels = px(38.);

#[derive(IntoElement)]
struct ControlIconRender {
    icon: ControlIcon,
    design: bool,
}

impl RenderOnce for ControlIconRender {
    fn render(self, _: &mut Window, cx: &mut App) -> impl IntoElement {
        let is_linux = cfg!(target_os = "linux");
        let is_windows = cfg!(target_os = "windows");
        let use_native_windows = is_windows && !self.design;
        let normal_fg = cx.theme().muted_foreground;
        let hover_fg = self.icon.hover_fg(cx);
        let hover_bg = self.icon.hover_bg(cx);
        let active_bg = self.icon.active_bg(cx);
        let icon = self.icon.clone();
        let on_close_window = match &self.icon {
            ControlIcon::Close { on_close_window } => on_close_window.clone(),
            _ => None,
        };
        let is_close = self.icon.is_close();

        let click_handler = move |_: &ClickEvent, window: &mut Window, cx: &mut App| {
            cx.stop_propagation();
            match icon {
                ControlIcon::Minimize => window.minimize_window(),
                ControlIcon::Restore | ControlIcon::Maximize => window.zoom_window(),
                ControlIcon::Close { .. } => {
                    if let Some(f) = on_close_window.clone() {
                        f(&ClickEvent::default(), window, cx);
                    } else {
                        window.remove_window();
                    }
                }
            }
        };

        if self.design {
            let strong_border = design_border_strong(cx);
            let topbar_hover = design_topbar_hover_bg(cx);
            return div()
                .id(self.icon.id())
                .w(DESIGN_WIN_BTN)
                .h(DESIGN_WIN_BTN)
                .flex()
                .flex_shrink_0()
                .items_center()
                .justify_center()
                .rounded(px(7.))
                .border_1()
                .border_color(cx.theme().border)
                .bg(cx.theme().secondary)
                .text_color(cx.theme().foreground)
                .cursor_pointer()
                .when(is_close, |this| {
                    this.hover(|style| {
                        style
                            .bg(cx.theme().danger)
                            .border_color(cx.theme().danger)
                            .text_color(cx.theme().danger_foreground)
                    })
                })
                .when(!is_close, |this| {
                    this.hover(|style| {
                        style
                            .bg(topbar_hover)
                            .border_color(strong_border)
                            .text_color(cx.theme().primary)
                    })
                })
                .when(is_windows, |this| {
                    this.window_control_area(self.icon.window_control_area())
                })
                .when(!is_windows, |this| {
                    this.on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .on_click(click_handler)
                })
                .child(Icon::new(self.icon.icon()).small());
        }

        div()
            .id(self.icon.id())
            .flex()
            .w(TITLE_BAR_HEIGHT)
            .h_full()
            .flex_shrink_0()
            .justify_center()
            .content_center()
            .items_center()
            .text_color(normal_fg)
            .hover(|style| style.bg(hover_bg).text_color(hover_fg))
            .active(|style| style.bg(active_bg).text_color(hover_fg))
            .when(use_native_windows, |this| {
                this.window_control_area(self.icon.window_control_area())
            })
            .when(is_linux, |this| {
                this.on_mouse_down(MouseButton::Left, move |_, window, cx| {
                    window.prevent_default();
                    cx.stop_propagation();
                })
                .on_click(click_handler)
            })
            .child(Icon::new(self.icon.icon()).small())
    }
}

#[derive(IntoElement)]
struct WindowControls {
    on_close_window: Option<Rc<Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>>>,
    design: bool,
}

impl RenderOnce for WindowControls {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        if cfg!(target_os = "macos") || cfg!(target_family = "wasm") {
            return div().id("window-controls");
        }

        let maximize = if window.is_maximized() {
            ControlIcon::restore()
        } else {
            ControlIcon::maximize()
        };

        if self.design {
            return h_flex()
                .id("window-controls")
                .items_center()
                .flex_shrink_0()
                .ml(px(6.))
                .pr(px(8.))
                .child(
                    div()
                        .w(px(1.))
                        .h(DESIGN_WIN_BTN)
                        .flex_none()
                        .bg(cx.theme().border),
                )
                .child(
                    h_flex()
                        .items_center()
                        .gap(px(8.))
                        .pl(px(6.))
                        .child(ControlIconRender {
                            icon: ControlIcon::minimize(),
                            design: true,
                        })
                        .child(ControlIconRender {
                            icon: maximize,
                            design: true,
                        })
                        .child(ControlIconRender {
                            icon: ControlIcon::close(self.on_close_window),
                            design: true,
                        }),
                );
        }

        h_flex()
            .id("window-controls")
            .items_center()
            .flex_shrink_0()
            .h_full()
            .child(ControlIconRender {
                icon: ControlIcon::minimize(),
                design: false,
            })
            .child(ControlIconRender {
                icon: maximize,
                design: false,
            })
            .child(ControlIconRender {
                icon: ControlIcon::close(self.on_close_window),
                design: false,
            })
    }
}

impl Styled for TitleBar {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl ParentElement for TitleBar {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.children.extend(elements);
    }
}

struct TitleBarState {
    should_move: bool,
}

impl Render for TitleBarState {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}

impl RenderOnce for TitleBar {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let is_client_decorated = matches!(window.window_decorations(), Decorations::Client { .. });
        let is_web = cfg!(target_family = "wasm");
        let is_linux = cfg!(target_os = "linux");
        let is_macos = cfg!(target_os = "macos");

        let state = window.use_state(cx, |_, _| TitleBarState { should_move: false });

        div().flex_shrink_0().child(
            div()
                .id("title-bar")
                .flex()
                .flex_row()
                .items_center()
                .h(TITLE_BAR_HEIGHT)
                .pl(TITLE_BAR_LEFT_PADDING)
                .map(|this| title_bar_bottom_rule(this, cx))
                .bg(cx.theme().title_bar)
                .refine_style(&self.style)
                .when(is_linux, |this| {
                    this.on_double_click(|_, window, _| window.zoom_window())
                })
                .when(is_macos, |this| {
                    this.on_double_click(|_, window, _| window.titlebar_double_click())
                })
                .on_mouse_down_out(window.listener_for(&state, |state, _, _, _| {
                    state.should_move = false;
                }))
                .on_mouse_down(
                    MouseButton::Left,
                    window.listener_for(&state, |state, _, _, _| {
                        state.should_move = true;
                    }),
                )
                .on_mouse_up(
                    MouseButton::Left,
                    window.listener_for(&state, |state, _, _, _| {
                        state.should_move = false;
                    }),
                )
                .on_mouse_move(window.listener_for(&state, |state, _, window, _| {
                    if state.should_move {
                        state.should_move = false;
                        window.start_window_move();
                    }
                }))
                .child(
                    h_flex()
                        .id("bar")
                        .h_full()
                        .flex_1()
                        .min_w_0()
                        .items_center()
                        .when(!is_web, |this| {
                            this.window_control_area(WindowControlArea::Drag)
                                .when(window.is_fullscreen(), |this| this.pl_3())
                                .when(is_linux && is_client_decorated, |this| {
                                    this.child(
                                        div()
                                            .top_0()
                                            .left_0()
                                            .absolute()
                                            .size_full()
                                            .h_full()
                                            .on_mouse_down(
                                                MouseButton::Right,
                                                move |ev, window, _| {
                                                    window.show_window_menu(ev.position)
                                                },
                                            ),
                                    )
                                })
                        })
                        .children(self.children),
                )
                .child(
                    h_flex()
                        .id("title-bar-trailing")
                        .items_center()
                        .flex_shrink_0()
                        .h_full()
                        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                        .children(self.trailing_before_controls),
                )
                .child(WindowControls {
                    on_close_window: self.on_close_window,
                    design: self.design_window_controls,
                }),
        )
    }
}
