use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use anyhow::{anyhow, Result};
use futures_channel::{mpsc, oneshot};
use futures_util::StreamExt;
use js_sys::{ArrayBuffer, Uint8Array};
use openim_protocol::{GeneralWsReq, GeneralWsResp};
use openim_transport_core::{
    decode_response_payload, encode_request_payload, ensure_connect_ack_from_bytes,
    ensure_connect_ack_from_text, heartbeat_ping_text, route_envelope, PendingRequests,
    TextHeartbeatFrame, TransportConfig, TransportEvent,
};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{BinaryType, CloseEvent, ErrorEvent, Event, MessageEvent, WebSocket};

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
        let url = config.connect_url()?;
        let socket = WebSocket::new(url.as_str()).map_err(js_error)?;
        socket.set_binary_type(BinaryType::Arraybuffer);

        wait_until_open(&socket).await?;

        let (frames_tx, frames_rx) = mpsc::unbounded();
        let (onmessage, onerror, onclose) = install_handlers(&socket, frames_tx);
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

async fn wait_until_open(socket: &WebSocket) -> Result<()> {
    let (tx, rx) = oneshot::channel::<Result<(), String>>();
    let tx = Rc::new(RefCell::new(Some(tx)));

    let open_tx = Rc::clone(&tx);
    let onopen = Closure::wrap(Box::new(move |_event: Event| {
        if let Some(tx) = open_tx.borrow_mut().take() {
            let _ = tx.send(Ok(()));
        }
    }) as Box<dyn FnMut(Event)>);

    let error_tx = Rc::clone(&tx);
    let onerror = Closure::wrap(Box::new(move |event: ErrorEvent| {
        if let Some(tx) = error_tx.borrow_mut().take() {
            let _ = tx.send(Err(event.message()));
        }
    }) as Box<dyn FnMut(ErrorEvent)>);

    socket.set_onopen(Some(onopen.as_ref().unchecked_ref()));
    socket.set_onerror(Some(onerror.as_ref().unchecked_ref()));

    let result = rx
        .await
        .map_err(|_| anyhow!("websocket open callback was dropped"))?;

    socket.set_onopen(None);
    socket.set_onerror(None);
    result.map_err(|message| anyhow!("websocket connect failed: {message}"))
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
