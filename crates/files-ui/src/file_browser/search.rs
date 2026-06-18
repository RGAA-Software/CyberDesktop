use std::borrow::BorrowMut;
use std::sync::atomic::Ordering;

use files_fs::{
    apply_tags_to_items, build_path_tag_index_from_config, parse_search_query, search_folder,
    SearchHit, SearchQuery, SearchScope, SortDirection, SortOption,
};
use gpui::SharedString;
use rust_i18n::t;

use crate::app_state::{TransferJobId, TransferStatusGlobal};

use super::*;

impl FileBrowser {
    pub fn current_browse_location(&self) -> &BrowseLocation {
        &self.browse_location
    }

    pub(super) fn shows_path_column(&self) -> bool {
        matches!(self.browse_location, BrowseLocation::SearchResults { .. })
    }

    pub fn open_global_search(
        &mut self,
        raw_query: String,
        scope: SearchScope,
        cx: &mut Context<Self>,
    ) {
        let parsed = parse_search_query(&raw_query);
        if matches!(parsed, SearchQuery::Plain { ref text } if text.is_empty()) {
            return;
        }

        self.cancel_pending_search(cx);

        if matches!(self.browse_location, BrowseLocation::Directory) {
            self.back_stack.push(self.current_dir.clone());
        }
        self.forward_stack.clear();
        self.clear_shell_menu_cache();
        self.browse_location = BrowseLocation::SearchResults {
            raw_query: raw_query.clone(),
            parsed_query: parsed.clone(),
            scope: scope.clone(),
        };
        self.search_query.clear();
        self._watcher_task.take();
        self._directory_watcher.take();
        self.watched_dir = None;
        self.group_option = GroupOption::None;
        self.collapsed_groups.clear();
        self.sort_preferences.option = SortOption::Path;
        self.sort_preferences.direction = SortDirection::Ascending;
        self.items.clear();
        self.display_items.clear();
        self.display_rows.clear();
        self.error = None;
        self.clear_selection();
        cx.notify();
        self.spawn_search_task(raw_query, scope, parsed, cx);
        Self::emit_location_changed(cx);
    }

    pub(super) fn refresh_search_results(&mut self, cx: &mut Context<Self>) {
        let BrowseLocation::SearchResults {
            raw_query,
            parsed_query,
            scope,
            ..
        } = self.browse_location.clone()
        else {
            return;
        };
        self.cancel_pending_search(cx);
        self.items.clear();
        self.error = None;
        self.apply_filter();
        cx.notify();
        self.spawn_search_task(raw_query, scope, parsed_query, cx);
    }

    fn spawn_search_task(
        &mut self,
        raw_query: String,
        scope: SearchScope,
        parsed: SearchQuery,
        cx: &mut Context<Self>,
    ) {
        let status_message =
            SharedString::from(format!("{}: {raw_query}", t!("search.status.running")));
        let (job_id, cancel) = TransferStatusGlobal::begin(status_message, 1, cx.borrow_mut());
        self._search_cancel = Some(cancel.clone());
        self._search_status_job = Some(job_id);
        let read_options = self.read_options;
        let sort = self.sort_preferences;

        cx.spawn(async move |browser, cx| {
            let cancel_for_task = cancel.clone();
            let result = cx
                .background_spawn(async move { search_folder(scope, &parsed, &cancel_for_task) })
                .await;
            let cancelled = cancel.load(Ordering::Relaxed);
            let failed = result.is_err();

            let _ = browser.update(cx, |browser, cx| {
                if !matches!(
                    browser.browse_location,
                    BrowseLocation::SearchResults { .. }
                ) {
                    browser.finish_search_status_job(job_id, cancelled, failed, cx);
                    return;
                }
                match result {
                    Ok(hits) => {
                        browser.load_search_hits(hits, read_options, sort);
                        browser.error = None;
                    }
                    Err(error) => {
                        browser.items.clear();
                        browser.error = Some(error.to_string());
                        browser.apply_filter();
                    }
                }
                browser._search_cancel = None;
                browser.finish_search_status_job(job_id, cancelled, failed, cx);
                cx.notify();
                Self::emit_location_changed(cx);
            });
        })
        .detach();
    }

    fn finish_search_status_job(
        &mut self,
        job_id: TransferJobId,
        cancelled: bool,
        failed: bool,
        cx: &mut Context<Self>,
    ) {
        if self._search_status_job.take() != Some(job_id) {
            return;
        }
        if cancelled {
            TransferStatusGlobal::cancel(job_id, cx.borrow_mut());
        } else if failed {
            TransferStatusGlobal::fail(job_id, cx.borrow_mut());
        } else {
            TransferStatusGlobal::set_progress(job_id, 1, cx.borrow_mut());
            TransferStatusGlobal::end(job_id, cx.borrow_mut());
        }
    }

    pub(super) fn cancel_pending_search(&mut self, cx: &mut Context<Self>) {
        if let Some(cancel) = self._search_cancel.take() {
            cancel.store(true, Ordering::Relaxed);
        }
        if let Some(job_id) = self._search_status_job.take() {
            TransferStatusGlobal::cancel(job_id, cx.borrow_mut());
        }
    }

    fn load_search_hits(
        &mut self,
        hits: Vec<SearchHit>,
        options: DirectoryReadOptions,
        sort: SortPreferences,
    ) {
        let mut items: Vec<FileItem> = hits
            .into_iter()
            .filter_map(|hit| FileItem::from_path(hit.path, options).ok())
            .collect();
        let tag_index = build_path_tag_index_from_config();
        apply_tags_to_items(&mut items, &tag_index);
        sort_items(&mut items, sort);
        self.items = items;
        self.apply_filter();
    }
}
