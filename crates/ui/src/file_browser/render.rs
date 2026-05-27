use super::*;

impl FileBrowser {
    /// Files-style toolbar above the file list (view, sort, new, delete).
    fn render_content_toolbar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let selected_count = self.selected_count();
        let show_hidden = self.read_options.show_hidden_items;
        let show_file_extensions = self.read_options.show_file_extensions;
        let sort_label = self.sort_label();

        let can_paste = AppFileClipboard::has_items(cx);

        h_flex()
            .id("content-toolbar")
            .w_full()
            .flex_none()
            .gap_2()
            .px_3()
            .py_1()
            .mb_1()
            .items_center()
            .border_b_1()
            .border_color(cx.theme().border)
            .child(
                toolbar_icon_button("content-cut")
                    .icon(toolbar_icon(IconName::Replace).path("icons/content_cut.svg"))
                    .tooltip(t!("files.menu.cut"))
                    .disabled(selected_count == 0)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.cut_items(cx);
                        cx.notify();
                    })),
            )
            .child(
                toolbar_icon_button("content-copy")
                    .icon(toolbar_icon(IconName::Copy).path("icons/content_copy.svg"))
                    .tooltip(t!("files.menu.copy"))
                    .disabled(selected_count == 0)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.copy_items(cx);
                        cx.notify();
                    })),
            )
            .child(
                toolbar_icon_button("content-paste")
                    .icon(toolbar_icon(IconName::Replace).path("icons/content_paste.svg"))
                    .tooltip(t!("files.menu.paste"))
                    .disabled(!can_paste)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.paste_items(window, cx);
                    })),
            )
            .child(
                toolbar_icon_button("content-rename")
                    .icon(toolbar_icon(IconName::File).path("icons/drive_file_rename_outline.svg"))
                    .tooltip(t!("files.menu.rename"))
                    .disabled(selected_count == 0)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.begin_rename(window, cx);
                        cx.notify();
                    })),
            )
            .child(
                toolbar_icon_button("content-properties")
                    .icon(toolbar_icon(IconName::Info))
                    .tooltip(t!("files.menu.properties"))
                    .disabled(selected_count == 0)
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.show_properties(cx);
                    })),
            )
            .child(div().w(px(1.)).h(px(20.)).bg(cx.theme().border))
            .child(
                toolbar_icon_button("content-new-folder")
                    .size(TOOLBAR_BUTTON_PX)
                    .icon(toolbar_icon(IconName::Folder).path("icons/create_new_folder.svg"))
                    .tooltip(t!("files.new_folder"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.create_new_folder(window, cx);
                        cx.notify();
                    })),
            )
            .child(
                toolbar_icon_button("content-new-file")
                    .size(TOOLBAR_BUTTON_PX)
                    .icon(toolbar_icon(IconName::File).path("icons/note_add.svg"))
                    .tooltip(t!("files.new_file"))
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.create_new_file(window, cx);
                        cx.notify();
                    })),
            )
            .child(div().w(px(1.)).h(px(20.)).bg(cx.theme().border))
            .child(
                toolbar_icon_button("content-view-details")
                    .icon(toolbar_icon(IconName::GalleryVerticalEnd).path("icons/view_headline.svg"))
                    .tooltip(t!("files.view.details"))
                    .when(self.view_mode == ViewMode::Details, |this| {
                        this.bg(cx.theme().accent).text_color(cx.theme().accent_foreground)
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.set_view_mode(ViewMode::Details, cx);
                    })),
            )
            .child(
                toolbar_icon_button("content-view-list")
                    .icon(toolbar_icon(IconName::PanelLeftOpen))
                    .tooltip(t!("files.view.list"))
                    .when(self.view_mode == ViewMode::List, |this| {
                        this.bg(cx.theme().accent).text_color(cx.theme().accent_foreground)
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.set_view_mode(ViewMode::List, cx);
                    })),
            )
            .child(
                toolbar_icon_button("content-view-grid")
                    .icon(toolbar_icon(IconName::LayoutDashboard))
                    .tooltip(t!("files.view.grid"))
                    .when(self.view_mode == ViewMode::Grid, |this| {
                        this.bg(cx.theme().accent).text_color(cx.theme().accent_foreground)
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.set_view_mode(ViewMode::Grid, cx);
                    })),
            )
            .child(
                toolbar_icon_button("content-view-cards")
                    .icon(toolbar_icon(IconName::LayoutDashboard).path("icons/view_cozy.svg"))
                    .tooltip(t!("files.view.cards"))
                    .when(self.view_mode == ViewMode::Cards, |this| {
                        this.bg(cx.theme().accent).text_color(cx.theme().accent_foreground)
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.set_view_mode(ViewMode::Cards, cx);
                    })),
            )
            .child(
                toolbar_icon_button("content-view-columns")
                    .icon(toolbar_icon(IconName::PanelLeft))
                    .tooltip(t!("files.view.columns"))
                    .when(self.view_mode == ViewMode::Columns, |this| {
                        this.bg(cx.theme().accent).text_color(cx.theme().accent_foreground)
                    })
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.set_view_mode(ViewMode::Columns, cx);
                    })),
            )
            .child(div().w(px(1.)).h(px(20.)).bg(cx.theme().border))
            .child(
                toolbar_icon_button("content-delete")
                    .icon(toolbar_icon(IconName::Delete))
                    .tooltip(t!("files.menu.delete"))
                    .disabled(selected_count == 0)
                    .on_click(cx.listener(|this, _, window, cx| {
                        this.perform_delete(window, cx);
                        cx.notify();
                    })),
            )
            .child(
                toolbar_dropdown_button("content-sort")
                    .button(
                        toolbar_labeled_button("content-sort-btn")
                            .label(sort_label)
                            .tooltip(t!("files.menu.sort")),
                    )
                    .dropdown_menu(move |menu, _, _| {
                        let hidden_label = if show_hidden {
                            t!("files.show_hidden.off")
                        } else {
                            t!("files.show_hidden.on")
                        };
                        let extensions_label = if show_file_extensions {
                            t!("files.show_extensions.off")
                        } else {
                            t!("files.show_extensions.on")
                        };
                        menu.menu(t!("files.sort.name"), Box::new(SortByName))
                            .menu(t!("files.sort.modified"), Box::new(SortByModified))
                            .menu(t!("files.sort.size"), Box::new(SortBySize))
                            .menu(t!("files.sort.type"), Box::new(SortByType))
                            .separator()
                            .menu(
                                t!("files.sort.toggle_direction"),
                                Box::new(ToggleSortDirection),
                            )
                            .menu(hidden_label, Box::new(ToggleShowHidden))
                            .menu(extensions_label, Box::new(ToggleShowFileExtensions))
                    }),
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
        let in_recycle_bin = self.browse_location == BrowseLocation::RecycleBin;

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
            .on_action(cx.listener(Self::on_delete))
            .on_action(cx.listener(Self::on_delete_permanent))
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
            .on_action(cx.listener(Self::on_copy_path))
            .on_action(cx.listener(Self::on_copy_items))
            .on_action(cx.listener(Self::on_cut_items))
            .on_action(cx.listener(Self::on_paste_items))
            .on_action(cx.listener(Self::on_compress_items))
            .on_action(cx.listener(Self::on_navigate_previous))
            .on_action(cx.listener(Self::on_navigate_next))
            .on_action(cx.listener(Self::on_sort_name))
            .on_action(cx.listener(Self::on_sort_created))
            .on_action(cx.listener(Self::on_sort_modified))
            .on_action(cx.listener(Self::on_sort_size))
            .on_action(cx.listener(Self::on_sort_type))
            .on_action(cx.listener(Self::on_toggle_sort_direction))
            .on_action(cx.listener(Self::on_toggle_show_hidden))
            .on_action(cx.listener(Self::on_toggle_show_file_extensions))
            .on_action(cx.listener(Self::on_open_in_new_pane))
            .on_action(cx.listener(Self::on_open_in_terminal))
            .on_action(cx.listener(Self::on_create_folder_from_selection))
            .on_action(cx.listener(Self::on_open_in_new_window))
            .on_action(cx.listener(Self::on_open_with_dialog))
            .on_action(cx.listener(Self::on_create_shortcut))
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
                        .child(
                            toolbar_icon_button("files-new-folder-btn")
                                .size(TOOLBAR_BUTTON_PX)
                                .icon(toolbar_icon(IconName::Folder).path("icons/create_new_folder.svg"))
                                .tooltip(t!("files.new_folder"))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.create_new_folder(window, cx);
                                    cx.notify();
                                })),
                        )
                        .child(
                            toolbar_icon_button("files-new-file-btn")
                                .size(TOOLBAR_BUTTON_PX)
                                .icon(toolbar_icon(IconName::File).path("icons/note_add.svg"))
                                .tooltip(t!("files.new_file"))
                                .on_click(cx.listener(|this, _, window, cx| {
                                    this.create_new_file(window, cx);
                                    cx.notify();
                                })),
                        )
                        .child(
                            toolbar_icon_button("files-view-details")
                                .icon(toolbar_icon(IconName::GalleryVerticalEnd).path("icons/view_headline.svg"))
                                .tooltip(t!("files.view.details"))
                                .when(self.view_mode == ViewMode::Details, |this| {
                                    this.bg(cx.theme().accent).text_color(cx.theme().accent_foreground)
                                })
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.set_view_mode(ViewMode::Details, cx);
                                })),
                        )
                        .child(
                            toolbar_icon_button("files-view-list")
                                .icon(toolbar_icon(IconName::PanelLeftOpen))
                                .tooltip(t!("files.view.list"))
                                .when(self.view_mode == ViewMode::List, |this| {
                                    this.bg(cx.theme().accent).text_color(cx.theme().accent_foreground)
                                })
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.set_view_mode(ViewMode::List, cx);
                                })),
                        )
                        .child(
                            toolbar_icon_button("files-view-grid")
                                .icon(toolbar_icon(IconName::LayoutDashboard))
                                .tooltip(t!("files.view.grid"))
                                .when(self.view_mode == ViewMode::Grid, |this| {
                                    this.bg(cx.theme().accent).text_color(cx.theme().accent_foreground)
                                })
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.set_view_mode(ViewMode::Grid, cx);
                                })),
                        )
                        .child(
                            toolbar_icon_button("files-view-cards")
                                .icon(toolbar_icon(IconName::LayoutDashboard).path("icons/view_cozy.svg"))
                                .tooltip(t!("files.view.cards"))
                                .when(self.view_mode == ViewMode::Cards, |this| {
                                    this.bg(cx.theme().accent).text_color(cx.theme().accent_foreground)
                                })
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.set_view_mode(ViewMode::Cards, cx);
                                })),
                        )
                        .child(
                            toolbar_icon_button("files-view-columns")
                                .icon(toolbar_icon(IconName::PanelLeft))
                                .tooltip(t!("files.view.columns"))
                                .when(self.view_mode == ViewMode::Columns, |this| {
                                    this.bg(cx.theme().accent).text_color(cx.theme().accent_foreground)
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
                                .dropdown_menu(move |menu, _, _| {
                                    let hidden_label = if show_hidden {
                                        t!("files.show_hidden.off")
                                    } else {
                                        t!("files.show_hidden.on")
                                    };
                                    let extensions_label = if show_file_extensions {
                                        t!("files.show_extensions.off")
                                    } else {
                                        t!("files.show_extensions.on")
                                    };
                                    menu.menu(t!("files.sort.name"), Box::new(SortByName))
                                        .menu(t!("files.sort.modified"), Box::new(SortByModified))
                                        .menu(t!("files.sort.size"), Box::new(SortBySize))
                                        .menu(t!("files.sort.type"), Box::new(SortByType))
                                        .separator()
                                        .menu(
                                            t!("files.sort.toggle_direction"),
                                            Box::new(ToggleSortDirection),
                                        )
                                        .menu(hidden_label, Box::new(ToggleShowHidden))
                                        .menu(extensions_label, Box::new(ToggleShowFileExtensions))
                                }),
                        )
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
                    .child(self.file_list(window, cx))
                    .when_some(self.render_main_sweep_overlay(cx), |this, overlay| {
                        this.child(overlay)
                    }),
            )
            .when(self.context_menu_open, |this| {
                this.child(self.render_context_menu_overlay(window))
            })
    }
}
