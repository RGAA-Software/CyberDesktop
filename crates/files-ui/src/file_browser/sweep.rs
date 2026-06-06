use super::*;

impl FileBrowser {
    pub(super) fn begin_sweep_selection(
        &mut self,
        surface: SweepSelectionSurface,
        start_position: Point<Pixels>,
        modifiers: Modifiers,
        cx: &mut Context<Self>,
    ) {
        let start_index = if modifiers.shift {
            match surface {
                SweepSelectionSurface::Main => self.anchor_index,
                SweepSelectionSurface::Column(col_index) => self
                    .anchor_index
                    .or_else(|| self.implicit_column_selected_index(col_index)),
            }
        } else {
            None
        };
        let base_selection = match surface {
            SweepSelectionSurface::Main => self.selected_paths.clone(),
            SweepSelectionSurface::Column(col_index) => {
                if self.selected_paths.is_empty() {
                    self.implicit_column_base_selection(col_index)
                } else {
                    self.selected_paths.clone()
                }
            }
        };
        self.sweep_selection = Some(SweepSelectionState {
            surface: surface.clone(),
            start_index,
            current_index: None,
            start_position,
            current_position: start_position,
            base_selection,
            modifiers,
        });

        match surface {
            SweepSelectionSurface::Main => {
                self.active_column_index = None;
                if !modifiers.secondary() && !modifiers.shift {
                    self.clear_selection();
                }
            }
            SweepSelectionSurface::Column(col_index) => {
                self.active_column_index = Some(col_index);
                self.column_selected_path = None;
                if !modifiers.secondary() && !modifiers.shift {
                    self.selected_paths.clear();
                }
            }
        }
        cx.notify();
    }

    pub(super) fn update_sweep_pointer(
        &mut self,
        surface: SweepSelectionSurface,
        position: Point<Pixels>,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = self.sweep_selection.as_mut() else {
            return;
        };
        if state.surface != surface || state.current_position == position {
            return;
        }
        state.current_position = position;
        if surface == SweepSelectionSurface::Main {
            self.update_main_sweep_selection_from_rect(cx);
        } else if let SweepSelectionSurface::Column(col_index) = surface {
            self.update_column_sweep_selection_from_rect(col_index, cx);
        }
        cx.notify();
    }

    pub(super) fn update_sweep_selection(
        &mut self,
        surface: SweepSelectionSurface,
        hover_index: usize,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = self.sweep_selection.as_mut() else {
            return;
        };
        if state.surface != surface {
            return;
        }
        if state.start_index.is_none() {
            state.start_index = Some(hover_index);
        }
        if state.current_index == Some(hover_index) {
            return;
        }
        state.current_index = Some(hover_index);

        if state.surface == SweepSelectionSurface::Main {
            self.update_main_sweep_selection_from_rect(cx);
            return;
        } else if let SweepSelectionSurface::Column(col_index) = state.surface {
            self.update_column_sweep_selection_from_rect(col_index, cx);
            return;
        }

        let anchor = state.start_index.unwrap_or(hover_index);
        let (start, end) = if anchor <= hover_index {
            (anchor, hover_index)
        } else {
            (hover_index, anchor)
        };

        let items: Vec<PathBuf> = match state.surface {
            SweepSelectionSurface::Main => self
                .display_items
                .get(start..=end)
                .unwrap_or(&[])
                .iter()
                .map(|item| item.path.clone())
                .collect(),
            SweepSelectionSurface::Column(col_index) => self
                .column_listings
                .get(col_index)
                .and_then(|items| items.get(start..=end))
                .unwrap_or(&[])
                .iter()
                .map(|item| item.path.clone())
                .collect(),
        };

        let mut selected_paths = if state.modifiers.secondary() {
            state.base_selection.clone()
        } else {
            BTreeSet::new()
        };

        if state.modifiers.secondary() {
            for path in items {
                if !selected_paths.insert(path.clone()) {
                    selected_paths.remove(&path);
                }
            }
        } else {
            selected_paths.extend(items);
        }

        self.selected_paths = selected_paths;
        self.focused_index = Some(hover_index);
        self.anchor_index = Some(anchor);
        if let SweepSelectionSurface::Column(col_index) = surface {
            self.active_column_index = Some(col_index);
            self.column_selected_path = None;
        }
        cx.notify();
    }

    pub(super) fn finish_sweep_selection(&mut self) {
        self.sweep_selection = None;
    }

    fn update_main_sweep_selection_from_rect(&mut self, cx: &mut Context<Self>) {
        let Some(state) = self.sweep_selection.as_ref() else {
            return;
        };
        if state.surface != SweepSelectionSurface::Main {
            return;
        }
        let Some(bounds) = self.main_sweep_bounds else {
            return;
        };

        let selection_rect = self.main_sweep_rect(bounds);
        let hit_indices = self
            .main_sweep_bounds
            .map(|bounds| self.main_sweep_hit_indices(selection_rect, bounds))
            .unwrap_or_default();
        let hit_paths = hit_indices
            .into_iter()
            .filter_map(|index| self.display_items.get(index).map(|item| item.path.clone()))
            .collect::<Vec<_>>();

        let mut selected_paths = if state.modifiers.secondary() {
            state.base_selection.clone()
        } else {
            BTreeSet::new()
        };

        if state.modifiers.secondary() {
            for path in hit_paths {
                if !selected_paths.insert(path.clone()) {
                    selected_paths.remove(&path);
                }
            }
        } else {
            selected_paths.extend(hit_paths);
        }

        self.selected_paths = selected_paths;
        self.focused_index = None;
        cx.notify();
    }

    fn update_column_sweep_selection_from_rect(
        &mut self,
        col_index: usize,
        cx: &mut Context<Self>,
    ) {
        let Some(state) = self.sweep_selection.as_ref() else {
            return;
        };
        if state.surface != SweepSelectionSurface::Column(col_index) {
            return;
        }
        let Some(bounds) = self.column_sweep_bounds.get(&col_index).copied() else {
            return;
        };

        let selection_rect = self.sweep_rect_in_bounds(bounds);
        let scroll_y = self
            .column_scroll_handles
            .get(col_index)
            .map(|handle| handle.offset().y)
            .unwrap_or(px(0.));
        let row_h = COLUMN_ROW_SIZE.height;
        let hit_paths = self
            .column_listings
            .get(col_index)
            .into_iter()
            .flatten()
            .enumerate()
            .filter_map(|(index, item)| {
                let row_rect = Bounds::new(
                    point(bounds.left(), bounds.top() + row_h * index + scroll_y),
                    size(bounds.size.width, row_h),
                );
                rects_intersect(selection_rect, row_rect).then_some(item.path.clone())
            })
            .collect::<Vec<_>>();

        let mut selected_paths = if state.modifiers.secondary() {
            state.base_selection.clone()
        } else {
            BTreeSet::new()
        };

        if state.modifiers.secondary() {
            for path in hit_paths {
                if !selected_paths.insert(path.clone()) {
                    selected_paths.remove(&path);
                }
            }
        } else {
            selected_paths.extend(hit_paths);
        }

        self.selected_paths = selected_paths;
        self.active_column_index = Some(col_index);
        self.column_selected_path = None;
        self.focused_index = None;
        cx.notify();
    }

    pub(super) fn main_sweep_rect(&self, bounds: Bounds<Pixels>) -> Bounds<Pixels> {
        self.sweep_rect_in_bounds(bounds)
    }

    pub(super) fn sweep_rect_in_bounds(&self, bounds: Bounds<Pixels>) -> Bounds<Pixels> {
        let state = self
            .sweep_selection
            .as_ref()
            .expect("sweep_rect_in_bounds called without sweep selection");
        let start = clamp_point_to_bounds(state.start_position, bounds);
        let current = clamp_point_to_bounds(state.current_position, bounds);
        let origin = point(start.x.min(current.x), start.y.min(current.y));
        let size = size(
            (start.x.max(current.x) - start.x.min(current.x)).max(px(1.)),
            (start.y.max(current.y) - start.y.min(current.y)).max(px(1.)),
        );
        Bounds::new(origin, size)
    }

    fn main_sweep_hit_indices(
        &self,
        selection_rect: Bounds<Pixels>,
        list_bounds: Bounds<Pixels>,
    ) -> Vec<usize> {
        match self.view_mode {
            ViewMode::Details | ViewMode::List => {
                self.main_list_sweep_hit_indices(selection_rect, list_bounds)
            }
            ViewMode::Grid => self.main_grid_sweep_hit_indices(selection_rect, list_bounds),
            ViewMode::Cards => self.main_cards_sweep_hit_indices(selection_rect, list_bounds),
            ViewMode::Columns => Vec::new(),
        }
    }

    fn main_list_sweep_hit_indices(
        &self,
        selection_rect: Bounds<Pixels>,
        host_bounds: Bounds<Pixels>,
    ) -> Vec<usize> {
        let list_bounds = self.main_list_content_bounds.unwrap_or(host_bounds);
        let scroll_y = self.scroll_handle.offset().y;
        let mut y = list_bounds.top() + scroll_y;
        let row_width = list_bounds.size.width.max(px(0.));

        self.display_rows
            .iter()
            .zip(self.item_sizes.iter())
            .enumerate()
            .filter_map(|(row_index, (_row, row_size))| {
                let row_rect = Bounds::new(
                    point(list_bounds.left(), y),
                    size(row_width, row_size.height),
                );
                y += row_size.height;
                if !rects_intersect(selection_rect, row_rect) {
                    return None;
                }
                item_index_at_row(&self.display_rows, row_index)
            })
            .collect()
    }

    pub(super) fn update_columns_horizontal_scrollbar_state(
        &mut self,
        bounds: Bounds<Pixels>,
        _cx: &mut Context<Self>,
    ) -> bool {
        let overflow = COLUMN_WIDTH * self.column_trail.len().max(1) > bounds.size.width;
        let overflow_changed = self.columns_horizontal_overflow != overflow;
        let bounds_changed = self.columns_shell_bounds != Some(bounds);

        self.columns_shell_bounds = Some(bounds);
        self.columns_horizontal_overflow = overflow;
        self.columns_horizontal_column_count = self.column_trail.len();

        bounds_changed || overflow_changed
    }

    fn main_grid_sweep_hit_indices(
        &self,
        selection_rect: Bounds<Pixels>,
        bounds: Bounds<Pixels>,
    ) -> Vec<usize> {
        let (cell_w, cell_h) = match self.view_size_level {
            1 => (px(96.), px(72.)),
            3 => (px(144.), px(104.)),
            _ => (px(112.), px(80.)),
        };
        let gap = px(8.);
        let padding = px(8.);
        let scroll_y = self.grid_scroll_handle.offset().y;
        let cells_per_row = self.grid_cells_per_row.unwrap_or(1).max(1);

        self.display_items
            .iter()
            .enumerate()
            .filter_map(|(index, _)| {
                let row = index / cells_per_row;
                let col = index % cells_per_row;
                let item_rect = Bounds::new(
                    point(
                        bounds.left() + padding + (cell_w + gap) * col,
                        bounds.top() + padding + (cell_h + gap) * row + scroll_y,
                    ),
                    size(cell_w, cell_h),
                );
                rects_intersect(selection_rect, item_rect).then_some(index)
            })
            .collect()
    }

    fn main_cards_sweep_hit_indices(
        &self,
        selection_rect: Bounds<Pixels>,
        bounds: Bounds<Pixels>,
    ) -> Vec<usize> {
        let cell_w = px(160.);
        let cell_h = px(120.);
        let gap = px(8.);
        let padding = px(8.);
        let scroll_y = self.cards_scroll_handle.offset().y;
        let cells_per_row = self.cards_cells_per_row.unwrap_or(1).max(1);

        self.display_items
            .iter()
            .enumerate()
            .filter_map(|(index, _)| {
                let row = index / cells_per_row;
                let col = index % cells_per_row;
                let item_rect = Bounds::new(
                    point(
                        bounds.left() + padding + (cell_w + gap) * col,
                        bounds.top() + padding + (cell_h + gap) * row + scroll_y,
                    ),
                    size(cell_w, cell_h),
                );
                rects_intersect(selection_rect, item_rect).then_some(index)
            })
            .collect()
    }
}

fn clamp_point_to_bounds(position: Point<Pixels>, bounds: Bounds<Pixels>) -> Point<Pixels> {
    point(
        position.x.max(bounds.origin.x).min(bounds.right()),
        position.y.max(bounds.origin.y).min(bounds.bottom()),
    )
}

fn rects_intersect(a: Bounds<Pixels>, b: Bounds<Pixels>) -> bool {
    a.left() < b.right() && a.right() > b.left() && a.top() < b.bottom() && a.bottom() > b.top()
}

pub(crate) fn point_in_bounds(position: Point<Pixels>, bounds: Bounds<Pixels>) -> bool {
    position.x >= bounds.left()
        && position.x < bounds.right()
        && position.y >= bounds.top()
        && position.y < bounds.bottom()
}

impl FileBrowser {
    pub(super) fn display_item_index_at_position(
        &self,
        position: Point<Pixels>,
        list_bounds: Option<Bounds<Pixels>>,
    ) -> Option<usize> {
        let bounds = list_bounds.or(self.main_sweep_bounds)?;
        let probe = Bounds::new(
            point(position.x - px(0.5), position.y - px(0.5)),
            size(px(1.), px(1.)),
        );
        let mut hits = self.main_sweep_hit_indices(probe, bounds);
        hits.sort_unstable();
        hits.last().copied()
    }

    pub(super) fn column_item_at_position(
        &self,
        position: Point<Pixels>,
    ) -> Option<(usize, usize)> {
        for (&col_index, bounds) in &self.column_sweep_bounds {
            if !point_in_bounds(position, *bounds) {
                continue;
            }
            let scroll_y = self
                .column_scroll_handles
                .get(col_index)
                .map(|handle| handle.offset().y)
                .unwrap_or(px(0.));
            let row_h = COLUMN_ROW_SIZE.height;
            let relative_y = position.y - bounds.top() - scroll_y;
            if relative_y < px(0.) {
                continue;
            }
            let index = (relative_y / row_h).floor() as usize;
            if self
                .column_listings
                .get(col_index)
                .is_some_and(|listing| listing.get(index).is_some())
            {
                return Some((col_index, index));
            }
        }
        None
    }
}
