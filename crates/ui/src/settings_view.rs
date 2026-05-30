use cyberfiles_core::{load_config, APP_NAME};
use gpui::{
    prelude::FluentBuilder, App, AppContext, Entity, InteractiveElement, IntoElement,
    ParentElement, SharedString, Styled, Window,
};
use gpui_component::{
    button::Button,
    group_box::GroupBoxVariant,
    h_flex,
    input::{Input, InputEvent, InputState},
    label::Label,
    setting::{RenderOptions, SettingField, SettingGroup, SettingItem, SettingPage, Settings},
    v_flex, ActiveTheme as _, AxisExt as _, IconName, Sizable as _, Size, ThemeMode,
};
use rust_i18n::t;

fn ts(text: impl AsRef<str>) -> SharedString {
    SharedString::from(text.as_ref())
}

use crate::app_state::AppNavigation;
use crate::icons::{folder_icon, home_icon, sidebar_icon};
use crate::shell::preferences::{
    add_file_tag, apply_border_radius, apply_context_menu_shell_submenu,
    apply_context_menu_show_compress, apply_context_menu_show_create_shortcut,
    apply_context_menu_show_file_tags, apply_context_menu_show_open_in_terminal,
    apply_context_menu_show_pin, apply_context_menu_show_send_to, apply_font_size,
    apply_home_widget_drives, apply_home_widget_file_tags, apply_home_widget_network,
    apply_home_widget_quick_access, apply_home_widget_recent, apply_locale,
    apply_open_text_with_cybereditor, apply_scrollbar_show,
    apply_sidebar_display_mode, apply_sidebar_section_cloud, apply_sidebar_section_drives,
    apply_sidebar_section_file_tags, apply_sidebar_section_library, apply_sidebar_section_network,
    apply_sidebar_section_pinned, apply_sidebar_section_wsl, apply_theme_mode, apply_theme_name,
    context_menu_shell_submenu, context_menu_show_compress, context_menu_show_create_shortcut,
    context_menu_show_file_tags, context_menu_show_open_in_terminal, context_menu_show_pin,
    context_menu_show_send_to, current_locale, home_widget_drives, home_widget_file_tags,
    home_widget_network, home_widget_quick_access, home_widget_recent, open_text_with_cybereditor,
    remove_file_tag,
    scrollbar_show_from_key, scrollbar_show_key, set_list_active_highlight, sidebar_display_mode,
    sidebar_section_cloud, sidebar_section_drives, sidebar_section_file_tags,
    sidebar_section_library, sidebar_section_network, sidebar_section_pinned, sidebar_section_wsl,
};
use crate::theme;
use cyberfiles_commands::shortcut_reference;

fn context_menu_settings_group(cx: &App) -> SettingGroup {
    SettingGroup::new()
        .title(ts(t!("settings.group.context_menu")))
        .items(vec![
            SettingItem::new(
                ts(t!("settings.context_menu.shell_submenu")),
                SettingField::switch(context_menu_shell_submenu, apply_context_menu_shell_submenu)
                    .default_value(context_menu_shell_submenu(cx)),
            )
            .description(ts(t!("settings.context_menu.shell_submenu.description"))),
            SettingItem::new(
                ts(t!("settings.context_menu.compress")),
                SettingField::switch(context_menu_show_compress, apply_context_menu_show_compress)
                    .default_value(context_menu_show_compress(cx)),
            ),
            SettingItem::new(
                ts(t!("settings.context_menu.send_to")),
                SettingField::switch(context_menu_show_send_to, apply_context_menu_show_send_to)
                    .default_value(context_menu_show_send_to(cx)),
            ),
            SettingItem::new(
                ts(t!("settings.context_menu.pin")),
                SettingField::switch(context_menu_show_pin, apply_context_menu_show_pin)
                    .default_value(context_menu_show_pin(cx)),
            ),
            SettingItem::new(
                ts(t!("settings.context_menu.open_in_terminal")),
                SettingField::switch(
                    context_menu_show_open_in_terminal,
                    apply_context_menu_show_open_in_terminal,
                )
                .default_value(context_menu_show_open_in_terminal(cx)),
            ),
            SettingItem::new(
                ts(t!("settings.context_menu.file_tags")),
                SettingField::switch(
                    context_menu_show_file_tags,
                    apply_context_menu_show_file_tags,
                )
                .default_value(context_menu_show_file_tags(cx)),
            ),
            SettingItem::new(
                ts(t!("settings.context_menu.create_shortcut")),
                SettingField::switch(
                    context_menu_show_create_shortcut,
                    apply_context_menu_show_create_shortcut,
                )
                .default_value(context_menu_show_create_shortcut(cx)),
            ),
        ])
}

fn actions_settings_group() -> SettingGroup {
    SettingGroup::new()
        .title(ts(t!("settings.group.actions")))
        .item(SettingItem::render(|_, _, cx| {
            v_flex()
                .gap_1()
                .w_full()
                .child(
                    Label::new(ts(t!("settings.actions.description")))
                        .text_sm()
                        .text_color(cx.theme().muted_foreground),
                )
                .children(
                    shortcut_reference()
                        .iter()
                        .enumerate()
                        .map(|(index, entry)| {
                            let label = t!(entry.message_key);
                            h_flex()
                                .id(("shortcut-row", index))
                                .w_full()
                                .items_center()
                                .justify_between()
                                .gap_3()
                                .child(Label::new(label).text_sm())
                                .child(
                                    Label::new(entry.keystroke)
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground),
                                )
                        }),
                )
        }))
}

fn folders_settings_group() -> SettingGroup {
    SettingGroup::new()
        .title(ts(t!("settings.group.folders")))
        .item(SettingItem::render(|_, _, cx| {
            let pinned = load_config().map(|c| c.pinned_folders).unwrap_or_default();
            v_flex()
                .gap_3()
                .w_full()
                .child(
                    Label::new(ts(t!("settings.folders.pinned.description")))
                        .text_sm()
                        .text_color(cx.theme().muted_foreground),
                )
                .when(pinned.is_empty(), |col| {
                    col.child(
                        Label::new(ts(t!("settings.folders.empty")))
                            .text_sm()
                            .text_color(cx.theme().muted_foreground),
                    )
                })
                .when(!pinned.is_empty(), |col| {
                    col.children(pinned.iter().enumerate().map(|(index, path)| {
                        let path_string = path.clone();
                        h_flex()
                            .id(("pinned-folder", index))
                            .w_full()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .child(Label::new(path.clone()).text_sm().truncate())
                            .child(
                                Button::new(("unpin-folder", index))
                                    .label(ts(t!("sidebar.menu.unpin")))
                                    .with_size(Size::Small)
                                    .on_click(move |_, _, cx| {
                                        AppNavigation::unpin_folder(&path_string, cx);
                                    }),
                            )
                    }))
                })
        }))
}

fn render_new_tag_name_input(
    options: &RenderOptions,
    window: &mut Window,
    cx: &mut App,
) -> impl IntoElement {
    struct TagNameInputState {
        input: Entity<InputState>,
        _subscription: gpui::Subscription,
    }

    let state = window
        .use_keyed_state(
            SharedString::from(format!(
                "tag-name-input-{}-{}-{}",
                options.page_ix, options.group_ix, options.item_ix
            )),
            cx,
            |window, cx| {
                let input = cx.new(|cx| {
                    InputState::new(window, cx)
                        .placeholder(SharedString::from(t!("settings.tags.add.placeholder")))
                });
                let subscription = cx.subscribe(&input, {
                    move |_, input, event: &InputEvent, cx| {
                        if let InputEvent::PressEnter { .. } = event {
                            add_file_tag(input.read(cx).value(), cx);
                            if let Some(window) = cx.active_window() {
                                let input = input.clone();
                                let _ = window.update(cx, |_, window, cx| {
                                    input.update(cx, |state, cx| {
                                        state.set_value("", window, cx);
                                    });
                                });
                            }
                        }
                    }
                });
                TagNameInputState {
                    input,
                    _subscription: subscription,
                }
            },
        )
        .read(cx);

    Input::new(&state.input)
        .disabled(options.disabled)
        .with_size(options.size)
        .map(|this| {
            if options.layout.is_horizontal() {
                this.w_64()
            } else {
                this.w_full()
            }
        })
}

fn tags_settings_group() -> SettingGroup {
    SettingGroup::new()
        .title(ts(t!("settings.group.tags")))
        .item(SettingItem::render(|_, _, cx| {
            let tags = load_config().map(|c| c.file_tags).unwrap_or_default();
            v_flex()
                .gap_3()
                .w_full()
                .child(
                    Label::new(ts(t!("settings.tags.list.description")))
                        .text_sm()
                        .text_color(cx.theme().muted_foreground),
                )
                .when(tags.is_empty(), |col| {
                    col.child(
                        Label::new(ts(t!("settings.tags.empty")))
                            .text_sm()
                            .text_color(cx.theme().muted_foreground),
                    )
                })
                .when(!tags.is_empty(), |col| {
                    col.children(tags.iter().enumerate().map(|(index, tag)| {
                        let name = tag.name.clone();
                        let summary = t!("settings.tags.path_count", count = tag.paths.len());
                        h_flex()
                            .id(("file-tag", index))
                            .w_full()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .child(
                                v_flex()
                                    .gap_0p5()
                                    .child(Label::new(name.clone()).text_sm())
                                    .child(
                                        Label::new(summary)
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground),
                                    ),
                            )
                            .child(
                                Button::new(("remove-tag", index))
                                    .label(ts(t!("settings.tags.remove")))
                                    .with_size(Size::Small)
                                    .on_click(move |_, _, cx| {
                                        remove_file_tag(&name, cx);
                                    }),
                            )
                    }))
                })
        }))
        .item(
            SettingItem::new(
                ts(t!("settings.tags.add")),
                SettingField::render(|options, window, cx| {
                    render_new_tag_name_input(options, window, cx)
                }),
            )
            .description(ts(t!("settings.tags.add.description"))),
        )
}

pub fn build_settings(cx: &App) -> Settings {
    let theme_options = theme::theme_set_options();

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

    let sidebar_mode_options = vec![
        ("expanded".into(), ts(t!("settings.sidebar.mode.expanded"))),
        ("compact".into(), ts(t!("settings.sidebar.mode.compact"))),
        ("minimal".into(), ts(t!("settings.sidebar.mode.minimal"))),
    ];

    Settings::new("cyberfiles-settings")
        .with_group_variant(GroupBoxVariant::Outline)
        .pages(vec![
            SettingPage::new(ts(t!("settings.page.general")))
                .default_open(true)
                .icon(sidebar_icon(IconName::Settings2))
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
                                .default_value(SharedString::from(
                                    format!("{}", cx.theme().font_size.as_f32().round() as i32),
                                )),
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
                                .default_value(SharedString::from(
                                    format!("{}", cx.theme().radius.as_f32().round() as i32),
                                )),
                            )
                            .description(ts(t!("settings.border_radius.description"))),
                            SettingItem::new(
                                ts(t!("settings.scrollbar")),
                                SettingField::dropdown(
                                    scrollbar_options,
                                    |cx: &App| scrollbar_show_key(cx.theme().scrollbar_show),
                                    |val: SharedString, cx: &mut App| {
                                        apply_scrollbar_show(
                                            scrollbar_show_from_key(val.as_ref()),
                                            cx,
                                        );
                                    },
                                )
                                .default_value(scrollbar_show_key(cx.theme().scrollbar_show)),
                            )
                            .description(ts(t!("settings.scrollbar.description"))),
                            SettingItem::new(
                                ts(t!("settings.list_highlight")),
                                SettingField::switch(
                                    |cx: &App| cx.theme().list.active_highlight,
                                    |checked: bool, cx: &mut App| {
                                        set_list_active_highlight(checked, cx);
                                    },
                                )
                                .default_value(cx.theme().list.active_highlight),
                            )
                            .description(ts(t!("settings.list_highlight.description"))),
                        ]),
                    SettingGroup::new()
                        .title(ts(t!("settings.group.file_open")))
                        .items(vec![SettingItem::new(
                            ts(t!("settings.open_with_cybereditor")),
                            SettingField::switch(
                                open_text_with_cybereditor,
                                apply_open_text_with_cybereditor,
                            )
                            .default_value(open_text_with_cybereditor(cx)),
                        )
                        .description(ts(t!("settings.open_with_cybereditor.description")))]),
                    context_menu_settings_group(cx),
                ]),
            SettingPage::new(ts(t!("settings.page.sidebar")))
                .icon(sidebar_icon(IconName::GalleryVerticalEnd))
                .groups(vec![SettingGroup::new()
                    .title(ts(t!("settings.group.sidebar")))
                    .items(vec![
                        SettingItem::new(
                            ts(t!("settings.sidebar.display_mode")),
                            SettingField::dropdown(
                                sidebar_mode_options,
                                sidebar_display_mode,
                                apply_sidebar_display_mode,
                            )
                            .default_value(sidebar_display_mode(cx)),
                        )
                        .description(ts(t!("settings.sidebar.display_mode.description"))),
                        SettingItem::new(
                            ts(t!("settings.sidebar.section.pinned")),
                            SettingField::switch(
                                sidebar_section_pinned,
                                apply_sidebar_section_pinned,
                            )
                            .default_value(sidebar_section_pinned(cx)),
                        ),
                        SettingItem::new(
                            ts(t!("settings.sidebar.section.library")),
                            SettingField::switch(
                                sidebar_section_library,
                                apply_sidebar_section_library,
                            )
                            .default_value(sidebar_section_library(cx)),
                        ),
                        SettingItem::new(
                            ts(t!("settings.sidebar.section.drives")),
                            SettingField::switch(
                                sidebar_section_drives,
                                apply_sidebar_section_drives,
                            )
                            .default_value(sidebar_section_drives(cx)),
                        ),
                        SettingItem::new(
                            ts(t!("settings.sidebar.section.cloud")),
                            SettingField::switch(
                                sidebar_section_cloud,
                                apply_sidebar_section_cloud,
                            )
                            .default_value(sidebar_section_cloud(cx)),
                        ),
                        SettingItem::new(
                            ts(t!("settings.sidebar.section.network")),
                            SettingField::switch(
                                sidebar_section_network,
                                apply_sidebar_section_network,
                            )
                            .default_value(sidebar_section_network(cx)),
                        ),
                        SettingItem::new(
                            ts(t!("settings.sidebar.section.wsl")),
                            SettingField::switch(sidebar_section_wsl, apply_sidebar_section_wsl)
                                .default_value(sidebar_section_wsl(cx)),
                        ),
                        SettingItem::new(
                            ts(t!("settings.sidebar.section.file_tags")),
                            SettingField::switch(
                                sidebar_section_file_tags,
                                apply_sidebar_section_file_tags,
                            )
                            .default_value(sidebar_section_file_tags(cx)),
                        ),
                    ])]),
            SettingPage::new(ts(t!("settings.page.folders")))
                .icon(folder_icon())
                .groups(vec![folders_settings_group()]),
            SettingPage::new(ts(t!("settings.page.tags")))
                .icon(sidebar_icon(IconName::Inbox))
                .groups(vec![tags_settings_group()]),
            SettingPage::new(ts(t!("settings.page.actions")))
                .icon(sidebar_icon(IconName::Redo2))
                .groups(vec![actions_settings_group()]),
            SettingPage::new(ts(t!("settings.page.home")))
                .icon(home_icon())
                .groups(vec![SettingGroup::new()
                    .title(ts(t!("settings.group.home_widgets")))
                    .items(vec![
                        SettingItem::new(
                            ts(t!("settings.home.widget.quick_access")),
                            SettingField::switch(
                                home_widget_quick_access,
                                apply_home_widget_quick_access,
                            )
                            .default_value(home_widget_quick_access(cx)),
                        ),
                        SettingItem::new(
                            ts(t!("settings.home.widget.drives")),
                            SettingField::switch(home_widget_drives, apply_home_widget_drives)
                                .default_value(home_widget_drives(cx)),
                        ),
                        SettingItem::new(
                            ts(t!("settings.home.widget.network")),
                            SettingField::switch(home_widget_network, apply_home_widget_network)
                                .default_value(home_widget_network(cx)),
                        ),
                        SettingItem::new(
                            ts(t!("settings.home.widget.file_tags")),
                            SettingField::switch(
                                home_widget_file_tags,
                                apply_home_widget_file_tags,
                            )
                            .default_value(home_widget_file_tags(cx)),
                        ),
                        SettingItem::new(
                            ts(t!("settings.home.widget.recent")),
                            SettingField::switch(home_widget_recent, apply_home_widget_recent)
                                .default_value(home_widget_recent(cx)),
                        ),
                    ])]),
            SettingPage::new(ts(t!("settings.page.about")))
                .icon(sidebar_icon(IconName::Info))
                .group(SettingGroup::new().item(SettingItem::render(|_, _, cx| {
                    v_flex()
                        .gap_3()
                        .w_full()
                        .items_center()
                        .justify_center()
                        .child(sidebar_icon(IconName::GalleryVerticalEnd))
                        .child(APP_NAME)
                        .child(
                            Label::new(ts(t!("settings.about.description", app = APP_NAME)))
                                .text_sm()
                                .text_color(cx.theme().muted_foreground),
                        )
                }))),
        ])
}
