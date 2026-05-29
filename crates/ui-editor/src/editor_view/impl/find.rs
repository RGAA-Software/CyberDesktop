//! `EngineEditor` — `find`.

use super::super::imports::*;
use gpui_component::input::Position as InputPosition;

impl EngineEditor {
    /// Seeds a single-line find/replace field and places the caret after the text.
    pub(crate) fn seed_find_input(
        input: &Entity<InputState>,
        text: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let col = text.chars().count() as u32;
        input.update(cx, |state, cx| {
            state.set_value(text, window, cx);
            if col > 0 {
                state.set_cursor_position(InputPosition::new(0, col), window, cx);
            } else {
                state.focus(window, cx);
            }
        });
    }

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
                    Self::seed_find_input(&query, seed, window, cx);
                } else {
                    find.query.update(cx, |s, cx| s.focus(window, cx));
                }
            }
            None => {
                let initial = seed.unwrap_or_default();
                let query = cx.new(|cx| {
                    InputState::new(window, cx)
                        .placeholder("Find")
                        .default_value(String::new())
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
                Self::seed_find_input(&query, initial, window, cx);
                self.find = Some(FindState {
                    query,
                    replace,
                    replace_mode,
                    case_sensitive: false,
                    whole_word: false,
                    regex: false,
                    status: String::new(),
                    cached_query: String::new(),
                    cached_options: SearchOptions::default(),
                    cached_searcher: None,
                    _subs: subs,
                });
            }
        }
        self.input_target = InputTarget::Document;
        if let Some(find) = self.find.as_mut() {
            find.status.clear();
        }
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

    /// Reuses a compiled [`Searcher`] until query or options change.
    pub(crate) fn find_searcher(&mut self, cx: &App) -> Option<&Searcher> {
        let query = self.find_query(cx);
        let find = self.find.as_mut()?;
        let options = find.options();
        if find.cached_searcher.is_none()
            || find.cached_query != query
            || find.cached_options != options
        {
            find.cached_query = query.clone();
            find.cached_options = options;
            find.cached_searcher = Searcher::new(&find.cached_query, options).ok();
        }
        find.cached_searcher.as_ref()
    }

    /// Full-document match count (Notepad++ Count); runs on a background thread.
    pub(crate) fn do_count(&mut self, cx: &mut Context<Self>) {
        let Some(options) = self.find.as_ref().map(|f| f.options()) else {
            return;
        };
        let query = self.find_query(cx);
        if query.is_empty() {
            return;
        }
        if let Some(find) = self.find.as_mut() {
            find.status = "Counting…".to_string();
        }
        cx.notify();

        let buffer = self.document.buffer().clone();
        let cache_query = query.clone();
        let cache_options = options;
        let task = cx.background_executor().spawn(async move {
            Searcher::new(&query, options).map(|searcher| searcher.count(&buffer))
        });

        cx.spawn(async move |this, cx| {
            let outcome = task.await;
            this.update(cx, |this, cx| {
                let Some(find) = this.find.as_mut() else {
                    return;
                };
                find.status = match outcome {
                    Ok(total) => {
                        if let Ok(searcher) = Searcher::new(&cache_query, cache_options) {
                            find.cached_query = cache_query;
                            find.cached_options = cache_options;
                            find.cached_searcher = Some(searcher);
                        }
                        if total == 0 {
                            "Count: 0 matches".to_string()
                        } else if total == 1 {
                            "Count: 1 match".to_string()
                        } else {
                            format!("Count: {total} matches")
                        }
                    }
                    Err(_) => "Bad pattern".to_string(),
                };
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    pub(crate) fn do_find(&mut self, forward: bool, cx: &mut Context<Self>) {
        let Some(options) = self.find.as_ref().map(|f| f.options()) else {
            return;
        };
        let query = self.find_query(cx);
        if query.is_empty() {
            return;
        }
        let Some(searcher) = self.find_searcher(cx).cloned() else {
            if let Some(find) = self.find.as_mut() {
                find.status = "Bad pattern".to_string();
            }
            cx.notify();
            return;
        };
        let buffer = self.document.buffer();
        let (start, end) = {
            let p = self.document.selections().primary();
            (p.start(), p.end())
        };
        let (found, wrapped) = if forward {
            match searcher.find_next_no_wrap(buffer, end) {
                Some(m) => (Some(m), false),
                None => (
                    searcher.find_next_wrap(buffer, end),
                    true,
                ),
            }
        } else {
            match searcher.find_prev_no_wrap(buffer, start) {
                Some(m) => (Some(m), false),
                None => (
                    searcher.find_prev_wrap(buffer, start),
                    true,
                ),
            }
        };
        let wrapped = wrapped && found.is_some();
        let status = if let Some(m) = found {
            self.document.set_selection(m.start, m.end);
            self.ensure_caret_visible();
            if wrapped {
                if forward {
                    "Reached end, continuing from start".to_string()
                } else {
                    "Reached start, continuing from end".to_string()
                }
            } else {
                String::new()
            }
        } else if forward {
            "No matches".to_string()
        } else {
            "No matches".to_string()
        };
        if let Some(find) = self.find.as_mut() {
            find.status = status;
        }
        cx.notify();
    }

    pub(crate) fn current_match(&self, searcher: &Searcher) -> Option<Match> {
        let p = self.document.selections().primary();
        if p.is_empty() {
            return None;
        }
        let start = p.start();
        let end = p.end();
        let buffer = self.document.buffer();
        searcher
            .find_next_no_wrap(buffer, start)
            .filter(|m| m.start == start && m.end == end)
            .or_else(|| {
                searcher
                    .find_prev_no_wrap(buffer, end)
                    .filter(|m| m.start == start && m.end == end)
            })
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
        let Some(searcher) = self.find_searcher(cx).cloned() else {
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
