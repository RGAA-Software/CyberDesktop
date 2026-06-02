//! Undo/redo orchestration for file operations.

use files_fs::{apply_redo, apply_undo};
use gpui::{AppContext, Context, Entity, Window};
use gpui_component::{notification::Notification, WindowExt as _};
use rust_i18n::t;

use crate::app_state::AppOperationHistory;
use crate::file_browser::FileBrowser;
use crate::main_page::MainPage;

async fn run_history_undo(cx: &mut gpui::AsyncApp) -> anyhow::Result<()> {
    let op = cx.update(|cx| AppOperationHistory::begin_undo(cx));
    let Some(op) = op else {
        return Err(anyhow::anyhow!("nothing to undo"));
    };
    let op_for_task = op.clone();
    let result = cx
        .background_spawn(async move { apply_undo(&op_for_task) })
        .await;
    cx.update(|cx| AppOperationHistory::finish_undo(cx, op, result))
}

async fn run_history_redo(cx: &mut gpui::AsyncApp) -> anyhow::Result<()> {
    let op = cx.update(|cx| AppOperationHistory::begin_redo(cx));
    let Some(op) = op else {
        return Err(anyhow::anyhow!("nothing to redo"));
    };
    let op_for_task = op.clone();
    let result = cx
        .background_spawn(async move { apply_redo(&op_for_task) })
        .await;
    cx.update(|cx| AppOperationHistory::finish_redo(cx, op, result))
}

fn notify_history_result(
    browser: &Entity<FileBrowser>,
    cx: &mut gpui::AsyncApp,
    result: anyhow::Result<()>,
    success_key: &str,
    failed_key: &str,
) {
    let _ = browser.update(cx, |browser, cx| {
        browser.reload(cx);
        if let Some(window) = cx.active_window() {
            let _ = window.update(cx, |_, window, cx| match result {
                Ok(()) => {
                    window.push_notification(Notification::success(t!(success_key)), cx);
                }
                Err(error) => {
                    window.push_notification(
                        Notification::error(format!("{}: {error}", t!(failed_key))),
                        cx,
                    );
                }
            });
        }
        cx.notify();
    });
}

/// Undo the last recorded file operation on a background thread, then refresh the browser.
pub fn spawn_history_undo(
    browser: Entity<FileBrowser>,
    _window: &mut Window,
    cx: &mut Context<MainPage>,
) {
    cx.spawn(async move |_, cx| {
        let result = run_history_undo(cx).await;
        notify_history_result(&browser, cx, result, "files.undo.done", "files.undo.failed");
    })
    .detach();
}

/// Redo the last undone file operation on a background thread, then refresh the browser.
pub fn spawn_history_redo(
    browser: Entity<FileBrowser>,
    _window: &mut Window,
    cx: &mut Context<MainPage>,
) {
    cx.spawn(async move |_, cx| {
        let result = run_history_redo(cx).await;
        notify_history_result(&browser, cx, result, "files.redo.done", "files.redo.failed");
    })
    .detach();
}

/// Undo from an active `FileBrowser` (keyboard focus on the file list).
pub fn spawn_history_undo_from_browser(
    browser: Entity<FileBrowser>,
    cx: &mut Context<FileBrowser>,
) {
    cx.spawn(async move |_, cx| {
        let result = run_history_undo(cx).await;
        notify_history_result(&browser, cx, result, "files.undo.done", "files.undo.failed");
    })
    .detach();
}

/// Redo from an active `FileBrowser` (keyboard focus on the file list).
pub fn spawn_history_redo_from_browser(
    browser: Entity<FileBrowser>,
    cx: &mut Context<FileBrowser>,
) {
    cx.spawn(async move |_, cx| {
        let result = run_history_redo(cx).await;
        notify_history_result(&browser, cx, result, "files.redo.done", "files.redo.failed");
    })
    .detach();
}
