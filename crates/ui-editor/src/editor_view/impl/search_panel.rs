//! `EngineEditor` — `search_panel`.

use super::super::imports::*;

impl EngineEditor {
    pub(crate) fn search_root(&self) -> PathBuf {
        self.document
            .path()
            .and_then(|p| p.parent())
            .map(Path::to_path_buf)
            .or_else(|| std::env::current_dir().ok())
            .unwrap_or_else(|| PathBuf::from("."))
    }

    pub(crate) fn open_search_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.find = None;
        self.goto = None;
        let seed = {
            let primary = self.document.selections().primary();
            if !primary.is_empty() {
                let text = self.document.buffer().slice_text(primary.range());
                if text.contains('\n') {
                    String::new()
                } else {
                    text
                }
            } else {
                String::new()
            }
        };
        let root = self.search_root();
        match self.search_panel.as_mut() {
            Some(panel) => {
                if !seed.is_empty() {
                    let query = panel.query.clone();
                    query.update(cx, |s, cx| s.set_value(seed, window, cx));
                }
                let query = self.search_panel.as_ref().unwrap().query.clone();
                query.update(cx, |s, cx| s.focus(window, cx));
            }
            None => {
                let query = cx.new(|cx| {
                    InputState::new(window, cx)
                        .placeholder("Find in files")
                        .default_value(seed)
                });
                let mut subs = Vec::new();
                subs.push(cx.subscribe(&query, |this, _, ev: &InputEvent, cx| {
                    if let InputEvent::PressEnter { .. } = ev {
                        this.run_global_search(cx);
                    }
                }));
                query.update(cx, |s, cx| s.focus(window, cx));
                self.search_panel = Some(SearchPanelState {
                    query,
                    root,
                    results: Vec::new(),
                    rows: Vec::new(),
                    status: String::new(),
                    case_sensitive: false,
                    whole_word: false,
                    regex: false,
                    generation: 0,
                    scroll: VirtualListScrollHandle::new(),
                    _subs: subs,
                });
            }
        }
        cx.notify();
    }

    pub(crate) fn close_search_panel(&mut self, cx: &mut Context<Self>) {
        self.search_panel = None;
        self.input_target = InputTarget::Document;
        self.needs_focus = true;
        cx.notify();
    }

    /// Cancels any in-flight global search and points the panel at `root`'s
    /// directory, clearing stale results. Called when switching tabs so the
    /// "Find in Files" scope follows the active file.
    pub(crate) fn retarget_search_panel(&mut self) {
        let root = self.search_root();
        if let Some(panel) = self.search_panel.as_mut() {
            panel.generation += 1; // cancel any pending search
            panel.root = root;
            panel.results.clear();
            panel.rows.clear();
            panel.status.clear();
        }
    }

    /// Kicks off a directory search on a background thread; results are applied
    /// to the panel when ready (stale generations are dropped).
    pub(crate) fn run_global_search(&mut self, cx: &mut Context<Self>) {
        let query = match self.search_panel.as_ref() {
            Some(panel) => panel.query.read(cx).value().to_string(),
            None => return,
        };
        let Some(panel) = self.search_panel.as_mut() else {
            return;
        };
        if query.trim().is_empty() {
            panel.results.clear();
            panel.rows.clear();
            panel.status = String::new();
            cx.notify();
            return;
        }
        panel.generation += 1;
        let generation = panel.generation;
        let root = panel.root.clone();
        let options = GlobalSearchOptions {
            case_sensitive: panel.case_sensitive,
            whole_word: panel.whole_word,
            regex: panel.regex,
            ..Default::default()
        };
        panel.status = "Searching…".to_string();
        cx.notify();

        let task = cx
            .background_executor()
            .spawn(async move { search_directory(&root, &query, &options) });
        cx.spawn(async move |this, cx| {
            let outcome = task.await;
            this.update(cx, |this, cx| {
                let Some(panel) = this.search_panel.as_mut() else {
                    return;
                };
                if panel.generation != generation {
                    return; // a newer search superseded this one
                }
                match outcome {
                    Ok(results) => {
                        let files = results.len();
                        let hits: usize = results.iter().map(|f| f.matches.len()).sum();
                        panel.status = if hits == 0 {
                            "No results".to_string()
                        } else {
                            format!("{hits} matches in {files} files")
                        };
                        panel.results = results;
                        panel.rebuild_rows();
                    }
                    Err(err) => {
                        panel.results.clear();
                        panel.rows.clear();
                        panel.status = format!("Error: {err}");
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn open_search_result(&mut self, path: PathBuf, line_number: u64, cx: &mut Context<Self>) {
        let same = self.document.path() == Some(path.as_path());
        if !same {
            self.open_path_in_tab(path, cx);
        }
        let line = (line_number.saturating_sub(1)) as usize;
        let last = self.document.buffer().line_count().saturating_sub(1);
        let line = line.min(last);
        let target = self
            .document
            .buffer()
            .position_to_char(Position::new(line, 0));
        self.document.set_caret(target);
        self.input_target = InputTarget::Document;
        self.needs_focus = true;
        self.ensure_caret_visible();
        cx.notify();
    }

    // ---- Find / Replace --------------------------------------------------

}
