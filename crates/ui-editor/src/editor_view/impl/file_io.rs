//! `EngineEditor` — `file_io`.

use super::super::imports::*;

impl EngineEditor {
    pub(crate) fn spawn_load(
        &mut self,
        path: PathBuf,
        target: usize,
        restore: Option<(usize, Pixels)>,
        cx: &mut Context<Self>,
    ) {
        let read_path = path.clone();
        let read = cx
            .background_executor()
            .spawn(async move { load_file(&read_path).ok() });
        cx.spawn(async move |this, cx| {
            let loaded = read.await;
            let _ = this.update(cx, |this, cx| {
                if let Some(loaded) = loaded {
                    this.install_loaded(target, loaded, path, restore, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Installs a freshly-loaded file into tab `target` (live fields if it is the
    /// active tab, otherwise its parked slot). Runs on the main thread.
    pub(crate) fn install_loaded(
        &mut self,
        target: usize,
        loaded: cyberfiles_text_engine::LoadedFile,
        path: PathBuf,
        restore: Option<(usize, Pixels)>,
        _cx: &mut Context<Self>,
    ) {
        let language = language_for_path(Some(&path));
        let meta = read_file_meta(&path);
        self.push_recent(path.clone());
        let mut document = Document::from_loaded(loaded, Some(path), language);
        let len = document.buffer().len_chars();
        let caret = restore.map(|(c, _)| c.min(len)).unwrap_or(0);
        document.set_caret(caret);
        let scroll_y = restore.map(|(_, s)| s).unwrap_or(px(0.0));
        let syntax = SyntaxState::new(language);

        if target == self.active {
            self.document = document;
            self.syntax = syntax;
            self.parsed_revision = None;
            self.scroll_x = px(0.0);
            self.scroll_y = scroll_y;
            self.marked_range = None;
            self.file_meta = meta;
            self.disk_changed = false;
            self.active_folds.clear();
            self.rebuild_display_lines();
            self.needs_focus = true;
            self.retarget_search_panel();
        } else if target < self.tabs.len() {
            let slot = &mut self.tabs[target];
            slot.document = document;
            slot.syntax = syntax;
            slot.parsed_revision = None;
            slot.scroll_x = px(0.0);
            slot.scroll_y = scroll_y;
            slot.file_meta = meta;
            slot.disk_changed = false;
            slot.active_folds.clear();
        }
    }

    pub(crate) fn open_file(&mut self, cx: &mut Context<Self>) {
        let start = self.document.path().map(Path::to_path_buf);
        cx.spawn(async move |this, cx| {
            let path = cx
                .background_spawn(async move {
                    crate::pick_open_file_path(start.as_deref())
                })
                .await;
            let Some(path) = path else {
                return;
            };
            let _ = this.update(cx, |this, cx| {
                this.open_path_in_tab(path, cx);
            });
        })
        .detach();
    }

    /// Opens `path`: re-uses an existing tab already showing it, opens into the
    /// current tab if it's pristine, otherwise spawns a new tab.
    pub(crate) fn open_path_in_tab(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.document.path() == Some(path.as_path()) {
            cx.notify();
            return;
        }
        if let Some(existing) = (0..self.tabs.len())
            .find(|&i| i != self.active && self.tabs[i].document.path() == Some(path.as_path()))
        {
            self.switch_to_tab(existing, cx);
            return;
        }
        if !self.active_is_pristine() {
            self.park_active();
            self.tabs.push(TabSlot::placeholder());
            let index = self.tabs.len() - 1;
            self.activate(index);
        }
        let target = self.active;
        self.spawn_load(path, target, None, cx);
        cx.notify();
    }

    pub(crate) fn new_file(&mut self, cx: &mut Context<Self>) {
        self.document = Document::empty();
        self.syntax = SyntaxState::new("text");
        self.parsed_revision = None;
        self.scroll_y = px(0.0);
        self.scroll_x = px(0.0);
        self.file_meta = None;
        self.disk_changed = false;
        self.marked_range = None;
        self.active_folds.clear();
        self.rebuild_display_lines();
        self.document.set_caret(0);
        cx.notify();
    }

    pub(crate) fn save_file(&mut self, cx: &mut Context<Self>) {
        match self.document.path().map(Path::to_path_buf) {
            Some(path) => self.spawn_save(path, cx),
            None => self.save_file_as(cx),
        }
    }

    pub(crate) fn save_file_as(&mut self, cx: &mut Context<Self>) {
        let default = self
            .document
            .path()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("untitled.txt"));
        cx.spawn(async move |this, cx| {
            let path = cx
                .background_spawn(async move { crate::pick_save_file_path(&default) })
                .await;
            let Some(path) = path else {
                return;
            };
            let _ = this.update(cx, |this, cx| {
                this.apply_save_path_and_spawn_save(path, cx);
            });
        })
        .detach();
    }

    /// Updates language/syntax for a new save path and starts a background write.
    pub(crate) fn apply_save_path_and_spawn_save(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let language = language_for_path(Some(&path));
        self.document.set_language(language);
        self.syntax = SyntaxState::new(language);
        self.parsed_revision = None;
        self.push_recent(path.clone());
        self.spawn_save(path, cx);
    }

    /// Encodes + writes `path` on a background thread, then records the save on
    /// the UI thread. Keeps large-file saves from freezing the window.
    pub(crate) fn spawn_save(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.spawn_save_with(path, cx, |_, _, _| {});
    }

    /// Like [`spawn_save`](Self::spawn_save), then runs `after` on the UI thread with
    /// whether the write succeeded.
    pub(crate) fn spawn_save_with<F>(
        &mut self,
        path: PathBuf,
        cx: &mut Context<Self>,
        after: F,
    ) where
        F: FnOnce(&mut Self, bool, &mut Context<Self>) + Send + 'static,
    {
        let target = self.active;
        let snapshot = self.document.save_snapshot();
        let snap_rev = snapshot.revision;
        let write_path = path.clone();
        let write = cx.background_executor().spawn(async move {
            let bytes = snapshot.encode();
            std::fs::write(&write_path, &bytes).is_ok()
        });
        cx.spawn(async move |this, cx| {
            let ok = write.await;
            let _ = this.update(cx, |this, cx| {
                if ok {
                    this.mark_tab_saved(target, path, snap_rev, cx);
                }
                after(this, ok, cx);
                cx.notify();
            });
        })
        .detach();
    }

    /// Saves the active tab (native save-as dialog on a worker thread when untitled).
    pub(crate) fn save_active_for_close<F>(&mut self, after: F, cx: &mut Context<Self>)
    where
        F: FnOnce(&mut Self, bool, &mut Context<Self>) + Send + 'static,
    {
        if let Some(path) = self.document.path().map(Path::to_path_buf) {
            self.spawn_save_with(path, cx, after);
            return;
        }
        let default = PathBuf::from("untitled.txt");
        cx.spawn(async move |this, cx| {
            let path = cx
                .background_spawn(async move { crate::pick_save_file_path(&default) })
                .await;
            let _ = this.update(cx, |this, cx| match path {
                Some(path) => {
                    let language = language_for_path(Some(&path));
                    this.document.set_language(language);
                    this.syntax = SyntaxState::new(language);
                    this.parsed_revision = None;
                    this.push_recent(path.clone());
                    this.spawn_save_with(path, cx, after);
                }
                None => after(this, false, cx),
            });
        })
        .detach();
    }

    /// Saves dirty tabs one at a time (dialogs off the UI thread), then closes the window.
    pub(crate) fn save_all_dirty_for_close(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let window_handle = window.window_handle();
        let weak = cx.weak_entity();
        cx.spawn(async move |_, cx| {
            loop {
                let prep = match weak.update(cx, |this, cx| {
                    let dirty = this.dirty_tabs();
                    if dirty.is_empty() {
                        return None;
                    }
                    let i = dirty[0];
                    if i != this.active {
                        this.switch_to_tab(i, cx);
                    }
                    let path = this.document.path().map(Path::to_path_buf);
                    let default = PathBuf::from("untitled.txt");
                    let target = this.active;
                    let snapshot = this.document.save_snapshot();
                    let snap_rev = snapshot.revision;
                    Some((path, default, target, snapshot, snap_rev))
                }) {
                    Ok(None) => {
                        let _ = weak.update(cx, |this, cx| {
                            this.pending_close = None;
                            this.allow_window_close = true;
                            cx.notify();
                        });
                        let _ = window_handle.update(cx, |_, window, _| {
                            window.remove_window();
                        });
                        return;
                    }
                    Ok(Some(t)) => t,
                    Err(_) => return,
                };
                let (path_opt, default, target, snapshot, snap_rev) = prep;
                let picked_new_path = path_opt.is_none();

                let path = match path_opt {
                    Some(p) => Some(p),
                    None => {
                        cx.background_spawn(async move {
                            crate::pick_save_file_path(&default)
                        })
                        .await
                    }
                };
                let Some(path) = path else {
                    let _ = weak.update(cx, |this, cx| {
                        this.pending_close = Some(CloseTarget::Window);
                        cx.notify();
                    });
                    return;
                };

                let write_path = path.clone();
                let bytes = snapshot.encode();
                let ok = cx
                    .background_executor()
                    .spawn(async move { std::fs::write(&write_path, &bytes).is_ok() })
                    .await;

                if !ok {
                    let _ = weak.update(cx, |this, cx| {
                        this.pending_close = Some(CloseTarget::Window);
                        cx.notify();
                    });
                    return;
                }

                let _ = weak.update(cx, |this, cx| {
                    if picked_new_path {
                        let language = language_for_path(Some(&path));
                        this.document.set_language(language);
                        this.syntax = SyntaxState::new(language);
                        this.parsed_revision = None;
                        this.push_recent(path.clone());
                    }
                    this.mark_tab_saved(target, path, snap_rev, cx);
                    cx.notify();
                });
            }
        })
        .detach();
    }

    /// Records a completed background save into tab `target` (live or parked).
    pub(crate) fn mark_tab_saved(
        &mut self,
        target: usize,
        path: PathBuf,
        snap_rev: u64,
        _cx: &mut Context<Self>,
    ) {
        let meta = read_file_meta(&path);
        if target == self.active {
            self.document.mark_saved(path, snap_rev);
            self.file_meta = meta;
            self.disk_changed = false;
        } else if target < self.tabs.len() {
            let slot = &mut self.tabs[target];
            slot.document.mark_saved(path, snap_rev);
            slot.file_meta = meta;
            slot.disk_changed = false;
        }
    }

    // ---- Clipboard -------------------------------------------------------

}
