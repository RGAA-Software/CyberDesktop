use super::*;
use files_core::record_path_history;
use files_fs::{normalize_directory_path, paths_equal_directory};

impl FileBrowser {
    fn record_current_directory_history(&self) {
        if self.browse_location == BrowseLocation::Directory {
            record_path_history(&self.current_dir);
        }
    }

    pub(super) fn apply_filter(&mut self) {
        self.display_items = filter_items_by_query(&self.items, &self.search_query);
        let grouping = self.grouping_enabled();
        self.display_rows = if grouping {
            build_display_rows(
                &self.display_items,
                self.group_option,
                self.group_date_unit,
                self.sort_preferences.direction,
                &self.collapsed_groups,
            )
        } else {
            (0..self.display_items.len())
                .map(DisplayRow::Item)
                .collect()
        };
        self.item_sizes = item_sizes_for_display_rows(
            &self.display_rows,
            self.view_mode,
            self.view_size_level,
        );
        if self.view_mode == ViewMode::Columns {
            self.refresh_column_listings();
        }
        self.clamp_focused_index();
    }

    pub(super) fn refresh_column_listings(&mut self) {
        self.column_trail = column_trail_for(&self.current_dir);
        self.column_listings = column_listings_for(
            &self.column_trail,
            &self.read_options,
            self.sort_preferences,
            &self.search_query,
        );
        self.column_scroll_handles = self
            .column_listings
            .iter()
            .map(|_| VirtualListScrollHandle::new())
            .collect();
    }

    pub(super) fn restart_directory_watcher(&mut self, cx: &mut Context<Self>) {
        self._watcher_task.take();
        self._directory_watcher.take();

        if self.browse_location != BrowseLocation::Directory {
            return;
        }

        // WSL paths are served by the Plan9 server; the notify watcher is unreliable there.
        if files_fs::is_wsl_path(&self.current_dir) {
            tracing::debug!(target: "wsl", path = %self.current_dir.display(), "skipping directory watcher for WSL path");
            return;
        }

        let Ok((watcher, events)) =
            DirectoryWatcher::watch(&self.current_dir, Duration::from_millis(300))
        else {
            return;
        };

        self._directory_watcher = Some(watcher);
        self._watcher_task = Some(cx.spawn(async move |browser, cx| {
            while events.recv_async().await.is_ok() {
                let _ = browser.update(cx, |browser, cx| {
                    browser.refresh();
                    cx.notify();
                });
            }
        }));
    }

    pub(crate) fn shows_directory(&self, path: &Path) -> bool {
        if self.current_dir == path {
            return true;
        }

        self.view_mode == ViewMode::Columns
            && self.browse_location == BrowseLocation::Directory
            && self.column_trail.iter().any(|entry| entry == path)
    }

    pub fn navigation_target(&self) -> NavigationTarget {
        match &self.browse_location {
            BrowseLocation::Directory => NavigationTarget::Path(self.current_dir.clone()),
            BrowseLocation::RecycleBin => NavigationTarget::RecycleBin,
            BrowseLocation::FileTag { tag_name } => NavigationTarget::FileTag(tag_name.clone()),
            BrowseLocation::SearchResults { raw_query, .. } => NavigationTarget::SearchResults {
                query: raw_query.clone(),
            },
        }
    }

    pub fn item_count(&self) -> usize {
        self.display_items.len()
    }

    pub fn selected_count(&self) -> usize {
        self.effective_selected_paths().len()
    }

    pub fn can_go_back(&self) -> bool {
        !self.back_stack.is_empty()
    }

    pub fn can_go_forward(&self) -> bool {
        !self.forward_stack.is_empty()
    }

    pub fn can_go_up(&self) -> bool {
        matches!(self.browse_location, BrowseLocation::Directory)
            && self.current_dir.parent().is_some()
    }

    pub fn go_back(&mut self, cx: &mut Context<Self>) {
        self.navigate_back(cx);
    }

    pub fn go_forward(&mut self, cx: &mut Context<Self>) {
        self.navigate_forward(cx);
    }

    pub fn go_up(&mut self, cx: &mut Context<Self>) {
        self.navigate_parent(cx);
    }

    pub fn reload(&mut self, cx: &mut Context<Self>) {
        if matches!(self.browse_location, BrowseLocation::SearchResults { .. }) {
            self.refresh_search_results(cx);
        } else {
            // Force-refresh: clear network cache so the next render re-enumerates.
            #[cfg(windows)]
            {
                let is_network = self.current_dir.to_string_lossy() == r"::{F02C1A0D-BE21-4350-88B0-7367FC96EF3C}"
                    || files_fs::is_network_computer_root(&self.current_dir);
                if is_network {
                    self.network_items_cache.remove(&self.current_dir);
                    self._network_load_task = None;
                }
            }
            self.refresh();
        }
    }

    pub fn open_directory_reset_history(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.browse_location = BrowseLocation::Directory;
        self.back_stack.clear();
        self.forward_stack.clear();
        self.current_dir = normalize_directory_path(&path);
        self.record_current_directory_history();
        self.clear_selection();
        self.refresh();
        self.restart_directory_watcher(cx);
        cx.notify();
    }

    pub fn open_file_tag(&mut self, tag_name: String, cx: &mut Context<Self>) {
        self.clear_shell_menu_cache();
        self.browse_location = BrowseLocation::FileTag {
            tag_name: tag_name.clone(),
        };
        self.back_stack.clear();
        self.forward_stack.clear();
        self.current_dir = home_navigation_path();
        self.clear_selection();
        self._watcher_task.take();
        self._directory_watcher.take();
        self.watched_dir = None;
        self.refresh();
        Self::emit_location_changed(cx);
        cx.notify();
    }

    pub fn open_recycle_bin(&mut self, cx: &mut Context<Self>) {
        self.clear_shell_menu_cache();
        self.browse_location = BrowseLocation::RecycleBin;
        self.back_stack.clear();
        self.forward_stack.clear();
        self.current_dir = platform::recycle_bin_folder().unwrap_or_else(home_navigation_path);
        self.clear_selection();
        self._watcher_task.take();
        self._directory_watcher.take();
        self.watched_dir = None;
        self.refresh();
        cx.notify();
    }

    pub(super) fn emit_location_changed(cx: &mut Context<Self>) {
        cx.notify();
        crate::app_state::AppNavigation::location_changed(cx);
    }

    pub(super) fn refresh(&mut self) {
        let (mut items, error) = match &self.browse_location {
            BrowseLocation::Directory => {
                #[cfg(windows)]
                {
                    let is_network = self.current_dir.to_string_lossy() == r"::{F02C1A0D-BE21-4350-88B0-7367FC96EF3C}"
                        || files_fs::is_network_computer_root(&self.current_dir);
                    if is_network {
                        // Do NOT clear the cache here — only reload() (F5/refresh button)
                        // should force re-enumeration. This keeps the cache alive when
                        // switching tabs or sidebar items back to a network location.
                        (Vec::new(), None)
                    } else {
                        load_files_dir(&self.current_dir, self.read_options, self.sort_preferences)
                    }
                }
                #[cfg(not(windows))]
                {
                    load_files_dir(&self.current_dir, self.read_options, self.sort_preferences)
                }
            }
            BrowseLocation::RecycleBin => {
                match read_recycle_bin(self.read_options, self.sort_preferences) {
                    Ok(items) => (items, None),
                    Err(error) => (Vec::new(), Some(error.to_string())),
                }
            }
            BrowseLocation::FileTag { tag_name } => {
                let paths = paths_for_file_tag(tag_name);
                (
                    file_items_for_tag_paths(&paths, self.read_options, self.sort_preferences),
                    None,
                )
            }
            BrowseLocation::SearchResults { .. } => (Vec::new(), None),
        };
        let tag_index = build_path_tag_index_from_config();
        apply_tags_to_items(&mut items, &tag_index);
        sort_items(&mut items, self.sort_preferences);
        self.items = items;
        self.error = error;
        self.apply_filter();
        if self.view_mode == ViewMode::Columns && self.browse_location == BrowseLocation::Directory
        {
            self.refresh_column_listings();
        }
        self.reconcile_selection();
        self.clamp_focused_index();
        self.list_icon_warm_token = self.list_icon_warm_token.wrapping_add(1);
    }

    pub(super) fn select_column_item(
        &mut self,
        col_index: usize,
        item: &FileItem,
        cx: &mut Context<Self>,
    ) {
        self.active_column_index = Some(col_index);
        match item.kind {
            FileItemKind::Folder => {
                if self.current_dir != item.path {
                    self.back_stack.push(self.current_dir.clone());
                    self.forward_stack.clear();
                }
                self.current_dir = item.path.clone();
                self.record_current_directory_history();
                self.column_trail.truncate(col_index + 1);
                self.column_trail.push(item.path.clone());
                self.column_selected_path = None;
                self.clear_selection();
                self.refresh();
                Self::emit_location_changed(cx);
            }
            FileItemKind::File | FileItemKind::Symlink | FileItemKind::Other => {
                self.open_item(item.path.clone(), item.kind, cx);
            }
        }
    }

    pub(super) fn activate_column(&mut self, col_index: usize, cx: &mut Context<Self>) {
        let Some(path) = self.column_trail.get(col_index).cloned() else {
            return;
        };

        self.active_column_index = Some(col_index);
        self.current_dir = path;
        self.record_current_directory_history();
        Self::emit_location_changed(cx);
        cx.notify();
    }

    pub(super) fn column_selection_name(&self, col_index: usize) -> Option<String> {
        let next = self.column_trail.get(col_index + 1)?;
        next.file_name().map(|n| n.to_string_lossy().to_string())
    }

    pub(super) fn clear_shell_menu_cache(&mut self) {
        platform::clear_shell_menu_session();
        if let Ok(mut guard) = self.shell_menu_cache.write() {
            *guard = None;
        }
        self._shell_menu_task.take();
        self.shell_menu_fetch_paths = None;
    }

    pub(super) fn navigate_to(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if !matches!(self.browse_location, BrowseLocation::Directory) {
            self.browse_location = BrowseLocation::Directory;
        }
        let path = normalize_directory_path(&path);
        if paths_equal_directory(&path, &self.current_dir) {
            return;
        }

        self.clear_shell_menu_cache();
        self.collapsed_groups.clear();
        self.back_stack.push(self.current_dir.clone());
        self.forward_stack.clear();
        self.current_dir = path;
        self.record_current_directory_history();
        self.clear_selection();
        self.refresh();
        Self::emit_location_changed(cx);
    }

    pub(super) fn navigate_back(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.back_stack.pop() else {
            return;
        };

        self.forward_stack.push(self.current_dir.clone());
        self.current_dir = path;
        self.record_current_directory_history();
        if matches!(self.browse_location, BrowseLocation::SearchResults { .. }) {
            self.browse_location = BrowseLocation::Directory;
            self.cancel_pending_search(cx);
        }
        self.clear_selection();
        self.refresh();
        Self::emit_location_changed(cx);
    }

    pub(super) fn navigate_forward(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.forward_stack.pop() else {
            return;
        };

        self.back_stack.push(self.current_dir.clone());
        self.current_dir = path;
        self.record_current_directory_history();
        self.clear_selection();
        self.refresh();
        Self::emit_location_changed(cx);
    }

    pub(super) fn navigate_parent(&mut self, cx: &mut Context<Self>) {
        if let Some(parent) = self.current_dir.parent() {
            self.navigate_to(parent.to_path_buf(), cx);
        }
    }

    pub fn select_path_after_load(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if let Some(index) = self
            .display_items
            .iter()
            .position(|item| item.path == path)
        {
            self.focused_index = Some(index);
            self.selected_paths.clear();
            self.selected_paths.insert(path);
            if let Some(row) = row_for_item_index(&self.display_rows, index) {
                self.scroll_handle
                    .scroll_to_item(row, ScrollStrategy::Center);
            }
        }
        cx.notify();
    }

    pub(crate) fn clear_selection(&mut self) {
        self.selected_paths.clear();
        self.anchor_index = None;
        self.focused_index = None;
        self.column_selected_path = None;
    }
}
