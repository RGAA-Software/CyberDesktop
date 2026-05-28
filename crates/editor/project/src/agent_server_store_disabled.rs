use std::{collections::HashMap, sync::Arc};

use fs::Fs;
use gpui::{Context, Entity, EventEmitter, SharedString};
use http_client::HttpClient;
use node_runtime::NodeRuntime;
use remote::RemoteClient;
use rpc::AnyProtoClient;

use crate::{ProjectEnvironment, worktree_store::WorktreeStore};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AgentId(pub SharedString);

impl std::fmt::Display for AgentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&'static str> for AgentId {
    fn from(value: &'static str) -> Self {
        AgentId(value.into())
    }
}

impl AgentId {
    pub fn new(id: impl Into<SharedString>) -> Self {
        AgentId(id.into())
    }
}

impl From<AgentId> for SharedString {
    fn from(value: AgentId) -> Self {
        value.0
    }
}

impl AsRef<str> for AgentId {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExternalAgentSource {
    Extension,
}

pub struct ExternalAgentEntry;

pub struct AgentServerStore {
    pub external_agents: HashMap<AgentId, ExternalAgentEntry>,
}

pub struct AgentServersUpdated;

impl EventEmitter<AgentServersUpdated> for AgentServerStore {}

impl AgentServerStore {
    pub fn init_headless(_session: &AnyProtoClient) {}

    pub fn init_remote(_session: &AnyProtoClient) {}

    pub fn collab() -> Self {
        Self {
            external_agents: HashMap::default(),
        }
    }

    pub fn local(
        _node_runtime: NodeRuntime,
        _fs: Arc<dyn Fs>,
        _project_environment: Entity<ProjectEnvironment>,
        _http_client: Arc<dyn HttpClient>,
        _cx: &mut Context<Self>,
    ) -> Self {
        Self::collab()
    }

    pub(crate) fn remote(
        _project_id: u64,
        _upstream_client: Entity<RemoteClient>,
        _worktree_store: Entity<WorktreeStore>,
    ) -> Self {
        Self::collab()
    }

    pub fn shared(&mut self, _project_id: u64, _client: AnyProtoClient, _cx: &mut Context<Self>) {}
}
