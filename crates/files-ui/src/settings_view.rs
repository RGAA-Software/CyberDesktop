use files_core::{load_config, APP_NAME};
use files_fs::{parse_tag_color_hex, TAG_COLOR_PRESETS};
use gpui::{
    div, prelude::FluentBuilder, px, rgb, Anchor, App, AppContext, Entity, InteractiveElement,
    IntoElement, ParentElement, SharedString, StatefulInteractiveElement, Styled, Window,
};
use gpui_component::{
    button::Button,
    group_box::GroupBoxVariant,
    h_flex,
    input::{Input, InputEvent, InputState},
    label::Label,
    setting::{RenderOptions, SettingField, SettingGroup, SettingItem, SettingPage, Settings},
    v_flex, ActiveTheme as _, AxisExt as _, IconName, Sizable as _, Size, ThemeMode,
    StyledExt as _,
};
use gpui_component::popover::Popover;
use rust_i18n::t;

fn ts(text: impl AsRef<str>) -> SharedString {
    SharedString::from(text.as_ref())
}

use crate::app_state::AppNavigation;
use crate::icons::{folder_icon, home_icon, sidebar_icon, tabs_icon};
use crate::shell::preferences::{
    add_file_tag, apply_border_radius, apply_context_menu_shell_submenu,
    apply_context_menu_show_compress, apply_context_menu_show_create_shortcut,
    apply_context_menu_show_extract, apply_context_menu_show_file_tags, apply_context_menu_show_open_in_terminal,
    apply_context_menu_show_pin, apply_context_menu_show_send_to, apply_font_size,
    apply_home_widget_drives, apply_home_widget_file_tags, apply_home_widget_network,
    apply_home_widget_quick_access, apply_home_widget_recent, apply_locale,
    apply_always_open_dual_pane_in_new_tab, apply_auto_restore_tabs, apply_shell_pane_arrangement,
    apply_show_open_in_new_pane, always_open_dual_pane_in_new_tab, shell_pane_arrangement,
    show_open_in_new_pane,
    apply_disable_direct_composition,
    apply_open_media_with_cybermediaplayer,
    apply_open_text_with_cybereditor, apply_scrollbar_show,
    apply_sidebar_display_mode, apply_sidebar_section_cloud, apply_sidebar_section_drives,
    apply_sidebar_section_file_tags, apply_sidebar_section_library, apply_sidebar_section_network,
    apply_sidebar_section_pinned, apply_sidebar_section_wsl, apply_theme_mode, apply_theme_name,
    context_menu_shell_submenu, context_menu_show_compress, context_menu_show_create_shortcut,
    context_menu_show_extract, context_menu_show_file_tags, context_menu_show_open_in_terminal, context_menu_show_pin,
    context_menu_show_send_to, current_locale, home_widget_drives, home_widget_file_tags,
    disable_direct_composition, home_widget_network, home_widget_quick_access, home_widget_recent, open_media_with_cybermediaplayer, open_text_with_cybereditor,
    auto_restore_tabs,
    remove_file_tag, set_file_tag_color,
    scrollbar_show_from_key, scrollbar_show_key, set_list_active_highlight, sidebar_display_mode,
    sidebar_section_cloud, sidebar_section_drives, sidebar_section_file_tags,
    sidebar_section_library, sidebar_section_network, sidebar_section_pinned, sidebar_section_wsl,
};
use app_ui::theme;
use files_commands::shortcut_reference;
use gpui::KeyDownEvent;

use crate::keybindings::{
    self, clear_conflict, conflict_action_id, display_keystroke_for, recording_action_id,
    reset_all_keybindings, reset_keybinding, start_recording,
};

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
                ts(t!("settings.context_menu.extract")),
                SettingField::switch(context_menu_show_extract, apply_context_menu_show_extract)
                    .default_value(context_menu_show_extract(cx)),
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
        .item(SettingItem::render(|_, _window, cx| {
            let recording = recording_action_id(cx);
            let conflict = conflict_action_id(cx);
            v_flex()
                .gap_2()
                .w_full()
                .on_key_down({
                    move |event: &KeyDownEvent, window, cx| {
                        if keybindings::handle_recording_key(event, cx) {
                            window.refresh();
                        }
                    }
                })
                .child(
                    Label::new(ts(t!("settings.actions.description")))
                        .text_sm()
                        .text_color(cx.theme().muted_foreground),
                )
                .child(
                    h_flex()
                        .w_full()
                        .justify_end()
                        .child(
                            Button::new("reset-all-keybindings")
                                .label(ts(t!("settings.actions.reset_all")))
                                .with_size(Size::Small)
                                .on_click(|_, window, cx| {
                                    let _ = reset_all_keybindings(cx);
                                    clear_conflict(cx);
                                    window.refresh();
                                }),
                        ),
                )
                .when(conflict.is_some(), |col| {
                    let conflict_id = conflict.clone().unwrap();
                    let conflict_label = files_commands::action_spec_by_id(&conflict_id)
                        .map(|spec| t!(spec.i18n_key).to_string())
                        .unwrap_or(conflict_id);
                    col.child(
                        Label::new(t!(
                            "settings.actions.conflict",
                            action = conflict_label
                        ))
                        .text_sm()
                        .text_color(cx.theme().danger),
                    )
                })
                .children(shortcut_reference().into_iter().enumerate().map(
                    |(index, entry)| {
                        let action_id = entry.action_id.to_string();
                        let label = t!(entry.message_key);
                        let is_recording = recording.as_deref() == Some(action_id.as_str());
                        let keystroke = display_keystroke_for(&action_id);
                        h_flex()
                            .id(("shortcut-row", index))
                            .w_full()
                            .items_center()
                            .justify_between()
                            .gap_3()
                            .py_1()
                            .when(is_recording, |row| {
                                row.bg(cx.theme().accent.opacity(0.15))
                            })
                            .child(Label::new(label).text_sm())
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        Label::new(if is_recording {
                                            t!("settings.actions.press_key").to_string()
                                        } else {
                                            keystroke
                                        })
                                        .text_xs()
                                        .text_color(if is_recording {
                                            cx.theme().primary
                                        } else {
                                            cx.theme().muted_foreground
                                        }),
                                    )
                                    .child(
                                        Button::new(("record-shortcut", index))
                                            .label(ts(t!("settings.actions.record")))
                                            .with_size(Size::Small)
                                            .on_click({
                                                let action_id = action_id.clone();
                                                move |_, window, cx| {
                                                    clear_conflict(cx);
                                                    start_recording(action_id.clone(), cx);
                                                    window.refresh();
                                                }
                                            }),
                                    )
                                    .child(
                                        Button::new(("reset-shortcut", index))
                                            .label(ts(t!("settings.actions.reset")))
                                            .with_size(Size::Small)
                                            .on_click({
                                                let action_id = action_id.clone();
                                                move |_, window, cx| {
                                                    let _ = reset_keybinding(&action_id, cx);
                                                    clear_conflict(cx);
                                                    window.refresh();
                                                }
                                            }),
                                    ),
                            )
                    },
                ))
        }))
}

fn search_reference_line(label: SharedString) -> impl IntoElement {
    div()
        .w_full()
        .py_1()
        .child(Label::new(label).text_sm())
}

fn search_reference_section(title: SharedString) -> impl IntoElement {
    Label::new(title).text_sm().font_semibold()
}

fn search_settings_group() -> SettingGroup {
    SettingGroup::new()
        .title(ts(t!("settings.group.search")))
        .item(SettingItem::render(|_, _, cx| {
            let ctrl_f = display_keystroke_for("focus_search");
            let ctrl_l = display_keystroke_for("focus_omnibar");
            let f5 = display_keystroke_for("refresh_directory");
            let back = display_keystroke_for("navigate_back");
            v_flex()
                .gap_1()
                .w_full()
                .child(
                    Label::new(ts(t!("settings.search.description")))
                        .text_sm()
                        .text_color(cx.theme().muted_foreground),
                )
                .child(search_reference_section(ts(t!("settings.search.section.shortcuts"))))
                .child(search_reference_line(
                    ts(t!(
                        "settings.search.item.global_mode",
                        key = ctrl_f.as_str()
                    )),
                ))
                .child(search_reference_line(
                    ts(t!(
                        "settings.search.item.path_mode",
                        key = ctrl_l.as_str()
                    )),
                ))
                .child(search_reference_line(
                    ts(t!("settings.search.item.submit")),
                ))
                .child(search_reference_line(
                    ts(t!("settings.search.item.history_up_down")),
                ))
                .child(search_reference_line(
                    ts(t!("settings.search.item.history_tab")),
                ))
                .child(search_reference_line(
                    ts(t!("settings.search.item.esc_mode")),
                ))
                .child(search_reference_line(
                    ts(t!(
                        "settings.search.item.esc_back",
                        key = back.as_str()
                    )),
                ))
                .child(search_reference_line(
                    ts(t!("settings.search.item.refresh", key = f5.as_str())),
                ))
                .child(search_reference_line(
                    ts(t!("settings.search.item.list_filter")),
                ))
                .child(search_reference_section(ts(t!("settings.search.section.syntax"))))
                .child(search_reference_line(
                    ts(t!("settings.search.item.plain")),
                ))
                .child(search_reference_line(
                    ts(t!("settings.search.item.tag")),
                ))
                .child(search_reference_line(
                    ts(t!("settings.search.item.aqs")),
                ))
                .child(search_reference_line(
                    ts(t!("settings.search.item.aqs_fallback")),
                ))
                .child(search_reference_line(
                    ts(t!("settings.search.item.aqs_examples")),
                ))
                .child(search_reference_section(ts(t!("settings.search.section.scope"))))
                .child(search_reference_line(
                    ts(t!("settings.search.item.scope_folder")),
                ))
                .child(search_reference_line(
                    ts(t!("settings.search.item.scope_home")),
                ))
                .child(search_reference_line(
                    ts(t!("settings.search.item.scope_repeat")),
                ))
                .child(search_reference_section(ts(t!("settings.search.section.results"))))
                .child(search_reference_line(
                    ts(t!("settings.search.item.results_sort")),
                ))
                .child(search_reference_line(
                    ts(t!("settings.search.item.results_open")),
                ))
                .child(search_reference_line(
                    ts(t!("settings.search.item.results_breadcrumb")),
                ))
                .child(search_reference_line(
                    ts(t!("settings.search.item.results_status")),
                ))
                .child(search_reference_section(ts(t!("settings.search.section.history"))))
                .child(search_reference_line(
                    ts(t!(
                        "settings.search.item.history_panel",
                        key = ctrl_f.as_str()
                    )),
                ))
                .child(search_reference_line(
                    ts(t!("settings.search.item.history_save")),
                ))
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

fn tag_color_dot(color: Option<&str>) -> impl IntoElement {
    let fill = color
        .and_then(parse_tag_color_hex)
        .map(rgb)
        .unwrap_or(rgb(0x54_6E_7A));
    div()
        .size(px(10.))
        .rounded_full()
        .flex_none()
        .bg(fill)
}

const TAG_COLOR_SWATCH: f32 = 20.;
const TAG_COLOR_GAP: f32 = 6.;
const TAG_COLOR_COLUMNS: f32 = 9.;
const TAG_COLOR_PANEL_WIDTH: f32 =
    TAG_COLOR_COLUMNS * TAG_COLOR_SWATCH + (TAG_COLOR_COLUMNS - 1.) * TAG_COLOR_GAP;

fn render_tag_color_picker_button(
    index: usize,
    name: String,
    current_color: Option<String>,
) -> impl IntoElement {
    let name_for_grid = name.clone();
    let current_for_grid = current_color.clone();
    div().flex_shrink_0().child(
        Popover::new(SharedString::from(format!("tag-color-popover-{index}")))
            .anchor(Anchor::BottomRight)
            .trigger(
                Button::new(("tag-color-picker", index))
                    .with_size(Size::Small)
                    .flex_shrink_0()
                    .label(ts(t!("settings.tags.change_color"))),
            )
            .content(move |_, _, cx| {
            let tag_name = name_for_grid.clone();
            let current_for_grid = current_for_grid.clone();
            h_flex()
                .gap(px(TAG_COLOR_GAP))
                .flex_wrap()
                .w(px(TAG_COLOR_PANEL_WIDTH))
                .children(TAG_COLOR_PRESETS.iter().enumerate().map(
                    |(color_ix, preset)| {
                        let tag_name = tag_name.clone();
                        let preset = (*preset).to_string();
                        let selected = current_for_grid.as_deref() == Some(preset.as_str());
                        let swatch = parse_tag_color_hex(&preset)
                            .map(rgb)
                            .unwrap_or(rgb(0x54_6E_7A));
                        div()
                            .id(("tag-color-pick", index * 100 + color_ix))
                            .size(px(TAG_COLOR_SWATCH))
                            .rounded_full()
                            .bg(swatch)
                            .cursor_pointer()
                            .border_2()
                            .border_color(if selected {
                                cx.theme().primary
                            } else {
                                cx.theme().border
                            })
                            .on_click(move |_, _, cx| {
                                set_file_tag_color(&tag_name, Some(preset.clone()), cx);
                            })
                    },
                ))
        }),
    )
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
                        let current_color = tag.color.clone();
                        h_flex()
                            .id(("file-tag", index))
                            .w_full()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .gap_1()
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .items_center()
                                            .child(tag_color_dot(current_color.as_deref()))
                                            .child(Label::new(name.clone()).text_sm()),
                                    )
                                    .child(
                                        div().pl(px(18.)).child(
                                            Label::new(summary)
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground),
                                        ),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .gap_1()
                                    .flex_none()
                                    .child(render_tag_color_picker_button(
                                        index,
                                        name.clone(),
                                        current_color,
                                    ))
                                    .child(
                                        Button::new(("remove-tag", index))
                                            .label(ts(t!("settings.tags.remove")))
                                            .with_size(Size::Small)
                                            .flex_shrink_0()
                                            .on_click(move |_, _, cx| {
                                                remove_file_tag(&name, cx);
                                            }),
                                    ),
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
        (app_ui::i18n::LOCALE_ZH_HANT.into(), "繁體中文".into()),
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

    Settings::new("cyber_desktop-settings")
        .with_group_variant(GroupBoxVariant::Outline)
        .sidebar_width(px(220.0))
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
                        .items(vec![
                            SettingItem::new(
                                ts(t!("settings.open_with_cybereditor")),
                                SettingField::switch(
                                    open_text_with_cybereditor,
                                    apply_open_text_with_cybereditor,
                                )
                                .default_value(open_text_with_cybereditor(cx)),
                            )
                            .description(ts(t!("settings.open_with_cybereditor.description"))),
                            SettingItem::new(
                                ts(t!("settings.open_with_cybermediaplayer")),
                                SettingField::switch(
                                    open_media_with_cybermediaplayer,
                                    apply_open_media_with_cybermediaplayer,
                                )
                                .default_value(open_media_with_cybermediaplayer(cx)),
                            )
                            .description(ts(t!("settings.open_with_cybermediaplayer.description"))),
                            SettingItem::new(
                                ts(t!("settings.disable_direct_composition")),
                                SettingField::switch(
                                    disable_direct_composition,
                                    apply_disable_direct_composition,
                                )
                                .default_value(disable_direct_composition(cx)),
                            )
                            .description(ts(t!("settings.disable_direct_composition.description"))),
                        ]),
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
            SettingPage::new(ts(t!("settings.page.search")))
                .icon(sidebar_icon(IconName::Search))
                .groups(vec![search_settings_group()]),
            SettingPage::new(ts(t!("settings.page.tabs")))
                .icon(tabs_icon())
                .groups(vec![
                    SettingGroup::new()
                        .title(ts(t!("settings.group.tabs")))
                        .items(vec![SettingItem::new(
                            ts(t!("settings.tabs.auto_restore")),
                            SettingField::switch(auto_restore_tabs, apply_auto_restore_tabs)
                                .default_value(auto_restore_tabs(cx)),
                        )
                        .description(ts(t!("settings.tabs.auto_restore.description")))]),
                    SettingGroup::new()
                        .title(ts(t!("settings.group.dual_pane")))
                        .items(vec![
                            SettingItem::new(
                                ts(t!("settings.dual_pane.always_open")),
                                SettingField::switch(
                                    always_open_dual_pane_in_new_tab,
                                    apply_always_open_dual_pane_in_new_tab,
                                )
                                .default_value(always_open_dual_pane_in_new_tab(cx)),
                            )
                            .description(ts(t!("settings.dual_pane.always_open.description"))),
                            SettingItem::new(
                                ts(t!("settings.dual_pane.arrangement")),
                                SettingField::dropdown(
                                    vec![
                                        ("vertical".into(), ts(t!("settings.dual_pane.vertical"))),
                                        (
                                            "horizontal".into(),
                                            ts(t!("settings.dual_pane.horizontal")),
                                        ),
                                    ],
                                    shell_pane_arrangement,
                                    apply_shell_pane_arrangement,
                                )
                                .default_value(shell_pane_arrangement(cx)),
                            )
                            .description(ts(t!("settings.dual_pane.arrangement.description"))),
                            SettingItem::new(
                                ts(t!("settings.dual_pane.show_open_in_new_pane")),
                                SettingField::switch(show_open_in_new_pane, apply_show_open_in_new_pane)
                                    .default_value(show_open_in_new_pane(cx)),
                            )
                            .description(ts(
                                t!("settings.dual_pane.show_open_in_new_pane.description"),
                            )),
                        ]),
                ]),
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
