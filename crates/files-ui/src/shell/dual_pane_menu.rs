//! Shared dual-pane popup / native menu builders (Files `ShellPanesPage` / `TabBar` parity).

use files_commands::{
    ArrangePanesHorizontally, ArrangePanesVertically, CloseActivePane, FocusOtherPane,
    SplitPaneHorizontally, SplitPaneVertically, ToggleDualPane,
};
use std::borrow::BorrowMut;

use gpui::AppContext;
use gpui_component::Icon;

use app_ui::popup_menu::{PopupMenu, PopupMenuItem};
use rust_i18n::t;

use crate::app_state::{AppNavigation, MainWindowState};
use crate::shell::pane_split::MULTI_PANE_WIDTH_THRESHOLD;
use crate::shell::PaneArrangement;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DualPaneMenuState {
    pub multi_pane_available: bool,
    pub dual: bool,
    pub arrangement: PaneArrangement,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DualPanePopupProfile {
    /// Title-bar tab strip: split, toggle, arrange, focus, close.
    TabBar,
    /// Home / Settings blank surface: split submenu + close pane (Files `HomePage` / `SettingsPage`).
    PageSurface,
}

pub fn multi_pane_available(cx: &mut (impl AppContext + BorrowMut<gpui::App>)) -> bool {
    let Some((width, _)) = MainWindowState::window_size(cx.borrow_mut()) else {
        return true;
    };
    width > MULTI_PANE_WIDTH_THRESHOLD.as_f32()
}

pub fn dual_pane_menu_state(cx: &mut (impl AppContext + BorrowMut<gpui::App>)) -> DualPaneMenuState {
    let multi_pane_available = multi_pane_available(cx);
    let app = cx.borrow_mut();
    let (dual, arrangement) = app
        .try_global::<AppNavigation>()
        .map(|nav| {
            let page = nav.main_page().read(app);
            (
                page.dual_pane_active(app),
                page.active_shell().read(app).arrangement(),
            )
        })
        .unwrap_or((false, PaneArrangement::Vertical));
    DualPaneMenuState {
        multi_pane_available,
        dual,
        arrangement,
    }
}

fn split_pane_icon() -> Icon {
    crate::tabler_icons::icon(crate::tabler_icons::LAYOUT_COLUMNS)
}

pub fn append_dual_pane_popup_menu(
    menu: PopupMenu,
    window: &mut gpui::Window,
    cx: &mut gpui::Context<PopupMenu>,
    state: DualPaneMenuState,
    profile: DualPanePopupProfile,
) -> PopupMenu {
    let DualPaneMenuState {
        multi_pane_available,
        dual,
        arrangement,
    } = state;

    let show_split = multi_pane_available && !dual;
    let show_arrange = dual;
    let show_close = dual;

    match profile {
        DualPanePopupProfile::TabBar => {
            let mut menu = menu;
            if show_split {
                let split_icon = split_pane_icon();
                menu = menu.submenu_with_icon(
                    Some(split_icon.clone()),
                    t!("menu.split_pane"),
                    window,
                    cx,
                    move |sub, _window, _cx| {
                        sub.item(
                            PopupMenuItem::new(t!("menu.split_pane_vertical"))
                                .action(Box::new(SplitPaneVertically)),
                        )
                        .item(
                            PopupMenuItem::new(t!("menu.split_pane_horizontal"))
                                .action(Box::new(SplitPaneHorizontally)),
                        )
                    },
                );
                menu = menu.item(
                    PopupMenuItem::new(t!("nav.split_pane"))
                        .icon(split_icon)
                        .action(Box::new(ToggleDualPane)),
                );
            }
            if show_arrange {
                menu = menu.submenu_with_icon(
                    Some(split_pane_icon()),
                    t!("menu.arrange_panes"),
                    window,
                    cx,
                    move |sub, _window, _cx| {
                        sub.item(
                            PopupMenuItem::new(t!("menu.split_pane_vertical"))
                                .checked(arrangement == PaneArrangement::Vertical)
                                .action(Box::new(ArrangePanesVertically)),
                        )
                        .item(
                            PopupMenuItem::new(t!("menu.split_pane_horizontal"))
                                .checked(arrangement == PaneArrangement::Horizontal)
                                .action(Box::new(ArrangePanesHorizontally)),
                        )
                    },
                );
            }
            menu.item(
                PopupMenuItem::new(t!("nav.focus_other_pane"))
                    .disabled(!dual)
                    .action(Box::new(FocusOtherPane)),
            )
            .item(
                PopupMenuItem::new(t!("nav.close_pane"))
                    .disabled(!show_close)
                    .action(Box::new(CloseActivePane)),
            )
        }
        DualPanePopupProfile::PageSurface => {
            let mut menu = menu;
            if show_split {
                menu = menu.submenu_with_icon(
                    Some(split_pane_icon()),
                    t!("menu.split_pane"),
                    window,
                    cx,
                    move |sub, _window, _cx| {
                        sub.item(
                            PopupMenuItem::new(t!("menu.split_pane_vertical")).on_click(
                                |_, _, cx| {
                                    AppNavigation::run_split_pane_vertically(cx);
                                    cx.stop_propagation();
                                },
                            ),
                        )
                        .item(
                            PopupMenuItem::new(t!("menu.split_pane_horizontal")).on_click(
                                |_, _, cx| {
                                    AppNavigation::run_split_pane_horizontally(cx);
                                    cx.stop_propagation();
                                },
                            ),
                        )
                    },
                );
            }
            if show_close {
                menu = menu.item(
                    PopupMenuItem::new(t!("nav.close_pane")).on_click(|_, _, cx| {
                        AppNavigation::run_close_active_pane(cx);
                        cx.stop_propagation();
                    }),
                );
            }
            menu
        }
    }
}
