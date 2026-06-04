use gpui::{App, Entity, Global, Menu, MenuItem, SharedString};
use gpui_component::{menu::AppMenuBar, GlobalState};

use rust_i18n::t;

use files_commands::{
    ArrangePanesHorizontally, ArrangePanesVertically, CloseActivePane, FocusOtherPane,
    ReopenClosedTab, SplitPaneHorizontally, SplitPaneVertically, ToggleDualPane,
};

use super::actions::ReopenClosedTabAt;
use files_core::load_config;

use super::actions::{About, Quit};
use super::dual_pane_menu::dual_pane_menu_state;
use super::navigation::NavigationTarget;

struct AppMenuState {
    menu_bar: Entity<AppMenuBar>,
    title: SharedString,
}

impl Global for AppMenuState {}

pub fn menu_bar(cx: &App) -> Entity<AppMenuBar> {
    cx.global::<AppMenuState>().menu_bar.clone()
}

pub fn init(title: impl Into<SharedString>, cx: &mut App) -> Entity<AppMenuBar> {
    let app_menu_bar = AppMenuBar::new(cx);
    let title: SharedString = title.into();
    cx.set_global(AppMenuState {
        menu_bar: app_menu_bar.clone(),
        title: title.clone(),
    });
    update_app_menu(cx);

    app_menu_bar
}

/// Reload native and in-window menus (e.g. after locale change).
///
/// Deferred so menu builders can read [`crate::main_page::MainPage`] without
/// double-leasing when this is called from inside a `MainPage::update` closure.
pub fn reload(cx: &mut App) {
    if !cx.has_global::<AppMenuState>() {
        return;
    }
    cx.defer(|cx| {
        if !cx.has_global::<AppMenuState>() {
            return;
        }
        update_app_menu(cx);
    });
}

fn update_app_menu(cx: &mut App) {
    let (title, app_menu_bar) = {
        let state = cx.global::<AppMenuState>();
        (state.title.clone(), state.menu_bar.clone())
    };

    let menus_for_platform = build_menus(title.clone(), cx);
    cx.set_menus(menus_for_platform);

    let owned: Vec<_> = build_menus(title, cx)
        .into_iter()
        .map(|menu| menu.owned())
        .collect();
    GlobalState::global_mut(cx).set_app_menus(owned);

    app_menu_bar.update(cx, |menu_bar, cx| {
        menu_bar.reload(cx);
    });
}

fn build_dual_pane_menu_items(cx: &mut App) -> Vec<MenuItem> {
    let state = dual_pane_menu_state(cx);
    let mut items = Vec::new();

    if state.multi_pane_available && !state.dual {
        items.push(MenuItem::submenu(Menu {
            name: t!("menu.split_pane").into(),
            items: vec![
                MenuItem::action(t!("menu.split_pane_vertical"), SplitPaneVertically),
                MenuItem::action(t!("menu.split_pane_horizontal"), SplitPaneHorizontally),
            ],
            disabled: false,
        }));
        items.push(MenuItem::action(t!("nav.split_pane"), ToggleDualPane));
    }

    if state.dual {
        items.push(MenuItem::submenu(Menu {
            name: t!("menu.arrange_panes").into(),
            items: vec![
                MenuItem::action(t!("menu.split_pane_vertical"), ArrangePanesVertically),
                MenuItem::action(t!("menu.split_pane_horizontal"), ArrangePanesHorizontally),
            ],
            disabled: false,
        }));
        items.push(MenuItem::action(t!("nav.focus_other_pane"), FocusOtherPane));
        items.push(MenuItem::action(t!("nav.close_pane"), CloseActivePane));
    }

    if !items.is_empty() {
        items.push(MenuItem::separator());
    }
    items
}

fn build_view_menu_items(cx: &mut App) -> Vec<MenuItem> {
    let closed = load_config()
        .map(|c| c.session_closed_tabs)
        .unwrap_or_default();

    let mut items = build_dual_pane_menu_items(cx);
    items.push(
        MenuItem::action(t!("nav.reopen_closed_tab"), ReopenClosedTab).disabled(closed.is_empty()),
    );

    if closed.is_empty() {
        return items;
    }

    items.push(MenuItem::separator());
    for (index, session) in closed.iter().enumerate() {
        let label = NavigationTarget::label_for_session_tab(&session.tab);
        items.push(MenuItem::action(label, ReopenClosedTabAt { index }));
    }

    items
}

fn build_menus(title: impl Into<SharedString>, cx: &mut App) -> Vec<Menu> {
    vec![
        Menu {
            name: title.into(),
            items: vec![
                MenuItem::action(t!("menu.about"), About),
                MenuItem::Separator,
                MenuItem::action(t!("menu.quit"), Quit),
            ],
            disabled: false,
        },
        Menu {
            name: t!("menu.view").into(),
            items: build_view_menu_items(cx),
            disabled: false,
        },
    ]
}
