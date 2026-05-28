use std::sync::Arc;

use gpui::{App, Context, Entity, EventEmitter, WeakEntity};
use remote::RemoteClient;

use crate::{Project, worktree_store::WorktreeStore};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ContextServerId(pub Arc<str>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextServerStatus {
    Stopped,
}

#[derive(Debug)]
pub struct ServerStatusChangedEvent {
    pub server_id: ContextServerId,
    pub status: ContextServerStatus,
}

pub fn init(_cx: &mut App) {}

pub struct ContextServerStore;

impl EventEmitter<ServerStatusChangedEvent> for ContextServerStore {}

impl ContextServerStore {
    pub fn local(
        _worktree_store: Entity<WorktreeStore>,
        _weak_project: Option<WeakEntity<Project>>,
        _headless: bool,
        _cx: &mut Context<Self>,
    ) -> Self {
        Self
    }

    pub fn remote(
        _project_id: u64,
        _upstream_client: Entity<RemoteClient>,
        _worktree_store: Entity<WorktreeStore>,
        _weak_project: Option<WeakEntity<Project>>,
        _cx: &mut Context<Self>,
    ) -> Self {
        Self
    }
}
