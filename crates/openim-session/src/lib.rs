use std::collections::BTreeMap;
use std::path::PathBuf;

use openim_domain::{
    conversation::{ConversationInfo, ConversationService},
    group::GroupService,
    message::{ChatMessage, MessageSender, MessageService},
    relation::RelationService,
    user::UserService,
};
use openim_errors::{OpenImError, Result};
use openim_storage_core::{openim_db_file, openim_indexeddb_name};
use openim_transport_core::TransportConfig;
use openim_types::{Platform, UserId};

pub type ListenerId = u64;

const TRANSPORT_TASK: &str = "transport";
const SYNC_TASK: &str = "sync";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionConfig {
    pub platform: Platform,
    pub api_addr: String,
    pub ws_addr: String,
    pub data_dir: Option<String>,
}

impl SessionConfig {
    pub fn new(
        platform: Platform,
        api_addr: impl Into<String>,
        ws_addr: impl Into<String>,
    ) -> Self {
        Self {
            platform,
            api_addr: api_addr.into(),
            ws_addr: ws_addr.into(),
            data_dir: None,
        }
    }

    pub fn with_data_dir(mut self, data_dir: impl Into<String>) -> Self {
        self.data_dir = Some(data_dir.into());
        self
    }

    fn validate(&self) -> Result<()> {
        ensure_not_empty(&self.api_addr, "api_addr")?;
        ensure_not_empty(&self.ws_addr, "ws_addr")?;
        if let Some(data_dir) = &self.data_dir {
            ensure_not_empty(data_dir, "data_dir")?;
        }
        Ok(())
    }

    pub fn transport_config(&self, credentials: &LoginCredentials) -> Result<TransportConfig> {
        credentials.validate()?;
        Ok(TransportConfig::new(
            self.ws_addr.clone(),
            credentials.user_id.clone(),
            credentials.token.clone(),
            self.platform.as_i32(),
        ))
    }

    pub fn storage_target(&self, login_user_id: &str) -> Result<StorageTarget> {
        ensure_not_empty(login_user_id, "login_user_id")?;
        if matches!(self.platform, Platform::Web | Platform::MiniWeb) {
            return Ok(StorageTarget::IndexedDb {
                name: openim_indexeddb_name(login_user_id)?,
            });
        }

        let Some(data_dir) = &self.data_dir else {
            return Ok(StorageTarget::Unconfigured);
        };
        Ok(StorageTarget::Sqlite {
            path: openim_db_file(data_dir, login_user_id)?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginCredentials {
    pub user_id: UserId,
    pub token: String,
}

impl LoginCredentials {
    pub fn new(user_id: impl Into<UserId>, token: impl Into<String>) -> Self {
        Self {
            user_id: user_id.into(),
            token: token.into(),
        }
    }

    fn validate(&self) -> Result<()> {
        ensure_not_empty(&self.user_id, "user_id")?;
        ensure_not_empty(&self.token, "token")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    Created,
    Initialized,
    LoggedIn,
    LoggedOut,
    Uninitialized,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionResourceKind {
    Storage,
    Transport,
    Sync,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionEvent {
    Initialized,
    LoggedIn {
        user_id: UserId,
    },
    LoggedOut {
        user_id: UserId,
    },
    Uninitialized,
    ListenerRegistered {
        listener_id: ListenerId,
    },
    ListenerUnregistered {
        listener_id: ListenerId,
    },
    TaskStarted {
        name: String,
    },
    TaskStopped {
        name: String,
    },
    ResourceOpened {
        kind: SessionResourceKind,
        name: String,
    },
    ResourceClosed {
        kind: SessionResourceKind,
        name: String,
    },
    NewMessages {
        messages: Vec<ChatMessage>,
    },
    ConversationChanged {
        conversations: Vec<ConversationInfo>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StorageTarget {
    Unconfigured,
    Sqlite { path: PathBuf },
    IndexedDb { name: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionResourceInfo {
    pub kind: SessionResourceKind,
    pub name: String,
}

pub trait SessionResourceHandle: Send {
    fn close(&mut self) -> Result<()>;
}

pub struct SessionResource {
    kind: SessionResourceKind,
    name: String,
    handle: Box<dyn SessionResourceHandle>,
}

impl SessionResource {
    pub fn new(
        kind: SessionResourceKind,
        name: impl Into<String>,
        handle: impl SessionResourceHandle + 'static,
    ) -> Result<Self> {
        let name = name.into();
        ensure_not_empty(&name, "resource_name")?;
        Ok(Self {
            kind,
            name,
            handle: Box::new(handle),
        })
    }

    pub fn kind(&self) -> SessionResourceKind {
        self.kind
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn info(&self) -> SessionResourceInfo {
        SessionResourceInfo {
            kind: self.kind,
            name: self.name.clone(),
        }
    }

    fn close(&mut self) -> Result<()> {
        self.handle.close()
    }
}

pub struct SessionRuntimeResources {
    user_id: UserId,
    transport: TransportConfig,
    storage: StorageTarget,
    resources: Vec<SessionResource>,
}

impl SessionRuntimeResources {
    pub fn new(
        user_id: impl Into<UserId>,
        transport: TransportConfig,
        storage: StorageTarget,
    ) -> Result<Self> {
        let user_id = user_id.into();
        ensure_not_empty(&user_id, "user_id")?;
        Ok(Self {
            user_id,
            transport,
            storage,
            resources: Vec::new(),
        })
    }

    pub fn add_resource(&mut self, resource: SessionResource) {
        self.resources.push(resource);
    }

    pub fn user_id(&self) -> &str {
        &self.user_id
    }

    pub fn transport(&self) -> &TransportConfig {
        &self.transport
    }

    pub fn storage(&self) -> &StorageTarget {
        &self.storage
    }

    pub fn resource_infos(&self) -> Vec<SessionResourceInfo> {
        self.resources.iter().map(SessionResource::info).collect()
    }

    fn ensure_matches(
        &self,
        credentials: &LoginCredentials,
        transport: &TransportConfig,
        storage: &StorageTarget,
    ) -> Result<()> {
        if self.user_id != credentials.user_id {
            return Err(OpenImError::sdk_internal(
                "resource adapter returned resources for a different user",
            ));
        }
        if !same_transport_config(&self.transport, transport) {
            return Err(OpenImError::sdk_internal(
                "resource adapter returned resources for a different transport config",
            ));
        }
        if self.storage != *storage {
            return Err(OpenImError::sdk_internal(
                "resource adapter returned resources for a different storage target",
            ));
        }
        Ok(())
    }

    fn close_all(&mut self) -> Result<Vec<SessionResourceInfo>> {
        let mut closed = Vec::new();
        let mut remaining = Vec::new();
        let mut first_error = None;

        while let Some(mut resource) = self.resources.pop() {
            let info = resource.info();
            match resource.close() {
                Ok(()) => closed.push(info),
                Err(err) => {
                    if first_error.is_none() {
                        first_error = Some(err);
                    }
                    remaining.push(resource);
                }
            }
        }

        remaining.reverse();
        self.resources = remaining;

        if let Some(err) = first_error {
            return Err(err);
        }

        Ok(closed)
    }
}

#[derive(Debug, Default)]
pub struct DomainServices {
    pub users: UserService,
    pub relations: RelationService,
    pub groups: GroupService,
    pub messages: MessageService,
    pub conversations: ConversationService,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskInfo {
    pub name: String,
    pub running: bool,
}

#[derive(Debug, Default)]
pub struct TaskSupervisor {
    tasks: BTreeMap<String, bool>,
}

impl TaskSupervisor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start(&mut self, name: &str) -> Result<bool> {
        ensure_not_empty(name, "task_name")?;
        let was_running = self.tasks.get(name).copied().unwrap_or(false);
        self.tasks.insert(name.to_string(), true);
        Ok(!was_running)
    }

    pub fn stop(&mut self, name: &str) -> Result<bool> {
        ensure_not_empty(name, "task_name")?;
        let Some(running) = self.tasks.get_mut(name) else {
            return Ok(false);
        };
        if !*running {
            return Ok(false);
        }
        *running = false;
        Ok(true)
    }

    pub fn stop_all(&mut self) -> Vec<String> {
        let stopped = self
            .tasks
            .iter()
            .filter_map(|(name, running)| running.then(|| name.clone()))
            .collect::<Vec<_>>();
        self.tasks.clear();
        stopped
    }

    pub fn is_running(&self, name: &str) -> bool {
        self.tasks.get(name).copied().unwrap_or(false)
    }

    pub fn tasks(&self) -> Vec<TaskInfo> {
        self.tasks
            .iter()
            .map(|(name, running)| TaskInfo {
                name: name.clone(),
                running: *running,
            })
            .collect()
    }
}

type SessionCallback = Box<dyn Fn(&SessionEvent) + Send + Sync + 'static>;

#[derive(Default)]
pub struct ListenerRegistry {
    next_id: ListenerId,
    listeners: BTreeMap<ListenerId, SessionCallback>,
}

impl ListenerRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register<F>(&mut self, callback: F) -> ListenerId
    where
        F: Fn(&SessionEvent) + Send + Sync + 'static,
    {
        self.next_id += 1;
        let listener_id = self.next_id;
        self.listeners.insert(listener_id, Box::new(callback));
        listener_id
    }

    pub fn unregister(&mut self, listener_id: ListenerId) -> bool {
        self.listeners.remove(&listener_id).is_some()
    }

    pub fn emit(&self, event: &SessionEvent) {
        for listener in self.listeners.values() {
            listener(event);
        }
    }

    pub fn len(&self) -> usize {
        self.listeners.len()
    }

    pub fn is_empty(&self) -> bool {
        self.listeners.is_empty()
    }
}

pub trait SessionResourceAdapter: Send {
    fn init(&mut self, config: &SessionConfig) -> Result<()>;
    fn login(
        &mut self,
        config: &SessionConfig,
        credentials: &LoginCredentials,
        transport: &TransportConfig,
        storage: &StorageTarget,
    ) -> Result<SessionRuntimeResources>;
    fn logout(&mut self, user_id: &str) -> Result<()>;
    fn uninit(&mut self) -> Result<()>;
}

pub trait SessionMessageTransport: MessageSender {
    fn pull_messages(
        &mut self,
        owner_user_id: &str,
        conversation_id: &str,
    ) -> Result<Vec<ChatMessage>>;
    fn pop_push_messages(&mut self, owner_user_id: &str) -> Result<Vec<ChatMessage>>;
}

#[derive(Debug, Default)]
pub struct NoopSessionResourceAdapter;

impl SessionResourceAdapter for NoopSessionResourceAdapter {
    fn init(&mut self, _config: &SessionConfig) -> Result<()> {
        Ok(())
    }

    fn login(
        &mut self,
        _config: &SessionConfig,
        credentials: &LoginCredentials,
        transport: &TransportConfig,
        storage: &StorageTarget,
    ) -> Result<SessionRuntimeResources> {
        SessionRuntimeResources::new(
            credentials.user_id.clone(),
            transport.clone(),
            storage.clone(),
        )
    }

    fn logout(&mut self, _user_id: &str) -> Result<()> {
        Ok(())
    }

    fn uninit(&mut self) -> Result<()> {
        Ok(())
    }
}

pub struct OpenImSession {
    config: SessionConfig,
    state: SessionState,
    login_user_id: Option<UserId>,
    domains: DomainServices,
    listeners: ListenerRegistry,
    tasks: TaskSupervisor,
    resources: Box<dyn SessionResourceAdapter>,
    runtime_resources: Option<SessionRuntimeResources>,
}

impl OpenImSession {
    pub fn new(config: SessionConfig) -> Result<Self> {
        Self::with_resource_adapter(config, Box::new(NoopSessionResourceAdapter))
    }

    pub fn with_resource_adapter(
        config: SessionConfig,
        resources: Box<dyn SessionResourceAdapter>,
    ) -> Result<Self> {
        config.validate()?;
        Ok(Self {
            config,
            state: SessionState::Created,
            login_user_id: None,
            domains: DomainServices::default(),
            listeners: ListenerRegistry::new(),
            tasks: TaskSupervisor::new(),
            resources,
            runtime_resources: None,
        })
    }

    pub fn init(&mut self) -> Result<()> {
        match self.state {
            SessionState::Created | SessionState::Uninitialized => {
                self.resources.init(&self.config)?;
                self.state = SessionState::Initialized;
                self.emit(SessionEvent::Initialized);
                Ok(())
            }
            SessionState::Initialized | SessionState::LoggedOut | SessionState::LoggedIn => Ok(()),
        }
    }

    pub fn login(&mut self, credentials: LoginCredentials) -> Result<()> {
        credentials.validate()?;
        match self.state {
            SessionState::Initialized | SessionState::LoggedOut => {}
            SessionState::LoggedIn => return Err(OpenImError::args("session already logged in")),
            SessionState::Created | SessionState::Uninitialized => {
                return Err(OpenImError::args("session is not initialized"));
            }
        }

        let transport = self.config.transport_config(&credentials)?;
        let storage = self.config.storage_target(&credentials.user_id)?;
        let runtime_resources =
            self.resources
                .login(&self.config, &credentials, &transport, &storage)?;
        runtime_resources.ensure_matches(&credentials, &transport, &storage)?;
        let opened_resources = runtime_resources.resource_infos();
        self.login_user_id = Some(credentials.user_id.clone());
        self.runtime_resources = Some(runtime_resources);
        self.domains = DomainServices::default();
        self.state = SessionState::LoggedIn;
        self.start_task(TRANSPORT_TASK)?;
        self.start_task(SYNC_TASK)?;
        for resource in opened_resources {
            self.emit(SessionEvent::ResourceOpened {
                kind: resource.kind,
                name: resource.name,
            });
        }
        self.emit(SessionEvent::LoggedIn {
            user_id: credentials.user_id,
        });
        Ok(())
    }

    pub fn logout(&mut self) -> Result<()> {
        let Some(user_id) = self.login_user_id.clone() else {
            return Ok(());
        };

        self.resources.logout(&user_id)?;
        let closed_resources = self.close_runtime_resources()?;
        self.stop_all_tasks();
        self.login_user_id = None;
        self.domains = DomainServices::default();
        self.state = SessionState::LoggedOut;
        for resource in closed_resources {
            self.emit(SessionEvent::ResourceClosed {
                kind: resource.kind,
                name: resource.name,
            });
        }
        self.emit(SessionEvent::LoggedOut { user_id });
        Ok(())
    }

    pub fn uninit(&mut self) -> Result<()> {
        self.resources.uninit()?;
        let closed_resources = self.close_runtime_resources()?;
        self.stop_all_tasks();
        self.login_user_id = None;
        self.domains = DomainServices::default();
        self.state = SessionState::Uninitialized;
        for resource in closed_resources {
            self.emit(SessionEvent::ResourceClosed {
                kind: resource.kind,
                name: resource.name,
            });
        }
        self.emit(SessionEvent::Uninitialized);
        Ok(())
    }

    pub fn register_listener<F>(&mut self, callback: F) -> ListenerId
    where
        F: Fn(&SessionEvent) + Send + Sync + 'static,
    {
        let listener_id = self.listeners.register(callback);
        self.emit(SessionEvent::ListenerRegistered { listener_id });
        listener_id
    }

    pub fn unregister_listener(&mut self, listener_id: ListenerId) -> bool {
        let removed = self.listeners.unregister(listener_id);
        if removed {
            self.emit(SessionEvent::ListenerUnregistered { listener_id });
        }
        removed
    }

    pub fn start_task(&mut self, name: &str) -> Result<()> {
        if self.tasks.start(name)? {
            self.emit(SessionEvent::TaskStarted {
                name: name.to_string(),
            });
        }
        Ok(())
    }

    pub fn stop_task(&mut self, name: &str) -> Result<()> {
        if self.tasks.stop(name)? {
            self.emit(SessionEvent::TaskStopped {
                name: name.to_string(),
            });
        }
        Ok(())
    }

    pub fn dispatch_new_messages(&self, messages: Vec<ChatMessage>) -> Result<()> {
        self.ensure_logged_in()?;
        if !messages.is_empty() {
            self.emit(SessionEvent::NewMessages { messages });
        }
        Ok(())
    }

    pub fn dispatch_conversation_changed(
        &self,
        conversations: Vec<ConversationInfo>,
    ) -> Result<()> {
        self.ensure_logged_in()?;
        if !conversations.is_empty() {
            self.emit(SessionEvent::ConversationChanged { conversations });
        }
        Ok(())
    }

    pub fn send_message(
        &mut self,
        message: ChatMessage,
        sender: &mut dyn MessageSender,
    ) -> Result<ChatMessage> {
        let owner_user_id = self.logged_in_user_id()?;
        ensure_outgoing_owner(&owner_user_id, &message)?;

        let sent = self.domains.messages.send_message(message, sender)?;
        let conversations =
            self.apply_messages_to_conversations(&owner_user_id, &[sent.clone()])?;
        self.dispatch_conversation_changed(conversations)?;
        Ok(sent)
    }

    pub fn pull_messages(
        &mut self,
        conversation_id: &str,
        transport: &mut dyn SessionMessageTransport,
    ) -> Result<Vec<ChatMessage>> {
        let owner_user_id = self.logged_in_user_id()?;
        ensure_not_empty(conversation_id, "conversation_id")?;

        let messages = transport.pull_messages(&owner_user_id, conversation_id)?;
        if messages.is_empty() {
            return Ok(messages);
        }
        for message in &messages {
            ensure_message_visible_to_owner(&owner_user_id, message)?;
        }

        self.domains
            .messages
            .sync_message_range(conversation_id, messages.clone())?;
        let conversations = self.apply_messages_to_conversations(&owner_user_id, &messages)?;
        self.dispatch_new_messages(messages.clone())?;
        self.dispatch_conversation_changed(conversations)?;
        Ok(messages)
    }

    pub fn receive_transport_pushes(
        &mut self,
        transport: &mut dyn SessionMessageTransport,
    ) -> Result<Vec<ChatMessage>> {
        let owner_user_id = self.logged_in_user_id()?;
        let pushed = transport.pop_push_messages(&owner_user_id)?;
        if pushed.is_empty() {
            return Ok(pushed);
        }

        let mut received = Vec::with_capacity(pushed.len());
        for message in pushed {
            ensure_message_visible_to_owner(&owner_user_id, &message)?;
            received.push(self.domains.messages.receive_message(message)?);
        }

        let conversations = self.apply_messages_to_conversations(&owner_user_id, &received)?;
        self.dispatch_new_messages(received.clone())?;
        self.dispatch_conversation_changed(conversations)?;
        Ok(received)
    }

    pub fn state(&self) -> SessionState {
        self.state
    }

    pub fn config(&self) -> &SessionConfig {
        &self.config
    }

    pub fn login_user_id(&self) -> Option<&str> {
        self.login_user_id.as_deref()
    }

    pub fn domains(&self) -> &DomainServices {
        &self.domains
    }

    pub fn domains_mut(&mut self) -> Result<&mut DomainServices> {
        self.ensure_logged_in()?;
        Ok(&mut self.domains)
    }

    pub fn listener_count(&self) -> usize {
        self.listeners.len()
    }

    pub fn tasks(&self) -> Vec<TaskInfo> {
        self.tasks.tasks()
    }

    pub fn is_task_running(&self, name: &str) -> bool {
        self.tasks.is_running(name)
    }

    pub fn runtime_resources(&self) -> Option<&SessionRuntimeResources> {
        self.runtime_resources.as_ref()
    }

    fn close_runtime_resources(&mut self) -> Result<Vec<SessionResourceInfo>> {
        let Some(resources) = &mut self.runtime_resources else {
            return Ok(Vec::new());
        };

        let closed = resources.close_all()?;
        self.runtime_resources = None;
        Ok(closed)
    }

    fn stop_all_tasks(&mut self) {
        for name in self.tasks.stop_all() {
            self.emit(SessionEvent::TaskStopped { name });
        }
    }

    fn emit(&self, event: SessionEvent) {
        self.listeners.emit(&event);
    }

    fn apply_messages_to_conversations(
        &mut self,
        owner_user_id: &str,
        messages: &[ChatMessage],
    ) -> Result<Vec<ConversationInfo>> {
        let mut changed = BTreeMap::<String, ConversationInfo>::new();
        for message in messages {
            let conversation = self
                .domains
                .conversations
                .apply_message(owner_user_id, message)?;
            changed.insert(conversation.conversation_id.clone(), conversation);
        }
        Ok(changed.into_values().collect())
    }

    fn logged_in_user_id(&self) -> Result<UserId> {
        self.ensure_logged_in()?;
        self.login_user_id
            .clone()
            .ok_or_else(|| OpenImError::sdk_internal("login user missing"))
    }

    fn ensure_logged_in(&self) -> Result<()> {
        if self.state != SessionState::LoggedIn {
            return Err(OpenImError::args("session is not logged in"));
        }
        Ok(())
    }
}

fn ensure_outgoing_owner(owner_user_id: &str, message: &ChatMessage) -> Result<()> {
    if message.send_id == owner_user_id {
        Ok(())
    } else {
        Err(OpenImError::args(
            "message send_id does not match login user",
        ))
    }
}

fn ensure_message_visible_to_owner(owner_user_id: &str, message: &ChatMessage) -> Result<()> {
    if message.send_id == owner_user_id || message.recv_id == owner_user_id {
        return Ok(());
    }
    if !message.group_id.is_empty() {
        return Ok(());
    }
    Err(OpenImError::args("message is outside login user scope"))
}

fn ensure_not_empty(value: &str, field: &str) -> Result<()> {
    if value.is_empty() {
        Err(OpenImError::args(format!("{field} is empty")))
    } else {
        Ok(())
    }
}

fn same_transport_config(left: &TransportConfig, right: &TransportConfig) -> bool {
    left.ws_addr == right.ws_addr
        && left.user_id == right.user_id
        && left.token == right.token
        && left.platform_id == right.platform_id
        && left.operation_id == right.operation_id
        && left.sdk_type == right.sdk_type
        && left.sdk_version == right.sdk_version
        && left.is_background == right.is_background
        && left.compression == right.compression
        && left.send_response == right.send_response
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use openim_domain::{
        conversation::ConversationInfo,
        file::FileDigest,
        message::{ChatMessage, MessageContent, MessageSender, MessageSnapshot, SendMessageAck},
        user::UserProfile,
    };
    use openim_types::{MessageStatus, SessionType};

    use super::*;

    fn config() -> SessionConfig {
        SessionConfig::new(
            Platform::Web,
            "https://api.openim.test",
            "wss://ws.openim.test",
        )
    }

    fn credentials() -> LoginCredentials {
        LoginCredentials::new("u1", "token")
    }

    fn inbound_message() -> ChatMessage {
        ChatMessage::incoming(
            "client-1",
            "server-1",
            "u2",
            "u1",
            SessionType::Single,
            MessageContent::Text {
                content: "hello".to_string(),
            },
            1,
            100,
        )
        .unwrap()
    }

    #[test]
    fn session_config_builds_transport_and_storage_targets() {
        let credentials = credentials();
        let transport = config().transport_config(&credentials).unwrap();

        assert_eq!(transport.ws_addr, "wss://ws.openim.test");
        assert_eq!(transport.user_id, "u1");
        assert_eq!(transport.token, "token");
        assert_eq!(transport.platform_id, Platform::Web.as_i32());
        assert!(transport.compression);
        assert_eq!(
            config().storage_target("u1").unwrap(),
            StorageTarget::IndexedDb {
                name: "OpenIM_v3_u1".to_string(),
            }
        );

        let native = SessionConfig::new(
            Platform::Macos,
            "https://api.openim.test",
            "wss://ws.openim.test",
        )
        .with_data_dir("db");
        assert_eq!(
            native.storage_target("u1").unwrap(),
            StorageTarget::Sqlite {
                path: std::env::current_dir()
                    .unwrap()
                    .join("db")
                    .join("OpenIM_v3_u1.db"),
            }
        );
    }

    #[test]
    fn lifecycle_starts_and_stops_tasks() {
        let events = Arc::new(Mutex::new(Vec::<SessionEvent>::new()));
        let captured = events.clone();
        let mut session = OpenImSession::new(config()).unwrap();
        let listener_id = session.register_listener(move |event| {
            captured.lock().unwrap().push(event.clone());
        });

        session.init().unwrap();
        session.login(credentials()).unwrap();

        assert_eq!(session.state(), SessionState::LoggedIn);
        assert_eq!(session.login_user_id(), Some("u1"));
        assert!(session.is_task_running(TRANSPORT_TASK));
        assert!(session.is_task_running(SYNC_TASK));
        assert_eq!(session.listener_count(), 1);

        session.unregister_listener(listener_id);
        session.uninit().unwrap();

        let events = events.lock().unwrap();
        assert!(events.contains(&SessionEvent::Initialized));
        assert!(events.contains(&SessionEvent::LoggedIn {
            user_id: "u1".to_string(),
        }));
        assert!(events.contains(&SessionEvent::TaskStarted {
            name: TRANSPORT_TASK.to_string(),
        }));
        assert!(!events.contains(&SessionEvent::Uninitialized));
    }

    #[test]
    fn logout_clears_login_scoped_resources_and_is_idempotent() {
        let mut session = OpenImSession::new(config()).unwrap();
        session.init().unwrap();
        session.login(credentials()).unwrap();
        session
            .domains_mut()
            .unwrap()
            .users
            .upsert_profile(UserProfile {
                user_id: "u1".to_string(),
                nickname: "Alice".to_string(),
                face_url: String::new(),
                ex: String::new(),
                updated_at: 1,
            })
            .unwrap();

        session.logout().unwrap();
        session.logout().unwrap();

        assert_eq!(session.state(), SessionState::LoggedOut);
        assert_eq!(session.login_user_id(), None);
        assert!(session.domains().users.is_empty());
        assert!(session.tasks().is_empty());
        assert!(session.domains_mut().is_err());
    }

    #[test]
    fn login_requires_initialized_session_and_credentials() {
        let mut session = OpenImSession::new(config()).unwrap();

        assert!(session.login(credentials()).is_err());
        session.init().unwrap();
        assert!(session.login(LoginCredentials::new("", "token")).is_err());
        assert!(session.login(LoginCredentials::new("u1", "")).is_err());
    }

    #[test]
    fn listener_unregister_stops_future_callbacks() {
        let first = Arc::new(Mutex::new(0));
        let second = Arc::new(Mutex::new(0));
        let mut session = OpenImSession::new(config()).unwrap();

        let first_events = first.clone();
        let first_id = session.register_listener(move |_| {
            *first_events.lock().unwrap() += 1;
        });
        let second_events = second.clone();
        session.register_listener(move |_| {
            *second_events.lock().unwrap() += 1;
        });

        session.unregister_listener(first_id);
        session.init().unwrap();

        assert_eq!(*first.lock().unwrap(), 2);
        assert_eq!(*second.lock().unwrap(), 3);
    }

    #[test]
    fn dispatches_message_and_conversation_events_only_after_login() {
        let events = Arc::new(Mutex::new(Vec::<SessionEvent>::new()));
        let captured = events.clone();
        let mut session = OpenImSession::new(config()).unwrap();
        session.register_listener(move |event| {
            captured.lock().unwrap().push(event.clone());
        });

        assert!(session.dispatch_new_messages(Vec::new()).is_err());

        session.init().unwrap();
        session.login(credentials()).unwrap();
        let message = inbound_message();
        let mut conversation = ConversationInfo::from_message("u1", &message).unwrap();
        conversation.latest_message = Some(MessageSnapshot::from(&message));
        conversation.latest_msg_send_time = message.send_time;

        session
            .dispatch_new_messages(vec![message.clone()])
            .unwrap();
        session
            .dispatch_conversation_changed(vec![conversation.clone()])
            .unwrap();
        session.dispatch_new_messages(Vec::new()).unwrap();

        let events = events.lock().unwrap();
        assert!(events.contains(&SessionEvent::NewMessages {
            messages: vec![message],
        }));
        assert!(events.contains(&SessionEvent::ConversationChanged {
            conversations: vec![conversation],
        }));
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, SessionEvent::NewMessages { .. }))
                .count(),
            1
        );
    }

    #[test]
    fn session_message_transport_sends_pulls_pushes_and_updates_conversations() {
        let events = Arc::new(Mutex::new(Vec::<SessionEvent>::new()));
        let captured = events.clone();
        let mut session = OpenImSession::new(config()).unwrap();
        session.register_listener(move |event| {
            captured.lock().unwrap().push(event.clone());
        });
        session.init().unwrap();
        session.login(credentials()).unwrap();

        let file = FileDigest {
            file_name: "report.pdf".to_string(),
            file_size: 42,
            content_type: "application/pdf".to_string(),
            sha256: "sha".to_string(),
        };
        let outgoing = ChatMessage::outgoing(
            "file-1",
            "u1",
            "u2",
            SessionType::Single,
            MessageContent::file_from_upload(&file, "https://cdn.openim.test/report.pdf").unwrap(),
            10,
        )
        .unwrap();
        let conversation_id = outgoing.conversation_id.clone();
        let pulled = ChatMessage::incoming(
            "pull-1",
            "server-pull-1",
            "u2",
            "u1",
            SessionType::Single,
            MessageContent::Text {
                content: "pulled".to_string(),
            },
            11,
            110,
        )
        .unwrap();
        let pushed = ChatMessage::incoming(
            "push-1",
            "server-push-1",
            "u2",
            "u1",
            SessionType::Single,
            MessageContent::Text {
                content: "pushed".to_string(),
            },
            12,
            120,
        )
        .unwrap();
        let mut transport = FakeSessionMessageTransport {
            pulled: vec![pulled.clone()],
            pushes: vec![pushed.clone()],
            next_seq: 10,
            ..FakeSessionMessageTransport::default()
        };

        let sent = session
            .send_message(outgoing.clone(), &mut transport)
            .unwrap();
        let pulled_messages = session
            .pull_messages(&conversation_id, &mut transport)
            .unwrap();
        let pushed_messages = session.receive_transport_pushes(&mut transport).unwrap();

        assert_eq!(sent.status, MessageStatus::SendSuccess);
        assert_eq!(sent.server_msg_id, Some("server-file-1".to_string()));
        assert_eq!(pulled_messages, vec![pulled.clone()]);
        assert_eq!(pushed_messages, vec![pushed.clone()]);
        assert_eq!(transport.sent, vec!["file-1".to_string()]);
        assert_eq!(
            transport.pull_requests,
            vec![("u1".to_string(), conversation_id.clone())]
        );
        assert_eq!(transport.push_requests, vec!["u1".to_string()]);

        assert_eq!(
            session
                .domains()
                .messages
                .history(
                    &conversation_id,
                    openim_types::Pagination {
                        page_number: 0,
                        show_number: 10,
                    },
                )
                .unwrap()
                .len(),
            3
        );
        let conversation = session
            .domains()
            .conversations
            .get_conversation("u1", &conversation_id)
            .unwrap()
            .unwrap();
        assert_eq!(conversation.unread_count, 2);
        assert_eq!(conversation.max_seq, 12);
        assert_eq!(conversation.latest_message.unwrap().client_msg_id, "push-1");

        let events = events.lock().unwrap();
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, SessionEvent::NewMessages { .. }))
                .count(),
            2
        );
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, SessionEvent::ConversationChanged { .. }))
                .count(),
            3
        );
    }

    #[test]
    fn resource_adapter_receives_lifecycle_boundaries() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let adapter = RecordingAdapter {
            calls: calls.clone(),
        };
        let mut session =
            OpenImSession::with_resource_adapter(config(), Box::new(adapter)).unwrap();

        session.init().unwrap();
        session.login(credentials()).unwrap();
        session.logout().unwrap();
        session.uninit().unwrap();

        assert_eq!(
            *calls.lock().unwrap(),
            vec![
                "init".to_string(),
                "login:u1:wss://ws.openim.test:OpenIM_v3_u1".to_string(),
                "logout:u1".to_string(),
                "uninit".to_string(),
            ]
        );
    }

    #[test]
    fn logout_closes_runtime_resource_handles() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let events = Arc::new(Mutex::new(Vec::<SessionEvent>::new()));
        let adapter = HandleAdapter {
            calls: calls.clone(),
        };
        let captured = events.clone();
        let mut session =
            OpenImSession::with_resource_adapter(config(), Box::new(adapter)).unwrap();
        session.register_listener(move |event| {
            captured.lock().unwrap().push(event.clone());
        });

        session.init().unwrap();
        session.login(credentials()).unwrap();

        let resources = session.runtime_resources().unwrap();
        assert_eq!(resources.user_id(), "u1");
        assert_eq!(
            resources.resource_infos(),
            vec![
                SessionResourceInfo {
                    kind: SessionResourceKind::Storage,
                    name: "storage:OpenIM_v3_u1".to_string(),
                },
                SessionResourceInfo {
                    kind: SessionResourceKind::Transport,
                    name: "transport:wss://ws.openim.test".to_string(),
                },
            ]
        );

        session.logout().unwrap();

        assert!(session.runtime_resources().is_none());
        assert_eq!(
            *calls.lock().unwrap(),
            vec![
                "close:transport:wss://ws.openim.test".to_string(),
                "close:storage:OpenIM_v3_u1".to_string(),
            ]
        );

        let events = events.lock().unwrap();
        assert!(events.contains(&SessionEvent::ResourceOpened {
            kind: SessionResourceKind::Storage,
            name: "storage:OpenIM_v3_u1".to_string(),
        }));
        assert!(events.contains(&SessionEvent::ResourceClosed {
            kind: SessionResourceKind::Transport,
            name: "transport:wss://ws.openim.test".to_string(),
        }));
    }

    #[test]
    fn uninit_closes_runtime_resource_handles() {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let adapter = HandleAdapter {
            calls: calls.clone(),
        };
        let mut session =
            OpenImSession::with_resource_adapter(config(), Box::new(adapter)).unwrap();

        session.init().unwrap();
        session.login(credentials()).unwrap();
        session.uninit().unwrap();

        assert!(session.runtime_resources().is_none());
        assert_eq!(session.state(), SessionState::Uninitialized);
        assert_eq!(
            *calls.lock().unwrap(),
            vec![
                "close:transport:wss://ws.openim.test".to_string(),
                "close:storage:OpenIM_v3_u1".to_string(),
            ]
        );
    }

    struct RecordingAdapter {
        calls: Arc<Mutex<Vec<String>>>,
    }

    impl SessionResourceAdapter for RecordingAdapter {
        fn init(&mut self, _config: &SessionConfig) -> Result<()> {
            self.calls.lock().unwrap().push("init".to_string());
            Ok(())
        }

        fn login(
            &mut self,
            _config: &SessionConfig,
            credentials: &LoginCredentials,
            transport: &TransportConfig,
            storage: &StorageTarget,
        ) -> Result<SessionRuntimeResources> {
            let storage_name = match storage {
                StorageTarget::IndexedDb { name } => name.clone(),
                StorageTarget::Sqlite { path } => path.display().to_string(),
                StorageTarget::Unconfigured => "unconfigured".to_string(),
            };
            self.calls.lock().unwrap().push(format!(
                "login:{}:{}:{}",
                credentials.user_id, transport.ws_addr, storage_name
            ));
            SessionRuntimeResources::new(
                credentials.user_id.clone(),
                transport.clone(),
                storage.clone(),
            )
        }

        fn logout(&mut self, user_id: &str) -> Result<()> {
            self.calls.lock().unwrap().push(format!("logout:{user_id}"));
            Ok(())
        }

        fn uninit(&mut self) -> Result<()> {
            self.calls.lock().unwrap().push("uninit".to_string());
            Ok(())
        }
    }

    struct HandleAdapter {
        calls: Arc<Mutex<Vec<String>>>,
    }

    impl SessionResourceAdapter for HandleAdapter {
        fn init(&mut self, _config: &SessionConfig) -> Result<()> {
            Ok(())
        }

        fn login(
            &mut self,
            _config: &SessionConfig,
            credentials: &LoginCredentials,
            transport: &TransportConfig,
            storage: &StorageTarget,
        ) -> Result<SessionRuntimeResources> {
            let mut resources = SessionRuntimeResources::new(
                credentials.user_id.clone(),
                transport.clone(),
                storage.clone(),
            )?;
            let storage_name = match storage {
                StorageTarget::IndexedDb { name } => format!("storage:{name}"),
                StorageTarget::Sqlite { path } => format!("storage:{}", path.display()),
                StorageTarget::Unconfigured => "storage:unconfigured".to_string(),
            };
            resources.add_resource(SessionResource::new(
                SessionResourceKind::Storage,
                storage_name.clone(),
                RecordingHandle {
                    name: storage_name,
                    calls: self.calls.clone(),
                },
            )?);
            let transport_name = format!("transport:{}", transport.ws_addr);
            resources.add_resource(SessionResource::new(
                SessionResourceKind::Transport,
                transport_name.clone(),
                RecordingHandle {
                    name: transport_name,
                    calls: self.calls.clone(),
                },
            )?);
            Ok(resources)
        }

        fn logout(&mut self, _user_id: &str) -> Result<()> {
            Ok(())
        }

        fn uninit(&mut self) -> Result<()> {
            Ok(())
        }
    }

    struct RecordingHandle {
        name: String,
        calls: Arc<Mutex<Vec<String>>>,
    }

    impl SessionResourceHandle for RecordingHandle {
        fn close(&mut self) -> Result<()> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("close:{}", self.name));
            Ok(())
        }
    }

    #[derive(Default)]
    struct FakeSessionMessageTransport {
        sent: Vec<String>,
        pull_requests: Vec<(String, String)>,
        push_requests: Vec<String>,
        pulled: Vec<ChatMessage>,
        pushes: Vec<ChatMessage>,
        next_seq: i64,
    }

    impl MessageSender for FakeSessionMessageTransport {
        fn send_message(&mut self, message: &ChatMessage) -> Result<SendMessageAck> {
            self.sent.push(message.client_msg_id.clone());
            let seq = self.next_seq;
            self.next_seq += 1;
            Ok(SendMessageAck {
                server_msg_id: format!("server-{}", message.client_msg_id),
                seq,
                send_time: message.send_time + 1000,
            })
        }
    }

    impl SessionMessageTransport for FakeSessionMessageTransport {
        fn pull_messages(
            &mut self,
            owner_user_id: &str,
            conversation_id: &str,
        ) -> Result<Vec<ChatMessage>> {
            self.pull_requests
                .push((owner_user_id.to_string(), conversation_id.to_string()));
            let messages = std::mem::take(&mut self.pulled)
                .into_iter()
                .filter(|message| message.conversation_id == conversation_id)
                .collect();
            Ok(messages)
        }

        fn pop_push_messages(&mut self, owner_user_id: &str) -> Result<Vec<ChatMessage>> {
            self.push_requests.push(owner_user_id.to_string());
            Ok(std::mem::take(&mut self.pushes))
        }
    }
}
