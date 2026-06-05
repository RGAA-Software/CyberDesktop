use super::super::group_labels::localized_group_title;
use super::*;

impl FileBrowser {
    fn sweep_gutter() -> impl IntoElement {
        div()
            .id("files-list-sweep-gutter")
            .w(SWEEP_GUTTER_WIDTH)
            .flex_shrink_0()
            .h_full()
    }

    fn table_list_body(
        &self,
        cx: &mut Context<Self>,
        details: bool,
    ) -> impl IntoElement {
        h_flex()
            .id("files-virtual-list-wrap")
            .flex_1()
            .min_h_0()
            .w_full()
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .child(
                        v_virtual_list(
                            cx.entity().clone(),
                            "files-virtual-list",
                            self.item_sizes.clone(),
                            move |this, visible_range, window, cx| {
                                visible_range
                                    .filter_map(|row_index| {
                                        this.render_display_row(row_index, window, cx, details)
                                    })
                                    .collect()
                            },
                        )
                        .track_scroll(&self.scroll_handle),
                    ),
            )
            .child(Self::sweep_gutter())
            .scrollbar(&self.scroll_handle, ScrollbarAxis::Vertical)
    }

    pub(super) fn details_table(
        &self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let show_path_column = self.shows_path_column();
        h_flex()
            .id("files-details-table")
            .size_full()
            .flex_1()
            .min_h_0()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().border)
            .overflow_hidden()
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .min_h_0()
                    .overflow_hidden()
                    .child(
                        h_flex()
                            .h(px(30.))
                            .px(px(16.))
                            .gap(px(8.))
                            .items_center()
                            .bg(cx.theme().background)
                            .border_b_1()
                            .border_color(cx.theme().border)
                            .text_xs()
                            .font_semibold()
                            .text_color(cx.theme().muted_foreground)
                            .child(div().w(FILE_LIST_ICON_TILE).flex_none())
                            .child(div().flex_1().min_w_0().child(t!("files.column.name")))
                            .when(show_path_column, |row| {
                                row.child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .child(t!("files.column.path")),
                                )
                            })
                            .child(div().w(px(120.)).child(t!("files.column.type")))
                            .child(div().w(px(100.)).child(t!("files.column.size")))
                            .child(div().w(px(168.)).child(t!("files.column.modified")))
                            .child(div().w(px(64.)).flex_none()),
                    )
                    .child(self.table_list_body(cx, true)),
            )
    }

    pub(super) fn list_view(
        &self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let show_path_column = self.shows_path_column();
        h_flex()
            .id("files-list-view")
            .size_full()
            .flex_1()
            .min_h_0()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().border)
            .overflow_hidden()
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
            .child(
                v_flex()
                    .flex_1()
                    .min_w_0()
                    .h_full()
                    .min_h_0()
                    .overflow_hidden()
                    .child(
                        h_flex()
                            .h(px(30.))
                            .px(px(16.))
                            .gap(px(8.))
                            .items_center()
                            .bg(cx.theme().background)
                            .border_b_1()
                            .border_color(cx.theme().border)
                            .text_xs()
                            .font_semibold()
                            .text_color(cx.theme().muted_foreground)
                            .child(div().w(FILE_LIST_ICON_TILE).flex_none())
                            .child(div().flex_1().min_w_0().child(t!("files.column.name")))
                            .when(show_path_column, |row| {
                                row.child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .child(t!("files.column.path")),
                                )
                            })
                            .child(div().w(px(64.)).flex_none()),
                    )
                    .child(self.table_list_body(cx, false)),
            )
    }

    fn render_display_row(
        &self,
        row_index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
        details: bool,
    ) -> Option<AnyElement> {
        match self.display_rows.get(row_index)? {
            DisplayRow::GroupHeader {
                key,
                title,
                count,
                collapsed,
            } => Some(self.group_header_row(
                row_index,
                key.clone(),
                title.clone(),
                *count,
                *collapsed,
                cx,
            )),
            DisplayRow::Item(item_index) => {
                let item = self.display_items.get(*item_index)?.clone();
                let selected = self.selected_paths.contains(&item.path);
                let drag_paths = self.drag_paths_for_item(*item_index, &item.path);
                let rename_input = self.renaming_input_for(&item.path);
                let show_path_column = self.shows_path_column();
                Some(if details {
                    Self::row(
                        window,
                        *item_index,
                        item,
                        selected,
                        drag_paths,
                        rename_input,
                        show_path_column,
                        cx,
                    )
                } else {
                    Self::list_row(
                        window,
                        *item_index,
                        item,
                        selected,
                        drag_paths,
                        rename_input,
                        show_path_column,
                        cx,
                    )
                })
            }
        }
    }

    fn group_header_row(
        &self,
        row_index: usize,
        key: String,
        title: String,
        count: usize,
        collapsed: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let label = localized_group_title(&key, &title);
        let chevron = if collapsed {
            compact_icon(IconName::ChevronRight)
        } else {
            compact_icon(IconName::ChevronDown)
        };
        h_flex()
            .id(("file-group-header", row_index))
            .w_full()
            .h_8()
            .flex_none()
            .px_3()
            .gap_2()
            .items_center()
            .bg(cx.theme().muted)
            .border_b_1()
            .border_color(cx.theme().border)
            .cursor_pointer()
            .hover(|this| this.bg(cx.theme().accent.opacity(0.25)))
            .on_click(cx.listener(move |this, _, _, cx| {
                this.toggle_group_collapsed(&key, cx);
            }))
            .child(div().text_color(cx.theme().muted_foreground).child(chevron))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_sm()
                    .font_semibold()
                    .text_color(cx.theme().foreground)
                    .child(format!("{label} ({count})")),
            )
            .into_any_element()
    }

    fn list_row(
        window: &mut Window,
        index: usize,
        item: FileItem,
        selected: bool,
        drag_paths: Vec<PathBuf>,
        rename_input: Option<Entity<InputState>>,
        show_path_column: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let parent_path = item_parent_path(&item);
        let double_click_path = item.path.clone();
        let kind = item.kind;
        let tags = item.tags.clone();
        let drop_target = item.clone();
        let drop_target_for_drop = drop_target.clone();
        let browser = cx.entity().clone();
        let cut_pending = path_is_cut_pending(&item.path, cx);
        let row_body = h_flex()
            .id(("file-list-row", index))
            .w_full()
            .h(FILE_LIST_ROW_HEIGHT)
            .flex_none()
            .group(FILE_LIST_ROW_GROUP)
            .px(px(16.))
            .gap(px(8.))
            .items_center()
            .border_b_1()
            .border_color(cx.theme().border.opacity(0.45))
            .when(!selected, |this| this.hover(|this| this.bg(cx.theme().list_hover)))
            .when(selected, |this| {
                this.bg(cx.theme().accent)
                    .text_color(cx.theme().accent_foreground)
            })
            .when(cut_pending, |this| this.opacity(CUT_PENDING_ITEM_OPACITY))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _event: &MouseDownEvent, _, cx| {
                    this.cancel_rename_if_active(cx);
                    Self::dismiss_main_page_path_edit_if_active(cx);
                    cx.stop_propagation();
                }),
            )
            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                window.focus(&this.focus_handle, cx);
                this.cancel_rename_if_active(cx);
                Self::dismiss_main_page_path_edit_if_active(cx);
                if event.click_count() == 2 {
                    this.open_item(double_click_path.clone(), kind, cx);
                } else {
                    this.handle_row_click(index, event, cx);
                    cx.notify();
                }
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    this.cancel_rename_if_active(cx);
                    Self::dismiss_main_page_path_edit_if_active(cx);
                    this.set_context_menu_extended_verbs(event.modifiers.shift);
                    this.prepare_context_menu_target(index);
                    this.open_context_menu(event.position, window, cx);
                }),
            )
            .on_mouse_move(cx.listener(move |this, _, _, cx| {
                this.update_sweep_selection(SweepSelectionSurface::Main, index, cx);
            }))
            .on_drag(
                DraggedFilePaths(drag_paths),
                {
                    let browser = browser.clone();
                    move |paths, grab_offset, window, cx| {
                        let _ = browser.update(cx, |this, cx| {
                            this.start_native_drag_session(paths.0.clone(), window, cx);
                            this.finish_sweep_selection();
                        });
                        DragPathPreview::new_entity(paths, grab_offset, browser.clone(), cx)
                    }
                },
            )
            .on_drag_move::<DraggedFilePaths>(cx.listener({
                let target = drop_target.clone();
                move |this, event: &DragMoveEvent<DraggedFilePaths>, window, cx| {
                    this.update_drag_hover_over_item_if_hovered(event, &target, window, cx);
                }
            }))
            .on_drag_move::<ExternalPaths>(cx.listener({
                let target = drop_target.clone();
                move |this, event: &DragMoveEvent<ExternalPaths>, window, cx| {
                    this.update_external_drag_hover_over_item_if_hovered(event, &target, window, cx);
                }
            }))
            .drag_over::<DraggedFilePaths>(|row, _, _, cx| {
                row.bg(cx.theme().primary.opacity(0.2))
                    .border_color(cx.theme().primary)
            })
            .drag_over::<ExternalPaths>(|row, _, _, cx| {
                row.bg(cx.theme().primary.opacity(0.2))
                    .border_color(cx.theme().primary)
            })
            .on_drop(cx.listener({
                let drop_target = drop_target_for_drop.clone();
                move |this, paths: &DraggedFilePaths, window, cx| {
                    this.handle_drop_on_item(paths.0.clone(), &drop_target, window, cx);
                }
            }))
            .on_drop(cx.listener({
                let drop_target = drop_target_for_drop.clone();
                move |this, paths: &ExternalPaths, window, cx| {
                    this.handle_external_drop_on_item(paths, &drop_target, window, cx);
                }
            }))
            .child(Self::row_list_icon(&item, px(14.), window, cx))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .text_ellipsis()
                    .text_sm()
                    .font_medium()
                    .text_color(cx.theme().foreground)
                    .child(if let Some(input) = rename_input {
                        Self::inline_name_editor(input, false, cx)
                    } else {
                        tag_badges::render_name_with_tags(item.display_name.clone(), &tags, cx)
                    }),
            )
            .when(show_path_column, |row| {
                row.child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .text_ellipsis()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(parent_path),
                )
            })
            .child(div().w(px(36.)).flex_none());
        Self::file_list_row_shell(("file-list-row-shell", index), selected, row_body, cx)
            .into_any_element()
    }

    fn row(
        window: &mut Window,
        index: usize,
        item: FileItem,
        selected: bool,
        drag_paths: Vec<PathBuf>,
        rename_input: Option<Entity<InputState>>,
        show_path_column: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let parent_path = item_parent_path(&item);
        let open_path = item.path.clone();
        let double_click_path = item.path.clone();
        let kind = item.kind;
        let tags = item.tags.clone();
        let drop_target = item.clone();
        let drop_target_for_drop = drop_target.clone();
        let browser = cx.entity().clone();
        let cut_pending = path_is_cut_pending(&item.path, cx);
        let row_body = h_flex()
            .id(("file-row", index))
            .w_full()
            .h(FILE_LIST_ROW_HEIGHT)
            .flex_none()
            .group(FILE_LIST_ROW_GROUP)
            .px(px(16.))
            .gap(px(8.))
            .items_center()
            .border_b_1()
            .border_color(cx.theme().border.opacity(0.45))
            .when(!selected, |this| this.hover(|this| this.bg(cx.theme().list_hover)))
            .when(selected, |this| {
                this.bg(cx.theme().accent)
                    .text_color(cx.theme().accent_foreground)
            })
            .when(cut_pending, |this| this.opacity(CUT_PENDING_ITEM_OPACITY))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, _event: &MouseDownEvent, _, cx| {
                    this.cancel_rename_if_active(cx);
                    Self::dismiss_main_page_path_edit_if_active(cx);
                    cx.stop_propagation();
                }),
            )
            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                window.focus(&this.focus_handle, cx);
                this.cancel_rename_if_active(cx);
                Self::dismiss_main_page_path_edit_if_active(cx);
                if event.click_count() == 2 {
                    this.open_item(double_click_path.clone(), kind, cx);
                } else {
                    this.handle_row_click(index, event, cx);
                    cx.notify();
                }
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    this.cancel_rename_if_active(cx);
                    Self::dismiss_main_page_path_edit_if_active(cx);
                    this.set_context_menu_extended_verbs(event.modifiers.shift);
                    this.prepare_context_menu_target(index);
                    this.open_context_menu(event.position, window, cx);
                }),
            )
            .on_mouse_move(cx.listener(move |this, _, _, cx| {
                this.update_sweep_selection(SweepSelectionSurface::Main, index, cx);
            }))
            .on_drag(
                DraggedFilePaths(drag_paths),
                {
                    let browser = browser.clone();
                    move |paths, grab_offset, window, cx| {
                        let _ = browser.update(cx, |this, cx| {
                            this.start_native_drag_session(paths.0.clone(), window, cx);
                            this.finish_sweep_selection();
                        });
                        DragPathPreview::new_entity(paths, grab_offset, browser.clone(), cx)
                    }
                },
            )
            .on_drag_move::<DraggedFilePaths>(cx.listener({
                let target = drop_target.clone();
                move |this, event: &DragMoveEvent<DraggedFilePaths>, window, cx| {
                    this.update_drag_hover_over_item_if_hovered(event, &target, window, cx);
                }
            }))
            .on_drag_move::<ExternalPaths>(cx.listener({
                let target = drop_target.clone();
                move |this, event: &DragMoveEvent<ExternalPaths>, window, cx| {
                    this.update_external_drag_hover_over_item_if_hovered(event, &target, window, cx);
                }
            }))
            .drag_over::<DraggedFilePaths>(|row, _, _, cx| {
                row.bg(cx.theme().primary.opacity(0.2))
                    .border_color(cx.theme().primary)
            })
            .drag_over::<ExternalPaths>(|row, _, _, cx| {
                row.bg(cx.theme().primary.opacity(0.2))
                    .border_color(cx.theme().primary)
            })
            .on_drop(cx.listener({
                let drop_target = drop_target_for_drop.clone();
                move |this, paths: &DraggedFilePaths, window, cx| {
                    this.handle_drop_on_item(paths.0.clone(), &drop_target, window, cx);
                }
            }))
            .on_drop(cx.listener({
                let drop_target = drop_target_for_drop.clone();
                move |this, paths: &ExternalPaths, window, cx| {
                    this.handle_external_drop_on_item(paths, &drop_target, window, cx);
                }
            }))
            .child(Self::row_list_icon(&item, px(14.), window, cx))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .text_ellipsis()
                    .text_sm()
                    .font_medium()
                    .text_color(cx.theme().foreground)
                    .child(if let Some(input) = rename_input {
                        Self::inline_name_editor(input, false, cx)
                    } else {
                        tag_badges::render_name_with_tags(item.display_name.clone(), &tags, cx)
                    }),
            )
            .when(show_path_column, |row| {
                row.child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .overflow_hidden()
                        .text_ellipsis()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(parent_path),
                )
            })
            .child(
                div()
                    .w(px(120.))
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(item_type_label(&item)),
            )
            .child(
                div()
                    .w(px(100.))
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(format_size(item.size)),
            )
            .child(
                div()
                    .w(px(168.))
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(format_system_time(item.modified)),
            )
            .child(
                h_flex()
                    .w(px(64.))
                    .flex_none()
                    .gap(px(2.))
                    .items_center()
                    .justify_end()
                    .child(
                        toolbar_icon_button(format!("open-item-{index}"))
                            .h(px(24.))
                            .w(px(24.))
                            .rounded(px(7.))
                            .icon(match kind {
                                FileItemKind::Folder => compact_icon(IconName::ChevronRight),
                                FileItemKind::File | FileItemKind::Symlink | FileItemKind::Other => {
                                    compact_icon(IconName::ExternalLink)
                                }
                            })
                            .tooltip(match kind {
                                FileItemKind::Folder => t!("files.open.folder"),
                                FileItemKind::File | FileItemKind::Symlink | FileItemKind::Other => {
                                    t!("files.open.file")
                                }
                            })
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.open_item(open_path.clone(), kind, cx);
                            })),
                    )
                    .child(
                        toolbar_icon_button(format!("file-row-more-{index}"))
                            .h(px(24.))
                            .w(px(24.))
                            .rounded(px(7.))
                            .icon(compact_icon(IconName::Ellipsis))
                            .opacity(if selected { 1. } else { 0. })
                            .group_hover(FILE_LIST_ROW_GROUP, |btn| btn.opacity(1.))
                            .hover(|btn| btn.bg(cx.theme().list_hover))
                            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                                cx.stop_propagation();
                                this.cancel_rename_if_active(cx);
                                this.set_context_menu_extended_verbs(event.modifiers().shift);
                                this.prepare_context_menu_target(index);
                                this.open_context_menu(window.mouse_position(), window, cx);
                            })),
                    ),
            );
        Self::file_list_row_shell(("file-row-shell", index), selected, row_body, cx)
            .into_any_element()
    }
}
