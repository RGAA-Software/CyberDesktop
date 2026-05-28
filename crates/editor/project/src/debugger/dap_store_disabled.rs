use super::breakpoint_store::BreakpointStore;
use crate::worktree_store::WorktreeStore;
use anyhow::Result;
use gpui::{App, Context, Entity, EventEmitter, Task};
use language::{Buffer, LanguageToolchainStore};
use node_runtime::NodeRuntime;
use rpc::AnyProtoClient;
use std::sync::Arc;

use crate::dap_shim::client::SessionId;

use super::session_disabled::Session;

#[derive(Debug)]
pub enum DapStoreEvent {}

pub struct DapStore {
    breakpoint_store: Entity<BreakpointStore>,
    worktree_store: Entity<WorktreeStore>,
}

impl EventEmitter<DapStoreEvent> for DapStore {}

impl DapStore {
    pub fn init(_client: &AnyProtoClient, _cx: &mut App) {}

    pub fn new_local(
        _http_client: Arc<dyn http_client::HttpClient>,
        _node_runtime: NodeRuntime,
        _fs: Arc<dyn fs::Fs>,
        _environment: Entity<crate::ProjectEnvironment>,
        _toolchain_store: Arc<dyn LanguageToolchainStore>,
        worktree_store: Entity<WorktreeStore>,
        breakpoint_store: Entity<BreakpointStore>,
        _is_headless: bool,
        _cx: &mut Context<Self>,
    ) -> Self {
        Self {
            breakpoint_store,
            worktree_store,
        }
    }

    pub fn new_remote(
        _project_id: u64,
        _remote_client: Entity<remote::RemoteClient>,
        breakpoint_store: Entity<BreakpointStore>,
        worktree_store: Entity<WorktreeStore>,
        _node_runtime: NodeRuntime,
        _http_client: Arc<dyn http_client::HttpClient>,
        _fs: Arc<dyn fs::Fs>,
        _cx: &mut Context<Self>,
    ) -> Self {
        Self {
            breakpoint_store,
            worktree_store,
        }
    }

    pub fn new_collab(
        _project_id: u64,
        _upstream_client: AnyProtoClient,
        breakpoint_store: Entity<BreakpointStore>,
        worktree_store: Entity<WorktreeStore>,
        _fs: Arc<dyn fs::Fs>,
        _cx: &mut Context<Self>,
    ) -> Self {
        Self {
            breakpoint_store,
            worktree_store,
        }
    }

    pub fn session_by_id(&self, _session_id: impl std::borrow::Borrow<SessionId>) -> Option<Entity<Session>> {
        None
    }

    pub fn sessions(&self) -> impl Iterator<Item = &Entity<Session>> {
        std::iter::empty()
    }

    pub fn breakpoint_store(&self) -> &Entity<BreakpointStore> {
        &self.breakpoint_store
    }

    pub fn worktree_store(&self) -> &Entity<WorktreeStore> {
        &self.worktree_store
    }

    pub fn resolve_inline_value_locations(
        &self,
        _session: Entity<Session>,
        _stack_frame_id: crate::dap_shim::StackFrameId,
        _buffer_handle: Entity<Buffer>,
        _inline_value_locations: Vec<crate::dap_shim::inline_value::InlineValueLocation>,
        _cx: &mut Context<Self>,
    ) -> Task<Result<Vec<crate::InlayHint>>> {
        Task::ready(Ok(Vec::new()))
    }

    pub fn shared(
        &mut self,
        _project_id: u64,
        _downstream_client: AnyProtoClient,
        _cx: &mut Context<Self>,
    ) {
    }

    pub fn unshared(&mut self, _cx: &mut Context<Self>) {}

    pub fn debug_scenario_for_build_task(
        &self,
        _build: task::TaskTemplate,
        _adapter: crate::dap_shim::DebugAdapterName,
        _label: gpui::SharedString,
        _cx: &mut App,
    ) -> Task<Option<task::DebugScenario>> {
        Task::ready(None)
    }
}
