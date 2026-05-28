use std::{collections::HashSet, ops::Range, sync::Arc};

use fs::Fs;
use gpui::{AsyncApp, Context, Entity, EventEmitter, WeakEntity};
use language::{Buffer, LanguageRegistry, OffsetUtf16, language_settings::LanguageSettings};
use util::rel_path::RelPath;
use lsp::{LanguageServer, LanguageServerId, LanguageServerName};
use node_runtime::NodeRuntime;

use crate::{PathChange, ProjectEntryId, Worktree, lsp_store::WorktreeId, worktree_store::WorktreeStore};

pub(crate) enum PrettierStoreEvent {
    LanguageServerRemoved(LanguageServerId),
    LanguageServerAdded {
        new_server_id: LanguageServerId,
        name: LanguageServerName,
        prettier_server: Arc<LanguageServer>,
    },
}

impl EventEmitter<PrettierStoreEvent> for PrettierStore {}

pub struct PrettierStore;

impl PrettierStore {
    pub fn new(
        _node: NodeRuntime,
        _fs: Arc<dyn Fs>,
        _languages: Arc<LanguageRegistry>,
        _worktree_store: Entity<WorktreeStore>,
        _: &mut Context<Self>,
    ) -> Self {
        Self
    }

    pub fn remove_worktree(&mut self, _id_to_remove: WorktreeId, _cx: &mut Context<Self>) {}

    pub fn install_default_prettier(
        &mut self,
        _worktree: Option<WorktreeId>,
        _plugins: impl Iterator<Item = Arc<str>>,
        _cx: &mut Context<Self>,
    ) {
    }

    pub fn on_settings_changed(
        &mut self,
        _language_formatters_to_check: Vec<(Option<WorktreeId>, LanguageSettings)>,
        _cx: &mut Context<Self>,
    ) {
    }

    pub fn update_prettier_settings(
        &self,
        _worktree: &Entity<Worktree>,
        _changes: &[(Arc<RelPath>, ProjectEntryId, PathChange)],
        _cx: &mut Context<Self>,
    ) {
    }
}

pub fn prettier_plugins_for_language(
    _language_settings: &LanguageSettings,
) -> Option<&HashSet<String>> {
    None
}

pub(super) async fn format_with_prettier(
    _prettier_store: &WeakEntity<PrettierStore>,
    _buffer: &Entity<Buffer>,
    _range_utf16: Option<Range<OffsetUtf16>>,
    _cx: &mut AsyncApp,
) -> Option<anyhow::Result<language::Diff>> {
    None
}
