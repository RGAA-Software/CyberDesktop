use super::*;

impl FileBrowser {
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
