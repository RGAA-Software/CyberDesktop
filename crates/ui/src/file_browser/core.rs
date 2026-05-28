use super::*;

impl FileBrowser {
    pub(super) fn update_drag_hover_at_position(
        &mut self,
        position: Point<Pixels>,
        paths: &[PathBuf],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if let Some(bounds) = self.main_sweep_bounds {
            if !crate::file_browser::sweep::point_in_bounds(position, bounds) {
                return;
            }
        }
        AppNavigation::cancel_breadcrumb_drag_preview(cx);
        if paths.is_empty() {
            self.clear_drag_hover_feedback(cx);
            return;
        }

        let target = match self.view_mode {
            ViewMode::Columns => self.column_item_at_position(position).and_then(
                |(col_index, row_index)| {
                    self.column_listings
                        .get(col_index)
                        .and_then(|listing| listing.get(row_index).cloned())
                },
            ),
            _ => self
                .display_item_index_at_position(position)
                .and_then(|index| self.display_items.get(index).cloned()),
        };

        if let Some(target) = target {
            self.set_drag_hover_feedback(&target, paths, window, cx);
        } else {
            self.clear_drag_hover_feedback(cx);
        }
    }

    pub(super) fn set_drag_hover_feedback(
        &mut self,
        target: &FileItem,
        paths: &[PathBuf],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if paths.is_empty() {
            self.clear_drag_hover_feedback(cx);
            return;
        }
        if paths.iter().any(|p| p == &target.path) {
            self.apply_drag_hover_hint(
                target.path.clone(),
                t!("files.drag.cannot_drop_here").to_string(),
                true,
                false,
                cx,
            );
            return;
        }
        self.drag_hover_generation = self.drag_hover_generation.saturating_add(1);
        self.drag_hover_target = Some(target.path.clone());
        let (hint, invalid, primary) = match target.kind {
            FileItemKind::Folder => {
                let copy = window.modifiers().control;
                let hint = if copy {
                    t!("files.drag.copy_to_folder", name = target.display_name.clone()).to_string()
                } else {
                    t!("files.drag.move_to_folder", name = target.display_name.clone()).to_string()
                };
                (hint, false, true)
            }
            FileItemKind::File | FileItemKind::Symlink | FileItemKind::Other => {
                if is_executable_or_script_path(&target.path) {
                    (
                        t!("files.drag.open_with_target", name = target.display_name.clone())
                            .to_string(),
                        false,
                        false,
                    )
                } else {
                    (t!("files.drag.cannot_use_target").to_string(), true, false)
                }
            }
        };
        self.apply_drag_hover_hint(target.path.clone(), hint, invalid, primary, cx);
    }

    pub(crate) fn set_breadcrumb_drag_hover_feedback(
        &mut self,
        target_dir: PathBuf,
        paths: &[PathBuf],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if paths.is_empty() {
            self.clear_drag_hover_feedback(cx);
            return;
        }

        if paths.iter().all(|p| p.parent() == Some(target_dir.as_path())) {
            self.apply_drag_hover_hint(
                target_dir,
                t!("files.drag.cannot_drop_here").to_string(),
                true,
                false,
                cx,
            );
            return;
        }

        let target_name = target_dir
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| target_dir.to_string_lossy().to_string());
        let hint = if window.modifiers().control {
            t!("files.drag.copy_to_folder", name = target_name).to_string()
        } else {
            t!("files.drag.move_to_folder", name = target_name).to_string()
        };
        self.apply_drag_hover_hint(target_dir, hint, false, true, cx);
    }

    fn apply_drag_hover_hint(
        &mut self,
        target: PathBuf,
        hint: String,
        invalid: bool,
        primary: bool,
        cx: &mut Context<Self>,
    ) {
        self.drag_hover_target = Some(target);
        self.drag_hover_hint = Some(hint);
        self.drag_hover_hint_invalid = invalid;
        self.drag_hover_hint_primary = primary;
        self.notify_drag_preview(cx);
        cx.notify();
    }

    pub(super) fn clear_drag_hover_feedback(&mut self, cx: &mut Context<Self>) {
        self.drag_hover_generation = self.drag_hover_generation.saturating_add(1);
        self.drag_hover_target = None;
        self.drag_hover_hint = None;
        self.drag_hover_hint_invalid = false;
        self.drag_hover_hint_primary = false;
        self.notify_drag_preview(cx);
        cx.notify();
    }

    pub(super) fn end_drag_session(&mut self, cx: &mut Context<Self>) {
        self.clear_drag_hover_feedback(cx);
        self.drag_preview = None;
    }

    fn notify_drag_preview(&self, cx: &mut Context<Self>) {
        if let Some(preview) = self.drag_preview.as_ref() {
            preview.update(cx, |_, cx| cx.notify());
        }
    }

    pub fn set_search_query(&mut self, query: String, cx: &mut Context<Self>) {
        self.search_query = query;
        self.apply_filter();
        cx.notify();
    }

    pub fn focus_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        AppNavigation::focus_search(window, cx);
    }

    pub(super) fn set_view_mode(&mut self, mode: ViewMode, cx: &mut Context<Self>) {
        if self.view_mode != mode {
            let was_columns = self.view_mode == ViewMode::Columns;
            self.view_mode = mode;
            self.grid_cells_per_row = None;
            self.cards_cells_per_row = None;
            self.item_sizes =
                item_sizes_for(self.display_items.len(), self.view_mode, self.view_size_level);
            self.selected_paths.clear();
            self.active_column_index = None;
            self.column_selected_path = None;
            self.focused_index = None;
            self.anchor_index = None;
            if mode == ViewMode::Columns {
                self.refresh_column_listings();
            } else if was_columns {
                self.refresh();
            }
            self.persist_prefs();
            cx.notify();
        }
    }

    pub(super) fn persist_prefs(&self) {
        let _ = save_file_browser_prefs(
            self.view_mode.config_value(),
            sort_option_config_value(self.sort_preferences.option),
            sort_direction_config_value(self.sort_preferences.direction),
            self.read_options.show_hidden_items,
            self.read_options.show_file_extensions,
        );
    }

    pub fn set_show_info_pane(&mut self, show: bool, cx: &mut Context<Self>) {
        if self.show_info_pane != show {
            self.show_info_pane = show;
            self.grid_cells_per_row = None;
            self.cards_cells_per_row = None;
            cx.notify();
        }
    }

    pub fn read_options(&self) -> &DirectoryReadOptions {
        &self.read_options
    }

    pub fn current_directory(&self) -> &PathBuf {
        &self.current_dir
    }

    pub(super) fn handle_drop(
        &mut self,
        paths: Vec<PathBuf>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.end_drag_session(cx);
        AppNavigation::cancel_breadcrumb_drag_preview(cx);
        if paths.is_empty() {
            return;
        }
        let dest = self.operation_directory();
        if paths.iter().all(|p| p.parent() == Some(dest.as_path())) {
            return;
        }
        let copy = window.modifiers().control;
        let kind = if copy {
            FileTransferKind::Copy
        } else {
            FileTransferKind::Move
        };
        let browser = cx.entity();
        spawn_file_transfer(browser, window, cx, kind, paths, dest);
    }

    pub(super) fn drag_paths_for_item(&self, _index: usize, path: &Path) -> Vec<PathBuf> {
        if self.selected_paths.contains(path) && !self.selected_paths.is_empty() {
            return self.selected_paths_vec();
        }
        vec![path.to_path_buf()]
    }

    pub(super) fn handle_drop_on_item(
        &mut self,
        paths: Vec<PathBuf>,
        target: &FileItem,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.end_drag_session(cx);
        AppNavigation::cancel_breadcrumb_drag_preview(cx);
        if paths.is_empty() {
            return;
        }

        match target.kind {
            FileItemKind::Folder => {
                let dest = target.path.clone();
                if paths.iter().all(|p| p.parent() == Some(dest.as_path())) {
                    return;
                }
                let copy = window.modifiers().control;
                let kind = if copy {
                    FileTransferKind::Copy
                } else {
                    FileTransferKind::Move
                };
                let browser = cx.entity();
                spawn_file_transfer(browser, window, cx, kind, paths, dest);
            }
            FileItemKind::File | FileItemKind::Symlink | FileItemKind::Other => {
                if !is_executable_or_script_path(&target.path) {
                    return;
                }

                let filtered_paths: Vec<PathBuf> = paths
                    .into_iter()
                    .filter(|path| path != &target.path)
                    .collect();
                if filtered_paths.is_empty() {
                    return;
                }
                if let Err(error) = open_paths_with_target(&target.path, &filtered_paths) {
                    self.error = Some(error.to_string());
                    cx.notify();
                }
            }
        }
    }
}
