use super::*;
use super::helpers::view_supports_grouping;
use gpui_component::{
    button::Button,
    Icon,
};

const ACTION_BAR_HEIGHT: Pixels = px(48.);

fn cmd_separator(cx: &App) -> impl IntoElement {
    div()
        .flex_none()
        .w(px(1.))
        .h(px(22.))
        .bg(cx.theme().border)
        .mx(px(6.))
}

fn icon_action_button(id: impl Into<ElementId>) -> Button {
    toolbar_icon_button(id)
        .h(px(32.))
        .w(px(32.))
        .rounded(px(10.))
}

impl FileBrowser {
    fn file_tag_is_empty(&self) -> bool {
        matches!(self.browse_location, BrowseLocation::FileTag { .. }) && self.items.is_empty()
    }

    fn render_file_tag_empty_state(&self, cx: &App) -> impl IntoElement {
        v_flex()
            .id("file-tag-empty")
            .size_full()
            .items_center()
            .justify_center()
            .gap_3()
            .child(
                div()
                    .text_color(cx.theme().muted_foreground)
                    .opacity(0.55)
                    .child(file_tag_empty_icon_element(cx)),
            )
            .child(
                Label::new(t!("file_tag.empty"))
                    .text_sm()
                    .text_color(cx.theme().muted_foreground),
            )
    }

    fn view_mode_button(
        &self,
        id: &'static str,
        mode: ViewMode,
        icon: Icon,
        tooltip: impl Into<SharedString>,
        cx: &mut Context<Self>,
    ) -> Button {
        let active = self.view_mode == mode;
        toolbar_icon_button(id)
            .h(px(32.))
            .w(px(32.))
            .rounded(px(8.))
            .icon(icon)
            .tooltip(tooltip)
            .when(active, |btn| {
                btn.bg(cx.theme().background)
                    .border_1()
                    .border_color(cx.theme().primary)
                    .text_color(cx.theme().accent_foreground)
            })
            .on_click(cx.listener(move |this, _, _, cx| {
                this.set_view_mode(mode, cx);
            }))
    }

    fn render_view_mode_group(&self, cx: &mut Context<Self>) -> impl IntoElement {
        h_flex()
            .id("view-mode-group")
            .flex_none()
            .gap(px(4.))
            .p(px(3.))
            .rounded(px(11.))
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().secondary)
            .child(self.view_mode_button(
                "view-details",
                ViewMode::Details,
                toolbar_tabler(tabler_icons::LIST_DETAILS),
                t!("files.view.details"),
                cx,
            ))
            .child(self.view_mode_button(
                "view-list",
                ViewMode::List,
                toolbar_icon(IconName::PanelLeftOpen),
                t!("files.view.list"),
                cx,
            ))
            .child(self.view_mode_button(
                "view-grid",
                ViewMode::Grid,
                toolbar_tabler(tabler_icons::LAYOUT_GRID),
                t!("files.view.grid"),
                cx,
            ))
            .child(self.view_mode_button(
                "view-cards",
                ViewMode::Cards,
                toolbar_tabler(tabler_icons::LAYOUT_BOARD),
                t!("files.view.cards"),
                cx,
            ))
            .child(self.view_mode_button(
                "view-columns",
                ViewMode::Columns,
                toolbar_tabler(tabler_icons::COLUMNS_3),
                t!("files.view.columns"),
                cx,
            ))
    }

    /// Action bar above the file list (new, views, clipboard, delete, sort).
    fn render_content_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let selected_count = self.selected_count();
        let in_recycle_bin = self.browse_location == BrowseLocation::RecycleBin;
        let show_hidden = self.read_options.show_hidden_items;
        let show_file_extensions = self.read_options.show_file_extensions;
        let sort_label = self.sort_label();
        let sort = self.sort_preferences;
        let group = self.group_option;
        let group_date_unit = self.group_date_unit;
        let grouping_available = view_supports_grouping(self.view_mode);
        let can_paste = AppFileClipboard::has_items(cx);

        h_flex()
            .id("action-bar")
            .w_full()
            .flex_none()
            .h(ACTION_BAR_HEIGHT)
            .min_h(ACTION_BAR_HEIGHT)
            .gap(px(5.))
            .px(px(16.))
            .items_center()
            .border_b_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .when(!in_recycle_bin, |bar| {
                bar.child(
                    icon_action_button("action-new-folder")
                        .icon(toolbar_tabler(tabler_icons::FOLDER_PLUS))
                        .tooltip(t!("files.new_folder"))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.create_new_folder(window, cx);
                        })),
                )
                .child(
                    icon_action_button("action-new-file")
                        .icon(toolbar_tabler(tabler_icons::FILE_PLUS))
                        .tooltip(t!("files.new_file"))
                        .on_click(cx.listener(|this, _, window, cx| {
                            this.create_new_file(window, cx);
                        })),
                )
            })
            .child(self.render_view_mode_group(cx))
            .child(cmd_separator(cx))
            .child(
                icon_action_button("action-copy")
                    .icon(toolbar_tabler(tabler_icons::COPY))
                    .tooltip(t!("files.menu.copy"))
                    .disabled(selected_count == 0)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.copy_items(cx);
                        cx.notify();
                    })),
            )
            .child(
                icon_action_button("action-cut")
                    .icon(toolbar_tabler(tabler_icons::CUT))
                    .tooltip(t!("files.menu.cut"))
                    .disabled(selected_count == 0)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.cut_items(cx);
                        cx.notify();
                    })),
            )
            .child(
                icon_action_button("action-paste")
                    .icon(toolbar_tabler(tabler_icons::CLIPBOARD))
                    .tooltip(t!("files.menu.paste"))
                    .disabled(!can_paste)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.paste_items(window, cx);
                    })),
            )
            .child(
                icon_action_button("action-rename")
                    .icon(toolbar_tabler(tabler_icons::PENCIL))
                    .tooltip(t!("files.menu.rename"))
                    .disabled(selected_count == 0)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.begin_rename(window, cx);
                        cx.notify();
                    })),
            )
            .child(
                icon_action_button("action-properties")
                    .icon(toolbar_icon(IconName::Info))
                    .tooltip(t!("files.menu.properties"))
                    .disabled(selected_count == 0)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.show_properties(cx);
                    })),
            )
            .child(cmd_separator(cx))
            .child(
                icon_action_button("action-delete")
                    .icon(toolbar_icon(IconName::Delete))
                    .tooltip(t!("files.menu.delete"))
                    .disabled(selected_count == 0)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.perform_delete(window, cx);
                        cx.notify();
                    })),
            )
            .child(div().flex_1().min_w_0())
            .child(
                h_flex()
                    .id("sort-area")
                    .flex_none()
                    .gap(px(8.))
                    .items_center()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(Label::new(t!("files.menu.sort")))
                    .child(
                        toolbar_dropdown_button("action-sort")
                            .button(
                                toolbar_labeled_button("action-sort-btn")
                                    .px(px(10.))
                                    .tooltip(t!("files.menu.sort"))
                                    .h(px(32.))
                                    .rounded(px(10.))
                                    .border_1()
                                    .border_color(cx.theme().border)
                                    .bg(cx.theme().secondary)
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_semibold()
                                            .text_color(cx.theme().foreground)
                                            .child(sort_label),
                                    ),
                            )
                            .dropdown_menu(move |menu, window, cx| {
                                build_sort_prefs_toolbar_menu(
                                    menu,
                                    sort,
                                    group,
                                    group_date_unit,
                                    show_hidden,
                                    show_file_extensions,
                                    false,
                                    grouping_available,
                                    window,
                                    cx,
                                )
                            }),
                    ),
            )
    }
}

impl Render for FileBrowser {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.watched_dir.as_ref() != Some(&self.current_dir) {
            self.watched_dir = Some(self.current_dir.clone());
            self.restart_directory_watcher(cx);
        }

        let viewport_width = window.viewport_size().width;
        if self.last_viewport_width != Some(viewport_width) {
            self.last_viewport_width = Some(viewport_width);
            self.grid_cells_per_row = None;
            self.cards_cells_per_row = None;
        }

        let current_dir = self.current_dir.to_string_lossy().to_string();
        let can_go_back = !self.back_stack.is_empty();
        let can_go_forward = !self.forward_stack.is_empty();
        let can_go_up = self.current_dir.parent().is_some();
        let selected_count = self.selected_count();
        let show_hidden = self.read_options.show_hidden_items;
        let show_file_extensions = self.read_options.show_file_extensions;
        let sort_label = self.sort_label();
        let sort = self.sort_preferences;
        let group = self.group_option;
        let group_date_unit = self.group_date_unit;
        let grouping_available = view_supports_grouping(self.view_mode);
        let in_recycle_bin = self.browse_location == BrowseLocation::RecycleBin;
        let in_search_results = matches!(self.browse_location, BrowseLocation::SearchResults { .. });
        let recycle_item_count = if in_recycle_bin {
            self.items.len()
        } else {
            0
        };

        let page_gap = if self.show_content_toolbar && !self.show_toolbar {
            px(0.)
        } else {
            px(12.)
        };

        v_flex()
            .id("files-page")
            .size_full()
            .min_h_0()
            .gap(page_gap)
            .track_focus(&self.focus_handle)
            .key_context(FILE_BROWSER)
            .on_action(cx.listener(Self::on_refresh))
            .on_action(cx.listener(Self::on_open_item))
            .on_action(cx.listener(Self::on_select_all))
            .on_action(cx.listener(Self::on_rename))
            .on_action(cx.listener(Self::on_cancel_rename))
            .on_action(cx.listener(Self::on_undo))
            .on_action(cx.listener(Self::on_redo))
            .on_action(cx.listener(Self::on_delete))
            .on_action(cx.listener(Self::on_delete_permanent))
            .on_action(cx.listener(Self::on_restore_recycle_items))
            .on_action(cx.listener(Self::on_restore_all_recycle_items))
            .on_action(cx.listener(Self::on_empty_recycle_bin))
            .on_action(cx.listener(Self::on_new_folder))
            .on_action(cx.listener(Self::on_new_file))
            .on_action(cx.listener(Self::on_view_details))
            .on_action(cx.listener(Self::on_view_list))
            .on_action(cx.listener(Self::on_view_grid))
            .on_action(cx.listener(Self::on_view_cards))
            .on_action(cx.listener(Self::on_view_columns))
            .on_action(cx.listener(Self::on_focus_search_action))
            .on_action(cx.listener(Self::on_shell_properties))
            .on_drop(cx.listener(|this, paths: &DraggedFilePaths, window, cx| {
                this.handle_drop(paths.0.clone(), window, cx);
            }))
            .on_drop(cx.listener(|this, paths: &ExternalPaths, window, cx| {
                this.handle_external_drop(paths, window, cx);
            }))
            .on_action(cx.listener(Self::on_copy_path))
            .on_action(cx.listener(Self::on_copy_items))
            .on_action(cx.listener(Self::on_cut_items))
            .on_action(cx.listener(Self::on_paste_items))
            .on_action(cx.listener(Self::on_compress_items))
            .on_action(cx.listener(Self::on_extract_here))
            .on_action(cx.listener(Self::on_extract_to_folder))
            .on_action(cx.listener(Self::on_navigate_previous))
            .on_action(cx.listener(Self::on_navigate_next))
            .on_action(cx.listener(Self::on_navigate_left))
            .on_action(cx.listener(Self::on_navigate_right))
            .on_action(cx.listener(Self::on_sort_name))
            .on_action(cx.listener(Self::on_sort_created))
            .on_action(cx.listener(Self::on_sort_modified))
            .on_action(cx.listener(Self::on_sort_size))
            .on_action(cx.listener(Self::on_sort_type))
            .on_action(cx.listener(Self::on_sort_tag))
            .on_action(cx.listener(Self::on_sort_path))
            .on_action(cx.listener(Self::on_group_none))
            .on_action(cx.listener(Self::on_group_name))
            .on_action(cx.listener(Self::on_group_modified_year))
            .on_action(cx.listener(Self::on_group_modified_month))
            .on_action(cx.listener(Self::on_group_modified_day))
            .on_action(cx.listener(Self::on_group_created_year))
            .on_action(cx.listener(Self::on_group_created_month))
            .on_action(cx.listener(Self::on_group_created_day))
            .on_action(cx.listener(Self::on_group_size))
            .on_action(cx.listener(Self::on_group_type))
            .on_action(cx.listener(Self::on_group_tag))
            .on_action(cx.listener(Self::on_toggle_sort_direction))
            .on_action(cx.listener(Self::on_sort_ascending))
            .on_action(cx.listener(Self::on_sort_descending))
            .on_action(cx.listener(Self::on_toggle_show_hidden))
            .on_action(cx.listener(Self::on_toggle_show_file_extensions))
            .on_action(cx.listener(Self::on_open_in_new_pane))
            .on_action(cx.listener(Self::on_open_in_terminal))
            .on_action(cx.listener(Self::on_create_folder_from_selection))
            .on_action(cx.listener(Self::on_open_in_new_window))
            .on_action(cx.listener(Self::on_open_with_dialog))
            .on_action(cx.listener(Self::on_create_shortcut))
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, _window, cx| {
                let key = event.keystroke.key.as_str();
                if key.len() == 1 {
                    let ch = key.chars().next().unwrap();
                    if ch.is_alphanumeric() {
                        this.handle_key_char(ch, cx);
                        cx.notify();
                    }
                }
            }))
            .when(self.show_content_toolbar, |this| {
                this.child(self.render_content_toolbar(cx))
            })
            .when(self.show_toolbar, |this| {
                this.child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .flex_wrap()
                        .child(
                            toolbar_icon_button("files-back")
                                .icon(toolbar_icon(IconName::ArrowLeft))
                                .tooltip(t!("nav.back"))
                                .disabled(!can_go_back)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.navigate_back(cx);
                                })),
                        )
                        .child(
                            toolbar_icon_button("files-forward")
                                .icon(toolbar_icon(IconName::ArrowRight))
                                .tooltip(t!("nav.forward"))
                                .disabled(!can_go_forward)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.navigate_forward(cx);
                                })),
                        )
                        .child(
                            toolbar_icon_button("files-up")
                                .icon(toolbar_icon(IconName::ArrowUp))
                                .tooltip(t!("nav.up"))
                                .disabled(!can_go_up)
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.navigate_parent(cx);
                                })),
                        )
                        .child(
                            toolbar_icon_button("files-refresh")
                                .icon(toolbar_icon(IconName::Redo2))
                                .tooltip(t!("nav.refresh"))
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.refresh();
                                    cx.notify();
                                })),
                        )
                        .when(in_recycle_bin, |this| {
                            this.child(
                                toolbar_labeled_button("files-restore-all-btn")
                                    .label(t!("files.recycle.restore_all"))
                                    .tooltip(t!("files.recycle.restore_all"))
                                    .disabled(recycle_item_count == 0)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.perform_restore_all_recycle(window, cx);
                                        cx.notify();
                                    })),
                            )
                            .child(
                                toolbar_labeled_button("files-empty-recycle-btn")
                                    .label(t!("files.recycle.empty"))
                                    .tooltip(t!("files.recycle.empty"))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.confirm_empty_recycle_bin(window, cx);
                                        cx.notify();
                                    })),
                            )
                        })
                        .when(!self.show_content_toolbar && !in_recycle_bin, |this| {
                            this.child(
                                toolbar_icon_button("files-new-folder-btn")
                                    .size(TOOLBAR_BUTTON_PX)
                                    .icon(toolbar_tabler(tabler_icons::FOLDER_PLUS))
                                    .tooltip(t!("files.new_folder"))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.create_new_folder(window, cx);
                                        cx.notify();
                                    })),
                            )
                            .child(
                                toolbar_icon_button("files-new-file-btn")
                                    .size(TOOLBAR_BUTTON_PX)
                                    .icon(toolbar_tabler(tabler_icons::FILE_PLUS))
                                    .tooltip(t!("files.new_file"))
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.create_new_file(window, cx);
                                        cx.notify();
                                    })),
                            )
                        })
                        .when(!self.show_content_toolbar, |this| {
                            this.child(
                                toolbar_icon_button("files-view-details")
                                    .icon(toolbar_tabler(tabler_icons::LIST_DETAILS))
                                    .tooltip(t!("files.view.details"))
                                    .when(self.view_mode == ViewMode::Details, |btn| {
                                        btn.bg(cx.theme().accent)
                                            .text_color(cx.theme().accent_foreground)
                                    })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.set_view_mode(ViewMode::Details, cx);
                                    })),
                            )
                            .child(
                                toolbar_icon_button("files-view-list")
                                    .icon(toolbar_icon(IconName::PanelLeftOpen))
                                    .tooltip(t!("files.view.list"))
                                    .when(self.view_mode == ViewMode::List, |btn| {
                                        btn.bg(cx.theme().accent)
                                            .text_color(cx.theme().accent_foreground)
                                    })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.set_view_mode(ViewMode::List, cx);
                                    })),
                            )
                            .child(
                                toolbar_icon_button("files-view-grid")
                                    .icon(toolbar_tabler(tabler_icons::LAYOUT_GRID))
                                    .tooltip(t!("files.view.grid"))
                                    .when(self.view_mode == ViewMode::Grid, |btn| {
                                        btn.bg(cx.theme().accent)
                                            .text_color(cx.theme().accent_foreground)
                                    })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.set_view_mode(ViewMode::Grid, cx);
                                    })),
                            )
                            .child(
                                toolbar_icon_button("files-view-cards")
                                    .icon(toolbar_tabler(tabler_icons::LAYOUT_BOARD))
                                    .tooltip(t!("files.view.cards"))
                                    .when(self.view_mode == ViewMode::Cards, |btn| {
                                        btn.bg(cx.theme().accent)
                                            .text_color(cx.theme().accent_foreground)
                                    })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.set_view_mode(ViewMode::Cards, cx);
                                    })),
                            )
                            .child(
                                toolbar_icon_button("files-view-columns")
                                    .icon(toolbar_tabler(tabler_icons::COLUMNS_3))
                                    .tooltip(t!("files.view.columns"))
                                    .when(self.view_mode == ViewMode::Columns, |btn| {
                                        btn.bg(cx.theme().accent)
                                            .text_color(cx.theme().accent_foreground)
                                    })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.set_view_mode(ViewMode::Columns, cx);
                                    })),
                            )
                            .child(
                                toolbar_icon_button("files-delete-btn")
                                    .icon(toolbar_icon(IconName::Delete))
                                    .tooltip(t!("files.menu.delete"))
                                    .disabled(selected_count == 0)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.perform_delete(window, cx);
                                        cx.notify();
                                    })),
                            )
                            .child(
                                toolbar_dropdown_button("files-sort")
                                    .button(
                                        toolbar_labeled_button("files-sort-btn")
                                            .label(sort_label)
                                            .tooltip(t!("files.menu.sort")),
                                    )
                                    .dropdown_menu(move |menu, window, cx| {
                                        build_sort_prefs_toolbar_menu(
                                            menu,
                                            sort,
                                            group,
                                            group_date_unit,
                                            show_hidden,
                                            show_file_extensions,
                                            false,
                                            grouping_available,
                                            window,
                                            cx,
                                        )
                                    }),
                            )
                        })
                        .child(
                            div()
                                .flex_1()
                                .min_w(px(120.))
                                .px_3()
                                .py_1()
                                .rounded(cx.theme().radius)
                                .border_1()
                                .border_color(cx.theme().border)
                                .text_color(cx.theme().muted_foreground)
                                .overflow_hidden()
                                .text_ellipsis()
                                .child(current_dir),
                        ),
                )
            })
            .when_some(self.error.as_ref(), |this, error| {
                this.when(!self.file_tag_is_empty(), |this| {
                    this.child(
                        div()
                            .px_3()
                            .py_2()
                            .rounded(cx.theme().radius)
                            .border_1()
                            .border_color(cx.theme().danger)
                            .text_color(cx.theme().danger)
                            .child(error.clone()),
                    )
                })
            })
            .when(in_recycle_bin, |this| {
                this.child(
                    div()
                        .px_3()
                        .py_2()
                        .rounded(cx.theme().radius)
                        .bg(cx.theme().muted)
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(t!("files.recycle.hint")),
                )
            })
            .when(in_search_results, |this| {
                this.child(
                    div()
                        .px_3()
                        .py_2()
                        .rounded(cx.theme().radius)
                        .bg(cx.theme().muted)
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(t!("search.results.hint")),
                )
            })
            .child(
                div()
                    .id("files-list-host")
                    .flex_1()
                    .min_h_0()
                    .size_full()
                    .overflow_hidden()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, event: &MouseDownEvent, _, cx| {
                            this.cancel_rename_if_active(cx);
                            Self::dismiss_main_page_path_edit_if_active(cx);
                            this.begin_sweep_selection(
                                SweepSelectionSurface::Main,
                                event.position,
                                event.modifiers,
                                cx,
                            );
                            cx.stop_propagation();
                        }),
                    )
                    .on_mouse_move(cx.listener(|this, event: &MouseMoveEvent, _, cx| {
                        this.update_sweep_pointer(SweepSelectionSurface::Main, event.position, cx);
                    }))
                    .on_prepaint({
                        let entity = cx.entity().clone();
                        move |bounds, _window, cx| {
                            let _ = entity.update(cx, |this, _cx| {
                                this.main_sweep_bounds = Some(bounds);
                            });
                        }
                    })
                    .on_mouse_up(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| {
                            this.finish_sweep_selection();
                            if cx.has_active_drag() {
                                cx.stop_active_drag(window);
                            }
                            this.end_drag_session(cx);
                            this.clear_native_drag_out();
                        }),
                    )
                    .on_mouse_up_out(
                        MouseButton::Left,
                        cx.listener(|this, _, window, cx| {
                            this.finish_sweep_selection();
                            if cx.has_active_drag() {
                                cx.stop_active_drag(window);
                            }
                            this.end_drag_session(cx);
                            this.clear_native_drag_out();
                        }),
                    )
                    .on_drag_move::<DraggedFilePaths>(cx.listener(
                        |this, event: &DragMoveEvent<DraggedFilePaths>, window, cx| {
                            let paths = event.drag(cx).0.clone();
                            this.update_drag_hover_at_position(
                                event.event.position,
                                &paths,
                                event.bounds,
                                window,
                                cx,
                            );
                        },
                    ))
                    .on_drag_move::<ExternalPaths>(cx.listener(
                        |this, event: &DragMoveEvent<ExternalPaths>, window, cx| {
                            let paths: Vec<PathBuf> = event.drag(cx).paths().to_vec();
                            this.update_external_drag_hover_at_position(
                                event.event.position,
                                &paths,
                                event.bounds,
                                window,
                                cx,
                            );
                        },
                    ))
                    .on_drop(cx.listener(|this, paths: &ExternalPaths, window, cx| {
                        this.handle_external_drop(paths, window, cx);
                    }))
                    .drag_over::<ExternalPaths>(|host, _, _, cx| {
                        host.bg(cx.theme().primary.opacity(0.08))
                    })
                    .on_mouse_down(
                        MouseButton::Middle,
                        cx.listener(|this, _, _, cx| {
                            this.cancel_rename_if_active(cx);
                            Self::dismiss_main_page_path_edit_if_active(cx);
                            this.clear_selection();
                            cx.notify();
                        }),
                    )
                    .on_mouse_down(
                        MouseButton::Right,
                        cx.listener(|this, event: &MouseDownEvent, window, cx| {
                            this.cancel_rename_if_active(cx);
                            Self::dismiss_main_page_path_edit_if_active(cx);
                            this.clear_selection();
                            this.set_context_menu_extended_verbs(event.modifiers.shift);
                            this.open_context_menu(event.position, window, cx);
                        }),
                    )
                    .when(self.file_tag_is_empty(), |this| {
                        this.child(self.render_file_tag_empty_state(cx))
                    })
                    .when(!self.file_tag_is_empty(), |this| {
                        this.child(self.file_list(window, cx))
                            .when_some(self.render_main_sweep_overlay(cx), |this, overlay| {
                                this.child(overlay)
                            })
                    }),
            )
            .when(self.context_menu_open, |this| {
                this.child(self.render_context_menu_overlay(window))
            })
    }
}
