use std::borrow::BorrowMut;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use cyber_desktop_fs::{ClipboardOperation, FileClipboard};
use gpui::{App, AppContext, Entity, Global, SharedString, Window};

use crate::main_page::MainPage;
use crate::shell::navigation::NavigationTarget;

/// Unique identifier for a background transfer job.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TransferJobId(pub u64);

/// Lifecycle of a transfer job shown in StatusCenter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TransferJobStatus {
    Running,
    Completed,
    Cancelled,
    Failed,
}

/// A single background file operation (copy, move, compress, paste).
#[derive(Clone)]
pub struct TransferJob {
    pub id: TransferJobId,
    pub message: SharedString,
    status: Arc<RwLock<TransferJobStatus>>,
    completed: Arc<AtomicU32>,
    pub total: u32,
    cancel: Arc<AtomicBool>,
}

impl TransferJob {
    pub fn new(id: TransferJobId, message: SharedString, total: u32) -> Self {
        Self {
            id,
            message,
            status: Arc::new(RwLock::new(TransferJobStatus::Running)),
            completed: Arc::new(AtomicU32::new(0)),
            total: total.max(1),
            cancel: Arc::new(AtomicBool::new(false)),
        }
    }

    pub fn cancel_flag(&self) -> Arc<AtomicBool> {
        self.cancel.clone()
    }

    pub fn request_cancel(&self) {
        self.cancel.store(true, Ordering::Relaxed);
    }

    pub fn set_completed(&self, completed: u32) {
        self.completed.store(completed, Ordering::Relaxed);
    }

    pub fn completed(&self) -> u32 {
        self.completed.load(Ordering::Relaxed)
    }

    pub fn fraction(&self) -> f32 {
        (self.completed() as f32 / self.total as f32).clamp(0., 1.)
    }

    pub fn status(&self) -> TransferJobStatus {
        self.status.read().ok().map(|g| *g).unwrap_or(TransferJobStatus::Running)
    }

    fn set_status(&self, status: TransferJobStatus) {
        if let Ok(mut guard) = self.status.write() {
            *guard = status;
        }
    }

    pub fn is_active(&self) -> bool {
        self.status() == TransferJobStatus::Running
    }
}

/// StatusCenter queue: multiple concurrent background file operations.
#[derive(Clone, Default)]
pub struct TransferStatusGlobal {
    jobs: Arc<RwLock<Vec<TransferJob>>>,
    next_id: Arc<AtomicU64>,
}

impl Global for TransferStatusGlobal {}

impl TransferStatusGlobal {
    pub fn init(cx: &mut App) {
        cx.set_global(Self::default());
    }

    /// Start a new job and return its ID + cancel flag.
    pub fn begin(message: SharedString, total: u32, cx: &mut App) -> (TransferJobId, Arc<AtomicBool>) {
        let Some(global) = cx.try_global::<Self>() else {
            let cancel = Arc::new(AtomicBool::new(false));
            return (TransferJobId(0), cancel);
        };
        let id = TransferJobId(global.next_id.fetch_add(1, Ordering::Relaxed));
        let job = TransferJob::new(id, message, total);
        let cancel = job.cancel_flag();
        if let Ok(mut guard) = global.jobs.write() {
            guard.push(job);
        }
        Self::notify_main_page(cx);
        (id, cancel)
    }

    pub fn set_progress(id: TransferJobId, completed: u32, cx: &mut App) {
        let Some(global) = cx.try_global::<Self>() else { return };
        if let Ok(guard) = global.jobs.read() {
            if let Some(job) = guard.iter().find(|j| j.id == id) {
                job.set_completed(completed);
            }
        }
        Self::notify_main_page(cx);
    }

    pub fn end(id: TransferJobId, cx: &mut App) {
        let Some(global) = cx.try_global::<Self>() else { return };
        if let Ok(guard) = global.jobs.read() {
            if let Some(job) = guard.iter().find(|j| j.id == id) {
                job.set_status(TransferJobStatus::Completed);
                job.set_completed(job.total);
            }
        }
        Self::notify_main_page(cx);
    }

    pub fn fail(id: TransferJobId, cx: &mut App) {
        let Some(global) = cx.try_global::<Self>() else { return };
        if let Ok(guard) = global.jobs.read() {
            if let Some(job) = guard.iter().find(|j| j.id == id) {
                job.set_status(TransferJobStatus::Failed);
            }
        }
        Self::notify_main_page(cx);
    }

    pub fn cancel(id: TransferJobId, cx: &mut App) {
        let Some(global) = cx.try_global::<Self>() else { return };
        if let Ok(guard) = global.jobs.read() {
            if let Some(job) = guard.iter().find(|j| j.id == id) {
                job.request_cancel();
                job.set_status(TransferJobStatus::Cancelled);
            }
        }
        Self::notify_main_page(cx);
    }

    pub fn request_cancel(id: TransferJobId, cx: &mut App) {
        let Some(global) = cx.try_global::<Self>() else { return };
        if let Ok(guard) = global.jobs.read() {
            if let Some(job) = guard.iter().find(|j| j.id == id) {
                job.request_cancel();
            }
        }
        Self::notify_main_page(cx);
    }

    /// Remove a single finished job from the list.
    pub fn dismiss(id: TransferJobId, cx: &mut App) {
        let Some(global) = cx.try_global::<Self>() else { return };
        if let Ok(mut guard) = global.jobs.write() {
            guard.retain(|j| j.id != id);
        }
        Self::notify_main_page(cx);
    }

    /// Remove all finished jobs (Completed / Cancelled / Failed).
    pub fn dismiss_completed(cx: &mut App) {
        let Some(global) = cx.try_global::<Self>() else { return };
        if let Ok(mut guard) = global.jobs.write() {
            guard.retain(|j| j.is_active());
        }
        Self::notify_main_page(cx);
    }

    pub fn all_jobs(cx: &App) -> Vec<TransferJob> {
        cx.try_global::<Self>()
            .and_then(|g| g.jobs.read().ok().map(|j| j.clone()))
            .unwrap_or_default()
    }

    pub fn has_finished(cx: &App) -> bool {
        Self::all_jobs(cx).iter().any(|j| !j.is_active())
    }

    fn notify_main_page(cx: &mut App) {
        if let Some(nav) = cx.try_global::<AppNavigation>() {
            let page = nav.main_page();
            let _ = page.update(cx, |_, cx| cx.notify());
        }
    }
}

/// Global handle so Home / pinned sidebar items can request tab navigation.
pub struct AppNavigation(Entity<MainPage>);

impl Global for AppNavigation {}

impl AppNavigation {
    pub fn set(main_page: Entity<MainPage>, cx: &mut App) {
        cx.set_global(Self(main_page));
    }

    pub fn main_page(&self) -> Entity<MainPage> {
        self.0.clone()
    }

    pub fn navigate_to_path(path: PathBuf, cx: &mut (impl AppContext + BorrowMut<App>)) {
        let page = cx.borrow_mut().global::<Self>().0.clone();
        page.update(cx, |page, cx| {
            page.navigate_to(NavigationTarget::Path(path), cx);
        });
    }

    pub fn navigate_to_file_tag(tag_name: String, cx: &mut (impl AppContext + BorrowMut<App>)) {
        let page = cx.borrow_mut().global::<Self>().0.clone();
        page.update(cx, |page, cx| {
            page.navigate_to(NavigationTarget::FileTag(tag_name), cx);
        });
    }

    pub fn open_path_in_new_tab(path: PathBuf, cx: &mut (impl AppContext + BorrowMut<App>)) {
        let page = cx.borrow_mut().global::<Self>().0.clone();
        page.update(cx, |page, cx| {
            page.open_path_in_new_tab(path, cx);
        });
    }

    /// Open a path in the secondary pane (enables dual pane if needed).
    pub fn open_path_in_secondary_pane(path: PathBuf, cx: &mut (impl AppContext + BorrowMut<App>)) {
        let page = cx.borrow_mut().global::<Self>().0.clone();
        page.update(cx, |page, cx| page.open_path_in_secondary_pane(path, cx));
    }

    pub fn focus_search(window: &mut Window, cx: &mut (impl AppContext + BorrowMut<App>)) {
        let page = cx.borrow_mut().global::<Self>().0.clone();
        page.update(cx, |page, cx| page.focus_search_input(window, cx));
    }

    pub fn cancel_breadcrumb_drag_preview(cx: &mut (impl AppContext + BorrowMut<App>)) {
        let page = cx.borrow_mut().global::<Self>().0.clone();
        page.update(cx, |page, _cx| {
            page.cancel_breadcrumb_drag_preview();
        });
    }

    /// Notify the shell so Omnibar breadcrumbs/path stay in sync with the active file browser.
    ///
    /// Deferred to avoid panics when called from nested updates (e.g. toolbar back inside `MainPage`).
    pub fn location_changed(cx: &mut (impl AppContext + BorrowMut<App>)) {
        let page = cx.borrow_mut().global::<Self>().0.clone();
        cx.borrow_mut().defer(move |cx| {
            let _ = page.update(cx, |page, cx| {
                // Folder open / back / up in the list must leave path-edit mode and show breadcrumbs.
                page.dismiss_omnibar_path_edit(cx);
                cx.notify();
            });
        });
    }

    pub fn pin_folder(path: PathBuf, cx: &mut (impl AppContext + BorrowMut<App>)) {
        let page = cx.borrow_mut().global::<Self>().0.clone();
        page.update(cx, |page, cx| {
            page.pin_folder_path(path, cx);
            page.refresh_home_widgets(cx);
        });
    }

    pub fn unpin_folder(path_string: &str, cx: &mut (impl AppContext + BorrowMut<App>)) {
        let page = cx.borrow_mut().global::<Self>().0.clone();
        page.update(cx, |page, cx| {
            page.unpin_folder_path(path_string, cx);
            page.refresh_home_widgets(cx);
        });
    }

    pub fn refresh_home_widgets(cx: &mut (impl AppContext + BorrowMut<App>)) {
        let nav = cx.borrow_mut().global::<Self>().0.clone();
        nav.update(cx, |page, cx| page.refresh_home_widgets(cx));
    }

    /// Refresh Home quick-access widgets and sidebar QA entries after Shell changes.
    pub fn refresh_quick_access(cx: &mut (impl AppContext + BorrowMut<App>)) {
        let page = cx.borrow_mut().global::<Self>().0.clone();
        page.update(cx, |page, cx| {
            page.refresh_sidebar_cache(cx);
            page.refresh_home_widgets(cx);
        });
    }
}

pub fn breadcrumb_navigation_target(path: &std::path::Path) -> NavigationTarget {
    let key = path.to_string_lossy();
    if key.eq_ignore_ascii_case("home") {
        NavigationTarget::Home
    } else if key.eq_ignore_ascii_case("settings") {
        NavigationTarget::Settings
    } else if key.eq_ignore_ascii_case("recycle") {
        NavigationTarget::RecycleBin
    } else if let Some(name) = key.strip_prefix("tag:") {
        NavigationTarget::FileTag(name.to_string())
    } else {
        NavigationTarget::Path(path.to_path_buf())
    }
}

/// In-app file clipboard for copy/cut/paste between folders (Files ShelfPane data source).
pub struct AppFileClipboard(Option<FileClipboard>);

impl Global for AppFileClipboard {}

impl Default for AppFileClipboard {
    fn default() -> Self {
        Self(None)
    }
}

impl AppFileClipboard {
    pub fn peek(cx: &App) -> Option<FileClipboard> {
        cx.try_global::<Self>().and_then(|c| c.0.clone())
    }

    pub fn take(cx: &mut (impl AppContext + BorrowMut<App>)) -> Option<FileClipboard> {
        let taken = cx.borrow_mut().global_mut::<Self>().0.take();
        if taken.is_some() {
            Self::notify_main_page(cx);
        }
        taken
    }

    pub fn store(
        operation: ClipboardOperation,
        paths: Vec<PathBuf>,
        cx: &mut (impl AppContext + BorrowMut<App>),
    ) {
        cx.borrow_mut()
            .set_global(Self(Some(FileClipboard::new(operation, paths))));
        Self::notify_main_page(cx);
    }

    pub fn set(clipboard: FileClipboard, cx: &mut (impl AppContext + BorrowMut<App>)) {
        cx.borrow_mut().set_global(Self(Some(clipboard)));
        Self::notify_main_page(cx);
    }

    pub fn clear(cx: &mut (impl AppContext + BorrowMut<App>)) {
        if cx.borrow_mut().global_mut::<Self>().0.is_some() {
            cx.borrow_mut().global_mut::<Self>().0 = None;
            Self::notify_main_page(cx);
        }
    }

    pub fn has_items(cx: &mut (impl AppContext + BorrowMut<App>)) -> bool {
        cx.borrow_mut()
            .try_global::<Self>()
            .map(|clipboard| clipboard.0.is_some())
            .unwrap_or(false)
    }

    fn notify_main_page(cx: &mut (impl AppContext + BorrowMut<App>)) {
        let Some(page) = cx
            .borrow_mut()
            .try_global::<AppNavigation>()
            .map(|nav| nav.main_page())
        else {
            return;
        };
        let _ = page.update(cx, |_, cx| cx.notify());
    }
}
