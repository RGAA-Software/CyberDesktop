//! `EngineEditor` — `tabs`.

use super::super::imports::*;

impl EngineEditor {
    /// marker). The active tab reads from the live fields; others from the slot.
    pub(crate) fn tab_title(&self, index: usize) -> String {
        let doc = if index == self.active {
            &self.document
        } else {
            &self.tabs[index].document
        };
        let name = doc
            .path()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| t!("editor.untitled").to_string());
        if doc.dirty() {
            format!("{name} \u{2022}")
        } else {
            name
        }
    }

    /// Moves the live fields into `tabs[active]` so the tab can be parked.
    pub(crate) fn park_active(&mut self) {
        let document = std::mem::replace(&mut self.document, Document::empty());
        let syntax = std::mem::replace(&mut self.syntax, SyntaxState::new("text"));
        let slot = &mut self.tabs[self.active];
        slot.document = document;
        slot.syntax = syntax;
        slot.parsed_revision = self.parsed_revision;
        slot.scroll_x = self.scroll_x;
        slot.scroll_y = self.scroll_y;
        slot.file_meta = self.file_meta;
        slot.disk_changed = self.disk_changed;
        slot.active_folds = std::mem::take(&mut self.active_folds);
        slot.show_preview = self.show_preview;
        slot.show_full_preview = self.show_full_preview;
    }

    /// Pulls `tabs[index]` into the live fields and makes it active.
    pub(crate) fn activate(&mut self, index: usize) {
        let slot = &mut self.tabs[index];
        self.document = std::mem::replace(&mut slot.document, Document::empty());
        self.syntax = std::mem::replace(&mut slot.syntax, SyntaxState::new("text"));
        self.parsed_revision = slot.parsed_revision;
        self.scroll_x = slot.scroll_x;
        self.scroll_y = slot.scroll_y;
        self.file_meta = slot.file_meta;
        self.disk_changed = slot.disk_changed;
        self.active_folds = std::mem::take(&mut slot.active_folds);
        self.show_preview = slot.show_preview;
        self.show_full_preview = slot.show_full_preview;
        self.line_width_cache.clear();
        self.syntax_parse_inflight = false;
        self.syntax_parse_target_rev = None;
        self.rebuild_display_lines();
        self.active = index;
        self.marked_range = None;
        self.input_target = InputTarget::Document;
        self.needs_focus = true;
    }

    pub(crate) fn switch_to_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if index == self.active || index >= self.tabs.len() {
            return;
        }
        self.park_active();
        self.activate(index);
        // The Find-in-Files scope follows the active file; cancel any in-flight
        // search and clear stale results.
        self.retarget_search_panel();
        self.sync_markdown_preview(cx);
        set_view_toggles(
            self.show_line_numbers,
            self.soft_wrap,
            self.show_preview,
            self.show_full_preview,
            cx,
        );
        self.pending_tab_scroll_to_ix = Some(index);
        cx.notify();
    }

    pub(crate) fn next_tab(&mut self, delta: isize, cx: &mut Context<Self>) {
        let n = self.tabs.len();
        if n <= 1 {
            return;
        }
        let next = (self.active as isize + delta).rem_euclid(n as isize) as usize;
        self.switch_to_tab(next, cx);
    }

    /// Opens a fresh empty tab and switches to it.
    pub(crate) fn new_tab(&mut self, cx: &mut Context<Self>) {
        self.park_active();
        self.tabs.push(TabSlot::placeholder());
        let index = self.tabs.len() - 1;
        // The slot is already an empty placeholder; activating drains it.
        self.activate(index);
        self.document.set_caret(0);
        self.retarget_search_panel();
        self.sync_markdown_preview(cx);
        set_view_toggles(
            self.show_line_numbers,
            self.soft_wrap,
            self.show_preview,
            self.show_full_preview,
            cx,
        );
        self.pending_tab_scroll_to_ix = Some(index);
        cx.notify();
    }

    pub(crate) fn close_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.tabs.len() {
            return;
        }
        let dirty = if index == self.active {
            self.document.dirty()
        } else {
            self.tabs[index].document.dirty()
        };
        if dirty && self.pending_close.is_none() {
            self.pending_close = Some(CloseTarget::Tab(index));
            cx.notify();
            return;
        }
        self.force_close_tab(index, cx);
    }

    /// Closes tab `index` unconditionally (no unsaved-changes prompt).
    pub(crate) fn force_close_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if index >= self.tabs.len() {
            return;
        }
        if self.tabs.len() == 1 {
            // Last tab: reset it to an empty untitled buffer instead of closing.
            self.new_file(cx);
            return;
        }
        if index == self.active {
            // Park then drop the active slot, then activate a neighbour.
            self.park_active();
            self.tabs.remove(index);
            let next = index.min(self.tabs.len() - 1);
            self.activate(next);
            self.sync_markdown_preview(cx);
            set_view_toggles(
                self.show_line_numbers,
                self.soft_wrap,
                self.show_preview,
                self.show_full_preview,
                cx,
            );
            self.pending_tab_scroll_to_ix = Some(next);
        } else {
            self.tabs.remove(index);
            if self.active > index {
                self.active -= 1;
            }
        }
        cx.notify();
    }

    // ---- Close confirmation ---------------------------------------------

    /// Indices of all tabs with unsaved changes.
    pub(crate) fn dirty_tabs(&self) -> Vec<usize> {
        (0..self.tabs.len())
            .filter(|&i| {
                if i == self.active {
                    self.document.dirty()
                } else {
                    self.tabs[i].document.dirty()
                }
            })
            .collect()
    }

    /// Clean display name (no dirty marker) for tab `index`.
    pub(crate) fn tab_name(&self, index: usize) -> String {
        let doc = if index == self.active {
            &self.document
        } else {
            &self.tabs[index].document
        };
        doc.path()
            .and_then(|p| p.file_name())
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| t!("editor.untitled").to_string())
    }
}
