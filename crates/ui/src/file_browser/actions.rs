use super::*;

impl FileBrowser {
    pub(super) fn on_refresh(&mut self, _: &RefreshDirectory, _: &mut Window, cx: &mut Context<Self>) {
        self.refresh();
        cx.notify();
    }

    pub(super) fn on_open_item(&mut self, _: &OpenItem, _: &mut Window, cx: &mut Context<Self>) {
        if self.renaming.is_some() {
            cx.stop_propagation();
            return;
        }
        self.open_focused(cx);
    }

    pub(super) fn on_select_all(&mut self, _: &SelectAll, _: &mut Window, cx: &mut Context<Self>) {
        self.select_all();
        cx.stop_propagation();
        cx.notify();
    }

    pub(super) fn on_rename(&mut self, _: &RenameItem, window: &mut Window, cx: &mut Context<Self>) {
        self.begin_rename(window, cx);
        cx.notify();
    }

    pub(super) fn on_cancel_rename(&mut self, _: &CancelRename, _: &mut Window, cx: &mut Context<Self>) {
        if self.renaming.is_some() {
            self.cancel_rename();
            cx.notify();
        }
    }

    pub(super) fn on_delete(&mut self, _: &DeleteItems, window: &mut Window, cx: &mut Context<Self>) {
        self.perform_delete(window, cx);
        cx.notify();
    }

    pub(super) fn on_delete_permanent(
        &mut self,
        _: &DeleteItemsPermanent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.perform_delete_permanent(window, cx);
        cx.notify();
    }

    pub(super) fn on_copy_items(&mut self, _: &CopyItems, _: &mut Window, cx: &mut Context<Self>) {
        self.copy_items(cx);
        cx.notify();
    }

    pub(super) fn on_cut_items(&mut self, _: &CutItems, _: &mut Window, cx: &mut Context<Self>) {
        self.cut_items(cx);
        cx.notify();
    }

    pub(super) fn on_compress_items(
        &mut self,
        _: &CompressItems,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.compress_items(window, cx);
    }

    pub(super) fn on_paste_items(&mut self, _: &PasteItems, window: &mut Window, cx: &mut Context<Self>) {
        self.paste_items(window, cx);
    }

    pub(super) fn on_new_folder(&mut self, _: &NewFolder, window: &mut Window, cx: &mut Context<Self>) {
        self.create_new_folder(window, cx);
        cx.notify();
    }

    pub(super) fn on_copy_path(&mut self, _: &CopyPath, _: &mut Window, cx: &mut Context<Self>) {
        self.copy_paths(cx);
    }

    pub(super) fn on_navigate_previous(
        &mut self,
        _: &NavigatePrevious,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.move_focus(-1);
        cx.notify();
    }

    pub(super) fn on_navigate_next(&mut self, _: &NavigateNext, _: &mut Window, cx: &mut Context<Self>) {
        self.move_focus(1);
        cx.notify();
    }

    pub(super) fn on_sort_name(&mut self, _: &SortByName, _: &mut Window, cx: &mut Context<Self>) {
        self.set_sort_option(SortOption::Name);
        cx.notify();
    }

    pub(super) fn on_sort_modified(&mut self, _: &SortByModified, _: &mut Window, cx: &mut Context<Self>) {
        self.set_sort_option(SortOption::DateModified);
        cx.notify();
    }

    pub(super) fn on_sort_created(&mut self, _: &SortByCreated, _: &mut Window, cx: &mut Context<Self>) {
        self.set_sort_option(SortOption::DateCreated);
        cx.notify();
    }

    pub(super) fn on_sort_size(&mut self, _: &SortBySize, _: &mut Window, cx: &mut Context<Self>) {
        self.set_sort_option(SortOption::Size);
        cx.notify();
    }

    pub(super) fn on_sort_type(&mut self, _: &SortByType, _: &mut Window, cx: &mut Context<Self>) {
        self.set_sort_option(SortOption::FileType);
        cx.notify();
    }

    pub(super) fn on_toggle_sort_direction(
        &mut self,
        _: &ToggleSortDirection,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.sort_preferences.direction = match self.sort_preferences.direction {
            SortDirection::Ascending => SortDirection::Descending,
            SortDirection::Descending => SortDirection::Ascending,
        };
        self.refresh();
        self.persist_prefs();
        cx.notify();
    }

    pub(super) fn on_toggle_show_hidden(
        &mut self,
        _: &ToggleShowHidden,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.read_options.show_hidden_items = !self.read_options.show_hidden_items;
        self.read_options.show_dot_files = self.read_options.show_hidden_items;
        self.refresh();
        self.persist_prefs();
        cx.notify();
    }

    pub(super) fn on_toggle_show_file_extensions(
        &mut self,
        _: &ToggleShowFileExtensions,
        _: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.read_options.show_file_extensions = !self.read_options.show_file_extensions;
        self.refresh();
        self.persist_prefs();
        cx.notify();
    }

    pub(super) fn on_open_in_new_pane(&mut self, _: &OpenInNewPane, _: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self.primary_path() else {
            return;
        };
        AppNavigation::open_path_in_secondary_pane(path, cx);
    }

    pub(super) fn on_open_in_terminal(
        &mut self,
        _: &OpenInTerminal,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mut paths: Vec<PathBuf> = self
            .selected_paths_vec()
            .into_iter()
            .filter(|path| path.is_dir())
            .collect();
        if paths.is_empty() {
            let Some(path) = self.primary_path() else {
                return;
            };
            paths.push(path);
        }
        if let Err(error) = open_paths_in_terminal(&paths) {
            window.push_notification(
                Notification::error(format!("{}: {error}", t!("files.terminal.error"))),
                cx,
            );
        }
    }

    pub(super) fn on_create_folder_from_selection(
        &mut self,
        _: &CreateFolderFromSelection,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.create_folder_from_selection(window, cx);
    }

    pub(super) fn on_open_in_new_window(
        &mut self,
        _: &OpenInNewWindow,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(path) = self.primary_path() else {
            return;
        };
        if let Err(error) = platform::open_in_new_explorer_window(&path) {
            window.push_notification(
                Notification::error(format!("{}: {error}", t!("files.open_new_window.error"))),
                cx,
            );
        }
    }

    pub(super) fn on_open_with_dialog(
        &mut self,
        _: &OpenWithDialog,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(path) = self.primary_path() else {
            return;
        };
        if path.is_dir() {
            return;
        }
        if let Err(error) = platform::show_open_with_dialog(&path) {
            window.push_notification(
                Notification::error(format!("{}: {error}", t!("files.open_with.error"))),
                cx,
            );
        }
    }

    pub(super) fn on_create_shortcut(
        &mut self,
        _: &CreateShortcut,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let paths = self.selected_paths_vec();
        if paths.is_empty() {
            return;
        }
        if let Err(error) = create_shortcuts_for_paths(&paths) {
            window.push_notification(
                Notification::error(format!("{}: {error}", t!("files.create_shortcut.error"))),
                cx,
            );
        } else {
            window.push_notification(
                Notification::success(t!("files.create_shortcut.success")),
                cx,
            );
            self.refresh();
            cx.notify();
        }
    }

    pub(super) fn on_new_file(&mut self, _: &NewFile, window: &mut Window, cx: &mut Context<Self>) {
        self.create_new_file(window, cx);
        cx.notify();
    }

    pub(super) fn on_view_details(&mut self, _: &ViewDetails, _: &mut Window, cx: &mut Context<Self>) {
        self.set_view_mode(ViewMode::Details, cx);
    }

    pub(super) fn on_view_grid(&mut self, _: &ViewGrid, _: &mut Window, cx: &mut Context<Self>) {
        self.set_view_mode(ViewMode::Grid, cx);
    }

    pub(super) fn on_view_cards(&mut self, _: &ViewCards, _: &mut Window, cx: &mut Context<Self>) {
        self.set_view_mode(ViewMode::Cards, cx);
    }

    pub(super) fn on_view_list(&mut self, _: &ViewList, _: &mut Window, cx: &mut Context<Self>) {
        self.set_view_mode(ViewMode::List, cx);
    }

    pub(super) fn on_view_columns(&mut self, _: &ViewColumns, _: &mut Window, cx: &mut Context<Self>) {
        self.set_view_mode(ViewMode::Columns, cx);
    }

    pub(super) fn on_focus_search_action(
        &mut self,
        _: &FocusSearch,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus_search(window, cx);
    }

    pub(super) fn on_shell_properties(
        &mut self,
        _: &ShellProperties,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.show_properties(cx);
    }

    pub(super) fn show_properties(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.primary_path() else {
            return;
        };
        let path = path.to_path_buf();
        cx.spawn(async move |_, cx| {
            let _ = cx
                .background_spawn(async move { platform::open_item_properties(&path) })
                .await;
        })
        .detach();
    }
}
