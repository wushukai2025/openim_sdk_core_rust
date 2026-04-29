use std::collections::BTreeMap;

use openim_domain::{group::GroupService, relation::RelationService, user::UserService};
use openim_errors::{OpenImError, Result};
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionEvent {
    Initialized,
    LoggedIn { user_id: UserId },
    LoggedOut { user_id: UserId },
    Uninitialized,
    ListenerRegistered { listener_id: ListenerId },
    ListenerUnregistered { listener_id: ListenerId },
    TaskStarted { name: String },
    TaskStopped { name: String },
}

#[derive(Debug, Default)]
pub struct DomainServices {
    pub users: UserService,
    pub relations: RelationService,
    pub groups: GroupService,
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

pub struct OpenImSession {
    config: SessionConfig,
    state: SessionState,
    login_user_id: Option<UserId>,
    domains: DomainServices,
    listeners: ListenerRegistry,
    tasks: TaskSupervisor,
}

impl OpenImSession {
    pub fn new(config: SessionConfig) -> Result<Self> {
        config.validate()?;
        Ok(Self {
            config,
            state: SessionState::Created,
            login_user_id: None,
            domains: DomainServices::default(),
            listeners: ListenerRegistry::new(),
            tasks: TaskSupervisor::new(),
        })
    }

    pub fn init(&mut self) -> Result<()> {
        match self.state {
            SessionState::Created | SessionState::Uninitialized => {
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

        self.login_user_id = Some(credentials.user_id.clone());
        self.domains = DomainServices::default();
        self.state = SessionState::LoggedIn;
        self.start_task(TRANSPORT_TASK)?;
        self.start_task(SYNC_TASK)?;
        self.emit(SessionEvent::LoggedIn {
            user_id: credentials.user_id,
        });
        Ok(())
    }

    pub fn logout(&mut self) -> Result<()> {
        let Some(user_id) = self.login_user_id.clone() else {
            return Ok(());
        };

        self.stop_all_tasks();
        self.login_user_id = None;
        self.domains = DomainServices::default();
        self.state = SessionState::LoggedOut;
        self.emit(SessionEvent::LoggedOut { user_id });
        Ok(())
    }

    pub fn uninit(&mut self) -> Result<()> {
        self.stop_all_tasks();
        self.login_user_id = None;
        self.domains = DomainServices::default();
        self.state = SessionState::Uninitialized;
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
        if self.state != SessionState::LoggedIn {
            return Err(OpenImError::args("session is not logged in"));
        }
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

    fn stop_all_tasks(&mut self) {
        for name in self.tasks.stop_all() {
            self.emit(SessionEvent::TaskStopped { name });
        }
    }

    fn emit(&self, event: SessionEvent) {
        self.listeners.emit(&event);
    }
}

fn ensure_not_empty(value: &str, field: &str) -> Result<()> {
    if value.is_empty() {
        Err(OpenImError::args(format!("{field} is empty")))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use openim_domain::user::UserProfile;

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
}
