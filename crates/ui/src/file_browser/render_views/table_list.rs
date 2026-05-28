use super::*;

impl FileBrowser {
    pub(super) fn details_table(
        &self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        v_flex()
            .id("files-details-table")
            .size_full()
            .flex_1()
            .min_h_0()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().border)
            .overflow_hidden()
            .child(
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
                    .child(div().w(px(110.)).child(t!("files.column.type")))
                    .child(div().w(px(100.)).child(t!("files.column.size")))
                    .child(div().w(px(150.)).child(t!("files.column.modified")))
                    .child(div().w(px(40.)).flex_none()),
            )
            .child(
                v_flex()
                    .id("files-virtual-list-wrap")
                    .flex_1()
                    .min_h_0()
                    .child(
                        v_virtual_list(
                            cx.entity().clone(),
                            "files-virtual-list",
                            self.item_sizes.clone(),
                            |this, visible_range, window, cx| {
                                visible_range
                                    .filter_map(|index| {
                                        let item = this.display_items.get(index)?.clone();
                                        let selected = this.selected_paths.contains(&item.path);
                                        let drag_paths =
                                            this.drag_paths_for_item(index, &item.path);
                                        let rename_input = this.renaming_input_for(&item.path);
                                        Some(Self::row(
                                            window,
                                            index,
                                            item,
                                            selected,
                                            drag_paths,
                                            rename_input,
                                            cx,
                                        ))
                                    })
                                    .collect()
                            },
                        )
                        .track_scroll(&self.scroll_handle),
                    )
                    .scrollbar(&self.scroll_handle, ScrollbarAxis::Vertical),
            )
    }

    pub(super) fn list_view(
        &self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        v_flex()
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
                    .child(div().w(px(40.)).flex_none()),
            )
            .child(
                v_flex()
                    .id("files-virtual-list-wrap")
                    .flex_1()
                    .min_h_0()
                    .child(
                        v_virtual_list(
                            cx.entity().clone(),
                            "files-virtual-list",
                            self.item_sizes.clone(),
                            |this, visible_range, window, cx| {
                                visible_range
                                    .filter_map(|index| {
                                        let item = this.display_items.get(index)?.clone();
                                        let selected = this.selected_paths.contains(&item.path);
                                        let drag_paths =
                                            this.drag_paths_for_item(index, &item.path);
                                        let rename_input = this.renaming_input_for(&item.path);
                                        Some(Self::list_row(
                                            window,
                                            index,
                                            item,
                                            selected,
                                            drag_paths,
                                            rename_input,
                                            cx,
                                        ))
                                    })
                                    .collect()
                            },
                        )
                        .track_scroll(&self.scroll_handle),
                    )
                    .scrollbar(&self.scroll_handle, ScrollbarAxis::Vertical),
            )
    }

    fn list_row(
        window: &mut Window,
        index: usize,
        item: FileItem,
        selected: bool,
        drag_paths: Vec<PathBuf>,
        rename_input: Option<Entity<InputState>>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let double_click_path = item.path.clone();
        let kind = item.kind;
        let drop_target = item.clone();
        h_flex()
            .id(("file-list-row", index))
            .w_full()
            .h_9()
            .flex_none()
            .px_3()
            .gap_3()
            .items_center()
            .border_b_1()
            .border_color(cx.theme().border)
            .hover(|this| this.bg(cx.theme().accent))
            .when(selected, |this| {
                this.bg(cx.theme().accent)
                    .text_color(cx.theme().accent_foreground)
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
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
                move |paths, _offset, _window, cx| {
                    cx.new(|_| DragPathPreview {
                        label: drag_preview_label(&paths.0).into(),
                    })
                },
            )
            .on_drop(cx.listener(move |this, paths: &DraggedFilePaths, window, cx| {
                this.handle_drop_on_item(paths.0.clone(), &drop_target, window, cx);
            }))
            .child(
                div()
                    .w(px(28.))
                    .flex_none()
                    .text_color(cx.theme().muted_foreground)
                    .child(Self::row_list_icon(&item, px(16.), window)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .text_ellipsis()
                    .text_sm()
                    .text_color(cx.theme().foreground)
                    .child(
                        rename_input.map_or_else(
                            || div().w_full().child(item.display_name.clone()).into_any_element(),
                            |input| Self::inline_name_editor(input, false, cx),
                        ),
                    ),
            )
            .into_any_element()
    }

    fn row(
        window: &mut Window,
        index: usize,
        item: FileItem,
        selected: bool,
        drag_paths: Vec<PathBuf>,
        rename_input: Option<Entity<InputState>>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let open_path = item.path.clone();
        let double_click_path = item.path.clone();
        let kind = item.kind;
        let drop_target = item.clone();
        h_flex()
            .id(("file-row", index))
            .w_full()
            .h_9()
            .flex_none()
            .px_3()
            .gap_3()
            .items_center()
            .border_b_1()
            .border_color(cx.theme().border)
            .hover(|this| this.bg(cx.theme().accent))
            .when(selected, |this| {
                this.bg(cx.theme().accent)
                    .text_color(cx.theme().accent_foreground)
            })
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _, _, cx| {
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
                move |paths, _offset, _window, cx| {
                    cx.new(|_| DragPathPreview {
                        label: drag_preview_label(&paths.0).into(),
                    })
                },
            )
            .on_drop(cx.listener(move |this, paths: &DraggedFilePaths, window, cx| {
                this.handle_drop_on_item(paths.0.clone(), &drop_target, window, cx);
            }))
            .child(
                div()
                    .w(px(28.))
                    .flex_none()
                    .text_color(cx.theme().muted_foreground)
                    .child(Self::row_list_icon(&item, px(16.), window)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .text_ellipsis()
                    .text_sm()
                    .text_color(cx.theme().foreground)
                    .child(
                        rename_input.map_or_else(
                            || div().w_full().child(item.display_name.clone()).into_any_element(),
                            |input| Self::inline_name_editor(input, false, cx),
                        ),
                    ),
            )
            .child(
                div()
                    .w(px(110.))
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
                    .w(px(150.))
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(format_system_time(item.modified)),
            )
            .child(
                div().w(px(40.)).flex_none().child(
                    toolbar_icon_button(format!("open-item-{index}"))
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
                ),
            )
            .into_any_element()
    }
}
