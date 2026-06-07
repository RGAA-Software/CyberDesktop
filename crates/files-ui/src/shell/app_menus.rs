use gpui::{App, Entity, Global, Menu, SharedString};
use gpui_component::{menu::AppMenuBar, GlobalState};

struct AppMenuState {
    menu_bar: Entity<AppMenuBar>,
    title: SharedString,
}

impl Global for AppMenuState {}

#[allow(dead_code)]
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

fn build_menus(title: impl Into<SharedString>, _cx: &mut App) -> Vec<Menu> {
    vec![Menu {
        name: title.into(),
        items: vec![],
        disabled: false,
    }]
}
