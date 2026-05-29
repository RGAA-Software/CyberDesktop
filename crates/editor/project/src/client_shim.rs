//! Stand-ins for `client` types when `collab-client` is disabled (local / notepad builds).

use anyhow::{Context as _, Result, anyhow};
use clock::SystemClock;
use futures::{
    FutureExt,
    future::{BoxFuture, Ready, ready},
};
use gpui::{AnyWeakEntity, App, AsyncApp, Context, Entity, EventEmitter, SharedString, SharedUri, WeakEntity};
use http_client::HttpClientWithUrl;
use parking_lot::Mutex;
use postage::watch;
use rpc::{
    ProtoClient, ProtoMessageHandlerSet,
    proto::{Envelope, EnvelopedMessage, RequestMessage},
};
use collections::HashMap;
use std::{
    any::TypeId,
    marker::PhantomData,
    sync::{Arc, Weak},
};
use text::ReplicaId;

pub use rpc::ErrorExt;
pub use rpc::{TypedEnvelope, proto};
// TypedEnvelope used in method signatures above via `rpc::TypedEnvelope` through the re-export.

pub enum Subscription {
    Entity {
        client: Weak<Client>,
        id: (TypeId, u64),
    },
    Message {
        client: Weak<Client>,
        id: TypeId,
    },
}

impl Drop for Subscription {
    fn drop(&mut self) {}
}

pub struct PendingEntitySubscription<T: 'static> {
    _entity_type: PhantomData<T>,
}

impl<T: 'static> PendingEntitySubscription<T> {
    pub fn set_entity(self, _entity: &Entity<T>, _cx: &AsyncApp) -> Subscription {
        Subscription::Message {
            client: Weak::new(),
            id: TypeId::of::<T>(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ChannelId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProjectId(pub u64);

impl ProjectId {
    pub fn to_proto(self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ParticipantIndex(pub u32);

pub type LegacyUserId = u64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    pub legacy_id: LegacyUserId,
    pub github_login: SharedString,
    pub avatar_uri: SharedUri,
    pub name: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Collaborator {
    pub peer_id: proto::PeerId,
    pub replica_id: ReplicaId,
    pub user_id: LegacyUserId,
    pub is_host: bool,
    pub committer_name: Option<String>,
    pub committer_email: Option<String>,
}

impl Collaborator {
    pub fn from_proto(message: proto::Collaborator) -> Result<Self> {
        Ok(Self {
            peer_id: message.peer_id.context("invalid peer id")?,
            replica_id: ReplicaId::new(message.replica_id as u16),
            user_id: message.user_id as LegacyUserId,
            is_host: message.is_host,
            committer_name: message.committer_name,
            committer_email: message.committer_email,
        })
    }
}

pub mod telemetry {
    use super::*;
    use worktree::{UpdatedEntriesSet, WorktreeId};

    pub struct Telemetry;

    impl Telemetry {
        pub fn new(
            _clock: Arc<dyn SystemClock>,
            _http: Arc<HttpClientWithUrl>,
            _cx: &mut App,
        ) -> Arc<Self> {
            Arc::new(Self)
        }

        pub fn log_edit_event(self: &Arc<Self>, _environment: &'static str, _is_via_ssh: bool) {}

        pub fn report_discovered_project_type_events(
            self: &Arc<Self>,
            _worktree_id: WorktreeId,
            _changes: &UpdatedEntriesSet,
        ) {
        }
    }
}

pub struct Client {
    http: Arc<HttpClientWithUrl>,
    telemetry: Arc<telemetry::Telemetry>,
    handler_set: Mutex<ProtoMessageHandlerSet>,
}

impl Client {
    pub fn new(
        clock: Arc<dyn SystemClock>,
        http: Arc<HttpClientWithUrl>,
        cx: &mut App,
    ) -> Arc<Self> {
        Arc::new(Self {
            telemetry: telemetry::Telemetry::new(clock, http.clone(), cx),
            http,
            handler_set: Mutex::new(ProtoMessageHandlerSet::default()),
        })
    }

    pub fn http_client(&self) -> Arc<HttpClientWithUrl> {
        self.http.clone()
    }

    pub fn telemetry(&self) -> &Arc<telemetry::Telemetry> {
        &self.telemetry
    }

    pub fn send<T: EnvelopedMessage>(&self, _message: T) -> Result<()> {
        Ok(())
    }

    pub fn request<T: RequestMessage>(
        &self,
        _message: T,
    ) -> impl futures::Future<Output = Result<T::Response>> + Send {
        ready(Err(anyhow!("collaboration client disabled")))
    }

    pub fn request_envelope<T: RequestMessage>(
        &self,
        _message: T,
    ) -> impl futures::Future<Output = Result<Envelope>> + Send {
        ready(Err(anyhow!("collaboration client disabled")))
    }

    pub fn status(&self) -> watch::Receiver<Status> {
        watch::channel_with(Status::SignedOut).1
    }

    pub fn subscribe_to_entity<T>(self: &Arc<Self>, _remote_id: u64) -> Result<PendingEntitySubscription<T>>
    where
        T: 'static,
    {
        Ok(PendingEntitySubscription {
            _entity_type: PhantomData,
        })
    }

    pub fn peer_id(&self) -> Option<proto::PeerId> {
        None
    }

    pub fn add_request_handler<M, E, H, F>(
        self: &Arc<Self>,
        _entity: WeakEntity<E>,
        _handler: H,
    ) -> Subscription
    where
        M: EnvelopedMessage,
        E: 'static,
        H: 'static + Send + Sync + Fn(Entity<E>, TypedEnvelope<M>, AsyncApp) -> F,
        F: 'static + futures::Future<Output = Result<()>>,
    {
        Subscription::Message {
            client: Weak::new(),
            id: TypeId::of::<M>(),
        }
    }

    pub fn add_message_handler<M, E, H, F>(
        self: &Arc<Self>,
        _entity: WeakEntity<E>,
        _handler: H,
    ) -> Subscription
    where
        M: EnvelopedMessage,
        E: 'static,
        H: 'static + Send + Sync + Fn(Entity<E>, TypedEnvelope<M>, AsyncApp) -> F,
        F: 'static + futures::Future<Output = Result<()>>,
    {
        Subscription::Message {
            client: Weak::new(),
            id: TypeId::of::<M>(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    SignedOut,
    UpgradeRequired,
    Authenticating,
    Authenticated,
    AuthenticationError,
    Connecting,
    ConnectionError,
    Connected {
        peer_id: proto::PeerId,
        connection_id: u32,
    },
    ConnectionLost,
    Reauthenticating,
    Reauthenticated,
    Reconnecting,
    ReconnectionError {
        next_reconnection: std::time::Instant,
    },
}

impl Status {
    pub fn is_connected(&self) -> bool {
        matches!(self, Self::Connected { .. })
    }
}

impl ProtoClient for Client {
    fn request(
        &self,
        _envelope: Envelope,
        _request_type: &'static str,
    ) -> BoxFuture<'static, Result<Envelope>> {
        ready(Err(anyhow!("collaboration client disabled"))).boxed()
    }

    fn send(&self, _envelope: Envelope, _message_type: &'static str) -> Result<()> {
        Ok(())
    }

    fn send_response(&self, _envelope: Envelope, _message_type: &'static str) -> Result<()> {
        Ok(())
    }

    fn message_handler_set(&self) -> &Mutex<ProtoMessageHandlerSet> {
        &self.handler_set
    }

    fn is_via_collab(&self) -> bool {
        false
    }

    fn has_wsl_interop(&self) -> bool {
        false
    }
}

pub struct UserStore {
    current_user: watch::Receiver<Option<Arc<User>>>,
    participant_indices: HashMap<LegacyUserId, ParticipantIndex>,
}

impl EventEmitter<()> for UserStore {}

impl UserStore {
    pub fn new(_client: Arc<Client>, _cx: &Context<Self>) -> Self {
        Self {
            current_user: watch::channel_with(None).1,
            participant_indices: HashMap::default(),
        }
    }

    pub fn current_user(&self) -> Option<Arc<User>> {
        self.current_user.borrow().clone()
    }

    pub fn watch_current_user(&self) -> watch::Receiver<Option<Arc<User>>> {
        self.current_user.clone()
    }

    pub fn participant_indices(&self) -> &HashMap<LegacyUserId, ParticipantIndex> {
        &self.participant_indices
    }

    pub fn participant_names(
        &self,
        _user_ids: impl Iterator<Item = u64>,
        _cx: &App,
    ) -> HashMap<u64, SharedString> {
        HashMap::default()
    }

    pub fn get_users(
        &mut self,
        _user_ids: Vec<LegacyUserId>,
        _cx: &mut Context<Self>,
    ) -> gpui::Task<Result<Vec<Arc<User>>>> {
        gpui::Task::ready(Ok(Vec::new()))
    }
}

pub fn parse_zed_link(_link: &str, _cx: &App) -> Option<()> {
    None
}
