//! Shell context menu session — 1:1 with Files.app.
//!
//! Files creates a **fresh** `ThreadWithMessageQueue` per menu open (`GetContextMenuForFiles`),
//! keeps it alive on the returned `ContextMenu` for lazy submenu expansion + verb invocation, and
//! disposes it (which `Join()`s the thread) when the menu closes. We mirror that: one owning STA
//! thread per session, stored in `SHELL_SESSION`, replaced on the next query and torn down on
//! `clear_session`.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use crate::com::ThreadWithMessageQueueWithPump;
use crate::context_menu::ShellContextMenuEntry;

/// Max time to wait for a Shell `QueryContextMenu`/enumerate before treating the worker as
/// deadlocked. Files relies on warm-up to never hit this; we keep it as a safety net so a
/// misbehaving third-party extension can't hang the UI forever.
const SHELL_QUERY_TIMEOUT: Duration = Duration::from_secs(6);

/// Max time to wait when releasing a prepared menu / invoking on the owning thread.
const SHELL_OP_TIMEOUT: Duration = Duration::from_secs(6);

/// The owning STA thread for the currently open Shell menu (Files: `ContextMenu._owningThread`).
/// `None` when no menu is open. Replaced per query; dropped (joined) on `clear_session`.
static SHELL_SESSION: Mutex<Option<ThreadWithMessageQueueWithPump>> = Mutex::new(None);

/// Only one Shell menu operation at a time — parallel `QueryContextMenu` hangs or poisons Shell.
static SHELL_OP_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn shell_op_lock() -> std::sync::MutexGuard<'static, ()> {
    SHELL_OP_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("shell menu op lock")
}

/// Non-blocking acquisition for background warm-up: if the user is already interacting
/// with the Shell menu (or another op is in flight), warm-up must yield, not queue.
pub(crate) fn try_shell_op_lock() -> Option<std::sync::MutexGuard<'static, ()>> {
    SHELL_OP_LOCK.get_or_init(|| Mutex::new(())).try_lock().ok()
}

/// Set while the background warm-up thread is running a Shell query. Interactive queries
/// yield immediately so they never queue behind a wedged warm-up or race QueryContextMenu.
static WARMUP_IN_PROGRESS: AtomicBool = AtomicBool::new(false);

pub(crate) fn set_warmup_in_progress(active: bool) {
    WARMUP_IN_PROGRESS.store(active, Ordering::Release);
}

fn warmup_in_progress() -> bool {
    WARMUP_IN_PROGRESS.load(Ordering::Acquire)
}

/// Consecutive interactive `QueryContextMenu` timeouts. Each timeout leaks a wedged STA thread that
/// may sit on the loader lock, so after a few we stop querying Shell extensions entirely
/// for the rest of the process (native menu items are unaffected). Reset on any success.
static CONSECUTIVE_QUERY_TIMEOUTS: AtomicU32 = AtomicU32::new(0);

/// After this many consecutive hangs the Shell query path is fused off.
const QUERY_TIMEOUT_FUSE_THRESHOLD: u32 = 2;

fn shell_query_fused() -> bool {
    CONSECUTIVE_QUERY_TIMEOUTS.load(Ordering::Relaxed) >= QUERY_TIMEOUT_FUSE_THRESHOLD
}

/// Called when background warm-up times out. Logs only — warm-up must not advance the
/// interactive fuse; the user has not triggered a menu query yet.
pub(crate) fn record_shell_query_timeout_from_warmup() {
    tracing::warn!(
        target: "shell_menu",
        "warm-up Layer A timed out; interactive shell menu queries are unaffected until the user opens one"
    );
}

/// Dispose the current owning thread (Files: `ContextMenu.Dispose` releases `_cMenu` + joins the
/// thread). Releases the prepared menu on its owning thread first, then drops it.
fn dispose_session() {
    let session = SHELL_SESSION.lock().expect("shell session slot").take();
    if let Some(thread) = session {
        // Release the COM menu on the thread that created it (apartment-bound), bounded so a
        // wedged thread doesn't stall teardown.
        let _ = thread.post_with_timeout(
            crate::hybrid_shell_session::release_prepared_hybrid_session,
            SHELL_OP_TIMEOUT,
        );
        // Dropping `thread` here runs ThreadWithMessageQueue::drop -> CompleteAdding + bounded join.
    }
}

pub fn clear_session() {
    let _guard = shell_op_lock();
    dispose_session();
}

/// Query top-level Shell verbs only; submenus load lazily (Files `loadSubmenus: false`).
///
/// Mirrors `ContextMenu.GetContextMenuForFiles`: spin up a fresh owning STA thread, build the menu
/// on it, and keep the thread alive for subsequent submenu/invoke calls.
pub fn query_with_session(
    paths: &[PathBuf],
    extended_verbs: bool,
    menu_icon_extract_px: u32,
) -> anyhow::Result<Vec<ShellContextMenuEntry>> {
    if shell_query_fused() {
        tracing::warn!(
            target: "shell_menu",
            "session: query skipped — fused off after {QUERY_TIMEOUT_FUSE_THRESHOLD} consecutive hangs"
        );
        return Ok(Vec::new());
    }
    if warmup_in_progress() {
        tracing::info!(
            target: "shell_menu",
            "session: query skipped — shell warm-up in progress"
        );
        return Ok(Vec::new());
    }
    let _guard = shell_op_lock();
    // Files disposes the previous ContextMenu before opening a new one.
    dispose_session();

    // Files: `var owningThread = new ThreadWithMessageQueue();`
    // We use a pumping STA because third-party shell extensions SendMessage during QueryContextMenu.
    let thread = ThreadWithMessageQueueWithPump::new("cyber_desktop-shell-menu");
    if thread.is_degraded() {
        tracing::warn!(
            target: "shell_menu",
            "session: STA pump unavailable (loader lock busy); skipping without fuse"
        );
        thread.abandon_wedged();
        return Ok(Vec::new());
    }
    let job_paths = paths.to_vec();
    let outcome = thread.post_with_timeout(
        move || {
            crate::context_menu::prepare_and_enumerate_top_level(
                &job_paths,
                extended_verbs,
                menu_icon_extract_px,
            )
        },
        SHELL_QUERY_TIMEOUT,
    );

    match outcome {
        Some(result) => {
            CONSECUTIVE_QUERY_TIMEOUTS.store(0, Ordering::Relaxed);
            // Keep the owning thread for lazy submenu + invoke (Files keeps `_owningThread`).
            *SHELL_SESSION.lock().expect("shell session slot") = Some(thread);
            result
        }
        None => {
            let timeouts = CONSECUTIVE_QUERY_TIMEOUTS.fetch_add(1, Ordering::Relaxed) + 1;
            tracing::warn!(
                target: "shell_menu",
                "session: QueryContextMenu hung past {:?} (consecutive timeout #{timeouts}); \
                 abandoning owning thread, showing no Shell items",
                SHELL_QUERY_TIMEOUT
            );
            if timeouts >= QUERY_TIMEOUT_FUSE_THRESHOLD {
                tracing::warn!(
                    target: "shell_menu",
                    "session: Shell menu queries fused off for this process after {timeouts} consecutive hangs"
                );
            }
            // Wedged thread: terminate it so the loader lock is released; do not join.
            thread.abandon_wedged();
            Ok(Vec::new())
        }
    }
}

/// Expand one Shell submenu on the owning STA thread (Files `LoadSubMenu` + `WM_INITMENUPOPUP`).
pub fn load_lazy_submenu(
    handler_clsid: Option<String>,
    parent_index: u32,
) -> anyhow::Result<Vec<ShellContextMenuEntry>> {
    let _guard = shell_op_lock();
    let slot = SHELL_SESSION.lock().expect("shell session slot");
    let Some(thread) = slot.as_ref() else {
        return Ok(Vec::new());
    };
    match thread.post_with_timeout(
        move || crate::context_menu::expand_lazy_submenu(handler_clsid.as_deref(), parent_index),
        SHELL_OP_TIMEOUT,
    ) {
        Some(result) => result,
        None => Ok(Vec::new()),
    }
}

/// Invoke on the owning STA thread (Files `_owningThread.PostMethod`).
pub fn invoke_on_session(handler_clsid: Option<String>, command_offset: u32) -> anyhow::Result<()> {
    let _guard = shell_op_lock();
    let slot = SHELL_SESSION.lock().expect("shell session slot");
    let Some(thread) = slot.as_ref() else {
        anyhow::bail!("no shell menu session for invoke");
    };
    match thread.post_with_timeout(
        move || crate::context_menu::invoke_prepared_menu(handler_clsid.as_deref(), command_offset),
        SHELL_OP_TIMEOUT,
    ) {
        Some(result) => result,
        None => anyhow::bail!("shell invoke timed out"),
    }
}
