//! Settings panel for CyberEditor (general appearance + interface, aligned with CyberFiles).

use gpui::{px, App, SharedString};
use gpui_component::{
    group_box::GroupBoxVariant,
    setting::{SettingField, SettingGroup, SettingItem, SettingPage, Settings},
    ActiveTheme as _, IconName, ThemeMode,
};
use rust_i18n::t;

use crate::toolbar_button::toolbar_icon;

use super::preferences::{
    apply_border_radius, apply_font_size, apply_locale, apply_scrollbar_show, apply_theme_mode,
    apply_theme_name, current_locale, scrollbar_show_from_key, scrollbar_show_key_for,
    set_list_active_highlight,
};
use crate::theme;

fn ts(text: impl AsRef<str>) -> SharedString {
    SharedString::from(text.as_ref())
}

const THEME_SETS: &[(&str, &str)] = &[
    ("CyberFiles", "CyberFiles"),
    ("CyberEditor", "CyberEditor"),
    ("CyberMediaPlayer", "CyberMediaPlayer"),
];

pub fn build_editor_settings(cx: &App) -> Settings {
    let theme_options = THEME_SETS
        .iter()
        .map(|(id, label)| (SharedString::from(*id), SharedString::from(*label)))
        .collect();

    let language_options = vec![
        ("en".into(), "English".into()),
        ("zh-CN".into(), "简体中文".into()),
        (crate::i18n::LOCALE_ZH_HANT.into(), "繁體中文".into()),
    ];

    let font_size_options = vec![
        ("14".into(), ts(t!("settings.font_size.small"))),
        ("16".into(), ts(t!("settings.font_size.medium"))),
        ("18".into(), ts(t!("settings.font_size.large"))),
    ];

    let radius_options = vec![
        ("0".into(), "0px".into()),
        ("4".into(), "4px".into()),
        ("6".into(), ts(t!("settings.radius.default"))),
        ("8".into(), "8px".into()),
    ];

    let scrollbar_options = vec![
        ("scrolling".into(), ts(t!("settings.scrollbar.scrolling"))),
        ("hover".into(), ts(t!("settings.scrollbar.hover"))),
        ("always".into(), ts(t!("settings.scrollbar.always"))),
    ];

    Settings::new("cybereditor-settings")
        .with_group_variant(GroupBoxVariant::Outline)
        .sidebar_width(px(220.0))
        .pages(vec![
            SettingPage::new(ts(t!("settings.page.general")))
                .default_open(true)
                .icon(toolbar_icon(IconName::Settings2).path("icons/settings-2.svg"))
                .groups(vec![
                    SettingGroup::new()
                        .title(ts(t!("settings.group.appearance")))
                        .items(vec![
                            SettingItem::new(
                                ts(t!("settings.dark_mode")),
                                SettingField::switch(
                                    |cx: &App| cx.theme().mode.is_dark(),
                                    |enabled: bool, cx: &mut App| {
                                        let mode = if enabled {
                                            ThemeMode::Dark
                                        } else {
                                            ThemeMode::Light
                                        };
                                        apply_theme_mode(mode, cx);
                                    },
                                )
                                .default_value(cx.theme().mode.is_dark()),
                            )
                            .description(ts(t!("settings.dark_mode.description"))),
                            SettingItem::new(
                                ts(t!("settings.language")),
                                SettingField::dropdown(
                                    language_options,
                                    current_locale,
                                    |locale: SharedString, cx: &mut App| {
                                        apply_locale(locale.as_ref(), cx);
                                    },
                                )
                                .default_value(current_locale(cx)),
                            )
                            .description(ts(t!("settings.language.description"))),
                            SettingItem::new(
                                ts(t!("settings.color_theme")),
                                SettingField::scrollable_dropdown(
                                    theme_options,
                                    theme::current_theme_set_id,
                                    |name: SharedString, cx: &mut App| {
                                        apply_theme_name(name, cx);
                                    },
                                )
                                .default_value(theme::current_theme_set_id(cx)),
                            )
                            .description(ts(t!("settings.color_theme.description"))),
                        ]),
                    SettingGroup::new()
                        .title(ts(t!("settings.group.interface")))
                        .items(vec![
                            SettingItem::new(
                                ts(t!("settings.font_size")),
                                SettingField::dropdown(
                                    font_size_options,
                                    |cx: &App| {
                                        format!("{}", cx.theme().font_size.as_f32().round() as i32)
                                            .into()
                                    },
                                    |val: SharedString, cx: &mut App| {
                                        if let Ok(size) = val.parse::<f32>() {
                                            apply_font_size(size, cx);
                                        }
                                    },
                                )
                                .default_value(SharedString::from(format!(
                                    "{}",
                                    cx.theme().font_size.as_f32().round() as i32
                                ))),
                            )
                            .description(ts(t!("settings.font_size.description"))),
                            SettingItem::new(
                                ts(t!("settings.border_radius")),
                                SettingField::dropdown(
                                    radius_options,
                                    |cx: &App| {
                                        format!("{}", cx.theme().radius.as_f32().round() as i32)
                                            .into()
                                    },
                                    |val: SharedString, cx: &mut App| {
                                        if let Ok(radius) = val.parse::<f32>() {
                                            apply_border_radius(radius, cx);
                                        }
                                    },
                                )
                                .default_value(SharedString::from(format!(
                                    "{}",
                                    cx.theme().radius.as_f32().round() as i32
                                ))),
                            )
                            .description(ts(t!("settings.border_radius.description"))),
                            SettingItem::new(
                                ts(t!("settings.scrollbar")),
                                SettingField::dropdown(
                                    scrollbar_options,
                                    scrollbar_show_key_for,
                                    |val: SharedString, cx: &mut App| {
                                        apply_scrollbar_show(scrollbar_show_from_key(val.as_ref()), cx);
                                    },
                                )
                                .default_value(scrollbar_show_key_for(cx)),
                            )
                            .description(ts(t!("settings.scrollbar.description"))),
                            SettingItem::new(
                                ts(t!("settings.list_highlight")),
                                SettingField::switch(
                                    |cx: &App| cx.theme().list.active_highlight,
                                    |enabled: bool, cx: &mut App| {
                                        set_list_active_highlight(enabled, cx);
                                    },
                                )
                                .default_value(cx.theme().list.active_highlight),
                            )
                            .description(ts(t!("settings.list_highlight.description"))),
                        ]),
                ]),
        ])
}
