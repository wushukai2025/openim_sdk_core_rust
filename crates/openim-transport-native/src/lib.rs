use std::collections::VecDeque;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use futures_util::{SinkExt, StreamExt};
use openim_protocol::{GeneralWsReq, GeneralWsResp};
use openim_transport_core::{
    build_get_newest_seq_request, decode_response_payload, encode_request_payload,
    ensure_connect_ack_from_bytes, ensure_connect_ack_from_text, heartbeat_ping_text,
    route_envelope, PendingRequests, ReconnectPolicy, TextHeartbeatFrame, TransportConfig,
    TransportEvent,
};
use tokio::net::TcpStream;
use tokio::time::{sleep, timeout};
use tokio_tungstenite::{
    connect_async, tungstenite::Message as WsMessage, MaybeTlsStream, WebSocketStream,
};

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

pub type ClientConfig = TransportConfig;
pub type OpenImWsClient = NativeWsClient;

pub struct NativeWsClient {
    config: TransportConfig,
    stream: WsStream,
    pending: PendingRequests,
    buffered_events: VecDeque<TransportEvent>,
    started_at: Instant,
}

impl NativeWsClient {
    pub async fn connect(config: TransportConfig) -> Result<Self> {
        let mut stream = connect_stream(&config).await?;

        if config.send_response {
            read_initial_response(&mut stream).await?;
        }

        Ok(Self {
            config,
            stream,
            pending: PendingRequests::default(),
            buffered_events: VecDeque::new(),
            started_at: Instant::now(),
        })
    }

    pub async fn connect_with_retries(
        config: TransportConfig,
        policy: ReconnectPolicy,
    ) -> Result<Self> {
        let mut attempt = 1;
        let mut last_error = None;

        loop {
            if policy.max_attempts > 0 && attempt > policy.max_attempts {
                let err = last_error
                    .take()
                    .unwrap_or_else(|| anyhow!("websocket reconnect attempts exhausted"));
                return Err(err);
            }

            match Self::connect(config.clone()).await {
                Ok(client) => return Ok(client),
                Err(err) => {
                    last_error = Some(err);
                    if let Some(delay) = policy.delay_for_attempt(attempt) {
                        sleep(delay).await;
                    }
                    attempt += 1;
                }
            }
        }
    }

    pub fn config(&self) -> &TransportConfig {
        &self.config
    }

    pub fn pending_requests(&self) -> &PendingRequests {
        &self.pending
    }

    pub async fn reconnect(&mut self, policy: ReconnectPolicy) -> Result<()> {
        let mut attempt = 1;
        let mut last_error = None;

        loop {
            if policy.max_attempts > 0 && attempt > policy.max_attempts {
                let err = last_error
                    .take()
                    .unwrap_or_else(|| anyhow!("websocket reconnect attempts exhausted"));
                return Err(err);
            }

            match connect_stream(&self.config).await {
                Ok(mut stream) => {
                    if self.config.send_response {
                        read_initial_response(&mut stream).await?;
                    }
                    self.stream = stream;
                    self.pending.clear();
                    self.buffered_events.clear();
                    self.started_at = Instant::now();
                    return Ok(());
                }
                Err(err) => {
                    last_error = Some(err);
                    if let Some(delay) = policy.delay_for_attempt(attempt) {
                        sleep(delay).await;
                    }
                    attempt += 1;
                }
            }
        }
    }

    pub async fn send_get_newest_seq(&mut self) -> Result<String> {
        let (envelope, msg_incr) = build_get_newest_seq_request(&self.config)?;
        self.send_request(&envelope).await?;
        Ok(msg_incr)
    }

    pub async fn send_request(&mut self, req: &GeneralWsReq) -> Result<()> {
        let payload = encode_request_payload(req, self.config.compression)?;
        self.stream.send(WsMessage::Binary(payload.into())).await?;
        self.pending
            .register_at(req.msg_incr.clone(), self.started_at.elapsed());
        Ok(())
    }

    pub async fn send_request_wait_response(
        &mut self,
        req: &GeneralWsReq,
        duration: Duration,
    ) -> Result<GeneralWsResp> {
        let msg_incr = req.msg_incr.clone();
        self.send_request(req).await?;

        let started = Instant::now();
        loop {
            let elapsed = started.elapsed();
            if elapsed >= duration {
                self.pending.resolve(&msg_incr);
                return Err(anyhow!(
                    "websocket request {} timed out after {}ms",
                    msg_incr,
                    duration.as_millis()
                ));
            }

            let remaining = duration.saturating_sub(elapsed);
            let event = match timeout(remaining, self.recv_event_from_stream()).await {
                Ok(result) => result?,
                Err(_) => {
                    self.pending.resolve(&msg_incr);
                    return Err(anyhow!(
                        "websocket request {} timed out after {}ms",
                        msg_incr,
                        duration.as_millis()
                    ));
                }
            };

            match event {
                TransportEvent::Response(resp) if resp.msg_incr == msg_incr => return Ok(resp),
                TransportEvent::Disconnected { reason } => {
                    return Err(anyhow!("websocket closed: {reason}"));
                }
                TransportEvent::Response(resp) => {
                    self.buffered_events
                        .push_back(TransportEvent::Response(resp));
                }
                TransportEvent::Push(resp) => {
                    self.buffered_events.push_back(TransportEvent::Push(resp));
                }
                _ => {}
            }
        }
    }

    pub async fn send_heartbeat_ping(&mut self) -> Result<()> {
        self.stream
            .send(WsMessage::Text(heartbeat_ping_text().into()))
            .await?;
        Ok(())
    }

    pub async fn recv_event(&mut self) -> Result<TransportEvent> {
        if let Some(event) = self.buffered_events.pop_front() {
            return Ok(event);
        }

        self.recv_event_from_stream().await
    }

    async fn recv_event_from_stream(&mut self) -> Result<TransportEvent> {
        loop {
            let Some(frame) = self.stream.next().await else {
                return Ok(TransportEvent::Disconnected {
                    reason: "websocket closed".to_string(),
                });
            };

            match frame? {
                WsMessage::Binary(data) => {
                    let resp = decode_response_payload(data.as_ref(), self.config.compression)?;
                    return Ok(route_envelope(resp, &mut self.pending));
                }
                WsMessage::Text(text) => {
                    let Some(frame) = openim_transport_core::text_heartbeat_frame(text.as_ref())?
                    else {
                        return Err(anyhow!("unexpected websocket text frame"));
                    };
                    match frame {
                        TextHeartbeatFrame::Ping { pong } => {
                            self.stream.send(WsMessage::Text(pong.into())).await?;
                            return Ok(TransportEvent::HeartbeatPing);
                        }
                        TextHeartbeatFrame::Pong => return Ok(TransportEvent::HeartbeatPong),
                    }
                }
                WsMessage::Ping(data) => {
                    self.stream.send(WsMessage::Pong(data)).await?;
                    return Ok(TransportEvent::HeartbeatPing);
                }
                WsMessage::Pong(_) => return Ok(TransportEvent::HeartbeatPong),
                WsMessage::Close(frame) => {
                    return Ok(TransportEvent::Disconnected {
                        reason: format!("{frame:?}"),
                    });
                }
                _ => {}
            }
        }
    }

    pub async fn recv_event_with_timeout(&mut self, duration: Duration) -> Result<TransportEvent> {
        match timeout(duration, self.recv_event()).await {
            Ok(result) => result,
            Err(_) => {
                let expired = self.pending.expire_at(self.started_at.elapsed(), duration);
                if let Some(msg_incr) = expired.into_iter().next() {
                    Ok(TransportEvent::RequestTimeout { msg_incr })
                } else {
                    Err(anyhow!(
                        "websocket receive timed out after {}ms",
                        duration.as_millis()
                    ))
                }
            }
        }
    }

    pub async fn recv_envelope(&mut self) -> Result<GeneralWsResp> {
        loop {
            match self.recv_event().await? {
                TransportEvent::Response(resp) | TransportEvent::Push(resp) => return Ok(resp),
                TransportEvent::Disconnected { reason } => {
                    return Err(anyhow!("websocket closed: {reason}"));
                }
                _ => {}
            }
        }
    }
}

async fn connect_stream(config: &TransportConfig) -> Result<WsStream> {
    let url = config.connect_url()?;
    let (stream, _) = connect_async(url.as_str())
        .await
        .with_context(|| format!("websocket connect failed: {url}"))?;
    Ok(stream)
}

async fn read_initial_response(stream: &mut WsStream) -> Result<()> {
    loop {
        let Some(frame) = stream.next().await else {
            return Err(anyhow!("websocket closed before initial response"));
        };

        match frame? {
            WsMessage::Text(text) => {
                let Some(frame) = openim_transport_core::text_heartbeat_frame(text.as_ref())?
                else {
                    return ensure_connect_ack_from_text(text.as_ref());
                };
                if let TextHeartbeatFrame::Ping { pong } = frame {
                    stream.send(WsMessage::Text(pong.into())).await?;
                }
            }
            WsMessage::Binary(data) => return ensure_connect_ack_from_bytes(data.as_ref()),
            other => return Err(anyhow!("unexpected initial websocket frame: {other:?}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::{SinkExt, StreamExt};
    use openim_protocol::{GeneralWsReq, GeneralWsResp, WsReqIdentifier};
    use tokio::net::{TcpListener, TcpStream};
    use tokio_tungstenite::accept_async;

    #[tokio::test]
    async fn native_client_routes_response_push_and_text_heartbeat() -> Result<()> {
        let ws_addr = spawn_echo_server(true).await?;
        let mut client = NativeWsClient::connect(test_config(ws_addr)).await?;

        client.send_heartbeat_ping().await?;
        assert!(matches!(
            client.recv_event().await?,
            TransportEvent::HeartbeatPong
        ));

        let req = GeneralWsReq::new(
            WsReqIdentifier::GetNewestSeq,
            "u1",
            "op1",
            "msg-1",
            Vec::new(),
        );
        client.send_request(&req).await?;
        assert!(client.pending_requests().contains("msg-1"));

        match client.recv_event().await? {
            TransportEvent::Response(resp) => {
                assert_eq!(resp.msg_incr, "msg-1");
                assert_eq!(resp.req_identifier, WsReqIdentifier::GetNewestSeq.as_i32());
            }
            other => panic!("expected correlated response, got {other:?}"),
        }
        assert!(client.pending_requests().is_empty());

        match client.recv_event().await? {
            TransportEvent::Push(resp) => {
                assert_eq!(resp.req_identifier, WsReqIdentifier::PushMsg.as_i32());
            }
            other => panic!("expected push event, got {other:?}"),
        }

        Ok(())
    }

    #[tokio::test]
    async fn native_client_reconnects_after_disconnect() -> Result<()> {
        let ws_addr = spawn_reconnect_server().await?;
        let mut client = NativeWsClient::connect(test_config(ws_addr)).await?;

        assert!(matches!(
            client.recv_event().await?,
            TransportEvent::Disconnected { .. }
        ));

        client
            .reconnect(ReconnectPolicy {
                max_attempts: 2,
                initial_delay: Duration::from_millis(1),
                max_delay: Duration::from_millis(1),
            })
            .await?;

        let req = GeneralWsReq::new(
            WsReqIdentifier::GetNewestSeq,
            "u1",
            "op1",
            "msg-after-reconnect",
            Vec::new(),
        );
        client.send_request(&req).await?;

        match client.recv_event().await? {
            TransportEvent::Response(resp) => {
                assert_eq!(resp.msg_incr, "msg-after-reconnect");
            }
            other => panic!("expected response after reconnect, got {other:?}"),
        }

        Ok(())
    }

    #[tokio::test]
    async fn native_client_can_wait_for_matching_response() -> Result<()> {
        let ws_addr = spawn_echo_server(true).await?;
        let mut client = NativeWsClient::connect(test_config(ws_addr)).await?;
        let req = GeneralWsReq::new(
            WsReqIdentifier::SendMsg,
            "u1",
            "op1",
            "msg-wait",
            Vec::new(),
        );

        let resp = client
            .send_request_wait_response(&req, Duration::from_secs(1))
            .await?;

        assert_eq!(resp.msg_incr, "msg-wait");
        assert_eq!(resp.req_identifier, WsReqIdentifier::SendMsg.as_i32());
        match client.recv_event().await? {
            TransportEvent::Push(resp) => {
                assert_eq!(resp.req_identifier, WsReqIdentifier::PushMsg.as_i32());
            }
            other => panic!("expected buffered or next push event, got {other:?}"),
        }

        Ok(())
    }

    async fn spawn_echo_server(send_push: bool) -> Result<String> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            serve_echo_connection(stream, send_push).await.unwrap();
        });
        Ok(format!("ws://{addr}/msg_gateway"))
    }

    async fn spawn_reconnect_server() -> Result<String> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        tokio::spawn(async move {
            let (first, _) = listener.accept().await.unwrap();
            let mut first = accept_async(first).await.unwrap();
            first
                .send(WsMessage::Text(r#"{"errCode":0}"#.into()))
                .await
                .unwrap();
            first.close(None).await.unwrap();

            let (second, _) = listener.accept().await.unwrap();
            serve_echo_connection(second, false).await.unwrap();
        });
        Ok(format!("ws://{addr}/msg_gateway"))
    }

    async fn serve_echo_connection(stream: TcpStream, send_push: bool) -> Result<()> {
        let mut ws = accept_async(stream).await?;
        ws.send(WsMessage::Text(r#"{"errCode":0}"#.into())).await?;

        while let Some(frame) = ws.next().await {
            match frame? {
                WsMessage::Binary(data) => {
                    let req: GeneralWsReq = serde_json::from_slice(data.as_ref())?;
                    let resp = GeneralWsResp {
                        req_identifier: req.req_identifier,
                        err_code: 0,
                        err_msg: String::new(),
                        msg_incr: req.msg_incr,
                        operation_id: req.operation_id,
                        data: Vec::new(),
                    };
                    ws.send(WsMessage::Binary(serde_json::to_vec(&resp)?.into()))
                        .await?;

                    if send_push {
                        let push = GeneralWsResp {
                            req_identifier: WsReqIdentifier::PushMsg.as_i32(),
                            err_code: 0,
                            err_msg: String::new(),
                            msg_incr: String::new(),
                            operation_id: "push-op".to_string(),
                            data: vec![1],
                        };
                        ws.send(WsMessage::Binary(serde_json::to_vec(&push)?.into()))
                            .await?;
                    }
                }
                WsMessage::Text(text) => {
                    if matches!(
                        openim_transport_core::text_heartbeat_frame(text.as_ref())?,
                        Some(TextHeartbeatFrame::Ping { .. })
                    ) {
                        ws.send(WsMessage::Text(r#"{"type":"pong"}"#.into()))
                            .await?;
                    }
                }
                WsMessage::Close(_) => break,
                _ => {}
            }
        }

        Ok(())
    }

    fn test_config(ws_addr: String) -> TransportConfig {
        let mut config = TransportConfig::new(ws_addr, "u1", "token", 5);
        config.operation_id = "op1".to_string();
        config.compression = false;
        config
    }
}
