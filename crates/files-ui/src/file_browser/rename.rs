use super::*;

impl FileBrowser {
    pub(super) fn begin_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self.primary_path() else {
            return;
        };
        let default_name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_default();
        let basename_selection_end = rename_basename_selection_end(&path, &default_name);
        let input = cx.new(|cx| InputState::new(window, cx).default_value(default_name));
        let input_for_focus = input.clone();
        let browser = cx.entity().clone();
        let rename_path = path.clone();
        let subscription = cx.subscribe(&input, move |_, _, event: &InputEvent, cx| match event {
            InputEvent::Focus => {
                let input = input_for_focus.clone();
                cx.defer(move |cx| {
                    let Some(window) = cx.active_window() else {
                        return;
                    };
                    let _ = window.update(cx, |_, window, cx| {
                        if let Some(selection_end) = basename_selection_end {
                            let _ = input.update(cx, |state, cx| {
                                state.set_cursor_position(
                                    Position::new(0, selection_end as u32),
                                    window,
                                    cx,
                                );
                            });
                            window.dispatch_action(Box::new(InputSelectToStartOfLine), cx);
                        } else {
                            window.dispatch_action(Box::new(InputSelectAll), cx);
                        }
                    });
                });
            }
            InputEvent::PressEnter { .. } => {
                cx.stop_propagation();
                let browser = browser.clone();
                let rename_path = rename_path.clone();
                cx.defer(move |cx| {
                    let Some(window) = cx.active_window() else {
                        return;
                    };
                    let _ = window.update(cx, |_, window, cx| {
                        let _ = browser.update(cx, |this, cx| {
                            if this
                                .renaming
                                .as_ref()
                                .is_some_and(|renaming| renaming.path == rename_path)
                            {
                                this.commit_rename(window, cx);
                                cx.notify();
                            }
                        });
                    });
                });
            }
            InputEvent::Blur => {
                let browser = browser.clone();
                let rename_path = rename_path.clone();
                cx.defer(move |cx| {
                    let Some(window) = cx.active_window() else {
                        return;
                    };
                    let _ = window.update(cx, |_, _, cx| {
                        let _ = browser.update(cx, |this, cx| {
                            if this
                                .renaming
                                .as_ref()
                                .is_some_and(|renaming| renaming.path == rename_path)
                            {
                                this.cancel_rename(cx);
                                cx.notify();
                            }
                        });
                    });
                });
            }
            _ => {}
        });
        input.update(cx, |state, cx| {
            state.focus(window, cx);
        });
        self.renaming = Some(RenameState {
            path,
            input,
            _subscription: subscription,
        });
    }

    pub(super) fn commit_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(renaming) = self.renaming.take() else {
            return;
        };
        let new_name = renaming.input.read(cx).value();
        match rename_path(&renaming.path, &new_name) {
            Ok(target) => {
                self.error = None;
                if renaming.path != target {
                    AppOperationHistory::record(
                        FileOperation::Rename {
                            from: renaming.path.clone(),
                            to: target.clone(),
                        },
                        cx,
                    );
                }
                let location_changed = self.rewrite_paths_after_rename(&renaming.path, &target);
                if self.selected_paths.remove(&renaming.path) {
                    self.selected_paths.insert(target);
                }
                self.refresh();
                if location_changed || self.watched_dir.as_ref() != Some(&self.current_dir) {
                    self.watched_dir = Some(self.current_dir.clone());
                    self.restart_directory_watcher(cx);
                }
                if location_changed {
                    Self::emit_location_changed(cx);
                }
                window.push_notification(Notification::success(t!("files.rename.success")), cx);
                self.refocus_file_list(cx);
            }
            Err(error) => {
                self.error = Some(error.to_string());
                self.renaming = Some(renaming);
            }
        }
    }

    pub(super) fn cancel_rename(&mut self, cx: &mut Context<Self>) {
        self.renaming = None;
        self.refocus_file_list(cx);
    }

    pub(super) fn cancel_rename_if_active(&mut self, cx: &mut Context<Self>) {
        if self.renaming.is_some() {
            self.cancel_rename(cx);
            cx.notify();
        }
    }

    pub(super) fn refocus_file_list(&self, cx: &mut Context<Self>) {
        let focus = self.focus_handle.clone();
        cx.defer(move |cx| {
            if let Some(window) = cx.active_window() {
                let _ = window.update(cx, |_, window, cx| {
                    window.focus(&focus, cx);
                });
            }
        });
    }

    pub(super) fn renaming_input_for(&self, path: &Path) -> Option<Entity<InputState>> {
        self.renaming
            .as_ref()
            .filter(|renaming| renaming.path == path)
            .map(|renaming| renaming.input.clone())
    }

    pub(super) fn inline_name_editor(
        input: Entity<InputState>,
        centered: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let input = if centered {
            Input::new(&input)
                .appearance(false)
                .small()
                .text_center()
                .into_any_element()
        } else {
            Input::new(&input)
                .appearance(false)
                .small()
                .w_full()
                .into_any_element()
        };

        div()
            .w_full()
            .min_w_0()
            .px_1()
            .py(px(1.))
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().primary)
            .bg(cx.theme().background)
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|_, _, _, cx| {
                    cx.stop_propagation();
                }),
            )
            .child(input)
            .into_any_element()
    }

    fn rewrite_paths_after_rename(&mut self, from: &Path, to: &Path) -> bool {
        let mut location_changed = false;

        if let Some(path) = renamed_path(&self.current_dir, from, to) {
            self.current_dir = path;
            location_changed = true;
        }

        rewrite_path_list(&mut self.back_stack, from, to);
        rewrite_path_list(&mut self.forward_stack, from, to);
        rewrite_path_list(&mut self.column_trail, from, to);

        if let Some(path) = self
            .watched_dir
            .as_ref()
            .and_then(|path| renamed_path(path, from, to))
        {
            self.watched_dir = Some(path);
        }

        if let Some((col_index, selected_path)) = self.column_selected_path.as_mut() {
            if let Some(path) = renamed_path(selected_path, from, to) {
                *selected_path = path;
            } else if *col_index >= self.column_trail.len().saturating_sub(1) {
                self.column_selected_path = None;
            }
        }

        self.column_listings = column_listings_for(
            &self.column_trail,
            &self.read_options,
            self.sort_preferences,
            &self.search_query,
        );
        while self.column_scroll_handles.len() < self.column_listings.len() {
            self.column_scroll_handles.push(VirtualListScrollHandle::new());
        }
        self.column_scroll_handles.truncate(self.column_listings.len());

        location_changed
    }
}

pub(super) fn renamed_path(path: &Path, from: &Path, to: &Path) -> Option<PathBuf> {
    if path == from {
        return Some(to.to_path_buf());
    }
    path.strip_prefix(from)
        .ok()
        .map(|suffix| to.join(suffix))
}

pub(super) fn rewrite_path_list(paths: &mut Vec<PathBuf>, from: &Path, to: &Path) {
    for path in paths.iter_mut() {
        if let Some(updated) = renamed_path(path, from, to) {
            *path = updated;
        }
    }
}

fn rename_basename_selection_end(path: &Path, default_name: &str) -> Option<usize> {
    let extension = path.extension()?;
    if !path.is_file() || extension.is_empty() {
        return None;
    }

    let stem = path.file_stem()?.to_string_lossy();
    let stem_with_dot = default_name.strip_suffix(extension.to_str()?)?.strip_suffix('.')?;
    if stem_with_dot != stem {
        return None;
    }

    Some(stem.encode_utf16().count())
}
