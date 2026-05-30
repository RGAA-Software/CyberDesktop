//! `EngineEditor` — Find in File (current tab only).

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use super::super::imports::*;
use cyberfiles_text_engine::{
    FindInLinesOutcome, LineMatch, SearchOptions, Searcher, FIND_IN_FILE_MAX_MATCHES,
};

impl EngineEditor {
    /// Path label for the active tab (used as Find in File scope).
    pub(crate) fn find_in_file_scope_path(&self) -> PathBuf {
        self.document
            .path()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("(Untitled)"))
    }

    /// Stops any in-flight Find in File search and clears the running flag.
    pub(crate) fn cancel_find_in_file_search(&mut self) {
        if let Some(panel) = self.search_panel.as_mut() {
            panel.generation = panel.generation.wrapping_add(1);
            if let Some(cancel) = panel.cancel.take() {
                cancel.store(true, Ordering::Release);
            }
            panel.searching = false;
            panel.lines_scanned = 0;
            panel.matches_so_far = 0;
        }
    }

    /// Keeps the panel aligned with the active tab (path + optional selection seed).
    pub(crate) fn sync_search_panel_scope(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
        seed_from_selection: bool,
    ) {
        let scope_path = self.find_in_file_scope_path();
        let seed = if seed_from_selection {
            let primary = self.document.selections().primary();
            if primary.is_empty() {
                None
            } else {
                let text = self.document.buffer().slice_text(primary.range());
                if text.contains('\n') {
                    None
                } else {
                    Some(text)
                }
            }
        } else {
            None
        };
        let Some(panel) = self.search_panel.as_mut() else {
            return;
        };
        panel.scope_path = scope_path;
        if let Some(text) = seed {
            let query = panel.query.clone();
            Self::seed_find_input(&query, text, window, cx);
        }
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
        match self.search_panel.as_mut() {
            Some(_) => {}
            None => {
                let query = cx.new(|cx| {
                    InputState::new(window, cx)
                        .placeholder(t!("editor.search_in_file.placeholder"))
                        .default_value(seed)
                });
                let mut subs = Vec::new();
                subs.push(cx.subscribe(&query, |this, _, ev: &InputEvent, cx| {
                    if let InputEvent::PressEnter { .. } = ev {
                        this.run_find_in_file(cx);
                    }
                }));
                self.search_panel = Some(SearchPanelState {
                    query,
                    scope_path: self.find_in_file_scope_path(),
                    results: Vec::new(),
                    rows: Vec::new(),
                    status: String::new(),
                    searching: false,
                    lines_scanned: 0,
                    matches_so_far: 0,
                    case_sensitive: false,
                    whole_word: false,
                    regex: false,
                    generation: 0,
                    cancel: None,
                    scroll: VirtualListScrollHandle::new(),
                    _subs: subs,
                });
            }
        }
        self.sync_search_panel_scope(window, cx, true);
        if let Some(panel) = self.search_panel.as_ref() {
            let query = panel.query.clone();
            query.update(cx, |s, cx| s.focus(window, cx));
        }
        cx.notify();
    }

    pub(crate) fn close_search_panel(&mut self, cx: &mut Context<Self>) {
        self.cancel_find_in_file_search();
        self.search_panel = None;
        self.input_target = InputTarget::Document;
        self.needs_focus = true;
        cx.notify();
    }

    /// Tab switch: stop search immediately and clear stale results.
    pub(crate) fn retarget_search_panel(&mut self) {
        self.cancel_find_in_file_search();
        let scope_path = self.find_in_file_scope_path();
        if let Some(panel) = self.search_panel.as_mut() {
            panel.scope_path = scope_path;
            panel.results.clear();
            panel.rows.clear();
            panel.status.clear();
        }
    }

    /// Searches the active tab's in-memory buffer on a background thread.
    pub(crate) fn run_find_in_file(&mut self, cx: &mut Context<Self>) {
        let query = match self.search_panel.as_ref() {
            Some(panel) => panel.query.read(cx).value().to_string(),
            None => return,
        };
        if query.trim().is_empty() {
            if let Some(panel) = self.search_panel.as_mut() {
                panel.results.clear();
                panel.rows.clear();
                panel.status = String::new();
            }
            cx.notify();
            return;
        }

        self.cancel_find_in_file_search();

        let buffer = self.document.buffer();
        let line_count = buffer.line_count();
        let mut lines = Vec::with_capacity(line_count);
        for i in 0..line_count {
            lines.push(buffer.line_text(i));
        }
        let scope_path = self.find_in_file_scope_path();

        let Some(panel) = self.search_panel.as_mut() else {
            return;
        };
        panel.scope_path = scope_path.clone();
        let generation = panel.generation;
        let options = SearchOptions {
            case_sensitive: panel.case_sensitive,
            whole_word: panel.whole_word,
            regex: panel.regex,
        };
        let cancel = Arc::new(AtomicBool::new(false));
        panel.cancel = Some(cancel.clone());
        panel.searching = true;
        panel.lines_scanned = 0;
        panel.matches_so_far = 0;
        let Ok(searcher) = Searcher::new(&query, options) else {
            panel.searching = false;
            panel.status = t!("editor.find.bad_pattern").to_string();
            cx.notify();
            return;
        };

        panel.status = t!("editor.search_in_file.searching").to_string();
        cx.notify();

        let lines_done = Arc::new(AtomicUsize::new(0));
        let matches_found = Arc::new(AtomicUsize::new(0));
        let lines_done_bg = lines_done.clone();
        let matches_found_bg = matches_found.clone();

        let task = cx.background_executor().spawn(async move {
            match searcher.find_in_lines(
                &lines,
                &cancel,
                Some(&lines_done_bg),
                Some(&matches_found_bg),
                FIND_IN_FILE_MAX_MATCHES,
            ) {
                FindInLinesOutcome::Cancelled => None,
                FindInLinesOutcome::Ok(hits) => {
                    let matches: Vec<LineMatch> = hits
                        .into_iter()
                        .map(|h| LineMatch {
                            line_number: h.line_number,
                            line_text: h.line_text,
                            start: h.start,
                            end: h.end,
                        })
                        .collect();
                    Some(FileMatches {
                        path: scope_path,
                        matches,
                    })
                }
            }
        });

        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                let still = this
                    .update(cx, |this, cx| {
                        let Some(panel) = this.search_panel.as_mut() else {
                            return false;
                        };
                        if panel.generation != generation || !panel.searching {
                            return false;
                        }
                        panel.lines_scanned = lines_done.load(Ordering::Relaxed);
                        panel.matches_so_far = matches_found.load(Ordering::Relaxed);
                        panel.status = t!(
                            "editor.search_in_file.progress",
                            lines = panel.lines_scanned,
                            matches = panel.matches_so_far
                        )
                        .to_string();
                        cx.notify();
                        true
                    })
                    .ok()
                    .unwrap_or(false);
                if !still {
                    break;
                }
            }
        })
        .detach();

        cx.spawn(async move |this, cx| {
            let outcome = task.await;
            this.update(cx, |this, cx| {
                let Some(panel) = this.search_panel.as_mut() else {
                    return;
                };
                if panel.generation != generation {
                    return;
                }
                panel.searching = false;
                panel.cancel = None;
                match outcome {
                    None => {
                        panel.status.clear();
                    }
                    Some(file) => {
                        let hits = file.matches.len();
                        panel.status = if hits == 0 {
                            t!("editor.search_in_file.no_results").to_string()
                        } else if hits >= FIND_IN_FILE_MAX_MATCHES {
                            t!("editor.search_in_file.limit", hits = hits).to_string()
                        } else {
                            t!("editor.search_in_file.hits", hits = hits).to_string()
                        };
                        panel.results = vec![file];
                        panel.rebuild_rows();
                    }
                }
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn open_search_result(&mut self, path: PathBuf, line_number: u64, cx: &mut Context<Self>) {
        let same = self
            .document
            .path()
            .map(|p| p == path.as_path())
            .unwrap_or(path.as_os_str() == "(Untitled)");
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
}
