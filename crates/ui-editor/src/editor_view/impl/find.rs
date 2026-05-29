//! `EngineEditor` — `find`.

use super::super::imports::*;

impl EngineEditor {
    pub(crate) fn open_find(&mut self, replace_mode: bool, window: &mut Window, cx: &mut Context<Self>) {
        self.search_panel = None;
        self.goto = None;
        let primary = self.document.selections().primary();
        let seed = if !primary.is_empty() {
            let text = self.document.buffer().slice_text(primary.range());
            if text.contains('\n') {
                None
            } else {
                Some(text)
            }
        } else {
            None
        };
        match self.find.as_mut() {
            Some(find) => {
                find.replace_mode = replace_mode || find.replace_mode;
                if let Some(seed) = seed {
                    let query = find.query.clone();
                    query.update(cx, |s, cx| s.set_value(seed, window, cx));
                }
                find.query.update(cx, |s, cx| s.focus(window, cx));
            }
            None => {
                let initial = seed.unwrap_or_default();
                let query = cx.new(|cx| {
                    InputState::new(window, cx)
                        .placeholder("Find")
                        .default_value(initial)
                });
                let replace =
                    cx.new(|cx| InputState::new(window, cx).placeholder("Replace with"));
                let mut subs = Vec::new();
                subs.push(cx.subscribe(&query, |this, _, ev: &InputEvent, cx| {
                    if let InputEvent::PressEnter { shift, .. } = ev {
                        this.do_find(!shift, cx);
                    }
                }));
                subs.push(cx.subscribe(&replace, |this, _, ev: &InputEvent, cx| {
                    if let InputEvent::PressEnter { .. } = ev {
                        this.do_replace(cx);
                    }
                }));
                query.update(cx, |s, cx| s.focus(window, cx));
                self.find = Some(FindState {
                    query,
                    replace,
                    replace_mode,
                    case_sensitive: false,
                    whole_word: false,
                    regex: false,
                    status: String::new(),
                    _subs: subs,
                });
            }
        }
        self.input_target = InputTarget::Document;
        self.update_find_status(cx);
        cx.notify();
    }

    pub(crate) fn close_find(&mut self, cx: &mut Context<Self>) {
        self.find = None;
        self.input_target = InputTarget::Document;
        self.needs_focus = true;
        cx.notify();
    }

    /// The current Find query text.
    pub(crate) fn find_query(&self, cx: &App) -> String {
        self.find
            .as_ref()
            .map(|f| f.query.read(cx).value().to_string())
            .unwrap_or_default()
    }

    /// The current Replace-with text.
    pub(crate) fn find_replace_text(&self, cx: &App) -> String {
        self.find
            .as_ref()
            .map(|f| f.replace.read(cx).value().to_string())
            .unwrap_or_default()
    }

    pub(crate) fn update_find_status(&mut self, cx: &mut Context<Self>) {
        let Some(options) = self.find.as_ref().map(|f| f.options()) else {
            return;
        };
        let query = self.find_query(cx);
        let status = if query.is_empty() {
            String::new()
        } else {
            match Searcher::new(&query, options) {
                Ok(searcher) => {
                    let head = self.document.selections().primary().start();
                    let (total, index) = searcher.count_and_index(self.document.buffer(), head);
                    if total == 0 {
                        "No matches".to_string()
                    } else {
                        match index {
                            Some(i) => format!("{i} of {total}"),
                            None => format!("{total} found"),
                        }
                    }
                }
                Err(_) => "Bad pattern".to_string(),
            }
        };
        if let Some(find) = self.find.as_mut() {
            find.status = status;
        }
    }

    pub(crate) fn do_find(&mut self, forward: bool, cx: &mut Context<Self>) {
        let Some(options) = self.find.as_ref().map(|f| f.options()) else {
            return;
        };
        let query = self.find_query(cx);
        if query.is_empty() {
            return;
        }
        let Ok(searcher) = Searcher::new(&query, options) else {
            if let Some(find) = self.find.as_mut() {
                find.status = "Bad pattern".to_string();
            }
            cx.notify();
            return;
        };
        let (start, end) = {
            let p = self.document.selections().primary();
            (p.start(), p.end())
        };
        let found = if forward {
            searcher.find_next(self.document.buffer(), end)
        } else {
            searcher.find_prev(self.document.buffer(), start)
        };
        if let Some(m) = found {
            self.document.set_selection(m.start, m.end);
            self.ensure_caret_visible();
        }
        self.update_find_status(cx);
        cx.notify();
    }

    pub(crate) fn current_match(&self, searcher: &Searcher) -> Option<Match> {
        let (start, end) = {
            let p = self.document.selections().primary();
            if p.is_empty() {
                return None;
            }
            (p.start(), p.end())
        };
        searcher
            .all_matches(self.document.buffer())
            .into_iter()
            .find(|cand| cand.start == start && cand.end == end)
    }

    pub(crate) fn do_replace(&mut self, cx: &mut Context<Self>) {
        let Some((options, regex)) = self.find.as_ref().map(|f| (f.options(), f.regex)) else {
            return;
        };
        let query = self.find_query(cx);
        if query.is_empty() {
            return;
        }
        let replace = self.find_replace_text(cx);
        let Ok(searcher) = Searcher::new(&query, options) else {
            return;
        };
        if let Some(m) = self.current_match(&searcher) {
            let replacement = searcher.replacement_for(self.document.buffer(), m, &replace, regex);
            self.document.replace_range(m.start..m.end, &replacement);
        }
        self.do_find(true, cx);
    }

    pub(crate) fn do_replace_all(&mut self, cx: &mut Context<Self>) {
        let Some((options, regex)) = self.find.as_ref().map(|f| (f.options(), f.regex)) else {
            return;
        };
        let query = self.find_query(cx);
        if query.is_empty() {
            return;
        }
        let replace = self.find_replace_text(cx);
        let Ok(searcher) = Searcher::new(&query, options) else {
            return;
        };
        // Single atomic transaction: O(matches) and one undo step even for tens
        // of thousands of matches.
        let count = self.document.replace_all(&searcher, &replace, regex);
        self.ensure_caret_visible();
        if let Some(find) = self.find.as_mut() {
            find.status = format!("Replaced {count}");
        }
        cx.notify();
    }

    // ---- Input -----------------------------------------------------------

}
