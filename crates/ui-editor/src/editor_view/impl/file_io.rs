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
            self.needs_focus = true;
        } else if target < self.tabs.len() {
            let slot = &mut self.tabs[target];
            slot.document = document;
            slot.syntax = syntax;
            slot.parsed_revision = None;
            slot.scroll_x = px(0.0);
            slot.scroll_y = scroll_y;
            slot.file_meta = meta;
            slot.disk_changed = false;
        }
    }

    pub(crate) fn open_file(&mut self, cx: &mut Context<Self>) {
        let start = self.document.path().map(Path::to_path_buf);
        if let Some(path) = crate::pick_open_file_path(start.as_deref()) {
            self.open_path_in_tab(path, cx);
        }
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
        if let Some(path) = crate::pick_save_file_path(&default) {
            let language = language_for_path(Some(&path));
            self.document.set_language(language);
            self.syntax = SyntaxState::new(language);
            self.parsed_revision = None;
            self.push_recent(path.clone());
            self.spawn_save(path, cx);
        }
    }

    /// Encodes + writes `path` on a background thread, then records the save on
    /// the UI thread. Keeps large-file saves from freezing the window.
    pub(crate) fn spawn_save(&mut self, path: PathBuf, cx: &mut Context<Self>) {
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
                cx.notify();
            });
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
