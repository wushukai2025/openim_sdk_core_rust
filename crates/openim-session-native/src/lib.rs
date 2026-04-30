#[cfg(not(target_arch = "wasm32"))]
mod native {
    use std::fs;
    use std::path::Path;
    use std::sync::mpsc::{self, Receiver, Sender};
    use std::thread;
    use std::time::Duration;

    use openim_errors::{ErrorCode, OpenImError, Result};
    use openim_session::{
        LoginCredentials, SessionConfig, SessionResource, SessionResourceAdapter,
        SessionResourceHandle, SessionResourceKind, SessionRuntimeResources, StorageTarget,
    };
    use openim_storage_core::StorageMigrator;
    use openim_storage_sqlite::SqliteStorage;
    use openim_transport_core::{ReconnectPolicy, TransportConfig, TransportEvent};
    use openim_transport_native::NativeWsClient;
    use reqwest::blocking::Client as HttpClient;
    use serde::{Deserialize, Serialize};
    use tokio::runtime::Builder as TokioRuntimeBuilder;

    const PARSE_TOKEN_ROUTE: &str = "/auth/parse_token";
    const TRANSPORT_WORKER_POLL_INTERVAL: Duration = Duration::from_millis(200);

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
            config: &SessionConfig,
            credentials: &LoginCredentials,
            transport: &TransportConfig,
            storage: &StorageTarget,
        ) -> Result<SessionRuntimeResources> {
            validate_http_login(config, credentials, transport)?;
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

            resources.add_resource(SessionResource::new(
                SessionResourceKind::Transport,
                format!("native-websocket:{}", transport.ws_addr),
                start_native_transport_task(transport)?,
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

    fn validate_http_login(
        config: &SessionConfig,
        credentials: &LoginCredentials,
        transport: &TransportConfig,
    ) -> Result<()> {
        let api_addr = config.api_addr.trim_end_matches('/');
        let url = format!("{api_addr}{PARSE_TOKEN_ROUTE}");
        let client = HttpClient::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|err| {
                OpenImError::new(
                    ErrorCode::NETWORK,
                    format!("build login validation client failed: {err}"),
                )
            })?;
        let response = client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("operationID", transport.operation_id.as_str())
            .header("token", credentials.token.as_str())
            .json(&ParseTokenRequest {
                token: credentials.token.as_str(),
            })
            .send()
            .map_err(|err| {
                OpenImError::new(
                    ErrorCode::NETWORK,
                    format!("login validation request failed: {err}"),
                )
            })?;
        let status = response.status();
        let body = response.bytes().map_err(|err| {
            OpenImError::sdk_internal(format!("read login validation response failed: {err}"))
        })?;
        let envelope: ApiEnvelope<ParseTokenResponse> =
            serde_json::from_slice(&body).map_err(|err| {
                OpenImError::sdk_internal(format!("decode login validation response failed: {err}"))
            })?;

        if envelope.err_code != 0 {
            let mut err = OpenImError::new(ErrorCode::new(envelope.err_code), envelope.err_msg);
            if !envelope.err_detail.is_empty() {
                err = err.with_detail(envelope.err_detail);
            }
            return Err(err);
        }

        if !status.is_success() {
            return Err(OpenImError::new(
                ErrorCode::NETWORK,
                format!("login validation returned unexpected status {status}"),
            ));
        }

        let Some(data) = envelope.data else {
            return Err(OpenImError::sdk_internal(
                "login validation response data is missing",
            ));
        };
        if data.user_id != credentials.user_id {
            return Err(OpenImError::args(format!(
                "parse_token response user_id {} does not match login user {}",
                data.user_id, credentials.user_id
            )));
        }
        if data.platform_id != transport.platform_id {
            return Err(OpenImError::args(format!(
                "parse_token response platform_id {} does not match login platform {}",
                data.platform_id, transport.platform_id
            )));
        }
        Ok(())
    }

    #[derive(Serialize)]
    struct ParseTokenRequest<'a> {
        token: &'a str,
    }

    #[derive(Deserialize)]
    struct ApiEnvelope<T> {
        #[serde(rename = "errCode")]
        err_code: i32,
        #[serde(rename = "errMsg")]
        err_msg: String,
        #[serde(rename = "errDlt", default)]
        err_detail: String,
        data: Option<T>,
    }

    #[derive(Deserialize)]
    struct ParseTokenResponse {
        #[serde(rename = "userID")]
        user_id: String,
        #[serde(rename = "platformID")]
        platform_id: i32,
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
        shutdown_tx: Option<Sender<()>>,
        event_rx: Receiver<TransportEvent>,
        join_handle: Option<thread::JoinHandle<Result<()>>>,
    }

    impl SessionResourceHandle for NativeTransportTaskResource {
        fn close(&mut self) -> Result<()> {
            if let Some(shutdown_tx) = self.shutdown_tx.take() {
                let _ = shutdown_tx.send(());
            }
            let Some(join_handle) = self.join_handle.take() else {
                return Ok(());
            };
            match join_handle.join() {
                Ok(result) => result,
                Err(_) => Err(OpenImError::sdk_internal(
                    "native transport worker thread panicked",
                )),
            }
        }

        fn drain_transport_events(&mut self) -> Result<Vec<TransportEvent>> {
            let mut events = Vec::new();
            while let Ok(event) = self.event_rx.try_recv() {
                events.push(event);
            }
            Ok(events)
        }
    }

    fn start_native_transport_task(
        transport: &TransportConfig,
    ) -> Result<NativeTransportTaskResource> {
        let (ready_tx, ready_rx) = mpsc::channel::<Result<()>>();
        let (shutdown_tx, shutdown_rx) = mpsc::channel::<()>();
        let (event_tx, event_rx) = mpsc::channel::<TransportEvent>();
        let transport = transport.clone();
        let join_handle = thread::spawn(move || {
            run_native_transport_worker(transport, ready_tx, shutdown_rx, event_tx)
        });

        match ready_rx.recv() {
            Ok(Ok(())) => Ok(NativeTransportTaskResource {
                shutdown_tx: Some(shutdown_tx),
                event_rx,
                join_handle: Some(join_handle),
            }),
            Ok(Err(err)) => {
                let _ = join_handle.join();
                Err(err)
            }
            Err(err) => {
                let _ = join_handle.join();
                Err(OpenImError::sdk_internal(format!(
                    "receive native transport worker startup signal failed: {err}"
                )))
            }
        }
    }

    fn run_native_transport_worker(
        transport: TransportConfig,
        ready_tx: Sender<Result<()>>,
        shutdown_rx: Receiver<()>,
        event_tx: Sender<TransportEvent>,
    ) -> Result<()> {
        let runtime = TokioRuntimeBuilder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|err| {
                OpenImError::sdk_internal(format!("create native transport runtime failed: {err}"))
            })?;
        runtime.block_on(async move {
            let mut client = match NativeWsClient::connect(transport.clone()).await {
                Ok(client) => {
                    let _ = ready_tx.send(Ok(()));
                    client
                }
                Err(err) => {
                    let msg = format!("native websocket connect failed: {err}");
                    let error = OpenImError::new(ErrorCode::NETWORK, msg);
                    let _ = ready_tx.send(Err(error.clone()));
                    return Err(error);
                }
            };

            loop {
                if shutdown_requested(&shutdown_rx) {
                    return close_native_transport_client(&mut client).await;
                }

                match tokio::time::timeout(TRANSPORT_WORKER_POLL_INTERVAL, client.recv_event())
                    .await
                {
                    Ok(Ok(event @ TransportEvent::Disconnected { .. })) => {
                        let _ = event_tx.send(event);
                        client
                            .reconnect(ReconnectPolicy::default())
                            .await
                            .map_err(|err| {
                                OpenImError::new(
                                    ErrorCode::NETWORK,
                                    format!("native websocket reconnect failed: {err}"),
                                )
                            })?;
                    }
                    Ok(Ok(event)) => {
                        if !matches!(
                            event,
                            TransportEvent::HeartbeatPing | TransportEvent::HeartbeatPong
                        ) {
                            let _ = event_tx.send(event);
                        }
                    }
                    Ok(Err(err)) => {
                        return Err(OpenImError::sdk_internal(format!(
                            "native transport worker recv failed: {err}"
                        )));
                    }
                    Err(_) => {}
                }
            }
        })
    }

    fn shutdown_requested(shutdown_rx: &Receiver<()>) -> bool {
        shutdown_rx.try_recv().is_ok()
    }

    async fn close_native_transport_client(client: &mut NativeWsClient) -> Result<()> {
        client.close().await.map_err(|err| {
            OpenImError::sdk_internal(format!("close native websocket failed: {err}"))
        })
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
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener as StdTcpListener;
    use std::sync::mpsc;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use futures_util::{SinkExt, StreamExt};
    use openim_protocol::{pb_sdkws, GeneralWsResp, WsReqIdentifier};
    use openim_session::{
        LoginCredentials, OpenImSession, SessionResourceInfo, SessionResourceKind, SessionState,
    };
    use openim_types::Platform;
    use prost::Message;
    use tokio::net::TcpListener;
    use tokio_tungstenite::{accept_async, tungstenite::Message as WsMessage};

    use super::NativeSessionResourceAdapter;

    #[test]
    fn native_adapter_opens_sqlite_storage_and_closes_resources_on_logout() {
        let data_dir = unique_data_dir("logout");
        let db_path = data_dir.join("OpenIM_v3_u1.db");
        let api_addr = spawn_parse_token_server("token", "u1", Platform::Macos.as_i32(), 0);
        let ws_addr = spawn_transport_server();
        let config = native_config(&data_dir, &api_addr, &ws_addr);
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
        let api_addr = spawn_parse_token_server("token", "u1", Platform::Macos.as_i32(), 0);
        let ws_addr = spawn_transport_server();
        let config = native_config(&data_dir, &api_addr, &ws_addr);
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

    #[test]
    fn native_adapter_rejects_parse_token_user_mismatch() {
        let data_dir = unique_data_dir("mismatch");
        let api_addr =
            spawn_parse_token_server("token", "unexpected-user", Platform::Macos.as_i32(), 0);
        let config = native_config(&data_dir, &api_addr, "ws://127.0.0.1:9/msg_gateway");
        let mut session =
            OpenImSession::with_resource_adapter(config, Box::new(NativeSessionResourceAdapter))
                .unwrap();

        session.init().unwrap();
        let err = session
            .login(LoginCredentials::new("u1", "token"))
            .unwrap_err();

        assert!(err
            .message()
            .contains("parse_token response user_id unexpected-user"));
        assert!(session.runtime_resources().is_none());
        let _ = fs::remove_dir_all(data_dir);
    }

    #[test]
    fn native_adapter_reconnects_transport_after_disconnect() {
        let data_dir = unique_data_dir("reconnect");
        let api_addr = spawn_parse_token_server("token", "u1", Platform::Macos.as_i32(), 0);
        let (ws_addr, accepted_count) = spawn_reconnecting_transport_server();
        let config = native_config(&data_dir, &api_addr, &ws_addr);
        let mut session =
            OpenImSession::with_resource_adapter(config, Box::new(NativeSessionResourceAdapter))
                .unwrap();

        session.init().unwrap();
        session.login(LoginCredentials::new("u1", "token")).unwrap();

        wait_for_counter(&accepted_count, 2);
        session.logout().unwrap();

        assert!(session.runtime_resources().is_none());
        let _ = fs::remove_dir_all(data_dir);
    }

    #[test]
    fn native_adapter_pumps_transport_push_into_session_events() {
        let data_dir = unique_data_dir("push");
        let api_addr = spawn_parse_token_server("token", "u1", Platform::Macos.as_i32(), 0);
        let ws_addr = spawn_push_transport_server();
        let config = native_config(&data_dir, &api_addr, &ws_addr);
        let mut session =
            OpenImSession::with_resource_adapter(config, Box::new(NativeSessionResourceAdapter))
                .unwrap();

        session.init().unwrap();
        session.login(LoginCredentials::new("u1", "token")).unwrap();

        let mut pushed = Vec::new();
        for _ in 0..50 {
            pushed = session.pump_transport_events().unwrap();
            if !pushed.is_empty() {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }

        assert_eq!(pushed.len(), 1);
        assert_eq!(pushed[0].client_msg_id, "push-1");
        assert_eq!(pushed[0].content.summary(), "native push");
        let conversation = session
            .domains()
            .conversations
            .get_conversation("u1", "si_u1_u2")
            .unwrap()
            .unwrap();
        assert_eq!(conversation.unread_count, 1);

        session.logout().unwrap();
        let _ = fs::remove_dir_all(data_dir);
    }

    fn native_config(
        data_dir: &std::path::Path,
        api_addr: &str,
        ws_addr: &str,
    ) -> openim_session::SessionConfig {
        openim_session::SessionConfig::new(Platform::Macos, api_addr, ws_addr)
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

    fn spawn_reconnecting_transport_server() -> (String, Arc<AtomicUsize>) {
        let (tx, rx) = mpsc::channel();
        let accepted_count = Arc::new(AtomicUsize::new(0));
        let accepted_for_thread = accepted_count.clone();
        thread::spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async move {
                let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
                let addr = listener.local_addr().unwrap();
                tx.send(format!("ws://{addr}/msg_gateway")).unwrap();

                let (first, _) = listener.accept().await.unwrap();
                accepted_for_thread.fetch_add(1, Ordering::SeqCst);
                let mut first = accept_async(first).await.unwrap();
                first
                    .send(WsMessage::Text(r#"{"errCode":0}"#.into()))
                    .await
                    .unwrap();
                first.close(None).await.unwrap();

                let (second, _) = listener.accept().await.unwrap();
                accepted_for_thread.fetch_add(1, Ordering::SeqCst);
                let mut second = accept_async(second).await.unwrap();
                second
                    .send(WsMessage::Text(r#"{"errCode":0}"#.into()))
                    .await
                    .unwrap();

                while let Some(frame) = second.next().await {
                    match frame.unwrap() {
                        WsMessage::Text(text) if text.contains(r#""type":"ping""#) => {
                            second
                                .send(WsMessage::Text(r#"{"type":"pong"}"#.into()))
                                .await
                                .unwrap();
                        }
                        WsMessage::Close(_) => break,
                        _ => {}
                    }
                }
            });
        });
        (rx.recv().unwrap(), accepted_count)
    }

    fn spawn_push_transport_server() -> String {
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
                ws.send(WsMessage::Binary(encode_push_frame().into()))
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

    fn spawn_parse_token_server(
        expected_token: &str,
        response_user_id: &str,
        response_platform_id: i32,
        err_code: i32,
    ) -> String {
        let expected_token = expected_token.to_string();
        let response_user_id = response_user_id.to_string();
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let listener = StdTcpListener::bind("127.0.0.1:0").unwrap();
            let addr = listener.local_addr().unwrap();
            tx.send(format!("http://{addr}")).unwrap();
            let (mut stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut request_line = String::new();
            reader.read_line(&mut request_line).unwrap();
            assert!(request_line.starts_with("POST /auth/parse_token "));

            let mut content_length = 0_usize;
            let mut token_header = String::new();
            let mut operation_id = String::new();
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                if line == "\r\n" {
                    break;
                }
                let lower = line.to_ascii_lowercase();
                if lower.starts_with("content-length:") {
                    content_length = line.split_once(':').unwrap().1.trim().parse().unwrap();
                } else if lower.starts_with("token:") {
                    token_header = line.split_once(':').unwrap().1.trim().to_string();
                } else if lower.starts_with("operationid:") {
                    operation_id = line.split_once(':').unwrap().1.trim().to_string();
                }
            }

            let mut body = vec![0_u8; content_length];
            reader.read_exact(&mut body).unwrap();
            let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(payload["token"], expected_token);
            assert_eq!(token_header, expected_token);
            assert!(!operation_id.is_empty());

            let response_body = if err_code == 0 {
                serde_json::json!({
                    "errCode": 0,
                    "errMsg": "",
                    "errDlt": "",
                    "data": {
                        "userID": response_user_id,
                        "platformID": response_platform_id,
                        "expireTimeSeconds": 123
                    }
                })
            } else {
                serde_json::json!({
                    "errCode": err_code,
                    "errMsg": "token invalid",
                    "errDlt": "",
                    "data": null
                })
            }
            .to_string();

            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            stream.write_all(response.as_bytes()).unwrap();
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

    fn encode_push_frame() -> Vec<u8> {
        let mut data = Vec::new();
        pb_sdkws::PushMessages {
            msgs: std::collections::HashMap::from([(
                "si_u1_u2".to_string(),
                pb_sdkws::PullMsgs {
                    msgs: vec![pb_sdkws::MsgData {
                        send_id: "u2".to_string(),
                        recv_id: "u1".to_string(),
                        client_msg_id: "push-1".to_string(),
                        server_msg_id: "server-push-1".to_string(),
                        session_type: 1,
                        content_type: 101,
                        content: br#"{"content":"native push"}"#.to_vec(),
                        seq: 6,
                        send_time: 60,
                        create_time: 50,
                        status: 2,
                        is_read: false,
                        ..Default::default()
                    }],
                    ..Default::default()
                },
            )]),
            ..Default::default()
        }
        .encode(&mut data)
        .unwrap();

        let payload = serde_json::to_vec(&GeneralWsResp {
            req_identifier: WsReqIdentifier::PushMsg.as_i32(),
            err_code: 0,
            err_msg: String::new(),
            msg_incr: String::new(),
            operation_id: "op1".to_string(),
            data,
        })
        .unwrap();
        openim_protocol::gzip_compress(&payload).unwrap()
    }

    fn wait_for_counter(counter: &Arc<AtomicUsize>, expected: usize) {
        for _ in 0..50 {
            if counter.load(Ordering::SeqCst) >= expected {
                return;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        panic!(
            "counter did not reach expected value: actual={}, expected={expected}",
            counter.load(Ordering::SeqCst)
        );
    }
}
