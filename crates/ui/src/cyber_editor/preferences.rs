//! Preference apply/persist for CyberEditor (shared `settings.json` with CyberFiles).

use cyberfiles_core::{load_config, save_config, AppConfig};
use gpui::{px, App, SharedString};
use gpui_component::{scroll::ScrollbarShow, ActiveTheme as _, Theme, ThemeMode};

use crate::i18n;
use crate::theme;

use super::app_menus;

fn scrollbar_show_key(show: ScrollbarShow) -> SharedString {
    match show {
        ScrollbarShow::Scrolling => "scrolling".into(),
        ScrollbarShow::Hover => "hover".into(),
        ScrollbarShow::Always => "always".into(),
    }
}

fn persist(cx: &mut App) {
    let prior = load_config().unwrap_or_default();
    let config = AppConfig {
        locale: i18n::locale().to_string(),
        dark_mode: cx.theme().mode.is_dark(),
        theme_name: theme::current_theme_set_id(cx).to_string(),
        font_size: cx.theme().font_size.as_f32(),
        border_radius: cx.theme().radius.as_f32(),
        scrollbar_show: scrollbar_show_key(cx.theme().scrollbar_show).to_string(),
        list_active_highlight: cx.theme().list.active_highlight,
        ..prior
    };
    let _ = save_config(&config);
}

pub fn current_locale(_cx: &App) -> SharedString {
    i18n::locale().to_string().into()
}

pub fn apply_locale(locale: &str, cx: &mut App) {
    i18n::set_locale(locale);
    app_menus::reload(cx);
    cx.refresh_windows();
    persist(cx);
}

pub fn apply_theme_mode(mode: ThemeMode, cx: &mut App) {
    let set_id = theme::current_theme_set_id(cx);
    theme::apply_set(set_id.as_ref(), mode, cx);
    cx.refresh_windows();
    persist(cx);
}

pub fn apply_theme_name(name: SharedString, cx: &mut App) {
    let mode = Theme::global(cx).mode;
    theme::apply_set(name.as_ref(), mode, cx);
    cx.refresh_windows();
    persist(cx);
}

pub fn apply_font_size(size: f32, cx: &mut App) {
    Theme::global_mut(cx).font_size = px(size);
    cx.refresh_windows();
    persist(cx);
}

pub fn apply_border_radius(radius: f32, cx: &mut App) {
    let theme = Theme::global_mut(cx);
    theme.radius = px(radius);
    theme.radius_lg = if theme.radius > px(0.) {
        theme.radius + px(2.)
    } else {
        px(0.)
    };
    cx.refresh_windows();
    persist(cx);
}

pub fn apply_scrollbar_show(show: ScrollbarShow, cx: &mut App) {
    Theme::global_mut(cx).scrollbar_show = show;
    cx.refresh_windows();
    persist(cx);
}

pub fn set_list_active_highlight(enabled: bool, cx: &mut App) {
    Theme::global_mut(cx).list.active_highlight = enabled;
    cx.refresh_windows();
    persist(cx);
}

pub fn scrollbar_show_from_key(key: &str) -> ScrollbarShow {
    match key {
        "hover" => ScrollbarShow::Hover,
        "always" => ScrollbarShow::Always,
        _ => ScrollbarShow::Scrolling,
    }
}

pub fn scrollbar_show_key_for(cx: &App) -> SharedString {
    scrollbar_show_key(cx.theme().scrollbar_show)
}
