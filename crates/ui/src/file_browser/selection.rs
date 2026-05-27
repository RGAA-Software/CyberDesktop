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
                self.column_scroll_handles.truncate(self.column_listings.len());
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
        match kind {
            FileItemKind::Folder => {
                if matches!(self.browse_location, BrowseLocation::FileTag { .. }) {
                    AppNavigation::navigate_to_path(path, cx);
                    return;
                }
                self.navigate_to(path, cx);
            }
            FileItemKind::File | FileItemKind::Symlink | FileItemKind::Other => {
                if let Err(error) = open_with_system(&path) {
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
        if self.display_items.is_empty() {
            return;
        }
        let index = self.focused_index.unwrap_or(0);
        let next =
            (index as isize + delta).clamp(0, self.display_items.len() as isize - 1) as usize;
        self.focused_index = Some(next);
        self.scroll_handle
            .scroll_to_item(next, ScrollStrategy::Center);
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

    pub fn primary_selected_item(&self) -> Option<&FileItem> {
        if self.view_mode == ViewMode::Columns
            && self.browse_location == BrowseLocation::Directory
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

        if self.view_mode == ViewMode::Columns
            && self.browse_location == BrowseLocation::Directory
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
}
