use super::*;

impl FileBrowser {
    pub(super) fn grid_view(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let (cell_w, cell_h, icon_size) = match self.view_size_level {
            1 => (GRID_CELL_SIZE_SMALL.width, GRID_CELL_SIZE_SMALL.height, px(16.)),
            3 => (GRID_CELL_SIZE_LARGE.width, GRID_CELL_SIZE_LARGE.height, px(24.)),
            _ => (GRID_CELL_SIZE.width, GRID_CELL_SIZE.height, px(20.)),
        };

        let estimated_available_width = {
            let sidebar_w = px(214.);
            let info_pane_w = if self.show_info_pane { px(300.) } else { px(0.) };
            let padding_border = px(18.);
            (window.viewport_size().width - sidebar_w - info_pane_w - padding_border).max(px(200.))
        };
        let gap = px(8.);
        let estimated_cells_per_row =
            ((estimated_available_width + gap) / (cell_w + gap)).max(1.) as usize;
        let cells_per_row = self.grid_cells_per_row.unwrap_or(estimated_cells_per_row);
        let row_count =
            (self.display_items.len() + cells_per_row.saturating_sub(1)) / cells_per_row;
        let item_sizes = Rc::new(vec![size(px(1.), cell_h); row_count.max(1)]);

        v_flex()
            .id("files-grid-view")
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
            .on_prepaint({
                let entity = cx.entity().clone();
                move |bounds, window, cx| {
                    let measured_width = bounds.size.width - px(18.);
                    let measured_cells =
                        ((measured_width + gap) / (cell_w + gap)).max(1.) as usize;
                    let changed = entity.update(cx, |this, _cx| {
                        let changed = this.grid_cells_per_row != Some(measured_cells);
                        this.grid_cells_per_row = Some(measured_cells);
                        changed
                    });
                    if changed {
                        window.refresh();
                    }
                }
            })
            .child(
                v_flex()
                    .id("files-grid-wrap")
                    .flex_1()
                    .min_h_0()
                    .size_full()
                    .p_2()
                    .child(
                        v_virtual_list(
                            cx.entity().clone(),
                            "files-grid-virtual-list",
                            item_sizes,
                            move |this, visible_range, window, cx| {
                                visible_range
                                    .filter_map(|row_ix| {
                                        let start = row_ix * cells_per_row;
                                        let end = (start + cells_per_row).min(this.display_items.len());
                                        if start >= this.display_items.len() {
                                            return None;
                                        }
                                        Some(
                                            h_flex()
                                                .w_full()
                                                .gap_2()
                                                .children(
                                                    (start..end).map(|index| {
                                                        let item = this.display_items[index].clone();
                                                        let selected = this.selected_paths.contains(&item.path);
                                                        let drag_paths = this.drag_paths_for_item(index, &item.path);
                                                        let rename_input = this.renaming_input_for(&item.path);
                                                        Self::grid_cell(
                                                            window,
                                                            index,
                                                            item,
                                                            selected,
                                                            drag_paths,
                                                            rename_input,
                                                            cell_w,
                                                            cell_h,
                                                            icon_size,
                                                            cx,
                                                        )
                                                    })
                                                )
                                                .into_any_element(),
                                        )
                                    })
                                    .collect()
                            },
                        )
                        .track_scroll(&self.grid_scroll_handle)
                        .gap_2(),
                    )
                    .scrollbar(&self.grid_scroll_handle, ScrollbarAxis::Vertical),
            )
    }

    pub(super) fn cards_view(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let cell_w = CARD_CELL_SIZE.width;
        let cell_h = CARD_CELL_SIZE.height;

        let estimated_available_width = {
            let sidebar_w = px(214.);
            let info_pane_w = if self.show_info_pane { px(300.) } else { px(0.) };
            let padding_border = px(18.);
            (window.viewport_size().width - sidebar_w - info_pane_w - padding_border).max(px(200.))
        };
        let gap = px(8.);
        let estimated_cells_per_row =
            ((estimated_available_width + gap) / (cell_w + gap)).max(1.) as usize;
        let cells_per_row = self.cards_cells_per_row.unwrap_or(estimated_cells_per_row);
        let row_count =
            (self.display_items.len() + cells_per_row.saturating_sub(1)) / cells_per_row;
        let item_sizes = Rc::new(vec![size(px(1.), cell_h); row_count.max(1)]);

        v_flex()
            .id("files-cards-view")
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
            .on_prepaint({
                let entity = cx.entity().clone();
                move |bounds, window, cx| {
                    let measured_width = bounds.size.width - px(18.);
                    let measured_cells =
                        ((measured_width + gap) / (cell_w + gap)).max(1.) as usize;
                    let changed = entity.update(cx, |this, _cx| {
                        let changed = this.cards_cells_per_row != Some(measured_cells);
                        this.cards_cells_per_row = Some(measured_cells);
                        changed
                    });
                    if changed {
                        window.refresh();
                    }
                }
            })
            .child(
                v_flex()
                    .id("files-cards-wrap")
                    .flex_1()
                    .min_h_0()
                    .size_full()
                    .p_2()
                    .child(
                        v_virtual_list(
                            cx.entity().clone(),
                            "files-cards-virtual-list",
                            item_sizes,
                            move |this, visible_range, window, cx| {
                                visible_range
                                    .filter_map(|row_ix| {
                                        let start = row_ix * cells_per_row;
                                        let end = (start + cells_per_row).min(this.display_items.len());
                                        if start >= this.display_items.len() {
                                            return None;
                                        }
                                        Some(
                                            h_flex()
                                                .w_full()
                                                .gap_2()
                                                .children(
                                                    (start..end).map(|index| {
                                                        let item = this.display_items[index].clone();
                                                        let selected = this.selected_paths.contains(&item.path);
                                                        let drag_paths = this.drag_paths_for_item(index, &item.path);
                                                        let rename_input = this.renaming_input_for(&item.path);
                                                        Self::card_cell(
                                                            window,
                                                            index,
                                                            item,
                                                            selected,
                                                            drag_paths,
                                                            rename_input,
                                                            cx,
                                                        )
                                                    })
                                                )
                                                .into_any_element(),
                                        )
                                    })
                                    .collect()
                            },
                        )
                        .track_scroll(&self.cards_scroll_handle)
                        .gap_2(),
                    )
                    .scrollbar(&self.cards_scroll_handle, ScrollbarAxis::Vertical),
            )
    }

    fn grid_cell(
        window: &mut Window,
        index: usize,
        item: FileItem,
        selected: bool,
        drag_paths: Vec<PathBuf>,
        rename_input: Option<Entity<InputState>>,
        cell_w: Pixels,
        cell_h: Pixels,
        icon_size: Pixels,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let double_click_path = item.path.clone();
        let kind = item.kind;
        let name = item.display_name.clone();
        let tags = item.tags.clone();
        let drop_target = item.clone();
        let drop_target_for_drop = drop_target.clone();
        let browser = cx.entity().clone();
        let cut_pending = path_is_cut_pending(&item.path, cx);
        v_flex()
            .id(("file-grid-cell", index))
            .w(cell_w)
            .h(cell_h)
            .flex_none()
            .p_2()
            .gap_1()
            .items_center()
            .justify_center()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().border)
            .when(!selected, |this| this.hover(|this| this.bg(cx.theme().list_hover)))
            .when(selected, |this| {
                this.bg(cx.theme().accent)
                    .border_color(cx.theme().primary)
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
            .on_mouse_move(cx.listener(move |this, _: &MouseMoveEvent, window, cx| {
                this.update_sweep_pointer(
                    SweepSelectionSurface::Main,
                    window.mouse_position(),
                    cx,
                );
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
            .drag_over::<DraggedFilePaths>(|cell, _, _, cx| {
                cell.bg(cx.theme().primary.opacity(0.2))
                    .border_color(cx.theme().primary)
            })
            .drag_over::<ExternalPaths>(|cell, _, _, cx| {
                cell.bg(cx.theme().primary.opacity(0.2))
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
            .when(!tags.is_empty(), |cell| {
                cell.child(
                    h_flex()
                        .w_full()
                        .justify_end()
                        .child(tag_badges::render_tag_badges(&tags, cx)),
                )
            })
            .child(Self::row_list_icon(&item, icon_size, window, cx))
            .child(
                div()
                    .w_full()
                    .text_center()
                    .text_xs()
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(
                        rename_input.map_or_else(
                            || div().w_full().child(name).into_any_element(),
                            |input| Self::inline_name_editor(input, true, cx),
                        ),
                    ),
            )
            .into_any_element()
    }

    fn card_cell(
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
        let name = item.display_name.clone();
        let tags = item.tags.clone();
        let drop_target = item.clone();
        let drop_target_for_drop = drop_target.clone();
        let browser = cx.entity().clone();
        let cut_pending = path_is_cut_pending(&item.path, cx);
        v_flex()
            .id(("file-card-cell", index))
            .w(CARD_CELL_SIZE.width)
            .h(CARD_CELL_SIZE.height)
            .flex_none()
            .p_2()
            .gap_1()
            .items_center()
            .justify_center()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().border)
            .when(!selected, |this| this.hover(|this| this.bg(cx.theme().list_hover)))
            .when(selected, |this| {
                this.bg(cx.theme().accent)
                    .border_color(cx.theme().primary)
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
            .on_mouse_move(cx.listener(move |this, _: &MouseMoveEvent, window, cx| {
                this.update_sweep_pointer(
                    SweepSelectionSurface::Main,
                    window.mouse_position(),
                    cx,
                );
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
            .drag_over::<DraggedFilePaths>(|cell, _, _, cx| {
                cell.bg(cx.theme().primary.opacity(0.2))
                    .border_color(cx.theme().primary)
            })
            .drag_over::<ExternalPaths>(|cell, _, _, cx| {
                cell.bg(cx.theme().primary.opacity(0.2))
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
            .when(!tags.is_empty(), |cell| {
                cell.child(
                    h_flex()
                        .w_full()
                        .justify_end()
                        .child(tag_badges::render_tag_badges(&tags, cx)),
                )
            })
            .child(Self::row_list_icon(&item, px(40.), window, cx))
            .child(
                div()
                    .w_full()
                    .text_center()
                    .text_sm()
                    .overflow_hidden()
                    .text_ellipsis()
                    .child(
                        rename_input.map_or_else(
                            || div().w_full().child(name).into_any_element(),
                            |input| Self::inline_name_editor(input, true, cx),
                        ),
                    ),
            )
            .when(item.size.is_some(), |this| {
                this.child(
                    div()
                        .w_full()
                        .text_center()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(format_size(item.size)),
                )
            })
            .when(item.modified.is_some(), |this| {
                this.child(
                    div()
                        .w_full()
                        .text_center()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .overflow_hidden()
                        .text_ellipsis()
                        .child(format_system_time(item.modified)),
                )
            })
            .into_any_element()
    }
}
