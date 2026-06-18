use super::*;

impl FileBrowser {
    pub(crate) fn cleanup_after_delete(&mut self, deleted_paths: &[PathBuf]) {
        let removes_path = |candidate: &Path| {
            deleted_paths
                .iter()
                .any(|deleted| candidate == deleted || candidate.starts_with(deleted))
        };

        self.back_stack.retain(|path| !removes_path(path));
        self.forward_stack.retain(|path| !removes_path(path));
        self.selected_paths.retain(|path| !removes_path(path));

        if self
            .column_selected_path
            .as_ref()
            .is_some_and(|(_, path)| removes_path(path))
        {
            self.column_selected_path = None;
        }

        if self.view_mode == ViewMode::Columns
            && self.browse_location == BrowseLocation::Directory
            && removes_path(&self.current_dir)
        {
            self.current_dir = self
                .current_dir
                .ancestors()
                .skip(1)
                .find(|path| !removes_path(path))
                .map(Path::to_path_buf)
                .unwrap_or_else(home_navigation_path);
            self.active_column_index = None;
            self.clear_selection();
        }
    }

    pub(super) fn operation_directory(&self) -> PathBuf {
        if self.view_mode == ViewMode::Columns && self.browse_location == BrowseLocation::Directory
        {
            if let Some((col_index, _)) = self.column_selected_path.as_ref() {
                if let Some(path) = self.column_trail.get(*col_index) {
                    return path.clone();
                }
            }
            if let Some(col_index) = self.active_column_index {
                if let Some(path) = self.column_trail.get(col_index) {
                    return path.clone();
                }
            }
            if let Some(parent) = self.selected_paths_common_parent() {
                return parent;
            }
        }

        self.current_dir.clone()
    }

    fn selected_paths_common_parent(&self) -> Option<PathBuf> {
        let mut paths = self.selected_paths.iter();
        let first_parent = paths.next()?.parent()?.to_path_buf();
        paths
            .all(|path| path.parent() == Some(first_parent.as_path()))
            .then_some(first_parent)
    }

    pub(super) fn create_folder_from_selection(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let paths = self.selected_paths_vec();
        if paths.is_empty() {
            return;
        }
        let destination = self.operation_directory();
        let name = unique_new_folder_name(&destination);
        match create_directory(&destination, &name) {
            Ok(dest_dir) => match move_items(&paths, &dest_dir) {
                Ok(()) => {
                    self.error = None;
                    self.refresh();
                    window.push_notification(
                        Notification::success(t!("files.create_folder_from_selection.success")),
                        cx,
                    );
                }
                Err(error) => self.error = Some(error.to_string()),
            },
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    pub fn create_new_folder(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let destination = self.operation_directory();
        let name = unique_new_folder_name(&destination);
        match create_directory(&destination, &name) {
            Ok(path) => {
                self.error = None;
                AppOperationHistory::record(
                    FileOperation::Create {
                        path: path.clone(),
                        is_dir: true,
                    },
                    cx,
                );
                self.refresh();
                if let Some(index) = self.display_items.iter().position(|item| item.path == path) {
                    self.focused_index = Some(index);
                    self.selected_paths.clear();
                    self.selected_paths.insert(path);
                    self.anchor_index = self.focused_index;
                    self.begin_rename(window, cx);
                } else {
                    window.push_notification(
                        Notification::success(t!("files.new_folder.success")),
                        cx,
                    );
                }
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    pub fn create_new_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let destination = self.operation_directory();
        let name = unique_new_file_name(&destination);
        match create_file(&destination, &name) {
            Ok(path) => {
                self.error = None;
                AppOperationHistory::record(
                    FileOperation::Create {
                        path: path.clone(),
                        is_dir: false,
                    },
                    cx,
                );
                self.refresh();
                if let Some(index) = self.display_items.iter().position(|item| item.path == path) {
                    self.focused_index = Some(index);
                    self.selected_paths.clear();
                    self.selected_paths.insert(path);
                    self.anchor_index = self.focused_index;
                    self.begin_rename(window, cx);
                } else {
                    window
                        .push_notification(Notification::success(t!("files.new_file.success")), cx);
                }
            }
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    pub(super) fn copy_paths(&mut self, cx: &mut Context<Self>) {
        let paths = self.selected_paths_vec();
        if paths.is_empty() {
            return;
        }
        let text = paths
            .iter()
            .map(|path| path.to_string_lossy().to_string())
            .collect::<Vec<_>>()
            .join("\n");
        cx.write_to_clipboard(ClipboardItem::new_string(text));
    }

    pub fn copy_items(&mut self, cx: &mut Context<Self>) {
        let paths = self.selected_paths_vec();
        if paths.is_empty() {
            return;
        }
        AppFileClipboard::store(ClipboardOperation::Copy, paths, cx);
    }

    pub fn cut_items(&mut self, cx: &mut Context<Self>) {
        let paths = self.selected_paths_vec();
        if paths.is_empty() {
            return;
        }
        AppFileClipboard::store(ClipboardOperation::Cut, paths, cx);
    }

    pub(super) fn compress_items(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = self.selected_paths_vec();
        if paths.is_empty() {
            return;
        }
        let destination = self.operation_directory();
        let zip_path = match unique_zip_output_path(&paths, &destination) {
            Ok(path) => path,
            Err(error) => {
                self.error = Some(error.to_string());
                return;
            }
        };
        let partial_path = temp_zip_output_path(&zip_path);
        let partial_created = match create_compress_partial_file(&partial_path) {
            Ok(created) => created,
            Err(error) => {
                self.error = Some(error.to_string());
                return;
            }
        };
        self.refresh();
        spawn_compress(
            cx.entity(),
            window,
            cx,
            paths,
            destination,
            zip_path,
            partial_path,
            partial_created,
        );
    }

    pub(super) fn extract_selection(
        &mut self,
        to_subfolder: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let archives: Vec<PathBuf> = self
            .selected_paths_vec()
            .into_iter()
            .filter(|path| detect_archive_format(path).is_some() && !path.is_dir())
            .collect();
        if archives.is_empty() {
            return;
        }
        let base = self.operation_directory();
        let mut jobs = Vec::with_capacity(archives.len());
        for archive in archives {
            let dest = if to_subfolder {
                extract_to_child_dir(&archive, &base)
            } else {
                base.clone()
            };
            if let Err(error) = std::fs::create_dir_all(&dest) {
                self.error = Some(error.to_string());
                return;
            }
            jobs.push((archive, dest));
        }
        self.refresh();
        spawn_extract(cx.entity(), window, cx, jobs);
    }

    pub fn paste_items(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let clipboard = AppFileClipboard::take(cx).or_else(|| {
            let paths = platform::read_clipboard_file_paths();
            (!paths.is_empty()).then(|| FileClipboard::new(ClipboardOperation::Copy, paths))
        });
        let Some(clipboard) = clipboard else {
            return;
        };
        if clipboard.paths.is_empty() {
            return;
        }

        let destination = self.operation_directory();
        let browser = cx.entity();
        spawn_paste_from_clipboard(browser, window, cx, clipboard, destination);
    }

    fn confirm_delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.confirm_delete_inner(window, cx, false);
    }

    fn confirm_delete_permanent(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.confirm_delete_inner(window, cx, true);
    }

    fn confirm_delete_inner(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        permanent: bool,
    ) {
        let paths = self.selected_paths_vec();
        if paths.is_empty() {
            return;
        }

        let count = paths.len();
        let description = SharedString::from(if count == 1 {
            paths[0].display().to_string()
        } else {
            t!("files.delete.description_many", count = count).to_string()
        });
        let paths = paths.clone();
        let browser = cx.entity();
        let title = SharedString::from(if permanent {
            t!("files.delete_permanent.title")
        } else {
            t!("files.delete.title")
        });
        let confirm = SharedString::from(if permanent {
            t!("files.delete_permanent.confirm")
        } else {
            t!("files.delete.confirm")
        });
        let success = SharedString::from(if permanent {
            t!("files.delete_permanent.success")
        } else {
            t!("files.delete.success")
        });

        window.open_alert_dialog(cx, move |alert, _window, _cx| {
            let paths = paths.clone();
            let browser = browser.clone();
            let title = title.clone();
            let confirm = confirm.clone();
            let success = success.clone();
            alert
                .title(title)
                .description(description.clone())
                .button_props(
                    DialogButtonProps::default()
                        .ok_variant(gpui_component::button::ButtonVariant::Danger)
                        .ok_text(confirm)
                        .cancel_text(SharedString::from(t!("files.cancel")))
                        .show_cancel(true),
                )
                .on_ok(move |_dialog, window, cx| {
                    let _ = browser.update(cx, |_, cx| {
                        spawn_delete(
                            cx.entity(),
                            window,
                            cx,
                            paths.clone(),
                            permanent,
                            success.clone(),
                        );
                    });
                    true
                })
        });
    }

    pub(crate) fn finish_delete(&mut self, deleted_paths: &[PathBuf], cx: &mut Context<Self>) {
        self.cleanup_after_delete(deleted_paths);
        self.clear_selection();
        self.reload(cx);
    }

    pub(super) fn perform_delete(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.confirm_delete(window, cx);
    }

    pub(super) fn perform_delete_permanent(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.confirm_delete_permanent(window, cx);
    }

    pub(super) fn perform_restore_recycle(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.browse_location != BrowseLocation::RecycleBin {
            return;
        }
        let paths = self.selected_paths_vec();
        if paths.is_empty() {
            return;
        }
        crate::file_ops::spawn_restore_recycle(
            cx.entity(),
            window,
            cx,
            paths,
            SharedString::from(t!("files.recycle.restore.success")),
        );
    }

    pub(super) fn perform_restore_all_recycle(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.browse_location != BrowseLocation::RecycleBin {
            return;
        }
        let paths: Vec<PathBuf> = self.items.iter().map(|item| item.path.clone()).collect();
        if paths.is_empty() {
            return;
        }
        crate::file_ops::spawn_restore_recycle(
            cx.entity(),
            window,
            cx,
            paths,
            SharedString::from(t!("files.recycle.restore_all.success")),
        );
    }

    pub(super) fn confirm_empty_recycle_bin(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.browse_location != BrowseLocation::RecycleBin {
            return;
        }
        let browser = cx.entity();
        window.open_alert_dialog(cx, move |alert, _window, _cx| {
            let browser = browser.clone();
            alert
                .title(SharedString::from(t!("files.recycle.empty.title")))
                .description(SharedString::from(t!("files.recycle.empty.confirm")))
                .button_props(
                    DialogButtonProps::default()
                        .ok_variant(gpui_component::button::ButtonVariant::Danger)
                        .ok_text(SharedString::from(t!("files.recycle.empty.action")))
                        .cancel_text(SharedString::from(t!("files.cancel")))
                        .show_cancel(true),
                )
                .on_ok(move |_dialog, window, cx| {
                    let _ = browser.update(cx, |_, cx| {
                        crate::file_ops::spawn_empty_recycle_bin(cx.entity(), window, cx);
                    });
                    true
                })
        });
    }
}
