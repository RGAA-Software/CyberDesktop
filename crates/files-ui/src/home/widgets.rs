//! Home page widget bodies (Files `*Widget` parity).

use std::path::PathBuf;
use std::time::SystemTime;

use chrono::{DateTime, Local};
use files_fs::{
    eject_drive, open_storage_sense_settings, recent_documents_enabled, DriveInfo, FileTagPreview,
    QuickAccessEntry, RecentItem,
};
use app_platform_windows::open_item_properties;
use gpui::{prelude::*, MouseButton, MouseDownEvent, *};
use gpui_component::{
    alert::Alert,
    button::{Button, ButtonVariants as _},
    h_flex,
    label::Label,
    notification::Notification,
    v_flex, ActiveTheme as _, IconName, Sizable as _, WindowExt as _,
};
use rust_i18n::t;

use crate::app_state::AppNavigation;
use crate::home::page::HomePage;
use crate::home::widget_shell::{
    block_home_page_context_menu, card_grid, space_progress_bar, CARD_MIN_HEIGHT, CARD_WIDTH,
    FOLDER_CARD_HEIGHT, FOLDER_CARD_WIDTH,
};
use crate::icons::{inline_icon, pin_icon};
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
        expanded: bool,
        section_key: &'static str,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let chevron = if expanded {
            IconName::ChevronDown
        } else {
            IconName::ChevronRight
        };
        Button::new(id)
            .ghost()
            .small()
            .w_full()
            .child(
                h_flex()
                    .w_full()
                    .gap_2()
                    .items_center()
                    .text_color(cx.theme().foreground)
                    .child(crate::icons::icon_foreground(chevron, cx))
                    .child(icon)
                    .child(
                        Label::new(title)
                            .text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(cx.theme().foreground),
                    ),
            )
            .on_click(cx.listener(move |this, _, _, cx| {
                this.toggle_expanded(section_key, cx);
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    this.open_section_menu(section_key, event.position, window, cx);
                }),
            )
    }

    pub(super) fn render_quick_access_widget(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        entries: &[QuickAccessEntry],
    ) -> impl IntoElement {
        let expanded = self.prefs.quick_access_expanded;
        block_home_page_context_menu(
            v_flex()
                .id("home-widget-quick-access")
                .w_full()
                .gap_1()
                .child(self.section_header(
                    "home-qa-header",
                    pin_icon(),
                    t!("home.widget.quick_access"),
                    expanded,
                    "quick_access",
                    cx,
                ))
                .when(expanded, |body| {
                    body.when(entries.is_empty(), |b| {
                        b.child(Alert::info(
                            "home-quick-access-empty",
                            t!("home.widget.quick_access.empty").to_string(),
                        ))
                    })
                    .when(!entries.is_empty(), |b| {
                        b.child(card_grid(entries.iter().enumerate().map(
                            |(index, entry)| {
                                self.folder_card(window, index, "home-qa", entry, cx)
                                    .into_any_element()
                            },
                        )))
                    })
                }),
        )
    }

    pub(super) fn render_drives_widget(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        drives: &[DriveInfo],
    ) -> impl IntoElement {
        let expanded = self.prefs.drives_expanded;
        block_home_page_context_menu(
            v_flex()
                .id("home-widget-drives")
                .w_full()
                .gap_1()
                .child(self.section_header(
                    "home-drives-header",
                    inline_icon(IconName::HardDrive),
                    t!("home.widget.drives"),
                    expanded,
                    "drives",
                    cx,
                ))
                .when(expanded, |body| {
                    body.child(card_grid(drives.iter().enumerate().map(
                        |(index, drive)| {
                            self.drive_card(window, index, "home-drive", drive, cx)
                                .into_any_element()
                        },
                    )))
                }),
        )
    }

    pub(super) fn render_network_widget(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        entries: &[NetworkEntry],
    ) -> impl IntoElement {
        let expanded = self.prefs.network_expanded;
        block_home_page_context_menu(
            v_flex()
                .id("home-widget-network")
                .w_full()
                .gap_1()
                .child(self.section_header(
                    "home-network-header",
                    inline_icon(IconName::Globe),
                    t!("home.widget.network"),
                    expanded,
                    "network",
                    cx,
                ))
                .when(expanded, |body| {
                    body.when(entries.is_empty(), |b| {
                        b.child(Alert::info(
                            "home-network-empty",
                            t!("home.widget.network.empty").to_string(),
                        ))
                    })
                    .when(!entries.is_empty(), |b| {
                        b.child(card_grid(entries.iter().enumerate().map(
                            |(index, entry)| {
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
                            },
                        )))
                    })
                }),
        )
    }

    pub(super) fn render_file_tags_widget(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        previews: &[FileTagPreview],
    ) -> impl IntoElement {
        let expanded = self.prefs.file_tags_expanded;
        block_home_page_context_menu(
            v_flex()
                .id("home-widget-tags")
                .w_full()
                .gap_1()
                .child(self.section_header(
                    "home-tags-header",
                    inline_icon(IconName::Inbox),
                    t!("home.widget.tags"),
                    expanded,
                    "file_tags",
                    cx,
                ))
                .when(expanded, |body| {
                    body.when(previews.is_empty(), |b| {
                        b.child(Alert::info(
                            "home-tags-empty",
                            t!("home.widget.tags.empty").to_string(),
                        ))
                    })
                    .when(!previews.is_empty(), |b| {
                        b.child(card_grid(previews.iter().enumerate().map(
                            |(index, preview)| {
                                self.tag_container(window, index, preview, cx)
                                    .into_any_element()
                            },
                        )))
                    })
                }),
        )
    }

    pub(super) fn render_recent_widget(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        recent: &[RecentItem],
    ) -> impl IntoElement {
        let expanded = self.prefs.recent_expanded;
        block_home_page_context_menu(
            v_flex()
                .id("home-widget-recent")
                .w_full()
                .gap_1()
                .child(self.section_header(
                    "home-recent-header",
                    inline_icon(IconName::Calendar),
                    t!("home.widget.recent"),
                    expanded,
                    "recent",
                    cx,
                ))
                .when(expanded, |body| {
                    body.when(!recent_documents_enabled(), |b| {
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
                    .when(
                        recent_documents_enabled() && !recent.is_empty(),
                        |b| {
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
                        },
                    )
                }),
        )
    }

    fn folder_card(
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
        self.ensure_home_thumbnail(&path, 32., window, cx);
        Button::new(SharedString::from(format!("{prefix}-folder-{index}")))
            .ghost()
            .w(FOLDER_CARD_WIDTH)
            .min_h(FOLDER_CARD_HEIGHT)
            .child(
                v_flex()
                    .items_center()
                    .justify_center()
                    .gap_1()
                    .child(
                        div()
                            .relative()
                            .child(self.home_card_image(&path, px(32.), window))
                            .when(pinned, |el| {
                                el.child(
                                    div()
                                        .absolute()
                                        .top_0()
                                        .right_0()
                                        .child(crate::icons::pin_icon()),
                                )
                            }),
                    )
                    .child(Label::new(label).text_sm().text_center()),
            )
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
        let space = drive.space_text();
        let frac = drive.used_fraction();
        self.ensure_home_thumbnail(&path, 32., window, cx);
        Button::new(SharedString::from(format!("{prefix}-drive-{index}")))
            .ghost()
            .w(CARD_WIDTH)
            .min_h(CARD_MIN_HEIGHT)
            .child(
                h_flex()
                    .w_full()
                    .gap_2()
                    .child(self.home_card_image(&path, px(32.), window))
                    .child(
                        v_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_1()
                            .child(
                                Label::new(title)
                                    .text_sm()
                                    .font_weight(gpui::FontWeight::MEDIUM),
                            )
                            .when_some(frac, |col, f| {
                                col.child(space_progress_bar(
                                    SharedString::from(format!("{prefix}-bar-{index}")),
                                    f,
                                ))
                            })
                            .when_some(space, |col, text| {
                                col.child(
                                    Label::new(text)
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground),
                                )
                            }),
                    ),
            )
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
    }

    fn recent_table_header(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .h_8()
            .px_3()
            .gap_3()
            .items_center()
            .bg(cx.theme().muted)
            .text_sm()
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
            .h_9()
            .flex_none()
            .px_3()
            .gap_3()
            .items_center()
            .border_b_1()
            .border_color(cx.theme().border)
            .hover(|this| this.bg(cx.theme().accent))
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
        div()
            .id(("home-tag-container", index))
            .w(CARD_WIDTH)
            .min_h(px(160.))
            .flex_none()
            .rounded(px(6.))
            .border_1()
            .border_color(cx.theme().border)
            .child(
                v_flex()
                    .w_full()
                    .child(
                        h_flex()
                            .w_full()
                            .px_2()
                            .py_1()
                            .border_b_1()
                            .border_color(cx.theme().border)
                            .items_center()
                            .child(
                                Button::new(("home-tag-view", index))
                                    .ghost()
                                    .small()
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .items_center()
                                            .child(tag_color_dot(preview.tag.color.as_deref(), cx))
                                            .child(
                                                Label::new(tag_name)
                                                    .text_sm()
                                                    .font_weight(gpui::FontWeight::SEMIBOLD),
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
                            .p_1()
                            .gap_px()
                            .when(preview.preview_items.is_empty(), |col| {
                                col.child(
                                    Label::new(t!("home.widget.tags.preview.empty"))
                                        .text_xs()
                                        .text_color(cx.theme().muted_foreground),
                                )
                            })
                            .children(preview.preview_items.iter().enumerate().map(
                                |(row, (name, file_path))| {
                                    let open = file_path.clone();
                                    Button::new(SharedString::from(format!(
                                        "home-tag-file-{index}-{row}"
                                    )))
                                    .ghost()
                                    .w_full()
                                    .child(
                                        h_flex()
                                            .gap_2()
                                            .items_center()
                                            .child(shell_icon_for_path(file_path, px(16.), window))
                                            .child(Label::new(name.clone()).text_sm().truncate()),
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
    div().size(px(10.)).rounded_full().bg(fill)
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
    _cx: &mut App,
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
