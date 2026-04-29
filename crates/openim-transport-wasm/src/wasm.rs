use std::time::Duration;

use anyhow::{anyhow, Result};
use futures_channel::mpsc;
use futures_util::StreamExt;
use js_sys::{ArrayBuffer, Uint8Array};
use openim_protocol::{GeneralWsReq, GeneralWsResp};
use openim_transport_core::{
    decode_response_payload, encode_request_payload, ensure_connect_ack_from_bytes,
    ensure_connect_ack_from_text, heartbeat_ping_text, route_envelope, PendingRequests,
    ReconnectPolicy, TextHeartbeatFrame, TransportConfig, TransportEvent,
};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{BinaryType, CloseEvent, ErrorEvent, MessageEvent, WebSocket};

pub type ClientConfig = TransportConfig;

enum RawFrame {
    Binary(Vec<u8>),
    Text(String),
    Closed(String),
}

pub struct WasmWsClient {
    config: TransportConfig,
    socket: WebSocket,
    pending: PendingRequests,
    frames: mpsc::UnboundedReceiver<Result<RawFrame>>,
    sent_clock: u64,
    _onmessage: Closure<dyn FnMut(MessageEvent)>,
    _onerror: Closure<dyn FnMut(ErrorEvent)>,
    _onclose: Closure<dyn FnMut(CloseEvent)>,
}

impl WasmWsClient {
    pub async fn connect(config: TransportConfig) -> Result<Self> {
        let (socket, frames_rx, onmessage, onerror, onclose) = open_socket(&config).await?;
        let mut client = Self {
            config,
            socket,
            pending: PendingRequests::default(),
            frames: frames_rx,
            sent_clock: 0,
            _onmessage: onmessage,
            _onerror: onerror,
            _onclose: onclose,
        };

        if client.config.send_response {
            client.read_initial_response().await?;
        }

        Ok(client)
    }

    pub fn config(&self) -> &TransportConfig {
        &self.config
    }

    pub async fn send_request(&mut self, req: &GeneralWsReq) -> Result<()> {
        let payload = encode_request_payload(req, self.config.compression)?;
        self.socket.send_with_u8_array(&payload).map_err(js_error)?;
        self.sent_clock = self.sent_clock.saturating_add(1);
        self.pending
            .register_at(req.msg_incr.clone(), Duration::from_millis(self.sent_clock));
        Ok(())
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

            match open_socket(&self.config).await {
                Ok((socket, frames, onmessage, onerror, onclose)) => {
                    self.socket = socket;
                    self.frames = frames;
                    self._onmessage = onmessage;
                    self._onerror = onerror;
                    self._onclose = onclose;
                    self.pending.clear();
                    self.sent_clock = 0;
                    if self.config.send_response {
                        self.read_initial_response().await?;
                    }
                    return Ok(());
                }
                Err(err) => {
                    last_error = Some(err);
                    if let Some(delay) = policy.delay_for_attempt(attempt) {
                        sleep(delay).await?;
                    }
                    attempt += 1;
                }
            }
        }
    }

    pub async fn send_heartbeat_ping(&mut self) -> Result<()> {
        self.socket
            .send_with_str(heartbeat_ping_text())
            .map_err(js_error)?;
        Ok(())
    }

    pub async fn recv_event(&mut self) -> Result<TransportEvent> {
        loop {
            match self.recv_raw().await? {
                RawFrame::Binary(data) => {
                    let resp = decode_response_payload(&data, self.config.compression)?;
                    return Ok(route_envelope(resp, &mut self.pending));
                }
                RawFrame::Text(text) => {
                    let Some(frame) = openim_transport_core::text_heartbeat_frame(&text)? else {
                        return Err(anyhow!("unexpected websocket text frame"));
                    };
                    match frame {
                        TextHeartbeatFrame::Ping { pong } => {
                            self.socket.send_with_str(&pong).map_err(js_error)?;
                            return Ok(TransportEvent::HeartbeatPing);
                        }
                        TextHeartbeatFrame::Pong => return Ok(TransportEvent::HeartbeatPong),
                    }
                }
                RawFrame::Closed(reason) => return Ok(TransportEvent::Disconnected { reason }),
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

    async fn read_initial_response(&mut self) -> Result<()> {
        loop {
            match self.recv_raw().await? {
                RawFrame::Binary(data) => return ensure_connect_ack_from_bytes(&data),
                RawFrame::Text(text) => {
                    let Some(frame) = openim_transport_core::text_heartbeat_frame(&text)? else {
                        return ensure_connect_ack_from_text(&text);
                    };
                    if let TextHeartbeatFrame::Ping { pong } = frame {
                        self.socket.send_with_str(&pong).map_err(js_error)?;
                    }
                }
                RawFrame::Closed(reason) => {
                    return Err(anyhow!(
                        "websocket closed before initial response: {reason}"
                    ));
                }
            }
        }
    }

    async fn recv_raw(&mut self) -> Result<RawFrame> {
        match self.frames.next().await {
            Some(result) => result,
            None => Err(anyhow!("websocket event channel closed")),
        }
    }
}

type SocketParts = (
    WebSocket,
    mpsc::UnboundedReceiver<Result<RawFrame>>,
    Closure<dyn FnMut(MessageEvent)>,
    Closure<dyn FnMut(ErrorEvent)>,
    Closure<dyn FnMut(CloseEvent)>,
);

async fn open_socket(config: &TransportConfig) -> Result<SocketParts> {
    let url = config.connect_url()?;
    let socket = WebSocket::new(url.as_str()).map_err(js_error)?;
    socket.set_binary_type(BinaryType::Arraybuffer);
    let (frames_tx, frames_rx) = mpsc::unbounded();
    let (onmessage, onerror, onclose) = install_handlers(&socket, frames_tx);
    wait_until_open(&socket).await?;

    Ok((socket, frames_rx, onmessage, onerror, onclose))
}

async fn wait_until_open(socket: &WebSocket) -> Result<()> {
    loop {
        match socket.ready_state() {
            WebSocket::OPEN => return Ok(()),
            WebSocket::CLOSING | WebSocket::CLOSED => {
                return Err(anyhow!("websocket closed before open"));
            }
            _ => sleep(Duration::from_millis(1)).await?,
        }
    }
}

async fn sleep(duration: Duration) -> Result<()> {
    let window = web_sys::window().ok_or_else(|| anyhow!("window is missing"))?;
    let promise = js_sys::Promise::new(&mut |resolve, _reject| {
        let callback = Closure::once_into_js(move || {
            let _ = resolve.call0(&JsValue::UNDEFINED);
        });
        let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(
            callback.as_ref().unchecked_ref(),
            duration.as_millis().min(i32::MAX as u128) as i32,
        );
    });

    JsFuture::from(promise).await.map_err(js_error)?;
    Ok(())
}

fn install_handlers(
    socket: &WebSocket,
    frames_tx: mpsc::UnboundedSender<Result<RawFrame>>,
) -> (
    Closure<dyn FnMut(MessageEvent)>,
    Closure<dyn FnMut(ErrorEvent)>,
    Closure<dyn FnMut(CloseEvent)>,
) {
    let message_tx = frames_tx.clone();
    let onmessage = Closure::wrap(Box::new(move |event: MessageEvent| {
        let data = event.data();
        if let Some(text) = data.as_string() {
            let _ = message_tx.unbounded_send(Ok(RawFrame::Text(text)));
            return;
        }

        match data.dyn_into::<ArrayBuffer>() {
            Ok(buffer) => {
                let array = Uint8Array::new(&buffer);
                let mut bytes = vec![0; array.length() as usize];
                array.copy_to(&mut bytes);
                let _ = message_tx.unbounded_send(Ok(RawFrame::Binary(bytes)));
            }
            Err(_) => {
                let _ = message_tx
                    .unbounded_send(Err(anyhow!("unsupported websocket message payload")));
            }
        }
    }) as Box<dyn FnMut(MessageEvent)>);

    let error_tx = frames_tx.clone();
    let onerror = Closure::wrap(Box::new(move |event: ErrorEvent| {
        let _ = error_tx.unbounded_send(Err(anyhow!("websocket error: {}", event.message())));
    }) as Box<dyn FnMut(ErrorEvent)>);

    let close_tx = frames_tx;
    let onclose = Closure::wrap(Box::new(move |event: CloseEvent| {
        let reason = if event.reason().is_empty() {
            format!("code={}", event.code())
        } else {
            format!("code={} reason={}", event.code(), event.reason())
        };
        let _ = close_tx.unbounded_send(Ok(RawFrame::Closed(reason)));
    }) as Box<dyn FnMut(CloseEvent)>);

    socket.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
    socket.set_onerror(Some(onerror.as_ref().unchecked_ref()));
    socket.set_onclose(Some(onclose.as_ref().unchecked_ref()));

    (onmessage, onerror, onclose)
}

fn js_error(value: JsValue) -> anyhow::Error {
    if let Some(message) = value.as_string() {
        return anyhow!("websocket js error: {message}");
    }

    anyhow!("websocket js error")
}

#[cfg(test)]
mod tests {
    use super::*;
    use openim_protocol::WsReqIdentifier;
    use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    async fn wasm_client_routes_response_push_and_reconnects() {
        let mut client = WasmWsClient::connect(test_config()).await.unwrap();

        client.send_heartbeat_ping().await.unwrap();
        assert!(matches!(
            client.recv_event().await.unwrap(),
            TransportEvent::HeartbeatPong
        ));

        let req = test_request("wasm-msg-1");
        client.send_request(&req).await.unwrap();
        match client.recv_event().await.unwrap() {
            TransportEvent::Response(resp) => {
                assert_eq!(resp.msg_incr, "wasm-msg-1");
                assert_eq!(resp.req_identifier, WsReqIdentifier::GetNewestSeq.as_i32());
            }
            other => panic!("expected response, got {other:?}"),
        }

        match client.recv_event().await.unwrap() {
            TransportEvent::Push(resp) => {
                assert_eq!(resp.req_identifier, WsReqIdentifier::PushMsg.as_i32());
            }
            other => panic!("expected push, got {other:?}"),
        }

        let req = test_request("wasm-close-after-response");
        client.send_request(&req).await.unwrap();
        assert!(matches!(
            client.recv_event().await.unwrap(),
            TransportEvent::Response(_)
        ));
        assert!(matches!(
            client.recv_event().await.unwrap(),
            TransportEvent::Disconnected { .. }
        ));

        client
            .reconnect(ReconnectPolicy {
                max_attempts: 3,
                initial_delay: Duration::from_millis(1),
                max_delay: Duration::from_millis(1),
            })
            .await
            .unwrap();

        let req = test_request("wasm-after-reconnect");
        client.send_request(&req).await.unwrap();
        match client.recv_event().await.unwrap() {
            TransportEvent::Response(resp) => {
                assert_eq!(resp.msg_incr, "wasm-after-reconnect");
            }
            other => panic!("expected response after reconnect, got {other:?}"),
        }
    }

    fn test_config() -> TransportConfig {
        let ws_addr = option_env!("OPENIM_TRANSPORT_FIXTURE_WS_ADDR")
            .unwrap_or("ws://127.0.0.1:19081/msg_gateway");
        TransportConfig {
            ws_addr: ws_addr.to_string(),
            user_id: "u1".to_string(),
            token: "token".to_string(),
            platform_id: 5,
            operation_id: "op1".to_string(),
            sdk_type: "js".to_string(),
            sdk_version: "rust-phase4-wasm-test".to_string(),
            is_background: false,
            compression: false,
            send_response: true,
        }
    }

    fn test_request(msg_incr: &str) -> GeneralWsReq {
        GeneralWsReq::new(
            WsReqIdentifier::GetNewestSeq,
            "u1",
            "op1",
            msg_incr,
            Vec::new(),
        )
    }
}
