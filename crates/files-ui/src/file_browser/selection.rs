use super::*;

impl FileBrowser {
    pub(super) fn render_column_sweep_overlay(
        &self,
        col_index: usize,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let state = self.sweep_selection.as_ref()?;
        if state.surface != SweepSelectionSurface::Column(col_index) {
            return None;
        }
        let bounds = self.column_sweep_bounds.get(&col_index).copied()?;
        let selection_rect = self.sweep_rect_in_bounds(bounds);

        Some(
            div()
                .id(("files-column-sweep-selection-overlay", col_index))
                .absolute()
                .left(selection_rect.left() - bounds.left())
                .top(selection_rect.top() - bounds.top())
                .w(selection_rect.size.width)
                .h(selection_rect.size.height)
                .border_1()
                .border_color(cx.theme().primary)
                .bg(cx.theme().primary.opacity(0.18))
                .into_any_element(),
        )
    }

    pub(super) fn render_main_sweep_overlay(&self, cx: &mut Context<Self>) -> Option<AnyElement> {
        let state = self.sweep_selection.as_ref()?;
        if state.surface != SweepSelectionSurface::Main {
            return None;
        }
        let bounds = self.main_sweep_bounds?;
        let selection_rect = self.main_sweep_rect(bounds);
        let left = selection_rect.left() - bounds.origin.x;
        let top = selection_rect.top() - bounds.origin.y;
        let width = selection_rect.size.width;
        let height = selection_rect.size.height;

        Some(
            div()
                .id("files-sweep-selection-overlay")
                .absolute()
                .left(left)
                .top(top)
                .w(width)
                .h(height)
                .border_1()
                .border_color(cx.theme().primary)
                .bg(cx.theme().primary.opacity(0.18))
                .into_any_element(),
        )
    }

    pub(super) fn handle_column_item_click(
        &mut self,
        col_index: usize,
        index: usize,
        item: &FileItem,
        modifiers: Modifiers,
        cx: &mut Context<Self>,
    ) {
        let path = item.path.clone();
        self.active_column_index = Some(col_index);

        if modifiers.shift {
            let anchor = self
                .anchor_index
                .or_else(|| self.implicit_column_selected_index(col_index))
                .unwrap_or(index);
            let (start, end) = if anchor <= index {
                (anchor, index)
            } else {
                (index, anchor)
            };
            self.selected_paths.clear();
            if let Some(items) = self.column_listings.get(col_index) {
                for i in start..=end {
                    if let Some(item) = items.get(i) {
                        self.selected_paths.insert(item.path.clone());
                    }
                }
            }
            self.column_selected_path = None;
            self.focused_index = Some(index);
            return;
        }

        if modifiers.secondary() {
            if self.selected_paths.is_empty() {
                self.selected_paths = self.implicit_column_base_selection(col_index);
            }
            if self.selected_paths.contains(&path) {
                self.selected_paths.remove(&path);
            } else {
                self.selected_paths.insert(path);
            }
            self.column_selected_path = None;
            self.anchor_index = Some(index);
            self.focused_index = Some(index);
            return;
        }

        self.anchor_index = Some(index);
        self.focused_index = Some(index);
        match item.kind {
            FileItemKind::Folder => {
                self.select_column_item(col_index, item, cx);
                self.anchor_index = Some(index);
                self.focused_index = Some(index);
            }
            FileItemKind::File | FileItemKind::Symlink | FileItemKind::Other => {
                self.column_selected_path = Some((col_index, item.path.clone()));
                self.selected_paths.clear();
                self.selected_paths.insert(item.path.clone());
                self.column_trail.truncate(col_index + 1);
                self.column_listings = column_listings_for(
                    &self.column_trail,
                    &self.read_options,
                    self.sort_preferences,
                    &self.search_query,
                );
                self.column_scroll_handles
                    .truncate(self.column_listings.len());
            }
        }
    }

    pub(super) fn implicit_column_selected_index(&self, col_index: usize) -> Option<usize> {
        let selected_path = self.column_trail.get(col_index + 1)?;
        self.column_listings
            .get(col_index)?
            .iter()
            .position(|item| item.path == *selected_path)
    }

    pub(super) fn implicit_column_base_selection(&self, col_index: usize) -> BTreeSet<PathBuf> {
        let mut base = BTreeSet::new();
        if let Some(index) = self.implicit_column_selected_index(col_index) {
            if let Some(item) = self
                .column_listings
                .get(col_index)
                .and_then(|items| items.get(index))
            {
                base.insert(item.path.clone());
            }
        }
        base
    }

    pub(super) fn handle_row_click(
        &mut self,
        index: usize,
        event: &ClickEvent,
        _cx: &mut Context<Self>,
    ) {
        let Some(item) = self.display_items.get(index) else {
            return;
        };
        let path = item.path.clone();
        let modifiers = event.modifiers();

        if modifiers.shift {
            let anchor = self.anchor_index.unwrap_or(index);
            let (start, end) = if anchor <= index {
                (anchor, index)
            } else {
                (index, anchor)
            };
            self.selected_paths.clear();
            for i in start..=end {
                if let Some(item) = self.display_items.get(i) {
                    self.selected_paths.insert(item.path.clone());
                }
            }
        } else if modifiers.secondary() {
            if self.selected_paths.contains(&path) {
                self.selected_paths.remove(&path);
            } else {
                self.selected_paths.insert(path.clone());
            }
            self.anchor_index = Some(index);
        } else {
            self.selected_paths.clear();
            self.selected_paths.insert(path);
            self.anchor_index = Some(index);
        }

        self.focused_index = Some(index);
    }

    pub(super) fn open_item(&mut self, path: PathBuf, kind: FileItemKind, cx: &mut Context<Self>) {
        if matches!(self.browse_location, BrowseLocation::SearchResults { .. }) {
            match kind {
                FileItemKind::Folder => {
                    cx.defer(move |cx| {
                        AppNavigation::navigate_to_path(path, cx);
                    });
                }
                FileItemKind::File | FileItemKind::Symlink | FileItemKind::Other => {
                    if let Some(parent) = path.parent() {
                        let dir = parent.to_path_buf();
                        cx.defer(move |cx| {
                            AppNavigation::navigate_to_directory_and_select(dir, path, cx);
                        });
                    }
                }
            }
            return;
        }

        // Look up the full item so we can check network_category for virtual folder items.
        let network_category = self
            .items
            .iter()
            .find(|item| item.path == path)
            .and_then(|item| item.network_category.as_deref());

        match kind {
            FileItemKind::Folder => {
                if matches!(self.browse_location, BrowseLocation::FileTag { .. }) {
                    AppNavigation::navigate_to_path(path, cx);
                    return;
                }
                // Network items: computers navigate internally, everything else uses ShellExecute
                // so Windows invokes the correct default handler.
                if let Some(cat) = network_category {
                    if cat == "network.category.computer" {
                        self.navigate_to(path, cx);
                    } else {
                        if let Err(error) = platform::shell_execute_open(&path) {
                            self.error = Some(error.to_string());
                        }
                    }
                    return;
                }
                self.navigate_to(path, cx);
            }
            FileItemKind::File | FileItemKind::Symlink | FileItemKind::Other => {
                // Network virtual-folder items: let Windows invoke the correct default handler.
                if network_category.is_some() {
                    if let Err(error) = platform::shell_execute_open(&path) {
                        self.error = Some(error.to_string());
                    }
                    return;
                }
                let use_cybereditor = files_core::open_text_with_cybereditor_enabled()
                    && editor_text_engine::is_cybereditor_openable(&path)
                    && !is_executable_or_script_path(&path);
                let use_cybermediaplayer =
                    files_core::open_media_with_cybermediaplayer_enabled() && is_media_file(&path);
                let result = if use_cybereditor {
                    open_with_cybereditor(&path)
                } else if use_cybermediaplayer {
                    open_with_cybermediaplayer(&path)
                } else {
                    open_with_system(&path)
                };
                if let Err(error) = result {
                    self.error = Some(error.to_string());
                }
            }
        }
    }

    pub(super) fn open_focused(&mut self, cx: &mut Context<Self>) {
        let Some(index) = self.focused_index else {
            return;
        };
        let Some(item) = self.display_items.get(index) else {
            return;
        };
        self.open_item(item.path.clone(), item.kind, cx);
    }

    pub(super) fn reconcile_selection(&mut self) {
        self.selected_paths
            .retain(|path| self.display_items.iter().any(|item| &item.path == path));
        if let Some(index) = self.focused_index {
            if index >= self.display_items.len() {
                self.focused_index = None;
            }
        }
    }

    pub(super) fn clamp_focused_index(&mut self) {
        if self.display_items.is_empty() {
            self.focused_index = None;
            return;
        }
        if let Some(index) = self.focused_index {
            if index >= self.display_items.len() {
                self.focused_index = Some(self.display_items.len() - 1);
            }
        }
    }

    pub(super) fn move_focus(&mut self, delta: isize) {
        if self.display_items.is_empty() || self.display_rows.is_empty() {
            return;
        }
        let start_row = self
            .focused_index
            .and_then(|index| row_for_item_index(&self.display_rows, index))
            .unwrap_or(0);
        let max_row = self.display_rows.len().saturating_sub(1);
        let mut row = start_row;
        loop {
            row = (row as isize + delta).clamp(0, max_row as isize) as usize;
            if let Some(item_index) = item_index_at_row(&self.display_rows, row) {
                self.focused_index = Some(item_index);
                self.selected_paths.clear();
                if let Some(item) = self.display_items.get(item_index) {
                    self.selected_paths.insert(item.path.clone());
                }
                self.anchor_index = Some(item_index);
                self.scroll_handle
                    .scroll_to_item(row, ScrollStrategy::Center);
                return;
            }
            if (delta < 0 && row == 0) || (delta > 0 && row == max_row) {
                return;
            }
        }
    }

    /// Navigate up/down within the current column in Columns view.
    pub(super) fn move_focus_column(&mut self, delta: isize) {
        if self.column_listings.is_empty() {
            return;
        }
        let col_index = self
            .active_column_index
            .unwrap_or_else(|| self.column_listings.len().saturating_sub(1));
        let items = match self.column_listings.get(col_index) {
            Some(items) if !items.is_empty() => items,
            _ => return,
        };

        let current_index = self
            .column_selected_path
            .as_ref()
            .filter(|(c, _)| *c == col_index)
            .and_then(|(_, path)| items.iter().position(|item| item.path == *path))
            .or_else(|| self.implicit_column_selected_index(col_index))
            .unwrap_or(0);

        let new_index = (current_index as isize + delta)
            .clamp(0, items.len().saturating_sub(1) as isize) as usize;

        let item = &items[new_index];
        self.selected_paths.clear();
        self.selected_paths.insert(item.path.clone());
        self.column_selected_path = Some((col_index, item.path.clone()));
        self.focused_index = Some(new_index);
        self.anchor_index = Some(new_index);
        self.active_column_index = Some(col_index);

        if let Some(scroll_handle) = self.column_scroll_handles.get(col_index) {
            scroll_handle.scroll_to_item(new_index, ScrollStrategy::Center);
        }
    }

    /// 2D focus navigation for Grid/Cards view (left/right/up/down within the tile grid).
    pub(super) fn move_focus_2d(&mut self, dx: isize, dy: isize) {
        if self.display_items.is_empty() {
            return;
        }

        let cells_per_row = match self.view_mode {
            ViewMode::Grid => self.grid_cells_per_row.unwrap_or(1).max(1),
            ViewMode::Cards => self.cards_cells_per_row.unwrap_or(1).max(1),
            _ => {
                // Non-grid modes fall back to vertical line-based movement.
                if dx != 0 {
                    return;
                }
                self.move_focus(dy.signum());
                return;
            }
        };

        let current_index = self
            .focused_index
            .unwrap_or(0)
            .min(self.display_items.len() - 1);
        let current_row = current_index / cells_per_row;
        let current_col = current_index % cells_per_row;

        let max_row = (self.display_items.len() - 1) / cells_per_row;
        let max_col = cells_per_row - 1;

        let new_row = (current_row as isize + dy).clamp(0, max_row as isize) as usize;
        let new_col = (current_col as isize + dx).clamp(0, max_col as isize) as usize;

        let new_index = new_row * cells_per_row + new_col;
        if new_index < self.display_items.len() {
            self.focused_index = Some(new_index);
            self.selected_paths.clear();
            if let Some(item) = self.display_items.get(new_index) {
                self.selected_paths.insert(item.path.clone());
            }
            self.anchor_index = Some(new_index);

            match self.view_mode {
                ViewMode::Grid => self
                    .grid_scroll_handle
                    .scroll_to_item(new_row, ScrollStrategy::Center),
                ViewMode::Cards => self
                    .cards_scroll_handle
                    .scroll_to_item(new_row, ScrollStrategy::Center),
                _ => self
                    .scroll_handle
                    .scroll_to_item(new_row, ScrollStrategy::Center),
            }
        }
    }

    pub fn select_all(&mut self) {
        if self.view_mode == ViewMode::Columns {
            let col_index = self
                .active_column_index
                .unwrap_or_else(|| self.column_listings.len().saturating_sub(1));
            if let Some(items) = self.column_listings.get(col_index) {
                self.selected_paths = items.iter().map(|item| item.path.clone()).collect();
                self.column_selected_path = None;
            } else {
                self.selected_paths.clear();
                self.column_selected_path = None;
            }
        } else {
            self.selected_paths = self
                .display_items
                .iter()
                .map(|item| item.path.clone())
                .collect();
        }
        if let Some(index) = self.focused_index {
            self.anchor_index = Some(index);
        } else if !self.display_items.is_empty() {
            self.anchor_index = Some(0);
            self.focused_index = Some(0);
        }
    }

    pub fn selected_file_items(&self) -> Vec<FileItem> {
        self.effective_selected_paths()
            .into_iter()
            .filter_map(|path| {
                self.display_items
                    .iter()
                    .find(|item| item.path == path)
                    .cloned()
                    .or_else(|| {
                        self.column_listings
                            .iter()
                            .flat_map(|list| list.iter())
                            .find(|item| item.path == path)
                            .cloned()
                    })
            })
            .collect()
    }

    pub fn primary_selected_item(&self) -> Option<&FileItem> {
        if self.view_mode == ViewMode::Columns && self.browse_location == BrowseLocation::Directory
        {
            if let Some((selected_col, selected_path)) = self.column_selected_path.as_ref() {
                if let Some(items) = self.column_listings.get(*selected_col) {
                    if let Some(item) = items.iter().find(|item| item.path == *selected_path) {
                        return Some(item);
                    }
                }
            }
        }

        if self.selected_paths.len() == 1 {
            let path = self.selected_paths.iter().next()?;
            return self
                .display_items
                .iter()
                .find(|item| &item.path == path)
                .or_else(|| {
                    self.column_listings
                        .iter()
                        .flat_map(|list| list.iter())
                        .find(|item| &item.path == path)
                });
        }

        if self.view_mode == ViewMode::Columns
            && self.browse_location == BrowseLocation::Directory
            && self.selected_paths.is_empty()
        {
            return self
                .column_listings
                .iter()
                .enumerate()
                .rev()
                .find_map(|(col_index, items)| {
                    let selected_path = self.column_trail.get(col_index + 1)?;
                    items.iter().find(|item| item.path == *selected_path)
                });
        }

        None
    }

    pub(super) fn effective_selected_paths(&self) -> Vec<PathBuf> {
        if !self.selected_paths.is_empty() {
            return self.selected_paths.iter().cloned().collect();
        }

        if self.view_mode == ViewMode::Columns && self.browse_location == BrowseLocation::Directory
        {
            return self.primary_path().into_iter().collect();
        }

        Vec::new()
    }

    pub(super) fn primary_path(&self) -> Option<PathBuf> {
        self.primary_selected_item().map(|item| item.path.clone())
    }

    pub(super) fn selected_paths_vec(&self) -> Vec<PathBuf> {
        self.effective_selected_paths()
    }

    /// Type-ahead jump to file by first-letter(s). Called on character key press.
    pub(super) fn handle_key_char(&mut self, ch: char, cx: &mut Context<Self>) {
        if self.renaming.is_some() {
            return;
        }

        let ch_lower = ch.to_lowercase().to_string();
        let new_string = if self.jump_string.len() == 1 && self.jump_string == ch_lower {
            // Pressing the same single letter again: cycle to next match.
            self.jump_string.clone()
        } else {
            self.jump_string.clone() + &ch_lower
        };
        self.jump_string = new_string;

        // Determine the item list and current index based on view mode.
        let (items, current_index) = match self.view_mode {
            ViewMode::Columns => {
                let col_index = self
                    .active_column_index
                    .unwrap_or_else(|| self.column_listings.len().saturating_sub(1));
                let items = self
                    .column_listings
                    .get(col_index)
                    .cloned()
                    .unwrap_or_default();
                let current = self
                    .column_selected_path
                    .as_ref()
                    .filter(|(c, _)| *c == col_index)
                    .and_then(|(_, path)| items.iter().position(|item| item.path == *path))
                    .or_else(|| self.implicit_column_selected_index(col_index));
                (items, current)
            }
            _ => {
                let items = self.display_items.clone();
                let current = self.focused_index;
                (items, current)
            }
        };

        if items.is_empty() {
            return;
        }

        let search = &self.jump_string;
        let _is_cycling = search.len() == 1;

        // Find match: start after current if cycling with a single letter.
        let match_index = if let Some(current) = current_index {
            let after = items
                .iter()
                .enumerate()
                .skip(current + 1)
                .find(|(_, item)| item.display_name.to_lowercase().starts_with(search))
                .map(|(i, _)| i);
            if let Some(idx) = after {
                Some(idx)
            } else {
                items
                    .iter()
                    .enumerate()
                    .find(|(_, item)| item.display_name.to_lowercase().starts_with(search))
                    .map(|(i, _)| i)
            }
        } else {
            items
                .iter()
                .enumerate()
                .find(|(_, item)| item.display_name.to_lowercase().starts_with(search))
                .map(|(i, _)| i)
        };

        if let Some(idx) = match_index {
            let item = &items[idx];
            match self.view_mode {
                ViewMode::Columns => {
                    let col_index = self
                        .active_column_index
                        .unwrap_or_else(|| self.column_listings.len().saturating_sub(1));
                    self.selected_paths.clear();
                    self.selected_paths.insert(item.path.clone());
                    self.column_selected_path = Some((col_index, item.path.clone()));
                    self.focused_index = Some(idx);
                    self.anchor_index = Some(idx);
                    self.active_column_index = Some(col_index);
                    if let Some(scroll_handle) = self.column_scroll_handles.get(col_index) {
                        scroll_handle.scroll_to_item(idx, ScrollStrategy::Center);
                    }
                }
                _ => {
                    self.focused_index = Some(idx);
                    self.selected_paths.clear();
                    self.selected_paths.insert(item.path.clone());
                    self.anchor_index = Some(idx);
                    if let Some(row) = row_for_item_index(&self.display_rows, idx) {
                        self.scroll_handle
                            .scroll_to_item(row, ScrollStrategy::Center);
                    }
                }
            }
        }

        // Restart clear timer.
        self._jump_string_task.take();
        let entity = cx.entity().clone();
        let _search_clone = self.jump_string.clone();
        self._jump_string_task = Some(cx.spawn(async move |_, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_secs(1))
                .await;
            let _ = entity.update(cx, |this, _| {
                this.jump_string.clear();
            });
        }));
    }
}
