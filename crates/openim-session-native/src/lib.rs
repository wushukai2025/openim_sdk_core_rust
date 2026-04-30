#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::fs;
    use std::path::Path;

    use openim_errors::{ErrorCode, OpenImError, Result};
    use openim_session::{
        LoginCredentials, SessionConfig, SessionResource, SessionResourceAdapter,
        SessionResourceHandle, SessionResourceKind, SessionRuntimeResources, StorageTarget,
    };
    use openim_storage_core::StorageMigrator;
    use openim_storage_sqlite::SqliteStorage;
    use openim_transport_core::TransportConfig;
    use openim_transport_native::NativeWsClient;
    use tokio::runtime::{Builder as TokioRuntimeBuilder, Runtime};

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

            let client = open_native_transport_client(transport)?;
            resources.add_resource(SessionResource::new(
                SessionResourceKind::Transport,
                format!("native-websocket:{}", transport.ws_addr),
                NativeTransportTaskResource {
                    client: Some(client.client),
                    runtime: Some(client.runtime),
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

    struct NativeTransportClient {
        runtime: Runtime,
        client: NativeWsClient,
    }

    fn open_native_transport_client(transport: &TransportConfig) -> Result<NativeTransportClient> {
        let runtime = TokioRuntimeBuilder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|err| {
                OpenImError::sdk_internal(format!("create native transport runtime failed: {err}"))
            })?;
        let client = runtime
            .block_on(NativeWsClient::connect(transport.clone()))
            .map_err(|err| {
                OpenImError::new(
                    ErrorCode::NETWORK,
                    format!("native websocket connect failed: {err}"),
                )
            })?;
        Ok(NativeTransportClient { runtime, client })
    }

    struct NativeTransportTaskResource {
        client: Option<NativeWsClient>,
        runtime: Option<Runtime>,
    }

    impl SessionResourceHandle for NativeTransportTaskResource {
        fn close(&mut self) -> Result<()> {
            let Some(runtime) = self.runtime.take() else {
                return Ok(());
            };
            let Some(mut client) = self.client.take() else {
                return Ok(());
            };
            runtime.block_on(client.close()).map_err(|err| {
                OpenImError::sdk_internal(format!("close native websocket failed: {err}"))
            })
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
    use std::sync::mpsc;
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    use futures_util::{SinkExt, StreamExt};
    use openim_session::{
        LoginCredentials, OpenImSession, SessionResourceInfo, SessionResourceKind, SessionState,
    };
    use openim_types::Platform;
    use tokio::net::TcpListener;
    use tokio_tungstenite::{accept_async, tungstenite::Message as WsMessage};

    use super::NativeSessionResourceAdapter;

    #[test]
    fn native_adapter_opens_sqlite_storage_and_closes_resources_on_logout() {
        let data_dir = unique_data_dir("logout");
        let db_path = data_dir.join("OpenIM_v3_u1.db");
        let ws_addr = spawn_transport_server();
        let config = native_config(&data_dir, &ws_addr);
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
                    name: format!("native-websocket:{ws_addr}"),
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
        let ws_addr = spawn_transport_server();
        let config = native_config(&data_dir, &ws_addr);
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

    fn native_config(data_dir: &std::path::Path, ws_addr: &str) -> openim_session::SessionConfig {
        openim_session::SessionConfig::new(Platform::Macos, "https://api.openim.test", ws_addr)
            .with_data_dir(data_dir.display().to_string())
    }

    fn spawn_transport_server() -> String {
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async move {
                let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
                let addr = listener.local_addr().unwrap();
                tx.send(format!("ws://{addr}/msg_gateway")).unwrap();
                let (stream, _) = listener.accept().await.unwrap();
                let mut ws = accept_async(stream).await.unwrap();
                ws.send(WsMessage::Text(r#"{"errCode":0}"#.into()))
                    .await
                    .unwrap();

                while let Some(frame) = ws.next().await {
                    match frame.unwrap() {
                        WsMessage::Text(text) if text.contains(r#""type":"ping""#) => {
                            ws.send(WsMessage::Text(r#"{"type":"pong"}"#.into()))
                                .await
                                .unwrap();
                        }
                        WsMessage::Close(_) => break,
                        _ => {}
                    }
                }
            });
        });
        rx.recv().unwrap()
    }

    fn unique_data_dir(tag: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("openim-session-native-{tag}-{nanos}"))
    }
}
