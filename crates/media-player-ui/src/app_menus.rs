//! Application menu bar for CyberMediaPlayer.

use gpui::{actions, App, Entity, Global, Menu, MenuItem, SharedString};
use gpui_component::{menu::AppMenuBar, GlobalState};

actions!(
    mediaplayer,
    [
        MpvOpenFile,
        MpvOpenFolder,
        MpvLoadSubtitle,
        MpvCycleSubTrack,
        MpvSetLoopNone,
        MpvSetLoopAll,
        MpvSetLoopSingle,
        MpvSetSpeed05,
        MpvSetSpeed10,
        MpvSetSpeed15,
        MpvSetSpeed20,
        MpvExitFullscreen
    ]
);

struct MediaPlayerMenuState {
    menu_bar: Entity<AppMenuBar>,
}

impl Global for MediaPlayerMenuState {}

pub fn menu_bar(cx: &App) -> Entity<AppMenuBar> {
    cx.global::<MediaPlayerMenuState>().menu_bar.clone()
}

pub fn init_media_player_menus(cx: &mut App) -> Entity<AppMenuBar> {
    let menu_bar = AppMenuBar::new(cx);
    cx.set_global(MediaPlayerMenuState {
        menu_bar: menu_bar.clone(),
    });
    reload(cx);
    menu_bar
}

pub fn reload(cx: &mut App) {
    if !cx.has_global::<MediaPlayerMenuState>() {
        return;
    }
    let menu_bar = cx.global::<MediaPlayerMenuState>().menu_bar.clone();
    cx.set_menus(build_menus());
    let owned = build_menus().into_iter().map(|menu| menu.owned()).collect();
    if cx.has_global::<GlobalState>() {
        GlobalState::global_mut(cx).set_app_menus(owned);
    }
    menu_bar.update(cx, |bar, cx| bar.reload(cx));
}

fn menu_title(label: impl Into<SharedString>, access_key: char) -> SharedString {
    let label = label.into();
    SharedString::from(format!("{}({access_key})", label.as_ref()))
}

fn build_menus() -> Vec<Menu> {
    vec![Menu {
        name: menu_title(SharedString::from("File"), 'F'),
        items: vec![
            MenuItem::action(SharedString::from("Open File"), MpvOpenFile),
            MenuItem::action(SharedString::from("Open Folder"), MpvOpenFolder),
            MenuItem::separator(),
            MenuItem::action(SharedString::from("Load Subtitle"), MpvLoadSubtitle),
            MenuItem::action(SharedString::from("CC Track"), MpvCycleSubTrack),
        ],
        disabled: false,
    }]
}
