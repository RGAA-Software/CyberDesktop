//! Shell context menu session — 1:1 with Files.app.
//!
//! Files creates a **fresh** `ThreadWithMessageQueue` per menu open (`GetContextMenuForFiles`),
//! keeps it alive on the returned `ContextMenu` for lazy submenu expansion + verb invocation, and
//! disposes it (which `Join()`s the thread) when the menu closes. We mirror that: one owning STA
//! thread per session, stored in `SHELL_SESSION`, replaced on the next query and torn down on
//! `clear_session`.

use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use crate::com::ThreadWithMessageQueue;
use crate::context_menu::{self, ShellContextMenuEntry};

/// Max time to wait for a Shell `QueryContextMenu`/enumerate before treating the worker as
/// deadlocked. Files relies on warm-up to never hit this; we keep it as a safety net so a
/// misbehaving third-party extension can't hang the UI forever.
const SHELL_QUERY_TIMEOUT: Duration = Duration::from_secs(6);

/// Max time to wait when releasing a prepared menu / invoking on the owning thread.
const SHELL_OP_TIMEOUT: Duration = Duration::from_secs(6);

/// The owning STA thread for the currently open Shell menu (Files: `ContextMenu._owningThread`).
/// `None` when no menu is open. Replaced per query; dropped (joined) on `clear_session`.
static SHELL_SESSION: Mutex<Option<ThreadWithMessageQueue>> = Mutex::new(None);

/// Only one Shell menu operation at a time — parallel `QueryContextMenu` hangs or poisons Shell.
static SHELL_OP_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn shell_op_lock() -> std::sync::MutexGuard<'static, ()> {
    SHELL_OP_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("shell menu op lock")
}

/// Dispose the current owning thread (Files: `ContextMenu.Dispose` releases `_cMenu` + joins the
/// thread). Releases the prepared menu on its owning thread first, then drops it.
fn dispose_session() {
    let session = SHELL_SESSION.lock().expect("shell session slot").take();
    if let Some(thread) = session {
        // Release the COM menu on the thread that created it (apartment-bound), bounded so a
        // wedged thread doesn't stall teardown.
        let _ = thread.post_with_timeout(context_menu::release_prepared_menu, SHELL_OP_TIMEOUT);
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
    let _guard = shell_op_lock();
    // Files disposes the previous ContextMenu before opening a new one.
    dispose_session();

    // Files: `var owningThread = new ThreadWithMessageQueue();`
    let thread = ThreadWithMessageQueue::new("cyber_desktop-shell-menu");
    let job_paths = paths.to_vec();
    let outcome = thread.post_with_timeout(
        move || {
            context_menu::prepare_and_enumerate_top_level(
                &job_paths,
                extended_verbs,
                menu_icon_extract_px,
            )
        },
        SHELL_QUERY_TIMEOUT,
    );

    match outcome {
        Some(result) => {
            // Keep the owning thread for lazy submenu + invoke (Files keeps `_owningThread`).
            *SHELL_SESSION.lock().expect("shell session slot") = Some(thread);
            result
        }
        None => {
            tracing::warn!(
                target: "shell_menu",
                "session: QueryContextMenu hung past {:?}; abandoning owning thread, showing no Shell items",
                SHELL_QUERY_TIMEOUT
            );
            // Wedged thread: do not store, do not join (would block). Leak it.
            std::mem::forget(thread);
            Ok(Vec::new())
        }
    }
}

/// Expand one Shell submenu on the owning STA thread (Files `LoadSubMenu` + `WM_INITMENUPOPUP`).
pub fn load_lazy_submenu(parent_index: u32) -> anyhow::Result<Vec<ShellContextMenuEntry>> {
    let _guard = shell_op_lock();
    let slot = SHELL_SESSION.lock().expect("shell session slot");
    let Some(thread) = slot.as_ref() else {
        return Ok(Vec::new());
    };
    match thread.post_with_timeout(
        move || context_menu::expand_lazy_submenu(parent_index),
        SHELL_OP_TIMEOUT,
    ) {
        Some(result) => result,
        None => Ok(Vec::new()),
    }
}

/// Invoke on the owning STA thread (Files `_owningThread.PostMethod`).
pub fn invoke_on_session(command_offset: u32) -> anyhow::Result<()> {
    let _guard = shell_op_lock();
    let slot = SHELL_SESSION.lock().expect("shell session slot");
    let Some(thread) = slot.as_ref() else {
        anyhow::bail!("no shell menu session for invoke");
    };
    match thread.post_with_timeout(
        move || context_menu::invoke_prepared_menu(command_offset),
        SHELL_OP_TIMEOUT,
    ) {
        Some(result) => result,
        None => anyhow::bail!("shell invoke timed out"),
    }
}
