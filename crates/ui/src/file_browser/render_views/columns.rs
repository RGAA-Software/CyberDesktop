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

                v_flex()
                    .id(("files-column", col_index))
                    .w(COLUMN_WIDTH)
                    .flex_none()
                    .h_full()
                    .min_h_0()
                    .border_r_1()
                    .border_color(cx.theme().border)
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
                            .h_8()
                            .px_2()
                            .flex_none()
                            .items_center()
                            .bg(if is_active {
                                cx.theme().primary
                            } else {
                                cx.theme().muted
                            })
                            .text_xs()
                            .text_color(if is_active {
                                cx.theme().primary_foreground
                            } else {
                                cx.theme().muted_foreground
                            })
                            .overflow_hidden()
                            .text_ellipsis()
                            .child(title),
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
                            .on_mouse_move(cx.listener(move |this, event: &MouseMoveEvent, _, cx| {
                                this.update_sweep_pointer(
                                    SweepSelectionSurface::Column(col_index),
                                    event.position,
                                    cx,
                                );
                            }))
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
                            .child(
                                v_virtual_list(
                                    cx.entity().clone(),
                                    format!("files-column-virtual-list-{col_index}"),
                                    item_sizes,
                                    move |this, visible_range, window, cx| {
                                        let has_explicit_column_selection = this
                                            .column_listings
                                            .get(col_index)
                                            .is_some_and(|items| {
                                                items.iter().any(|item| {
                                                    this.selected_paths.contains(&item.path)
                                                })
                                            })
                                            || this
                                                .column_selected_path
                                                .as_ref()
                                                .is_some_and(|(selected_col, _)| {
                                                    *selected_col == col_index
                                                });
                                        visible_range
                                            .filter_map(|index| {
                                                let item = this.column_listings.get(col_index)?.get(index)?.clone();
                                                let is_selected = if has_explicit_column_selection {
                                                    if item.kind == FileItemKind::Folder {
                                                        this.selected_paths.contains(&item.path)
                                                    } else {
                                                        this.column_selected_path
                                                            == Some((col_index, item.path.clone()))
                                                            || this.selected_paths.contains(&item.path)
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
                                                let drag_paths = vec![item.path.clone()];
                                                let rename_input = this.renaming_input_for(&item.path);
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
                            .when_some(self.render_column_sweep_overlay(col_index, cx), |this, overlay| {
                                this.child(overlay)
                            }),
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
        h_flex()
            .id(format!("file-column-row-{col_index}-{name}"))
            .w_full()
            .h_8()
            .flex_none()
            .px_2()
            .gap_2()
            .items_center()
            .text_sm()
            .text_color(cx.theme().foreground)
            .hover(|this| this.bg(cx.theme().accent))
            .when(selected, |this| {
                this.bg(cx.theme().accent)
                    .text_color(cx.theme().accent_foreground)
            })
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
                this.update_sweep_selection(
                    SweepSelectionSurface::Column(col_index),
                    index,
                    cx,
                );
            }))
            .on_drag(
                DraggedFilePaths(drag_paths),
                |paths, _offset, _window, cx| {
                    let label = if paths.0.len() == 1 {
                        paths.0[0]
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| t!("files.type.file").to_string())
                    } else {
                        format!("{} {}", paths.0.len(), t!("files.status.items"))
                    };
                    cx.new(|_| DragPathPreview {
                        label: label.into(),
                    })
                },
            )
            .child(
                div()
                    .w(px(20.))
                    .flex_none()
                    .child(Self::row_list_icon(&item, px(16.), window)),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .text_ellipsis()
                    .text_sm()
                    .child(
                        rename_input.map_or_else(
                            || div().w_full().child(name).into_any_element(),
                            |input| Self::inline_name_editor(input, false, cx),
                        ),
                    ),
            )
            .into_any_element()
    }
}
