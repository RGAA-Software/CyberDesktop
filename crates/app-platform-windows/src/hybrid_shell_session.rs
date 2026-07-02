//! Hybrid Shell context-menu session: Layer A (default aggregate) + Layer B (per-handler extensions).
//!
//! The session is apartment-bound: it is created and lives on a single STA thread. The
//! [`PREPARED_HYBRID_SESSION`] thread-local owns the current menu so that lazy submenu
//! expansion and verb invocation happen on the thread that created the COM objects.
//!
//! Layer A is kept alive in this session. Layer B handlers are recreated on demand for
//! invoke/lazy expansion; only their menu entries are cached after the query.

use std::cell::RefCell;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::com::ThreadWithMessageQueueWithPump;
use crate::context_menu::{
    create_context_menu, enumerate_popup_menu, expand_lazy_submenu_inner, ContextMenuHandle,
    ShellContextMenuEntry,
};
use crate::per_handler_shell::{
    expand_handler_submenu_by_clsid, invoke_handler_by_clsid, probe_all_handlers_timed_for_paths,
    InitStyle, HANDLER_PROBE_TIMEOUT,
};
use crate::shell_icon::menu_icon_pixel_size;

thread_local! {
    static PREPARED_HYBRID_SESSION: RefCell<Option<HybridSession>> = const { RefCell::new(None) };
}

/// Whether Layer B (per-handler in-process probing) runs during menu preparation.
///
/// Layer B instantiates every registered folder handler individually, which multiplies
/// hang risk on machines with misbehaving extensions; this switch lets diagnostics and
/// future kill-switch config disable it while keeping Layer A (the aggregate Shell menu).
///
/// Default OFF: Layer B is suspected of permanently wedging the app on some machines,
/// and its main payoff (the "New" submenu) is now covered by the native shell_new
/// enumeration. Diagnostics (`cyber_files --shell-menu-test`) can still opt back in.
static LAYER_B_ENABLED: AtomicBool = AtomicBool::new(false);

pub fn set_shell_menu_layer_b_enabled(enabled: bool) {
    LAYER_B_ENABLED.store(enabled, Ordering::Relaxed);
}

pub fn shell_menu_layer_b_enabled() -> bool {
    LAYER_B_ENABLED.load(Ordering::Relaxed)
}

pub(crate) fn release_prepared_hybrid_session() {
    PREPARED_HYBRID_SESSION.with(|slot| {
        if let Some(session) = slot.borrow_mut().take() {
            unsafe {
                session.release();
            }
        }
    });
}

pub(crate) struct HybridSession {
    layer_a: Option<ContextMenuHandle>,
    paths: Vec<PathBuf>,
}

impl HybridSession {
    /// Build Layer A + Layer B top-level entries and store the session in the thread-local.
    ///
    /// Must be called on the owning STA thread.
    pub(crate) unsafe fn prepare_and_store(
        paths: &[PathBuf],
        extended_verbs: bool,
        menu_icon_extract_px: u32,
    ) -> anyhow::Result<Vec<ShellContextMenuEntry>> {
        crate::shell_menu_icon::set_menu_icon_extract_px(menu_icon_extract_px);
        release_prepared_hybrid_session();

        let layer_a = create_context_menu(paths, extended_verbs)?;
        let layer_a_entries = enumerate_popup_menu(
            layer_a.popup,
            &layer_a.menu,
            0,
            false,
            &layer_a.primary_path,
            true,
            None,
        )?;

        let mut merged = layer_a_entries;
        if shell_menu_layer_b_enabled() {
            let (handler_records, _handler_errors, _handler_timeouts) =
                probe_all_handlers_timed_for_paths(
                    paths,
                    InitStyle::ShellAccurate,
                    false,
                    HANDLER_PROBE_TIMEOUT,
                );
            merge_handler_records_into(&mut merged, handler_records, paths);
        } else {
            tracing::info!(target: "shell_menu", "hybrid prepare: Layer B disabled; aggregate menu only");
        }

        let session = HybridSession {
            layer_a: Some(layer_a),
            paths: paths.to_vec(),
        };
        PREPARED_HYBRID_SESSION.with(|slot| *slot.borrow_mut() = Some(session));

        Ok(merged)
    }

    /// Invoke a command by offset, routing to Layer A or Layer B by `handler_clsid`.
    pub(crate) unsafe fn invoke_prepared(
        handler_clsid: Option<&str>,
        command_offset: u32,
    ) -> anyhow::Result<()> {
        let paths: Option<Vec<PathBuf>> = PREPARED_HYBRID_SESSION
            .with(|slot| slot.borrow().as_ref().map(|session| session.paths.clone()));
        let Some(paths) = paths else {
            anyhow::bail!("no prepared hybrid shell context menu for invoke");
        };

        if let Some(clsid) = handler_clsid {
            return invoke_handler_by_clsid(&paths, clsid, command_offset);
        }

        PREPARED_HYBRID_SESSION.with(|slot| {
            let session = slot.borrow();
            let Some(session) = session.as_ref() else {
                anyhow::bail!("no prepared hybrid shell context menu for invoke");
            };
            let Some(layer_a) = &session.layer_a else {
                anyhow::bail!("no Layer A menu for invoke");
            };
            use windows::core::PCSTR;
            use windows::Win32::UI::Shell::CMINVOKECOMMANDINFO;
            let mut info = CMINVOKECOMMANDINFO::default();
            info.cbSize = std::mem::size_of::<CMINVOKECOMMANDINFO>() as u32;
            info.lpVerb = PCSTR::from_raw(command_offset as usize as *const u8);
            info.nShow = 1;
            layer_a.menu.InvokeCommand(&info)?;
            Ok(())
        })
    }

    /// Expand a lazy submenu by parent index, routing to Layer A or Layer B.
    pub(crate) unsafe fn expand_lazy_submenu(
        handler_clsid: Option<&str>,
        parent_index: u32,
    ) -> anyhow::Result<Vec<ShellContextMenuEntry>> {
        let paths: Option<Vec<PathBuf>> = PREPARED_HYBRID_SESSION
            .with(|slot| slot.borrow().as_ref().map(|session| session.paths.clone()));
        let Some(paths) = paths else {
            anyhow::bail!("no prepared hybrid shell context menu for submenu expansion");
        };

        if let Some(clsid) = handler_clsid {
            return expand_handler_submenu_by_clsid(&paths, clsid, parent_index);
        }

        PREPARED_HYBRID_SESSION.with(|slot| {
            let session = slot.borrow();
            let Some(session) = session.as_ref() else {
                anyhow::bail!("no prepared hybrid shell context menu for submenu expansion");
            };
            let Some(layer_a) = &session.layer_a else {
                anyhow::bail!("no Layer A menu for submenu expansion");
            };
            let primary_path = layer_a.primary_path.clone();
            expand_lazy_submenu_inner(layer_a.popup, &layer_a.menu, parent_index, &primary_path)
        })
    }

    unsafe fn release(self) {
        if let Some(layer_a) = self.layer_a {
            layer_a.release();
        }
    }
}

fn entry_label(entry: &ShellContextMenuEntry) -> Option<&str> {
    match entry {
        ShellContextMenuEntry::Item { label, .. } => Some(label),
        ShellContextMenuEntry::Submenu { label, .. } => Some(label),
        _ => None,
    }
}

fn entry_verb(entry: &ShellContextMenuEntry) -> Option<&str> {
    match entry {
        ShellContextMenuEntry::Item { command_string, .. } => command_string.as_deref(),
        _ => None,
    }
}

fn handler_item_to_shell_entry(
    item: &crate::per_handler_shell::HandlerMenuItem,
    clsid: &str,
) -> ShellContextMenuEntry {
    if item.is_submenu() {
        ShellContextMenuEntry::Submenu {
            label: item.label.clone(),
            children: item
                .children
                .iter()
                .map(|c| handler_item_to_shell_entry(c, clsid))
                .collect(),
            icon_png: None,
            lazy_parent_index: None,
            handler_clsid: Some(clsid.to_string()),
        }
    } else {
        ShellContextMenuEntry::Item {
            label: item.label.clone(),
            command_offset: item.command_offset,
            command_string: item.command_string.clone(),
            icon_png: None,
            handler_clsid: Some(clsid.to_string()),
        }
    }
}

/// Merge Layer B handler records into the running list, deduplicating by label/verb.
unsafe fn merge_handler_records_into(
    merged: &mut Vec<ShellContextMenuEntry>,
    handler_records: Vec<crate::per_handler_shell::HandlerProbeRecord>,
    _paths: &[PathBuf],
) {
    let mut seen: HashSet<String> = HashSet::new();
    for entry in merged.iter() {
        if let Some(label) = entry_label(entry) {
            seen.insert(label.to_ascii_lowercase());
        }
        if let Some(verb) = entry_verb(entry) {
            seen.insert(verb.to_ascii_lowercase());
        }
    }

    for rec in handler_records {
        for item in &rec.items {
            let label_lower = item.label.to_ascii_lowercase();
            if seen.contains(&label_lower) {
                continue;
            }
            if let Some(verb) = item.command_string.as_deref() {
                let verb_lower = verb.to_ascii_lowercase();
                if seen.contains(&verb_lower) {
                    continue;
                }
                seen.insert(verb_lower);
            }
            seen.insert(label_lower);
            merged.push(handler_item_to_shell_entry(item, &rec.clsid));
        }
    }
}

/// Entry point used by diagnostics: returns merged entries without keeping a session.
#[allow(dead_code)]
pub(crate) unsafe fn query_hybrid_entries(
    paths: &[PathBuf],
    extended_verbs: bool,
    menu_icon_extract_px: u32,
) -> anyhow::Result<Vec<ShellContextMenuEntry>> {
    crate::shell_menu_icon::set_menu_icon_extract_px(menu_icon_extract_px);

    let layer_a = create_context_menu(paths, extended_verbs)?;
    let layer_a_entries = enumerate_popup_menu(
        layer_a.popup,
        &layer_a.menu,
        0,
        false,
        &layer_a.primary_path,
        true,
        None,
    )?;
    layer_a.release();

    let mut merged = layer_a_entries;
    if shell_menu_layer_b_enabled() {
        let (handler_records, _handler_errors, _handler_timeouts) =
            probe_all_handlers_timed_for_paths(
                paths,
                InitStyle::ShellAccurate,
                false,
                HANDLER_PROBE_TIMEOUT,
            );
        merge_handler_records_into(&mut merged, handler_records, paths);
    }

    Ok(merged)
}

/// Warm-up variant: Layer A aggregate runs under a bounded timeout so a wedged Shell
/// extension cannot hang the background warm-up thread. If Layer A times out we fall
/// back to Layer B only (the handlers we actually want to preload).
pub(crate) unsafe fn query_hybrid_entries_for_warmup(
    paths: &[PathBuf],
    extended_verbs: bool,
    menu_icon_extract_px: u32,
    layer_a_timeout: Duration,
) -> anyhow::Result<Vec<ShellContextMenuEntry>> {
    crate::shell_menu_icon::set_menu_icon_extract_px(menu_icon_extract_px);

    let (layer_a_entries, layer_a_timed_out) = {
        let paths = paths.to_vec();
        let sta = ThreadWithMessageQueueWithPump::new("cyber_desktop-layer-a-warmup");
        let outcome = sta.post_with_timeout(
            move || unsafe {
                let handle = create_context_menu(&paths, extended_verbs)?;
                let entries = enumerate_popup_menu(
                    handle.popup,
                    &handle.menu,
                    0,
                    false,
                    &handle.primary_path,
                    true,
                    None,
                )?;
                handle.release();
                Ok::<_, anyhow::Error>(entries)
            },
            layer_a_timeout,
        );
        let timed_out = outcome.is_none();
        if timed_out {
            sta.abandon_wedged();
            crate::shell_menu_session::record_shell_query_timeout_from_warmup();
        }
        let entries = match outcome {
            Some(Ok(entries)) => entries,
            Some(Err(e)) => {
                tracing::warn!(target: "shell_menu", error = ?e, "warm-up Layer A failed");
                Vec::new()
            }
            None => {
                tracing::warn!(
                    target: "shell_menu",
                    "warm-up Layer A timed out after {layer_a_timeout:?}"
                );
                Vec::new()
            }
        };
        (entries, timed_out)
    };

    // If Layer A wedged, the process-wide Shell COM state is likely poisoned by a misbehaving
    // third-party extension. Running Layer B per-handler probes in that state tends to hang as
    // well and keeps the warm-up thread alive indefinitely, so we bail out early.
    if layer_a_timed_out {
        tracing::warn!(
            target: "shell_menu",
            "warm-up aborted: Layer A aggregate timed out; skipping Layer B preload"
        );
        return Ok(Vec::new());
    }

    let mut merged = layer_a_entries;
    if shell_menu_layer_b_enabled() {
        let (handler_records, _handler_errors, _handler_timeouts) =
            probe_all_handlers_timed_for_paths(
                paths,
                InitStyle::ShellAccurate,
                false,
                HANDLER_PROBE_TIMEOUT,
            );
        merge_handler_records_into(&mut merged, handler_records, paths);
    }

    Ok(merged)
}

/// Keep the warm-up directory helper accessible from the warm-up path.
pub(crate) fn warmup_directory() -> PathBuf {
    std::env::var("SHELL_WARMUP_DIR")
        .ok()
        .map(PathBuf::from)
        .filter(|p| p.exists())
        .unwrap_or_else(|| {
            let userprofile = std::env::var("USERPROFILE")
                .map(PathBuf::from)
                .unwrap_or_else(|_| std::env::temp_dir());
            let desktop = userprofile.join("Desktop");
            if desktop.exists() {
                desktop
            } else {
                let documents = userprofile.join("Documents");
                if documents.exists() {
                    documents
                } else {
                    std::env::temp_dir()
                }
            }
        })
}

/// Scale-aware icon size for the current process (warm-up/diagnostics).
pub(crate) fn warmup_icon_px() -> u32 {
    menu_icon_pixel_size(crate::system_scale_factor())
}
