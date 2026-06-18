//! Background copy/move with status notifications (Files StatusCenter subset).

use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Duration;

use files_fs::{
    archive_progress_total, are_paths_on_same_drive, compress_paths_to_zip_at_path_cancellable,
    count_delete_items, delete_paths_cancellable, empty_recycle_bin, extract_archive_cancellable,
    is_archive_path, paths_conflict, recycle_paths_cancellable, restore_recycle_items,
    transfer_one_cancellable, ClipboardOperation, CompressCancelled, ConflictResolution,
    DeleteCancelled, ExtractCancelled, FileClipboard, FileOperation, TransferCancelled,
    TransferOutcome,
};
use gpui::{
    px, AppContext, Context, Entity, Modifiers, ParentElement, SharedString, Styled, WeakEntity,
    Window,
};
use gpui_component::{
    button::Button, dialog::DialogFooter, label::Label, notification::Notification, v_flex,
    WindowExt as _,
};
use rust_i18n::t;

use crate::app_state::{
    AppFileClipboard, AppOperationHistory, TransferJobId, TransferStatusGlobal,
};
use crate::file_browser::create_shortcuts_in_folder;
use crate::file_browser::FileBrowser;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum FileTransferKind {
    Copy,
    Move,
    Link,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum DropOperationHint {
    Copy,
    Move,
    Link,
}

/// Files `BaseLayoutViewModel.DragOverAsync` copy/move/link resolution.
pub fn drop_operation_hint(
    modifiers: Modifiers,
    source_paths: &[PathBuf],
    destination: &Path,
) -> DropOperationHint {
    if modifiers.alt || (modifiers.control && modifiers.shift) {
        return DropOperationHint::Link;
    }
    if modifiers.control && !modifiers.shift {
        return DropOperationHint::Copy;
    }
    if modifiers.shift && !modifiers.control {
        return DropOperationHint::Move;
    }
    if transfer_involves_archive(source_paths, destination) {
        return DropOperationHint::Copy;
    }
    if are_paths_on_same_drive(source_paths, destination) {
        DropOperationHint::Move
    } else {
        DropOperationHint::Copy
    }
}

pub fn file_transfer_kind_for_drop(
    modifiers: Modifiers,
    source_paths: &[PathBuf],
    destination: &Path,
) -> FileTransferKind {
    match drop_operation_hint(modifiers, source_paths, destination) {
        DropOperationHint::Copy => FileTransferKind::Copy,
        DropOperationHint::Move => FileTransferKind::Move,
        DropOperationHint::Link => FileTransferKind::Link,
    }
}

fn transfer_involves_archive(source_paths: &[PathBuf], destination: &Path) -> bool {
    source_paths.iter().any(|path| is_archive_path(path)) || is_archive_path(destination)
}

fn operation_for_kind(kind: FileTransferKind) -> ClipboardOperation {
    match kind {
        FileTransferKind::Copy => ClipboardOperation::Copy,
        FileTransferKind::Move => ClipboardOperation::Cut,
        FileTransferKind::Link => ClipboardOperation::Copy,
    }
}

fn begin_transfer_status(
    message: SharedString,
    total: u32,
    cx: &mut gpui::AsyncApp,
) -> (TransferJobId, std::sync::Arc<std::sync::atomic::AtomicBool>) {
    cx.update(|cx| TransferStatusGlobal::begin(message, total, cx))
}

fn set_transfer_progress(id: TransferJobId, completed: u32, cx: &mut gpui::AsyncApp) {
    let _ = cx.update(|cx| TransferStatusGlobal::set_progress(id, completed, cx));
}

fn end_transfer_status(id: TransferJobId, cx: &mut gpui::AsyncApp) {
    let _ = cx.update(|cx| TransferStatusGlobal::end(id, cx));
}

fn fail_transfer_status(id: TransferJobId, cx: &mut gpui::AsyncApp) {
    let _ = cx.update(|cx| TransferStatusGlobal::fail(id, cx));
}

fn cancel_transfer_status(id: TransferJobId, cx: &mut gpui::AsyncApp) {
    let _ = cx.update(|cx| TransferStatusGlobal::cancel(id, cx));
}

fn record_transfer_outcome(
    kind: FileTransferKind,
    outcome: &TransferOutcome,
    cx: &mut (impl gpui::AppContext + std::borrow::BorrowMut<gpui::App>),
) {
    if outcome.transfers.is_empty() {
        return;
    }
    let op = match kind {
        FileTransferKind::Copy => FileOperation::Copy {
            copies: outcome.transfers.clone(),
        },
        FileTransferKind::Move => FileOperation::Move {
            moves: outcome.transfers.clone(),
        },
        FileTransferKind::Link => return,
    };
    let _ = AppOperationHistory::record(op, cx);
}

/// Run copy/move off the UI thread; show in-progress and result notifications.
pub fn spawn_file_transfer(
    browser: Entity<FileBrowser>,
    window: &mut Window,
    cx: &mut Context<FileBrowser>,
    kind: FileTransferKind,
    sources: Vec<PathBuf>,
    destination: PathBuf,
) {
    if sources.is_empty() {
        return;
    }

    if kind == FileTransferKind::Link {
        spawn_shortcut_drop(browser, window, cx, sources, destination);
        return;
    }

    let count = sources.len();
    let progress = match kind {
        FileTransferKind::Copy => t!("files.transfer.copying", count = count),
        FileTransferKind::Move => t!("files.transfer.moving", count = count),
        FileTransferKind::Link => unreachable!(),
    };
    let progress_status: SharedString = progress.clone().into();
    window.push_notification(Notification::info(progress), cx);

    let dest_for_reload = destination.clone();
    let total = count as u32;
    let weak = browser.downgrade();
    cx.spawn(async move |this, cx| {
        let (job_id, cancel) = begin_transfer_status(progress_status, total, cx);
        let result =
            run_transfer_with_conflicts(weak, cx, kind, sources, destination, cancel, job_id).await;

        // Update status on AsyncApp first — does not depend on window visibility.
        let _ = cx.update(|cx| match &result {
            Ok(outcome) if outcome.cancelled => TransferStatusGlobal::cancel(job_id, cx),
            Ok(outcome) if outcome.transferred > 0 => TransferStatusGlobal::end(job_id, cx),
            Ok(_) => TransferStatusGlobal::end(job_id, cx),
            Err(_) => TransferStatusGlobal::fail(job_id, cx),
        });

        let _ = cx.update(|cx| {
            if let Ok(outcome) = &result {
                if outcome.transferred > 0 {
                    record_transfer_outcome(kind, outcome, cx);
                }
            }
        });

        let _ = this.update(cx, |browser, cx| {
            let Some(window) = cx.active_window() else {
                cx.notify();
                return;
            };
            let _ = window.update(cx, |_, window, cx| match &result {
                Ok(outcome) if outcome.cancelled => {
                    window
                        .push_notification(Notification::info(t!("files.transfer.cancelled")), cx);
                }
                Ok(outcome) if outcome.transferred > 0 => {
                    window.push_notification(Notification::success(t!("files.transfer.done")), cx);
                }
                Ok(_) => {}
                Err(error) => {
                    window.push_notification(
                        Notification::error(format!("{}: {error}", t!("files.transfer.failed"))),
                        cx,
                    );
                }
            });

            if matches!(result, Ok(outcome) if outcome.transferred > 0)
                && browser.shows_directory(&dest_for_reload)
            {
                browser.reload(cx);
            }
            cx.notify();
        });
    })
    .detach();
}

fn spawn_shortcut_drop(
    _browser: Entity<FileBrowser>,
    window: &mut Window,
    cx: &mut Context<FileBrowser>,
    sources: Vec<PathBuf>,
    destination: PathBuf,
) {
    let count = sources.len();
    let progress = t!("files.transfer.linking", count = count);
    window.push_notification(Notification::info(progress.clone()), cx);

    let dest_for_reload = destination.clone();
    cx.spawn(async move |this, cx| {
        let sources_for_task = sources;
        let destination_for_task = destination;
        let join = thread::spawn(move || {
            create_shortcuts_in_folder(&sources_for_task, &destination_for_task)
        });
        let result = join
            .join()
            .unwrap_or_else(|_| Err(anyhow::anyhow!("shortcut creation thread panicked")));

        let ok = result.is_ok();
        let _ = this.update(cx, |browser, cx| {
            let Some(window) = cx.active_window() else {
                cx.notify();
                return;
            };
            let _ = window.update(cx, |_, window, cx| match &result {
                Ok(()) => {
                    window.push_notification(
                        Notification::success(t!("files.create_shortcut.success")),
                        cx,
                    );
                }
                Err(error) => {
                    window.push_notification(
                        Notification::error(format!(
                            "{}: {error}",
                            t!("files.create_shortcut.error")
                        )),
                        cx,
                    );
                }
            });

            if ok && browser.shows_directory(&dest_for_reload) {
                browser.reload(cx);
            }
            cx.notify();
        });
    })
    .detach();
}

/// Paste from a taken clipboard (same semantics as synchronous paste, but non-blocking).
pub fn spawn_paste_from_clipboard(
    browser: Entity<FileBrowser>,
    window: &mut Window,
    cx: &mut Context<FileBrowser>,
    clipboard: FileClipboard,
    destination: PathBuf,
) {
    if clipboard.paths.is_empty() {
        return;
    }
    let kind = match clipboard.operation {
        ClipboardOperation::Copy => FileTransferKind::Copy,
        ClipboardOperation::Cut => FileTransferKind::Move,
    };
    let operation = clipboard.operation;
    let paths = clipboard.paths;
    let paths_for_clipboard = paths.clone();

    let progress_status: SharedString = t!("files.transfer.pasting", count = paths.len()).into();
    window.push_notification(
        Notification::info(t!("files.transfer.pasting", count = paths.len())),
        cx,
    );

    let total = paths_for_clipboard.len() as u32;
    let weak = browser.downgrade();
    cx.spawn(async move |this, cx| {
        let (job_id, cancel) = begin_transfer_status(progress_status, total, cx);
        let result =
            run_transfer_with_conflicts(weak, cx, kind, paths, destination, cancel, job_id).await;

        // Update status on AsyncApp first — does not depend on window visibility.
        let _ = cx.update(|cx| match &result {
            Ok(outcome) if outcome.cancelled => TransferStatusGlobal::cancel(job_id, cx),
            Ok(outcome) if outcome.transferred > 0 => TransferStatusGlobal::end(job_id, cx),
            Ok(_) => TransferStatusGlobal::end(job_id, cx),
            Err(_) => TransferStatusGlobal::fail(job_id, cx),
        });

        let _ = cx.update(|cx| {
            if let Ok(outcome) = &result {
                if outcome.transferred > 0 {
                    record_transfer_outcome(kind, outcome, cx);
                }
            }
        });

        let _ = this.update(cx, |browser, cx| {
            let Some(window) = cx.active_window() else {
                cx.notify();
                return;
            };
            let _ = window.update(cx, |_, window, cx| match &result {
                Ok(outcome) if outcome.cancelled => {
                    AppFileClipboard::set(
                        FileClipboard::new(operation, paths_for_clipboard.clone()),
                        cx,
                    );
                    window
                        .push_notification(Notification::info(t!("files.transfer.cancelled")), cx);
                }
                Ok(outcome) if outcome.transferred > 0 => {
                    if operation == ClipboardOperation::Copy {
                        AppFileClipboard::store(operation, paths_for_clipboard.clone(), cx);
                    }
                    window.push_notification(Notification::success(t!("files.paste.success")), cx);
                }
                Ok(_) => {
                    AppFileClipboard::set(
                        FileClipboard::new(operation, paths_for_clipboard.clone()),
                        cx,
                    );
                }
                Err(error) => {
                    AppFileClipboard::set(
                        FileClipboard::new(operation, paths_for_clipboard.clone()),
                        cx,
                    );
                    window.push_notification(
                        Notification::error(format!("{}: {error}", t!("files.paste.error"))),
                        cx,
                    );
                }
            });

            if matches!(result, Ok(outcome) if outcome.transferred > 0) {
                browser.reload(cx);
            }
            cx.notify();
        });
    })
    .detach();
}

/// Compress selected paths into a zip in `destination` (parent folder of selection).
pub fn spawn_compress(
    _browser: Entity<FileBrowser>,
    window: &mut Window,
    cx: &mut Context<FileBrowser>,
    sources: Vec<PathBuf>,
    destination: PathBuf,
    zip_path: PathBuf,
    partial_path: PathBuf,
    partial_created: bool,
) {
    if sources.is_empty() {
        return;
    }
    let count = sources.len();
    let message: SharedString = t!("files.transfer.compressing", count = count).into();
    window.push_notification(
        Notification::info(t!("files.transfer.compressing", count = count)),
        cx,
    );
    let dest_for_reload = destination.clone();
    let total = count as u32;
    cx.spawn(async move |this, cx| {
        let (job_id, cancel) = begin_transfer_status(message, total, cx);
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            cancel_transfer_status(job_id, cx);
            return;
        }
        let sources_for_task = sources.clone();
        let zip_path_for_task = zip_path.clone();
        let cancel_for_task = cancel.clone();
        let (progress_tx, progress_rx) = mpsc::channel::<u32>();
        let join = thread::spawn(move || {
            compress_paths_to_zip_at_path_cancellable(
                &sources_for_task,
                &zip_path_for_task,
                &cancel_for_task,
                |completed, _total| {
                    let _ = progress_tx.send(completed);
                },
            )
        });

        while !join.is_finished() {
            while let Ok(completed) = progress_rx.try_recv() {
                set_transfer_progress(job_id, completed, cx);
            }
            let _ = cx
                .background_spawn(async move {
                    thread::sleep(Duration::from_millis(50));
                })
                .await;
        }
        while let Ok(completed) = progress_rx.try_recv() {
            set_transfer_progress(job_id, completed, cx);
        }

        let result = join
            .join()
            .map_err(|_| anyhow::anyhow!("compress thread panicked"))
            .and_then(|inner| inner);
        let done_ok = matches!(&result, Ok(_));
        if !done_ok && partial_created {
            let _ = std::fs::remove_file(&partial_path);
        }
        if done_ok {
            set_transfer_progress(job_id, total, cx);
            end_transfer_status(job_id, cx);
        } else if result.as_ref().is_err_and(|e| e.is::<CompressCancelled>()) {
            cancel_transfer_status(job_id, cx);
        } else {
            fail_transfer_status(job_id, cx);
        }

        let _ = this.update(cx, |browser, cx| {
            let Some(window) = cx.active_window() else {
                cx.notify();
                return;
            };
            let _ = window.update(cx, |_, window, cx| match &result {
                Ok(_) => {
                    window.push_notification(Notification::success(t!("files.compress.done")), cx);
                }
                Err(error) if error.is::<CompressCancelled>() => {
                    window
                        .push_notification(Notification::info(t!("files.transfer.cancelled")), cx);
                }
                Err(error) => {
                    window.push_notification(
                        Notification::error(format!("{}: {error}", t!("files.compress.failed"))),
                        cx,
                    );
                }
            });
            if done_ok && browser.shows_directory(&dest_for_reload) {
                browser.reload(cx);
            }
            cx.notify();
        });
    })
    .detach();
}

/// Extract selected archives into destination folder(s).
pub fn spawn_extract(
    _browser: Entity<FileBrowser>,
    window: &mut Window,
    cx: &mut Context<FileBrowser>,
    jobs: Vec<(PathBuf, PathBuf)>,
) {
    if jobs.is_empty() {
        return;
    }
    let count = jobs.len();
    let message: SharedString = t!("files.extract.extracting", count = count).into();
    window.push_notification(
        Notification::info(t!("files.extract.extracting", count = count)),
        cx,
    );
    tracing::debug!(target: "extract", jobs = count, "spawn_extract");
    let count_started = std::time::Instant::now();
    let total = jobs
        .iter()
        .map(|(archive, _)| archive_progress_total(archive))
        .sum::<u32>()
        .max(1);
    tracing::debug!(
        target: "extract",
        progress_total = total,
        elapsed = ?count_started.elapsed(),
        "computed extract progress total"
    );
    cx.spawn(async move |this, cx| {
        let (job_id, cancel) = begin_transfer_status(message, total, cx);
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            cancel_transfer_status(job_id, cx);
            return;
        }
        let jobs_for_task = jobs.clone();
        let cancel_for_task = cancel.clone();
        let (progress_tx, progress_rx) = mpsc::channel::<u32>();
        let join = thread::spawn(move || {
            let thread_started = std::time::Instant::now();
            tracing::debug!(target: "extract", "extract worker thread started");
            let mut result: anyhow::Result<()> = Ok(());
            let mut base = 0u32;
            for (archive, dest) in jobs_for_task.iter() {
                if cancel_for_task.load(std::sync::atomic::Ordering::Relaxed) {
                    result = Err(ExtractCancelled.into());
                    break;
                }
                if let Err(error) = std::fs::create_dir_all(dest) {
                    result = Err(error.into());
                    break;
                }
                let archive_result = extract_archive_cancellable(
                    archive,
                    dest,
                    &cancel_for_task,
                    |step, _step_total| {
                        let overall = base + step;
                        let _ = progress_tx.send(overall);
                    },
                );
                if let Err(error) = archive_result {
                    result = Err(error);
                    break;
                }
                base += archive_progress_total(archive);
            }
            tracing::debug!(
                target: "extract",
                elapsed = ?thread_started.elapsed(),
                ok = result.is_ok(),
                "extract worker thread finished"
            );
            result
        });

        while !join.is_finished() {
            while let Ok(completed) = progress_rx.try_recv() {
                set_transfer_progress(job_id, completed, cx);
            }
            let _ = cx
                .background_spawn(async move {
                    thread::sleep(Duration::from_millis(50));
                })
                .await;
        }
        while let Ok(completed) = progress_rx.try_recv() {
            set_transfer_progress(job_id, completed, cx);
        }

        let result = join
            .join()
            .map_err(|_| anyhow::anyhow!("extract thread panicked"))
            .and_then(|inner| inner);
        let done_ok = result.is_ok();
        if done_ok {
            set_transfer_progress(job_id, total, cx);
            end_transfer_status(job_id, cx);
        } else if result.as_ref().is_err_and(|e| e.is::<ExtractCancelled>()) {
            cancel_transfer_status(job_id, cx);
        } else {
            fail_transfer_status(job_id, cx);
        }

        let _ = this.update(cx, |browser, cx| {
            let Some(window) = cx.active_window() else {
                cx.notify();
                return;
            };
            let _ = window.update(cx, |_, window, cx| match &result {
                Ok(()) => {
                    window.push_notification(Notification::success(t!("files.extract.done")), cx);
                }
                Err(error) if error.is::<ExtractCancelled>() => {
                    window
                        .push_notification(Notification::info(t!("files.transfer.cancelled")), cx);
                }
                Err(error) => {
                    window.push_notification(
                        Notification::error(format!("{}: {error}", t!("files.extract.failed"))),
                        cx,
                    );
                }
            });
            if done_ok {
                browser.reload(cx);
            }
            cx.notify();
        });
    })
    .detach();
}

/// Delete selected paths with StatusCenter progress (recycle bin or permanent).
pub fn spawn_delete(
    _browser: Entity<FileBrowser>,
    window: &mut Window,
    cx: &mut Context<FileBrowser>,
    paths: Vec<PathBuf>,
    permanent: bool,
    success_message: SharedString,
) {
    if paths.is_empty() {
        return;
    }

    let count = paths.len();
    let message: SharedString = t!("files.delete.deleting", count = count).into();
    window.push_notification(
        Notification::info(t!("files.delete.deleting", count = count)),
        cx,
    );

    cx.spawn(async move |this, cx| {
        let paths_for_count = paths.clone();
        let total = cx
            .background_spawn(async move { count_delete_items(&paths_for_count) })
            .await
            .max(1);

        let (job_id, cancel) = begin_transfer_status(message, total, cx);
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            cancel_transfer_status(job_id, cx);
            return;
        }

        let paths_for_task = paths.clone();
        let cancel_for_task = cancel.clone();
        let (progress_tx, progress_rx) = mpsc::channel::<u32>();
        let permanent_for_task = permanent;
        let join = thread::spawn(move || {
            let mut on_progress = |step: u32, _step_total: u32| {
                let _ = progress_tx.send(step);
            };
            if permanent_for_task {
                delete_paths_cancellable(&paths_for_task, &cancel_for_task, &mut on_progress)
            } else {
                recycle_paths_cancellable(&paths_for_task, &cancel_for_task, &mut on_progress)
            }
        });

        while !join.is_finished() {
            while let Ok(completed) = progress_rx.try_recv() {
                set_transfer_progress(job_id, completed, cx);
            }
            let _ = cx
                .background_spawn(async move {
                    thread::sleep(Duration::from_millis(50));
                })
                .await;
        }
        while let Ok(completed) = progress_rx.try_recv() {
            set_transfer_progress(job_id, completed, cx);
        }

        let result = join
            .join()
            .map_err(|_| anyhow::anyhow!("delete thread panicked"))
            .and_then(|inner| inner);

        let done_ok = result.is_ok();
        if done_ok && !permanent {
            let originals = paths.clone();
            let _ = cx.update(|cx| {
                AppOperationHistory::record(FileOperation::Recycle { originals }, cx);
            });
        }
        if done_ok {
            set_transfer_progress(job_id, total, cx);
            end_transfer_status(job_id, cx);
        } else if result.as_ref().is_err_and(|e| e.is::<DeleteCancelled>()) {
            cancel_transfer_status(job_id, cx);
        } else {
            fail_transfer_status(job_id, cx);
        }

        let cleanup_paths = paths.clone();
        let _ = this.update(cx, |browser, cx| {
            let Some(window) = cx.active_window() else {
                if done_ok {
                    browser.finish_delete(&cleanup_paths, cx);
                }
                cx.notify();
                return;
            };

            let _ = window.update(cx, |_, window, cx| match &result {
                Ok(()) => {
                    window.push_notification(Notification::success(success_message.clone()), cx);
                }
                Err(error) if error.is::<DeleteCancelled>() => {
                    window
                        .push_notification(Notification::info(t!("files.transfer.cancelled")), cx);
                }
                Err(error) => {
                    window.push_notification(
                        Notification::error(format!("{}: {error}", t!("files.delete.error"))),
                        cx,
                    );
                }
            });
            match &result {
                Ok(()) => browser.finish_delete(&cleanup_paths, cx),
                Err(error) if error.is::<DeleteCancelled>() => browser.reload(cx),
                Err(_) => browser.reload(cx),
            }
            cx.notify();
        });
    })
    .detach();
}

async fn run_transfer_with_conflicts(
    _browser: WeakEntity<FileBrowser>,
    cx: &mut gpui::AsyncApp,
    kind: FileTransferKind,
    sources: Vec<PathBuf>,
    destination: PathBuf,
    cancel: std::sync::Arc<std::sync::atomic::AtomicBool>,
    job_id: TransferJobId,
) -> anyhow::Result<TransferOutcome> {
    let operation = operation_for_kind(kind);
    let mut skip_all = false;
    let mut replace_all = false;
    let mut outcome = TransferOutcome::default();
    for (index, source) in sources.into_iter().enumerate() {
        if cancel.load(std::sync::atomic::Ordering::Relaxed) {
            outcome.cancelled = true;
            return Ok(outcome);
        }
        let file_name = source
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("invalid source path {}", source.display()))?;
        let target = destination.join(file_name);

        if paths_conflict(&source, &target) {
            if skip_all {
                continue;
            }
            if !replace_all {
                let resolution = prompt_conflict(cx, &source, &target).await;
                match resolution {
                    ConflictResolution::Skip => continue,
                    ConflictResolution::SkipAll => {
                        skip_all = true;
                        continue;
                    }
                    ConflictResolution::Replace => {}
                    ConflictResolution::ReplaceAll => replace_all = true,
                    ConflictResolution::Cancel => {
                        outcome.cancelled = true;
                        return Ok(outcome);
                    }
                }
            }
        }

        let must_replace = paths_conflict(&source, &target);
        let source_path = source.clone();
        let dest_dir = destination.clone();
        let cancel_for_task = cancel.clone();
        match cx
            .background_spawn(async move {
                transfer_one_cancellable(
                    &source_path,
                    &dest_dir,
                    operation,
                    must_replace,
                    &cancel_for_task,
                )
            })
            .await
        {
            Ok(()) => {
                outcome.transferred += 1;
                outcome.transfers.push((source, target));
                set_transfer_progress(job_id, (index + 1) as u32, cx);
            }
            Err(error) if error.is::<TransferCancelled>() => {
                outcome.cancelled = true;
                return Ok(outcome);
            }
            Err(error) => return Err(error),
        }
    }

    Ok(outcome)
}

async fn prompt_conflict(
    cx: &mut gpui::AsyncApp,
    source: &Path,
    target: &Path,
) -> ConflictResolution {
    let (tx, rx) = mpsc::sync_channel(1);
    let tx = Arc::new(tx);
    let name = source
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| source.display().to_string());
    let target_display = target.display().to_string();
    let title = SharedString::from(t!("files.conflict.title"));
    let description = SharedString::from(t!(
        "files.conflict.description",
        name = name,
        path = target_display
    ));
    let replace_label = SharedString::from(t!("files.conflict.replace"));
    let replace_all_label = SharedString::from(t!("files.conflict.replace_all"));
    let skip_label = SharedString::from(t!("files.conflict.skip"));
    let skip_all_label = SharedString::from(t!("files.conflict.skip_all"));
    let cancel_label = SharedString::from(t!("files.cancel"));

    let _ = cx.update(|cx| {
        let Some(window) = cx.active_window() else {
            let _ = tx.send(ConflictResolution::Cancel);
            return;
        };
        let _ = window.update(cx, |_, window, cx| {
            window.open_dialog(cx, move |dialog, _window, _cx| {
                dialog.title(title.clone()).w(px(600.)).footer(
                    v_flex()
                        .gap_3()
                        .px_4()
                        .pt_3()
                        .child(Label::new(description.clone()))
                        .child(
                            DialogFooter::new()
                                .justify_center()
                                .child(conflict_button(
                                    "conflict-cancel",
                                    cancel_label.clone(),
                                    ConflictResolution::Cancel,
                                    tx.clone(),
                                ))
                                .child(conflict_button(
                                    "conflict-skip-all",
                                    skip_all_label.clone(),
                                    ConflictResolution::SkipAll,
                                    tx.clone(),
                                ))
                                .child(conflict_button(
                                    "conflict-skip",
                                    skip_label.clone(),
                                    ConflictResolution::Skip,
                                    tx.clone(),
                                ))
                                .child(conflict_button(
                                    "conflict-replace-all",
                                    replace_all_label.clone(),
                                    ConflictResolution::ReplaceAll,
                                    tx.clone(),
                                ))
                                .child(conflict_button(
                                    "conflict-replace",
                                    replace_label.clone(),
                                    ConflictResolution::Replace,
                                    tx.clone(),
                                )),
                        ),
                )
            });
        });
    });

    cx.background_spawn(async move { rx.recv().unwrap_or(ConflictResolution::Cancel) })
        .await
}

fn conflict_button(
    id: &'static str,
    label: SharedString,
    resolution: ConflictResolution,
    tx: Arc<mpsc::SyncSender<ConflictResolution>>,
) -> Button {
    Button::new(id).label(label).on_click(move |_, window, cx| {
        let _ = tx.send(resolution);
        window.close_dialog(cx);
    })
}

/// Restore recycle-bin items on a background thread, then refresh the listing.
pub fn spawn_restore_recycle(
    _browser: Entity<FileBrowser>,
    window: &mut Window,
    cx: &mut Context<FileBrowser>,
    paths: Vec<PathBuf>,
    success_message: SharedString,
) {
    if paths.is_empty() {
        return;
    }

    let count = paths.len();
    window.push_notification(
        Notification::info(t!("files.recycle.restore.in_progress", count = count)),
        cx,
    );

    cx.spawn(async move |this, cx| {
        let paths_for_task = paths.clone();
        let result = cx
            .background_spawn(async move { restore_recycle_items(&paths_for_task) })
            .await;

        let _ = this.update(cx, |browser, cx| {
            let Some(window) = cx.active_window() else {
                browser.reload(cx);
                browser.clear_selection();
                cx.notify();
                return;
            };

            let _ = window.update(cx, |_, window, cx| match &result {
                Ok(()) => {
                    window.push_notification(Notification::success(success_message.clone()), cx);
                }
                Err(error) => {
                    window.push_notification(
                        Notification::error(format!(
                            "{}: {error}",
                            t!("files.recycle.restore.failed")
                        )),
                        cx,
                    );
                }
            });
            browser.clear_selection();
            browser.reload(cx);
            cx.notify();
        });
    })
    .detach();
}

/// Empty the Recycle Bin on a background thread, then refresh the listing.
pub fn spawn_empty_recycle_bin(
    _browser: Entity<FileBrowser>,
    window: &mut Window,
    cx: &mut Context<FileBrowser>,
) {
    window.push_notification(
        Notification::info(t!("files.recycle.empty.in_progress")),
        cx,
    );

    cx.spawn(async move |this, cx| {
        let result = cx
            .background_spawn(async move { empty_recycle_bin() })
            .await;

        let _ = this.update(cx, |browser, cx| {
            let Some(window) = cx.active_window() else {
                browser.reload(cx);
                browser.clear_selection();
                cx.notify();
                return;
            };

            let _ = window.update(cx, |_, window, cx| match &result {
                Ok(()) => {
                    window.push_notification(
                        Notification::success(t!("files.recycle.empty.success")),
                        cx,
                    );
                }
                Err(error) => {
                    window.push_notification(
                        Notification::error(format!(
                            "{}: {error}",
                            t!("files.recycle.empty.failed")
                        )),
                        cx,
                    );
                }
            });
            browser.clear_selection();
            browser.reload(cx);
            cx.notify();
        });
    })
    .detach();
}
