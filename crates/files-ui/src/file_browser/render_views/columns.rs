use super::*;

impl FileBrowser {
    pub(super) fn columns_view(
        &self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let columns = self
            .column_trail
            .iter()
            .enumerate()
            .zip(self.column_listings.iter())
            .zip(self.column_scroll_handles.iter())
            .map(|(((col_index, col_path), items), scroll_handle)| {
                let title = col_path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| col_path.to_string_lossy().to_string());
                let selected_name = self.column_selection_name(col_index);
                let item_count = items.len();
                let item_sizes = Rc::new(vec![COLUMN_ROW_SIZE; item_count.max(1)]);

                let is_active = self.active_column_index == Some(col_index);
                let column_depth = col_index + 1;

                v_flex()
                    .id(("files-column", col_index))
                    .w(COLUMN_WIDTH)
                    .flex_none()
                    .h_full()
                    .min_h_0()
                    .rounded_t(COLUMNS_TITLE_RADIUS)
                    .border_1()
                    .border_color(if is_active {
                        cx.theme().primary
                    } else {
                        cx.theme().border
                    })
                    .bg(cx.theme().background)
                    .overflow_hidden()
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(move |this, _, _, cx| {
                            this.cancel_rename_if_active(cx);
                            Self::dismiss_main_page_path_edit_if_active(cx);
                            this.activate_column(col_index, cx);
                            cx.stop_propagation();
                        }),
                    )
                    .child(
                        h_flex()
                            .flex_none()
                            .w_full()
                            .h(COLUMNS_TITLE_HEIGHT)
                            .pl(px(12.))
                            .pr(px(10.))
                            .items_center()
                            .gap(px(8.))
                            .rounded_t(COLUMNS_TITLE_RADIUS)
                            .border_b_1()
                            .border_color(if is_active {
                                cx.theme().primary
                            } else {
                                cx.theme().border
                            })
                            .bg(if is_active {
                                cx.theme().accent
                            } else {
                                cx.theme().background
                            })
                            .overflow_hidden()
                            .child(
                                Label::new(title)
                                    .flex_1()
                                    .min_w_0()
                                    .text_sm()
                                    .when(is_active, |label| label.font_semibold())
                                    .text_color(if is_active {
                                        cx.theme().accent_foreground
                                    } else {
                                        cx.theme().foreground
                                    })
                                    .truncate(),
                            )
                            .child(
                                Label::new(column_depth.to_string())
                                    .flex_none()
                                    .text_sm()
                                    .when(is_active, |label| label.font_semibold())
                                    .text_color(if is_active {
                                        cx.theme().accent_foreground
                                    } else {
                                        cx.theme().foreground
                                    }),
                            ),
                    )
                    .child(
                        v_flex()
                            .id(("files-column-content", col_index))
                            .flex_1()
                            .min_h_0()
                            .size_full()
                            .overflow_hidden()
                            .on_mouse_down(
                                MouseButton::Left,
                                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                                    this.cancel_rename_if_active(cx);
                                    Self::dismiss_main_page_path_edit_if_active(cx);
                                    this.begin_sweep_selection(
                                        SweepSelectionSurface::Column(col_index),
                                        event.position,
                                        event.modifiers,
                                        cx,
                                    );
                                    cx.stop_propagation();
                                }),
                            )
                            .on_mouse_move(cx.listener(
                                move |this, event: &MouseMoveEvent, _, cx| {
                                    this.update_sweep_pointer(
                                        SweepSelectionSurface::Column(col_index),
                                        event.position,
                                        cx,
                                    );
                                },
                            ))
                            .on_prepaint({
                                let entity = cx.entity().clone();
                                move |bounds, _window, cx| {
                                    let _ = entity.update(cx, |this, _cx| {
                                        this.column_sweep_bounds.insert(col_index, bounds);
                                    });
                                }
                            })
                            .on_mouse_up(
                                MouseButton::Left,
                                cx.listener(|this, _, _, _| {
                                    this.finish_sweep_selection();
                                }),
                            )
                            .on_mouse_up_out(
                                MouseButton::Left,
                                cx.listener(|this, _, _, _| {
                                    this.finish_sweep_selection();
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
                            .child(
                                v_virtual_list(
                                    cx.entity().clone(),
                                    format!("files-column-virtual-list-{col_index}"),
                                    item_sizes,
                                    move |this, visible_range, window, cx| {
                                        let has_explicit_column_selection =
                                            this.column_listings.get(col_index).is_some_and(
                                                |items| {
                                                    items.iter().any(|item| {
                                                        this.selected_paths.contains(&item.path)
                                                    })
                                                },
                                            ) || this.column_selected_path.as_ref().is_some_and(
                                                |(selected_col, _)| *selected_col == col_index,
                                            );
                                        visible_range
                                            .filter_map(|index| {
                                                let item = this
                                                    .column_listings
                                                    .get(col_index)?
                                                    .get(index)?
                                                    .clone();
                                                let is_selected = if has_explicit_column_selection {
                                                    if item.kind == FileItemKind::Folder {
                                                        this.selected_paths.contains(&item.path)
                                                    } else {
                                                        this.column_selected_path
                                                            == Some((col_index, item.path.clone()))
                                                            || this
                                                                .selected_paths
                                                                .contains(&item.path)
                                                    }
                                                } else if item.kind == FileItemKind::Folder {
                                                    selected_name.as_deref()
                                                        == Some(item.display_name.as_str())
                                                        || this.selected_paths.contains(&item.path)
                                                } else {
                                                    this.column_selected_path
                                                        == Some((col_index, item.path.clone()))
                                                        || this.selected_paths.contains(&item.path)
                                                };
                                                let drag_paths =
                                                    this.drag_paths_for_item(index, &item.path);
                                                let rename_input =
                                                    this.renaming_input_for(&item.path);
                                                Some(Self::column_cell(
                                                    window,
                                                    col_index,
                                                    index,
                                                    item,
                                                    is_selected,
                                                    drag_paths,
                                                    rename_input,
                                                    cx,
                                                ))
                                            })
                                            .collect()
                                    },
                                )
                                .track_scroll(scroll_handle),
                            )
                            .when_some(
                                self.render_column_sweep_overlay(col_index, cx),
                                |this, overlay| this.child(overlay),
                            ),
                    )
                    .scrollbar(scroll_handle, ScrollbarAxis::Vertical)
            })
            .collect::<Vec<_>>();

        v_flex()
            .id("files-columns-shell")
            .size_full()
            .flex_1()
            .min_h_0()
            .on_prepaint({
                let entity = cx.entity().clone();
                move |bounds, window, cx| {
                    let changed = entity.update(cx, |this, cx| {
                        this.update_columns_horizontal_scrollbar_state(bounds, cx)
                    });
                    if changed {
                        window.refresh();
                    }
                }
            })
            .child(
                h_flex()
                    .id("files-columns-wrap")
                    .flex_1()
                    .min_h_0()
                    .w_full()
                    .items_start()
                    .gap(px(10.))
                    .overflow_x_scroll()
                    .map(|mut this| {
                        this.style().restrict_scroll_to_axis = Some(true);
                        this
                    })
                    .track_scroll(&self.columns_horizontal_scroll_handle)
                    .on_mouse_down(
                        MouseButton::Left,
                        cx.listener(|this, _, _, cx| {
                            this.cancel_rename_if_active(cx);
                            Self::dismiss_main_page_path_edit_if_active(cx);
                            this.active_column_index = None;
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
                    .children(columns),
            )
            .when(self.columns_horizontal_overflow, |this| {
                this.child(
                    Scrollbar::horizontal(&self.columns_horizontal_scroll_handle)
                        .id("files-columns-horizontal-scrollbar")
                        .scrollbar_show(ScrollbarShow::Always),
                )
            })
    }

    fn column_cell(
        window: &mut Window,
        col_index: usize,
        index: usize,
        item: FileItem,
        selected: bool,
        drag_paths: Vec<PathBuf>,
        rename_input: Option<Entity<InputState>>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let kind = item.kind;
        let name = item.display_name.clone();
        let item_click = item.clone();
        let drop_target_for_drop = item.clone();
        let browser = cx.entity().clone();
        let cut_pending = path_is_cut_pending(&item.path, cx);
        h_flex()
            .id(format!("file-column-row-{col_index}-{name}"))
            .w_full()
            .h(FILE_LIST_ROW_HEIGHT)
            .flex_none()
            .px(px(12.))
            .gap(px(8.))
            .items_center()
            .text_sm()
            .text_color(cx.theme().foreground)
            .when(!selected, |this| {
                this.hover(|this| this.bg(cx.theme().list_hover))
            })
            .when(selected, |this| {
                this.bg(cx.theme().accent)
                    .text_color(cx.theme().accent_foreground)
            })
            .when(cut_pending, |this| this.opacity(CUT_PENDING_ITEM_OPACITY))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(move |this, event: &MouseDownEvent, _, cx| {
                    this.cancel_rename_if_active(cx);
                    Self::dismiss_main_page_path_edit_if_active(cx);
                    if event.modifiers.shift || event.modifiers.secondary() {
                        this.begin_sweep_selection(
                            SweepSelectionSurface::Column(col_index),
                            event.position,
                            event.modifiers,
                            cx,
                        );
                    }
                    cx.stop_propagation();
                }),
            )
            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                cx.stop_propagation();
                window.focus(&this.focus_handle, cx);
                this.cancel_rename_if_active(cx);
                Self::dismiss_main_page_path_edit_if_active(cx);
                if event.modifiers().shift || event.modifiers().secondary() {
                    this.handle_column_item_click(
                        col_index,
                        index,
                        &item_click,
                        event.modifiers(),
                        cx,
                    );
                } else if kind == FileItemKind::Folder {
                    this.select_column_item(col_index, &item_click, cx);
                } else if event.click_count() == 2 {
                    this.open_item(item_click.path.clone(), kind, cx);
                } else {
                    this.handle_column_item_click(
                        col_index,
                        index,
                        &item_click,
                        event.modifiers(),
                        cx,
                    );
                }
                cx.notify();
            }))
            .on_mouse_down(
                MouseButton::Right,
                cx.listener(move |this, event: &MouseDownEvent, window, cx| {
                    cx.stop_propagation();
                    this.cancel_rename_if_active(cx);
                    Self::dismiss_main_page_path_edit_if_active(cx);
                    this.set_context_menu_extended_verbs(event.modifiers.shift);
                    this.prepare_column_context_menu_target(col_index, index);
                    this.open_context_menu(event.position, window, cx);
                }),
            )
            .on_mouse_move(cx.listener(move |this, _, _, cx| {
                this.update_sweep_selection(SweepSelectionSurface::Column(col_index), index, cx);
            }))
            .on_drag(DraggedFilePaths(drag_paths), {
                let browser = browser.clone();
                move |paths, grab_offset, window, cx| {
                    let _ = browser.update(cx, |this, cx| {
                        this.start_native_drag_session(paths.0.clone(), window, cx);
                        this.finish_sweep_selection();
                    });
                    DragPathPreview::new_entity(paths, grab_offset, browser.clone(), cx)
                }
            })
            .on_drag_move::<DraggedFilePaths>(cx.listener({
                let target = item.clone();
                move |this, event: &DragMoveEvent<DraggedFilePaths>, window, cx| {
                    this.update_drag_hover_over_item_if_hovered(event, &target, window, cx);
                }
            }))
            .on_drag_move::<ExternalPaths>(cx.listener({
                let target = item.clone();
                move |this, event: &DragMoveEvent<ExternalPaths>, window, cx| {
                    this.update_external_drag_hover_over_item_if_hovered(
                        event, &target, window, cx,
                    );
                }
            }))
            .drag_over::<DraggedFilePaths>(|row, _, _, cx| {
                row.bg(cx.theme().primary.opacity(0.2))
                    .text_color(cx.theme().primary_foreground)
            })
            .drag_over::<ExternalPaths>(|row, _, _, cx| {
                row.bg(cx.theme().primary.opacity(0.2))
                    .text_color(cx.theme().primary_foreground)
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
            .child(
                div()
                    .w(FILE_LIST_ICON_TILE)
                    .flex_none()
                    .child(Self::row_list_icon(&item, FILE_LIST_ICON_SIZE, window, cx)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .text_ellipsis()
                    .text_sm()
                    .child(rename_input.map_or_else(
                        || div().w_full().child(name).into_any_element(),
                        |input| Self::inline_name_editor(input, false, cx),
                    )),
            )
            .into_any_element()
    }
}
