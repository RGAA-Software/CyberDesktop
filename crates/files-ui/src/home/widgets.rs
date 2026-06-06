//! Home page widget bodies (Files `*Widget` parity).

use std::path::PathBuf;
use std::time::SystemTime;

use chrono::{DateTime, Local};
use files_fs::{
    eject_drive, open_storage_sense_settings, recent_documents_enabled, DriveInfo, FileTagPreview,
    QuickAccessEntry, RecentItem,
};
use app_platform_windows::open_item_properties;
use gpui::{prelude::*, MouseButton, *};
use gpui_component::{
    alert::Alert,
    button::{Button, ButtonVariants as _},
    h_flex,
    label::Label,
    notification::Notification,
    v_flex, ActiveTheme as _, Sizable as _, StyledExt as _, WindowExt as _,
};
use rust_i18n::t;

use crate::app_state::AppNavigation;
use crate::home::page::HomePage;
use crate::home::widget_shell::{
    block_home_page_context_menu, bordered_home_card, home_card_grid, net_notice,
    space_progress_bar, tag_cols_grid, DRIVE_CARD_PADDING_X, DRIVE_CARD_PADDING_Y,
    DRIVE_ICON_TILE, HOME_CARD_RADIUS, QA_ICON_INNER, QA_ICON_TILE, QA_ITEM_HEIGHT,
    QA_ITEM_PADDING_X, QA_ITEM_PADDING_Y, RECENT_HEADER_HEIGHT, RECENT_ROW_HEIGHT,
};
use crate::icons::{pin_icon, toolbar_tabler};
use crate::tabler_icons;
use app_ui::popup_menu::{ContextMenuExt as _, PopupMenu, PopupMenuItem};
use crate::shell_icon::shell_icon_for_path;

#[cfg(windows)]
use app_platform_windows::{list_known_folder_folders, FOLDERID_NETWORK};

#[derive(Clone)]
pub struct NetworkEntry {
    pub label: String,
    pub path: PathBuf,
}

pub fn load_network_entries() -> Vec<NetworkEntry> {
    #[cfg(windows)]
    {
        list_known_folder_folders(&FOLDERID_NETWORK)
            .unwrap_or_default()
            .into_iter()
            .filter(|e| !e.path.as_os_str().is_empty())
            .map(|e| NetworkEntry {
                label: e.display_name,
                path: e.path,
            })
            .collect()
    }
    #[cfg(not(windows))]
    {
        Vec::new()
    }
}

impl HomePage {
    fn section_header(
        &self,
        id: impl Into<ElementId>,
        icon: impl IntoElement,
        title: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        h_flex()
            .id(id)
            .w_full()
            .mb(px(12.))
            .gap(px(8.))
            .items_center()
            .child(icon)
            .child(
                Label::new(title)
                    .text_sm()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .text_color(cx.theme().foreground),
            )
    }

    fn section_icon(path: &'static str, cx: &App) -> impl IntoElement {
        div()
            .flex_none()
            .text_color(cx.theme().primary)
            .child(toolbar_tabler(path))
    }

    pub(super) fn render_quick_access_widget(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        entries: &[QuickAccessEntry],
    ) -> impl IntoElement {
        block_home_page_context_menu(
            v_flex()
                .id("home-widget-quick-access")
                .w_full()
                .gap_1()
                .child(self.section_header(
                    "home-qa-header",
                    Self::section_icon(tabler_icons::PIN, cx),
                    t!("home.widget.quick_access"),
                    cx,
                ))
                .when(entries.is_empty(), |b| {
                    b.child(Alert::info(
                        "home-quick-access-empty",
                        t!("home.widget.quick_access.empty").to_string(),
                    ))
                })
                .when(!entries.is_empty(), |b| {
                    b.child(home_card_grid(
                        self.layout_width(window),
                        entries.iter().enumerate().map(|(index, entry)| {
                            self.qa_item(window, index, "home-qa", entry, cx)
                                .into_any_element()
                        }),
                    ))
                }),
        )
    }

    pub(super) fn render_drives_widget(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        drives: &[DriveInfo],
    ) -> impl IntoElement {
        block_home_page_context_menu(
            v_flex()
                .id("home-widget-drives")
                .w_full()
                .gap_1()
                .child(self.section_header(
                    "home-drives-header",
                    Self::section_icon(tabler_icons::SERVER, cx),
                    t!("home.widget.drives"),
                    cx,
                ))
                .child(home_card_grid(
                    self.layout_width(window),
                    drives.iter().enumerate().map(|(index, drive)| {
                        self.drive_card(window, index, "home-drive", drive, cx)
                            .into_any_element()
                    }),
                )),
        )
    }

    pub(super) fn render_network_widget(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        entries: &[NetworkEntry],
    ) -> impl IntoElement {
        block_home_page_context_menu(
            v_flex()
                .id("home-widget-network")
                .w_full()
                .gap_1()
                .child(self.section_header(
                    "home-network-header",
                    Self::section_icon(tabler_icons::NETWORK, cx),
                    t!("home.widget.network"),
                    cx,
                ))
                .when(entries.is_empty(), |b| {
                    b.child(net_notice(
                        "home-network-notice",
                        toolbar_tabler(tabler_icons::INFO_CIRCLE),
                        t!("home.widget.network.empty"),
                        cx,
                    ))
                })
                .when(!entries.is_empty(), |b| {
                    b.child(home_card_grid(
                        self.layout_width(window),
                        entries.iter().enumerate().map(|(index, entry)| {
                            let drive = DriveInfo {
                                path: entry.path.clone(),
                                label: entry.label.clone(),
                                volume_label: None,
                                total_bytes: None,
                                free_bytes: None,
                                is_removable: false,
                                is_network: true,
                            };
                            self.drive_card(window, index, "home-network", &drive, cx)
                                .into_any_element()
                        }),
                    ))
                }),
        )
    }

    pub(super) fn render_file_tags_widget(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        previews: &[FileTagPreview],
    ) -> impl IntoElement {
        block_home_page_context_menu(
            v_flex()
                .id("home-widget-tags")
                .w_full()
                .gap_1()
                .child(self.section_header(
                    "home-tags-header",
                    Self::section_icon(tabler_icons::TAG, cx),
                    t!("home.widget.tags"),
                    cx,
                ))
                .when(previews.is_empty(), |b| {
                    b.child(Alert::info(
                        "home-tags-empty",
                        t!("home.widget.tags.empty").to_string(),
                    ))
                })
                .when(!previews.is_empty(), |b| {
                    b.child(tag_cols_grid(
                        self.layout_width(window),
                        previews.iter().enumerate().map(|(index, preview)| {
                            self.tag_container(window, index, preview, cx)
                                .into_any_element()
                        }),
                    ))
                }),
        )
    }

    pub(super) fn render_recent_widget(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        recent: &[RecentItem],
    ) -> impl IntoElement {
        block_home_page_context_menu(
            v_flex()
                .id("home-widget-recent")
                .w_full()
                .gap_1()
                .child(self.section_header(
                    "home-recent-header",
                    Self::section_icon(tabler_icons::HISTORY, cx),
                    t!("home.widget.recent"),
                    cx,
                ))
                .when(!recent_documents_enabled(), |b| {
                    b.child(Alert::warning(
                        "home-recent-disabled",
                        t!("home.widget.recent.disabled").to_string(),
                    ))
                })
                .when(recent_documents_enabled() && recent.is_empty(), |b| {
                    b.child(Alert::info(
                        "home-recent-empty",
                        t!("home.widget.recent.empty").to_string(),
                    ))
                })
                .when(recent_documents_enabled() && !recent.is_empty(), |b| {
                    b.child(
                        v_flex()
                            .w_full()
                            .rounded(cx.theme().radius)
                            .border_1()
                            .border_color(cx.theme().border)
                            .overflow_hidden()
                            .child(self.recent_table_header(cx))
                            .children(recent.iter().enumerate().map(|(index, item)| {
                                self.recent_row(window, index, item, cx).into_any_element()
                            })),
                    )
                }),
        )
    }

    fn qa_item(
        &mut self,
        window: &mut Window,
        index: usize,
        prefix: &str,
        entry: &QuickAccessEntry,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let path = entry.path.clone();
        let label = entry.label.clone();
        let pinned = entry.is_pinned;
        let subtitle = path.parent().map(|p| p.display().to_string()).unwrap_or_default();
        self.ensure_home_thumbnail(&path, QA_ICON_INNER.as_f32(), window, cx);
        bordered_home_card(format!("{prefix}-qa-{index}"), cx)
            .w_full()
            .h(QA_ITEM_HEIGHT)
            .px(QA_ITEM_PADDING_X)
            .py(QA_ITEM_PADDING_Y)
            .flex()
            .items_center()
            .cursor_pointer()
            .hover(|card| card.bg(cx.theme().list_hover))
            .on_click(cx.listener({
                let path = path.clone();
                move |_, event, window, cx| {
                    open_path(&path, event, window, cx);
                }
            }))
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
            .context_menu({
                let path = path.clone();
                let pinned = pinned;
                move |menu, window, cx| folder_context_menu(menu, &path, pinned, window, cx)
            })
            .child(
                h_flex()
                    .w_full()
                    .h_full()
                    .gap(px(12.))
                    .items_center()
                    .child(
                        div()
                            .size(QA_ICON_TILE)
                            .flex_none()
                            .rounded(HOME_CARD_RADIUS)
                            .bg(cx.theme().accent)
                            .text_color(cx.theme().primary)
                            .flex()
                            .items_center()
                            .justify_center()
                            .relative()
                            .child(self.home_card_image(&path, QA_ICON_INNER, window))
                            .when(pinned, |el| {
                                el.child(
                                    div()
                                        .absolute()
                                        .top(px(2.))
                                        .right(px(2.))
                                        .child(pin_icon()),
                                )
                            }),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap(px(3.))
                            .child(
                                Label::new(label)
                                    .text_sm()
                                    .font_weight(gpui::FontWeight::SEMIBOLD)
                                    .truncate(),
                            )
                            .when(!subtitle.is_empty(), |col| {
                                col.child(
                                    Label::new(subtitle)
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground)
                                        .truncate(),
                                )
                            }),
                    ),
            )
    }

    fn drive_card(
        &mut self,
        window: &mut Window,
        index: usize,
        prefix: &str,
        drive: &DriveInfo,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let path = drive.path.clone();
        let title = drive.label.clone();
        let total_label = drive
            .total_bytes
            .map(format_bytes_label)
            .unwrap_or_default();
        let used_label = drive
            .total_bytes
            .zip(drive.free_bytes)
            .map(|(total, free)| format_bytes_label(total.saturating_sub(free)));
        let free_label = drive.free_bytes.map(format_bytes_label);
        let frac = drive.used_fraction();
        self.ensure_home_thumbnail(&path, DRIVE_ICON_TILE.as_f32(), window, cx);
        bordered_home_card(format!("{prefix}-drive-{index}"), cx)
            .w_full()
            .px(DRIVE_CARD_PADDING_X)
            .py(DRIVE_CARD_PADDING_Y)
            .cursor_pointer()
            .hover(|card| card.bg(cx.theme().list_hover))
            .on_click(cx.listener({
                let path = path.clone();
                move |_, event, window, cx| {
                    open_path(&path, event, window, cx);
                }
            }))
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
            .context_menu({
                let drive = drive.clone();
                move |menu, window, cx| drive_context_menu(menu, &drive, window, cx)
            })
            .child(
                v_flex()
                    .w_full()
                    .gap(px(10.))
                    .child(
                        h_flex()
                            .w_full()
                            .gap(px(10.))
                            .items_center()
                            .child(
                                div()
                                    .size(DRIVE_ICON_TILE)
                                    .flex_none()
                                    .rounded(px(6.))
                                    .bg(cx.theme().list_hover)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(self.home_card_image(&path, px(18.), window)),
                            )
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .gap(px(2.))
                                    .child(
                                        Label::new(title)
                                            .text_sm()
                                            .font_weight(gpui::FontWeight::MEDIUM)
                                            .truncate(),
                                    )
                                    .when(!total_label.is_empty(), |col| {
                                        col.child(
                                            Label::new(total_label)
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground),
                                        )
                                    }),
                            ),
                    )
                    .when_some(frac, |col, f| {
                        col.child(space_progress_bar(
                            SharedString::from(format!("{prefix}-bar-{index}")),
                            f,
                        ))
                    })
                    .when(
                        used_label.is_some() || free_label.is_some(),
                        |col| {
                            col.child(
                                h_flex()
                                    .w_full()
                                    .justify_between()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .when_some(used_label.clone(), |row, used| {
                                        row.child(Label::new(used))
                                    })
                                    .when_some(free_label.clone(), |row, free| {
                                        row.child(Label::new(free))
                                    }),
                            )
                        },
                    ),
            )
    }

    fn recent_table_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .h(RECENT_HEADER_HEIGHT)
            .px(px(10.))
            .gap(px(8.))
            .items_center()
            .bg(cx.theme().background)
            .border_b_1()
            .border_color(cx.theme().border)
            .text_xs()
            .font_semibold()
            .text_color(cx.theme().muted_foreground)
            .child(div().w(px(28.)).flex_none())
            .child(div().flex_1().min_w_0().child(t!("files.column.name")))
            .child(div().w(px(210.)).child(t!("info_pane.path")))
            .child(div().w(px(150.)).child(t!("files.column.modified")))
    }

    fn recent_row(
        &self,
        window: &mut Window,
        index: usize,
        item: &RecentItem,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let path = item.path.clone();
        let name = item.label.clone();
        let location = item
            .path
            .parent()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        let modified = format_system_time(item.modified);
        h_flex()
            .id(("home-recent-row", index))
            .w_full()
            .h(RECENT_ROW_HEIGHT)
            .flex_none()
            .px(px(10.))
            .gap(px(8.))
            .items_center()
            .border_b_1()
            .border_color(cx.theme().border)
            .hover(|this| this.bg(cx.theme().secondary))
            .on_click(cx.listener({
                let path = path.clone();
                move |_, event, window, cx| {
                    open_path(&path, event, window, cx);
                }
            }))
            .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation())
            .context_menu({
                let path = path.clone();
                move |menu, window, cx| file_context_menu(menu, &path, window, cx)
            })
            .child(div().w(px(28.)).flex_none().child(shell_icon_for_path(
                &item.path,
                px(16.),
                window,
            )))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .text_ellipsis()
                    .text_sm()
                    .text_color(cx.theme().foreground)
                    .child(name),
            )
            .child(
                div()
                    .w(px(210.))
                    .min_w_0()
                    .overflow_hidden()
                    .text_ellipsis()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(location),
            )
            .child(
                div()
                    .w(px(150.))
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(modified),
            )
    }

    fn tag_container(
        &self,
        window: &mut Window,
        index: usize,
        preview: &FileTagPreview,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let tag_name = preview.tag.name.clone();
        let view_more = tag_name.clone();
        bordered_home_card(("home-tag-container", index), cx)
            .w_full()
            .self_start()
            .overflow_hidden()
            .child(
                v_flex()
                    .w_full()
                    .child(
                        h_flex()
                            .w_full()
                            .px(px(14.))
                            .py(px(10.))
                            .border_b_1()
                            .border_color(cx.theme().border)
                            .items_center()
                            .child(
                                Button::new(("home-tag-view", index))
                                    .ghost()
                                    .small()
                                    .child(
                                        h_flex()
                                            .gap(px(7.))
                                            .items_center()
                                            .child(tag_color_dot(preview.tag.color.as_deref(), cx))
                                            .child(
                                                Label::new(tag_name)
                                                    .text_sm()
                                                    .font_weight(gpui::FontWeight::MEDIUM),
                                            ),
                                    )
                                    .on_click(cx.listener(move |_, _, _, cx| {
                                        AppNavigation::navigate_to_file_tag(view_more.clone(), cx);
                                    }))
                                    .on_mouse_down(MouseButton::Right, |_, _, cx| {
                                        cx.stop_propagation()
                                    }),
                            ),
                    )
                    .child(
                        v_flex()
                            .w_full()
                            .overflow_hidden()
                            .when(preview.preview_items.is_empty(), |col| {
                                col.child(
                                    div()
                                        .px(px(14.))
                                        .py(px(8.))
                                        .child(
                                            Label::new(t!("home.widget.tags.preview.empty"))
                                                .text_xs()
                                                .text_color(cx.theme().muted_foreground),
                                        ),
                                )
                            })
                            .children(preview.preview_items.iter().enumerate().map(
                                |(row, (name, file_path))| {
                                    let open = file_path.clone();
                                    let is_last = row + 1 == preview.preview_items.len();
                                    div()
                                        .id(SharedString::from(format!(
                                            "home-tag-file-{index}-{row}"
                                        )))
                                        .w_full()
                                        .flex()
                                        .items_center()
                                        .child(
                                            h_flex()
                                                .w_full()
                                                .px(px(14.))
                                                .py(px(7.))
                                                .gap(px(8.))
                                                .items_center()
                                                .cursor_pointer()
                                                .text_sm()
                                                .text_color(cx.theme().muted_foreground)
                                                .when(!is_last, |row| {
                                                    row.border_b_1().border_color(cx.theme().border)
                                                })
                                                .hover(|row| row.bg(cx.theme().list_hover))
                                                .child(
                                                    div()
                                                        .flex_none()
                                                        .child(shell_icon_for_path(
                                                            file_path,
                                                            px(14.),
                                                            window,
                                                        )),
                                                )
                                                .child(
                                                    div()
                                                        .flex_1()
                                                        .min_w_0()
                                                        .overflow_hidden()
                                                        .text_ellipsis()
                                                        .child(name.clone()),
                                                ),
                                        )
                                        .on_click(cx.listener({
                                            let open = open.clone();
                                            move |_, event, window, cx| {
                                                open_path(&open, event, window, cx);
                                            }
                                        }))
                                        .on_mouse_down(MouseButton::Right, |_, _, cx| {
                                            cx.stop_propagation()
                                        })
                                        .context_menu({
                                            let open = open.clone();
                                            move |menu, window, cx| {
                                                file_context_menu(menu, &open, window, cx)
                                            }
                                        })
                                        .into_any_element()
                                },
                            )),
                    ),
            )
    }
}

fn tag_color_dot(color: Option<&str>, cx: &mut App) -> impl IntoElement {
    let fill = color
        .and_then(parse_hex_color)
        .unwrap_or(cx.theme().primary);
    div().size(px(9.)).rounded_full().bg(fill)
}

fn format_bytes_label(bytes: u64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = bytes as f64;
    let mut unit = 0usize;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{} {}", bytes, UNITS[0])
    } else {
        format!("{value:.1} {UN}", UN = UNITS[unit])
    }
}

fn parse_hex_color(s: &str) -> Option<Hsla> {
    let hex = s.trim().trim_start_matches('#');
    match hex.len() {
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some(gpui::rgb(((r as u32) << 16) | ((g as u32) << 8) | (b as u32)).into())
        }
        _ => None,
    }
}

fn open_path(path: &PathBuf, event: &ClickEvent, _window: &Window, cx: &mut App) {
    if event.click_count() < 2 {
        return;
    }
    if event.modifiers().control {
        AppNavigation::open_path_in_new_tab(path.clone(), cx);
    } else {
        AppNavigation::navigate_to_path(path.clone(), cx);
    }
}

fn format_system_time(time: Option<SystemTime>) -> String {
    let Some(time) = time else {
        return String::new();
    };
    let local_time: DateTime<Local> = time.into();
    local_time.format("%Y-%m-%d %H:%M").to_string()
}

fn drive_context_menu(
    menu: PopupMenu,
    drive: &DriveInfo,
    window: &mut Window,
    cx: &mut App,
) -> PopupMenu {
    let path = drive.path.clone();
    let is_pinned = false;
    let can_eject = drive.is_removable || drive.is_network;
    let eject_drive_info = drive.clone();
    let mut menu = folder_context_menu(menu, &path, is_pinned, window, cx);
    if can_eject {
        let label = if drive.is_network {
            t!("home.menu.disconnect")
        } else {
            t!("home.menu.eject")
        };
        menu = menu.item(PopupMenuItem::new(label).on_click(move |_, window, cx| {
            match eject_drive(&eject_drive_info) {
                Ok(()) => {
                    AppNavigation::refresh_quick_access(cx);
                    window.push_notification(Notification::success(t!("home.eject.done")), cx);
                }
                Err(error) => {
                    window.push_notification(
                        Notification::error(format!("{}: {error}", t!("home.eject.failed"))),
                        cx,
                    );
                }
            }
            cx.stop_propagation();
        }));
    }
    if !drive.is_removable && !drive.is_network && drive.total_bytes.is_some() {
        menu = menu.item(PopupMenuItem::new(t!("home.menu.storage_sense")).on_click(
            move |_, _, cx| {
                if let Err(error) = open_storage_sense_settings() {
                    tracing::warn!(target: "home", error = ?error, "failed to open storage sense");
                }
                cx.stop_propagation();
            },
        ));
    }
    menu
}

fn folder_context_menu(
    menu: PopupMenu,
    path: &PathBuf,
    is_pinned: bool,
    _window: &mut Window,
    cx: &mut App,
) -> PopupMenu {
    let path_string = path.to_string_lossy().to_string();
    let path_open = path.clone();
    let path_tab = path.clone();
    let path_pin = path.clone();
    let path_unpin = path_string.clone();
    let path_props = path.clone();
    let mut menu = menu.item(PopupMenuItem::new(t!("sidebar.menu.open")).on_click(
        move |_, _, cx| {
            AppNavigation::navigate_to_path(path_open.clone(), cx);
        },
    ));
    menu = menu.item(
        PopupMenuItem::new(t!("sidebar.menu.open_new_tab")).on_click(move |_, _, cx| {
            AppNavigation::open_path_in_new_tab(path_tab.clone(), cx);
        }),
    );
    if crate::shell::preferences::show_open_in_new_pane(cx) {
        let path_pane = path.clone();
        menu = menu.item(
            PopupMenuItem::new(t!("files.menu.open_in_new_pane")).on_click(move |_, _, cx| {
                AppNavigation::open_path_in_secondary_pane(path_pane.clone(), cx);
                cx.stop_propagation();
            }),
        );
    }
    if is_pinned {
        menu = menu.item(
            PopupMenuItem::new(t!("sidebar.menu.unpin")).on_click(move |_, _, cx| {
                AppNavigation::unpin_folder(&path_unpin, cx);
            }),
        );
    } else {
        menu = menu.item(
            PopupMenuItem::new(t!("sidebar.menu.pin")).on_click(move |_, _, cx| {
                AppNavigation::pin_folder(path_pin.clone(), cx);
            }),
        );
    }
    menu.item(
        PopupMenuItem::new(t!("files.menu.properties")).on_click(move |_, _, cx| {
            let _ = open_item_properties(path_props.as_path());
            cx.stop_propagation();
        }),
    )
}

fn file_context_menu(
    menu: PopupMenu,
    path: &PathBuf,
    _window: &mut Window,
    _cx: &mut App,
) -> PopupMenu {
    let path_open = path.clone();
    let path_tab = path.clone();
    let path_props = path.clone();
    menu.item(
        PopupMenuItem::new(t!("sidebar.menu.open")).on_click(move |_, _, cx| {
            AppNavigation::navigate_to_path(path_open.clone(), cx);
        }),
    )
    .item(
        PopupMenuItem::new(t!("sidebar.menu.open_new_tab")).on_click(move |_, _, cx| {
            AppNavigation::open_path_in_new_tab(path_tab.clone(), cx);
        }),
    )
    .item(
        PopupMenuItem::new(t!("files.menu.properties")).on_click(move |_, _, cx| {
            let _ = open_item_properties(path_props.as_path());
            cx.stop_propagation();
        }),
    )
}
