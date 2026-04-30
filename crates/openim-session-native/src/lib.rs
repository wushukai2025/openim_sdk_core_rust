#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::fs;
    use std::path::Path;

    use openim_errors::{OpenImError, Result};
    use openim_session::{
        LoginCredentials, SessionConfig, SessionResource, SessionResourceAdapter,
        SessionResourceHandle, SessionResourceKind, SessionRuntimeResources, StorageTarget,
    };
    use openim_storage_core::StorageMigrator;
    use openim_storage_sqlite::SqliteStorage;
    use openim_transport_core::TransportConfig;

    #[derive(Debug, Default)]
    pub struct NativeSessionResourceAdapter;

    impl NativeSessionResourceAdapter {
        pub fn new() -> Self {
            Self
        }
    }

    impl SessionResourceAdapter for NativeSessionResourceAdapter {
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

            match storage {
                StorageTarget::Sqlite { path } => {
                    let storage = open_sqlite_storage(path)?;
                    resources.add_resource(SessionResource::new(
                        SessionResourceKind::Storage,
                        format!("sqlite:{}", path.display()),
                        SqliteStorageResource {
                            storage: Some(storage),
                        },
                    )?);
                }
                StorageTarget::IndexedDb { .. } => {
                    return Err(OpenImError::args(
                        "indexeddb storage target requires a wasm resource adapter",
                    ));
                }
                StorageTarget::Unconfigured => {}
            }

            let connect_url = transport.connect_url().map_err(|err| {
                OpenImError::args(format!("invalid native transport config: {err}"))
            })?;
            resources.add_resource(SessionResource::new(
                SessionResourceKind::Transport,
                format!("native-websocket:{}", transport.ws_addr),
                NativeTransportTaskResource {
                    connect_url: connect_url.to_string(),
                },
            )?);
            resources.add_resource(SessionResource::new(
                SessionResourceKind::Sync,
                format!("sync:{}", credentials.user_id),
                NativeSyncTaskResource {
                    user_id: credentials.user_id.clone(),
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

    fn open_sqlite_storage(path: &Path) -> Result<SqliteStorage> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                OpenImError::sdk_internal(format!("create sqlite storage directory failed: {err}"))
            })?;
        }

        let storage = SqliteStorage::open(path)?;
        storage.migrate()?;
        Ok(storage)
    }

    struct SqliteStorageResource {
        storage: Option<SqliteStorage>,
    }

    impl SessionResourceHandle for SqliteStorageResource {
        fn close(&mut self) -> Result<()> {
            self.storage.take();
            Ok(())
        }
    }

    struct NativeTransportTaskResource {
        connect_url: String,
    }

    impl SessionResourceHandle for NativeTransportTaskResource {
        fn close(&mut self) -> Result<()> {
            ensure_not_empty(&self.connect_url, "connect_url")
        }
    }

    struct NativeSyncTaskResource {
        user_id: String,
    }

    impl SessionResourceHandle for NativeSyncTaskResource {
        fn close(&mut self) -> Result<()> {
            ensure_not_empty(&self.user_id, "user_id")
        }
    }

    fn ensure_not_empty(value: &str, field: &str) -> Result<()> {
        if value.is_empty() {
            Err(OpenImError::sdk_internal(format!("{field} is empty")))
        } else {
            Ok(())
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use native::NativeSessionResourceAdapter;

#[cfg(target_arch = "wasm32")]
mod unsupported {
    use openim_errors::{OpenImError, Result};
    use openim_session::{
        LoginCredentials, SessionConfig, SessionResourceAdapter, SessionRuntimeResources,
        StorageTarget,
    };
    use openim_transport_core::TransportConfig;

    #[derive(Debug, Default)]
    pub struct NativeSessionResourceAdapter;

    impl NativeSessionResourceAdapter {
        pub fn new() -> Self {
            Self
        }
    }

    impl SessionResourceAdapter for NativeSessionResourceAdapter {
        fn init(&mut self, _config: &SessionConfig) -> Result<()> {
            Err(unsupported())
        }

        fn login(
            &mut self,
            _config: &SessionConfig,
            _credentials: &LoginCredentials,
            _transport: &TransportConfig,
            _storage: &StorageTarget,
        ) -> Result<SessionRuntimeResources> {
            Err(unsupported())
        }

        fn logout(&mut self, _user_id: &str) -> Result<()> {
            Err(unsupported())
        }

        fn uninit(&mut self) -> Result<()> {
            Err(unsupported())
        }
    }

    fn unsupported() -> OpenImError {
        OpenImError::args("openim-session-native is only available on native targets")
    }
}

#[cfg(target_arch = "wasm32")]
pub use unsupported::NativeSessionResourceAdapter;

#[cfg(test)]
mod tests {
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use openim_session::{
        LoginCredentials, OpenImSession, SessionResourceInfo, SessionResourceKind, SessionState,
    };
    use openim_types::Platform;

    use super::NativeSessionResourceAdapter;

    #[test]
    fn native_adapter_opens_sqlite_storage_and_closes_resources_on_logout() {
        let data_dir = unique_data_dir("logout");
        let db_path = data_dir.join("OpenIM_v3_u1.db");
        let config = native_config(&data_dir);
        let mut session =
            OpenImSession::with_resource_adapter(config, Box::new(NativeSessionResourceAdapter))
                .unwrap();

        session.init().unwrap();
        session.login(LoginCredentials::new("u1", "token")).unwrap();

        assert!(db_path.exists());
        assert_eq!(
            session.runtime_resources().unwrap().resource_infos(),
            vec![
                SessionResourceInfo {
                    kind: SessionResourceKind::Storage,
                    name: format!("sqlite:{}", db_path.display()),
                },
                SessionResourceInfo {
                    kind: SessionResourceKind::Transport,
                    name: "native-websocket:wss://ws.openim.test/msg_gateway".to_string(),
                },
                SessionResourceInfo {
                    kind: SessionResourceKind::Sync,
                    name: "sync:u1".to_string(),
                },
            ]
        );

        session.logout().unwrap();

        assert_eq!(session.state(), SessionState::LoggedOut);
        assert!(session.runtime_resources().is_none());
        let _ = fs::remove_dir_all(data_dir);
    }

    #[test]
    fn native_adapter_closes_resources_on_uninit() {
        let data_dir = unique_data_dir("uninit");
        let config = native_config(&data_dir);
        let mut session =
            OpenImSession::with_resource_adapter(config, Box::new(NativeSessionResourceAdapter))
                .unwrap();

        session.init().unwrap();
        session.login(LoginCredentials::new("u1", "token")).unwrap();
        session.uninit().unwrap();

        assert_eq!(session.state(), SessionState::Uninitialized);
        assert!(session.runtime_resources().is_none());
        let _ = fs::remove_dir_all(data_dir);
    }

    fn native_config(data_dir: &std::path::Path) -> openim_session::SessionConfig {
        openim_session::SessionConfig::new(
            Platform::Macos,
            "https://api.openim.test",
            "wss://ws.openim.test/msg_gateway",
        )
        .with_data_dir(data_dir.display().to_string())
    }

    fn unique_data_dir(tag: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("openim-session-native-{tag}-{nanos}"))
    }
}
