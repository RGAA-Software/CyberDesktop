//! Shell context menu session (Files: one `ThreadWithMessageQueue`, serialized COM work).

use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use crate::com::StaMessageThread;
use crate::context_menu::{self, ShellContextMenuEntry};

/// One STA worker for all Shell menu COM (query / lazy submenu / invoke). Never spawn per query.
static SHELL_STA: OnceLock<Arc<StaMessageThread>> = OnceLock::new();

/// Only one Shell menu operation at a time — parallel `QueryContextMenu` hangs or poisons Shell.
static SHELL_OP_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn shell_sta() -> Arc<StaMessageThread> {
    SHELL_STA
        .get_or_init(|| Arc::new(StaMessageThread::new("cyber_desktop-shell-menu")))
        .clone()
}

fn shell_op_lock() -> std::sync::MutexGuard<'static, ()> {
    SHELL_OP_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .expect("shell menu op lock")
}

pub fn clear_session() {
    let _guard = shell_op_lock();
    shell_sta().dispatch(context_menu::release_prepared_menu);
}

/// Query top-level Shell verbs only; submenus load lazily (Files `loadSubmenus: false`).
pub fn query_with_session(
    paths: &[PathBuf],
    extended_verbs: bool,
    menu_icon_extract_px: u32,
) -> anyhow::Result<Vec<ShellContextMenuEntry>> {
    let _guard = shell_op_lock();
    let paths = paths.to_vec();
    shell_sta().post(move || {
        context_menu::prepare_and_enumerate_top_level(&paths, extended_verbs, menu_icon_extract_px)
    })
}

/// Expand one Shell submenu on the owning STA thread (Files `LoadSubMenu` + `WM_INITMENUPOPUP`).
pub fn load_lazy_submenu(parent_index: u32) -> anyhow::Result<Vec<ShellContextMenuEntry>> {
    let _guard = shell_op_lock();
    shell_sta().post(move || context_menu::expand_lazy_submenu(parent_index))
}

/// Invoke on the owning STA thread (Files `_owningThread.PostMethod`).
pub fn invoke_on_session(command_offset: u32) -> anyhow::Result<()> {
    let _guard = shell_op_lock();
    shell_sta().post(move || context_menu::invoke_prepared_menu(command_offset))
}
