use std::collections::HashMap;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use openim_protocol::{
    constants::{GZIP_COMPRESSION, PHASE1_SDK_VERSION, SDK_TYPE_JS},
    decode_json_response, encode_json_request, gen_msg_incr, gen_operation_id, gzip_compress,
    gzip_decompress, pb_msg, pb_sdkws, GeneralWsReq, GeneralWsResp, GetMaxSeqReq, WsReqIdentifier,
};
use prost::Message as ProstMessage;
use serde::Deserialize;
use url::Url;

pub type ClientConfig = TransportConfig;

#[derive(Debug, Clone)]
pub struct TransportConfig {
    pub ws_addr: String,
    pub user_id: String,
    pub token: String,
    pub platform_id: i32,
    pub operation_id: String,
    pub sdk_type: String,
    pub sdk_version: String,
    pub is_background: bool,
    pub compression: bool,
    pub send_response: bool,
}

impl TransportConfig {
    pub fn new(
        ws_addr: impl Into<String>,
        user_id: impl Into<String>,
        token: impl Into<String>,
        platform_id: i32,
    ) -> Self {
        Self {
            ws_addr: ws_addr.into(),
            user_id: user_id.into(),
            token: token.into(),
            platform_id,
            operation_id: gen_operation_id(),
            sdk_type: SDK_TYPE_JS.to_string(),
            sdk_version: PHASE1_SDK_VERSION.to_string(),
            is_background: false,
            compression: true,
            send_response: true,
        }
    }

    pub fn connect_url(&self) -> Result<Url> {
        let mut url =
            Url::parse(&self.ws_addr).context("ws_addr must be a valid ws:// or wss:// URL")?;
        {
            let mut query = url.query_pairs_mut();
            query.append_pair("sendID", &self.user_id);
            query.append_pair("token", &self.token);
            query.append_pair("platformID", &self.platform_id.to_string());
            query.append_pair("operationID", &self.operation_id);
            query.append_pair(
                "isBackground",
                if self.is_background { "true" } else { "false" },
            );
            query.append_pair("sdkVersion", &self.sdk_version);
            query.append_pair("sdkType", &self.sdk_type);
            if self.compression {
                query.append_pair("compression", GZIP_COMPRESSION);
            }
            if self.send_response {
                query.append_pair("isMsgResp", "true");
            }
        }
        Ok(url)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportEvent {
    Connected,
    Disconnected { reason: String },
    Response(GeneralWsResp),
    Push(GeneralWsResp),
    HeartbeatPing,
    HeartbeatPong,
    ReconnectScheduled { attempt: u32, delay: Duration },
    RequestTimeout { msg_incr: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncomingTransportMessage {
    pub send_id: String,
    pub recv_id: String,
    pub group_id: String,
    pub client_msg_id: String,
    pub server_msg_id: String,
    pub session_type: i32,
    pub content_type: i32,
    pub content_json: String,
    pub seq: i64,
    pub send_time: i64,
    pub create_time: i64,
    pub status: i32,
    pub is_read: bool,
    pub attached_info: String,
    pub ex: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeartbeatConfig {
    pub interval: Duration,
    pub timeout: Duration,
}

impl Default for HeartbeatConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(30),
            timeout: Duration::from_secs(10),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReconnectPolicy {
    pub max_attempts: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
}

impl ReconnectPolicy {
    pub fn delay_for_attempt(&self, attempt: u32) -> Option<Duration> {
        if attempt == 0 || (self.max_attempts > 0 && attempt > self.max_attempts) {
            return None;
        }

        let shift = attempt.saturating_sub(1).min(31);
        let multiplier = 1u128 << shift;
        let delay_ms = self
            .initial_delay
            .as_millis()
            .saturating_mul(multiplier)
            .min(self.max_delay.as_millis())
            .min(u64::MAX as u128);

        Some(Duration::from_millis(delay_ms as u64))
    }
}

impl Default for ReconnectPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 5,
            initial_delay: Duration::from_millis(300),
            max_delay: Duration::from_secs(5),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PendingRequests {
    entries: HashMap<String, Duration>,
}

impl PendingRequests {
    pub fn register_at(&mut self, msg_incr: impl Into<String>, sent_at: Duration) {
        let msg_incr = msg_incr.into();
        if !msg_incr.is_empty() {
            self.entries.insert(msg_incr, sent_at);
        }
    }

    pub fn contains(&self, msg_incr: &str) -> bool {
        self.entries.contains_key(msg_incr)
    }

    pub fn resolve(&mut self, msg_incr: &str) -> bool {
        self.entries.remove(msg_incr).is_some()
    }

    pub fn expire_at(&mut self, now: Duration, timeout: Duration) -> Vec<String> {
        let expired = self
            .entries
            .iter()
            .filter_map(|(msg_incr, sent_at)| {
                if now.saturating_sub(*sent_at) >= timeout {
                    Some(msg_incr.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        for msg_incr in &expired {
            self.entries.remove(msg_incr);
        }

        expired
    }

    pub fn clear(&mut self) {
        self.entries.clear();
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextHeartbeatFrame {
    Ping { pong: String },
    Pong,
}

pub fn heartbeat_ping_text() -> &'static str {
    r#"{"type":"ping"}"#
}

pub fn text_heartbeat_frame(text: &str) -> Result<Option<TextHeartbeatFrame>> {
    let Ok(mut message) = serde_json::from_str::<serde_json::Value>(text) else {
        return Ok(None);
    };

    match message.get("type").and_then(serde_json::Value::as_str) {
        Some("ping") => {
            if let Some(obj) = message.as_object_mut() {
                obj.insert(
                    "type".to_string(),
                    serde_json::Value::String("pong".to_string()),
                );
            }
            Ok(Some(TextHeartbeatFrame::Ping {
                pong: serde_json::to_string(&message)?,
            }))
        }
        Some("pong") => Ok(Some(TextHeartbeatFrame::Pong)),
        _ => Ok(None),
    }
}

pub fn text_pong_response(text: &str) -> Result<Option<String>> {
    Ok(match text_heartbeat_frame(text)? {
        Some(TextHeartbeatFrame::Ping { pong }) => Some(pong),
        _ => None,
    })
}

pub fn encode_request_payload(req: &GeneralWsReq, compression: bool) -> Result<Vec<u8>> {
    let mut payload = encode_json_request(req)?;
    if compression {
        payload = gzip_compress(&payload)?;
    }
    Ok(payload)
}

pub fn decode_response_payload(payload: &[u8], compression: bool) -> Result<GeneralWsResp> {
    let decoded;
    let payload = if compression {
        decoded = gzip_decompress(payload)?;
        decoded.as_slice()
    } else {
        payload
    };

    Ok(decode_json_response(payload)?)
}

pub fn build_get_newest_seq_request(config: &TransportConfig) -> Result<(GeneralWsReq, String)> {
    let req = GetMaxSeqReq {
        user_id: config.user_id.clone(),
    };
    build_protobuf_request(config, WsReqIdentifier::GetNewestSeq, &req)
}

pub fn build_send_msg_request(
    config: &TransportConfig,
    msg: pb_sdkws::MsgData,
) -> Result<(GeneralWsReq, String)> {
    build_protobuf_request(config, WsReqIdentifier::SendMsg, &msg)
}

pub fn build_pull_msg_by_range_request(
    config: &TransportConfig,
    seq_ranges: Vec<pb_sdkws::SeqRange>,
    order: pb_sdkws::PullOrder,
) -> Result<(GeneralWsReq, String)> {
    let req = pb_sdkws::PullMessageBySeqsReq {
        user_id: config.user_id.clone(),
        seq_ranges,
        order: order as i32,
    };
    build_protobuf_request(config, WsReqIdentifier::PullMsgByRange, &req)
}

pub fn build_pull_msg_by_seq_list_request(
    config: &TransportConfig,
    conversations: Vec<pb_msg::ConversationSeqs>,
    order: pb_sdkws::PullOrder,
) -> Result<(GeneralWsReq, String)> {
    let req = pb_msg::GetSeqMessageReq {
        user_id: config.user_id.clone(),
        conversations,
        order: order as i32,
    };
    build_protobuf_request(config, WsReqIdentifier::PullMsgBySeqList, &req)
}

pub fn build_get_conversations_has_read_and_max_seq_request(
    config: &TransportConfig,
    conversation_ids: Vec<String>,
    return_pinned: bool,
) -> Result<(GeneralWsReq, String)> {
    let req = pb_msg::GetConversationsHasReadAndMaxSeqReq {
        user_id: config.user_id.clone(),
        conversation_i_ds: conversation_ids,
        return_pinned,
    };
    build_protobuf_request(config, WsReqIdentifier::GetConvMaxReadSeq, &req)
}

pub fn build_pull_conversation_last_message_request(
    config: &TransportConfig,
    conversation_ids: Vec<String>,
) -> Result<(GeneralWsReq, String)> {
    let req = pb_msg::GetLastMessageReq {
        user_id: config.user_id.clone(),
        conversation_i_ds: conversation_ids,
    };
    build_protobuf_request(config, WsReqIdentifier::PullConvLastMessage, &req)
}

pub fn ensure_success_response(resp: &GeneralWsResp) -> Result<()> {
    if resp.err_code == 0 {
        return Ok(());
    }

    Err(anyhow!(
        "websocket response error: identifier={} code={} msg={}",
        resp.req_identifier,
        resp.err_code,
        resp.err_msg
    ))
}

pub fn decode_response_data<M>(resp: &GeneralWsResp) -> Result<M>
where
    M: ProstMessage + Default,
{
    ensure_success_response(resp)?;
    M::decode(resp.data.as_slice()).context("decode websocket protobuf response data failed")
}

pub fn decode_send_msg_response(resp: &GeneralWsResp) -> Result<pb_msg::SendMsgResp> {
    decode_response_data(resp)
}

pub fn decode_pull_msg_by_range_response(
    resp: &GeneralWsResp,
) -> Result<pb_sdkws::PullMessageBySeqsResp> {
    decode_response_data(resp)
}

pub fn decode_pull_msg_by_seq_list_response(
    resp: &GeneralWsResp,
) -> Result<pb_msg::GetSeqMessageResp> {
    decode_response_data(resp)
}

pub fn decode_get_conversations_has_read_and_max_seq_response(
    resp: &GeneralWsResp,
) -> Result<pb_msg::GetConversationsHasReadAndMaxSeqResp> {
    decode_response_data(resp)
}

pub fn decode_pull_conversation_last_message_response(
    resp: &GeneralWsResp,
) -> Result<pb_msg::GetLastMessageResp> {
    decode_response_data(resp)
}

pub fn decode_push_messages_response(
    resp: &GeneralWsResp,
) -> Result<Vec<IncomingTransportMessage>> {
    let push: pb_sdkws::PushMessages = decode_response_data(resp)?;
    let mut messages = Vec::new();
    append_pull_messages_map(&mut messages, push.msgs);
    append_pull_messages_map(&mut messages, push.notification_msgs);
    messages.sort_by(|left, right| {
        left.seq
            .cmp(&right.seq)
            .then_with(|| left.send_time.cmp(&right.send_time))
            .then_with(|| left.client_msg_id.cmp(&right.client_msg_id))
    });
    Ok(messages)
}

pub fn route_envelope(resp: GeneralWsResp, pending: &mut PendingRequests) -> TransportEvent {
    if resp.req_identifier == WsReqIdentifier::PushMsg.as_i32() {
        return TransportEvent::Push(resp);
    }

    if !resp.msg_incr.is_empty() && pending.resolve(&resp.msg_incr) {
        TransportEvent::Response(resp)
    } else {
        TransportEvent::Push(resp)
    }
}

pub fn ensure_connect_ack_from_text(text: &str) -> Result<()> {
    ensure_connect_ack(serde_json::from_str::<ApiResponse>(text)?)
}

pub fn ensure_connect_ack_from_bytes(data: &[u8]) -> Result<()> {
    ensure_connect_ack(serde_json::from_slice::<ApiResponse>(data)?)
}

#[derive(Debug, Deserialize)]
struct ApiResponse {
    #[serde(rename = "errCode")]
    err_code: i32,
    #[serde(default, rename = "errMsg")]
    err_msg: String,
    #[serde(default, rename = "errDlt")]
    err_detail: String,
}

fn ensure_connect_ack(response: ApiResponse) -> Result<()> {
    if response.err_code == 0 {
        return Ok(());
    }

    Err(anyhow!(
        "websocket auth failed: code={} msg={} detail={}",
        response.err_code,
        response.err_msg,
        response.err_detail
    ))
}

fn build_protobuf_request<M>(
    config: &TransportConfig,
    req_identifier: WsReqIdentifier,
    message: &M,
) -> Result<(GeneralWsReq, String)>
where
    M: ProstMessage,
{
    let mut data = Vec::new();
    message.encode(&mut data)?;

    let msg_incr = gen_msg_incr(&config.user_id);
    let envelope = GeneralWsReq::new(
        req_identifier,
        config.user_id.clone(),
        config.operation_id.clone(),
        msg_incr.clone(),
        data,
    );

    Ok((envelope, msg_incr))
}

fn append_pull_messages_map(
    out: &mut Vec<IncomingTransportMessage>,
    pull_messages: std::collections::HashMap<String, pb_sdkws::PullMsgs>,
) {
    for messages in pull_messages.into_values() {
        for message in messages.msgs {
            out.push(IncomingTransportMessage {
                send_id: message.send_id,
                recv_id: message.recv_id,
                group_id: message.group_id,
                client_msg_id: message.client_msg_id,
                server_msg_id: message.server_msg_id,
                session_type: message.session_type,
                content_type: message.content_type,
                content_json: String::from_utf8_lossy(&message.content).into_owned(),
                seq: message.seq,
                send_time: message.send_time,
                create_time: message.create_time,
                status: message.status,
                is_read: message.is_read,
                attached_info: message.attached_info,
                ex: message.ex,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connect_url_uses_js_encoder_branch_and_gzip() {
        let mut config = TransportConfig::new("ws://example.com/msg_gateway", "u1", "token", 5);
        config.operation_id = "op1".to_string();

        let url = config.connect_url().unwrap();
        let query = url.query().unwrap();

        assert!(query.contains("sendID=u1"));
        assert!(query.contains("token=token"));
        assert!(query.contains("platformID=5"));
        assert!(query.contains("operationID=op1"));
        assert!(query.contains("sdkType=js"));
        assert!(query.contains("compression=gzip"));
        assert!(query.contains("isMsgResp=true"));
    }

    #[test]
    fn text_ping_is_answered_as_text_pong() {
        assert_eq!(
            text_pong_response(r#"{"type":"ping","operationID":"op1"}"#).unwrap(),
            Some(r#"{"operationID":"op1","type":"pong"}"#.to_string())
        );
        assert_eq!(text_pong_response(r#"{"type":"pong"}"#).unwrap(), None);
    }

    #[test]
    fn pending_request_routes_response_once() {
        let mut pending = PendingRequests::default();
        pending.register_at("u1-1", Duration::from_secs(1));

        let response = GeneralWsResp {
            req_identifier: WsReqIdentifier::GetNewestSeq.as_i32(),
            err_code: 0,
            err_msg: String::new(),
            msg_incr: "u1-1".to_string(),
            operation_id: "op1".to_string(),
            data: Vec::new(),
        };

        assert!(matches!(
            route_envelope(response.clone(), &mut pending),
            TransportEvent::Response(_)
        ));
        assert!(matches!(
            route_envelope(response, &mut pending),
            TransportEvent::Push(_)
        ));
    }

    #[test]
    fn pending_request_expiry_removes_timed_out_items() {
        let mut pending = PendingRequests::default();
        pending.register_at("old", Duration::from_secs(1));
        pending.register_at("new", Duration::from_secs(9));

        let expired = pending.expire_at(Duration::from_secs(12), Duration::from_secs(5));

        assert_eq!(expired, vec!["old".to_string()]);
        assert!(!pending.contains("old"));
        assert!(pending.contains("new"));
    }

    #[test]
    fn reconnect_policy_uses_capped_backoff() {
        let policy = ReconnectPolicy {
            max_attempts: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_millis(250),
        };

        assert_eq!(
            policy.delay_for_attempt(1),
            Some(Duration::from_millis(100))
        );
        assert_eq!(
            policy.delay_for_attempt(2),
            Some(Duration::from_millis(200))
        );
        assert_eq!(
            policy.delay_for_attempt(3),
            Some(Duration::from_millis(250))
        );
        assert_eq!(policy.delay_for_attempt(4), None);
    }

    #[test]
    fn send_msg_request_uses_generated_msg_data_payload() {
        let config = test_config();
        let (req, msg_incr) = build_send_msg_request(
            &config,
            pb_sdkws::MsgData {
                send_id: "u1".to_string(),
                recv_id: "u2".to_string(),
                client_msg_id: "client-1".to_string(),
                content: b"hello".to_vec(),
                ..Default::default()
            },
        )
        .unwrap();

        let decoded = pb_sdkws::MsgData::decode(req.data.as_slice()).unwrap();

        assert_eq!(req.req_identifier, WsReqIdentifier::SendMsg.as_i32());
        assert_eq!(req.operation_id, "op1");
        assert_eq!(req.msg_incr, msg_incr);
        assert_eq!(decoded.send_id, "u1");
        assert_eq!(decoded.recv_id, "u2");
        assert_eq!(decoded.content, b"hello".to_vec());
    }

    #[test]
    fn pull_msg_by_range_request_matches_msggateway_payload() {
        let config = test_config();
        let (req, _) = build_pull_msg_by_range_request(
            &config,
            vec![pb_sdkws::SeqRange {
                conversation_id: "si_u1_u2".to_string(),
                begin: 1,
                end: 10,
                num: 20,
            }],
            pb_sdkws::PullOrder::Desc,
        )
        .unwrap();

        let decoded = pb_sdkws::PullMessageBySeqsReq::decode(req.data.as_slice()).unwrap();

        assert_eq!(req.req_identifier, WsReqIdentifier::PullMsgByRange.as_i32());
        assert_eq!(decoded.user_id, "u1");
        assert_eq!(decoded.seq_ranges[0].conversation_id, "si_u1_u2");
        assert_eq!(decoded.order, pb_sdkws::PullOrder::Desc as i32);
    }

    #[test]
    fn pull_msg_by_seq_list_request_matches_msggateway_payload() {
        let config = test_config();
        let (req, _) = build_pull_msg_by_seq_list_request(
            &config,
            vec![pb_msg::ConversationSeqs {
                conversation_id: "si_u1_u2".to_string(),
                seqs: vec![2, 3, 5],
            }],
            pb_sdkws::PullOrder::Asc,
        )
        .unwrap();

        let decoded = pb_msg::GetSeqMessageReq::decode(req.data.as_slice()).unwrap();

        assert_eq!(
            req.req_identifier,
            WsReqIdentifier::PullMsgBySeqList.as_i32()
        );
        assert_eq!(decoded.user_id, "u1");
        assert_eq!(decoded.conversations[0].seqs, vec![2, 3, 5]);
        assert_eq!(decoded.order, pb_sdkws::PullOrder::Asc as i32);
    }

    #[test]
    fn conversation_seq_requests_use_generated_msg_payloads() {
        let config = test_config();
        let (read_req, _) = build_get_conversations_has_read_and_max_seq_request(
            &config,
            vec!["si_u1_u2".to_string()],
            true,
        )
        .unwrap();
        let (last_msg_req, _) =
            build_pull_conversation_last_message_request(&config, vec!["g_group1".to_string()])
                .unwrap();

        let read_decoded =
            pb_msg::GetConversationsHasReadAndMaxSeqReq::decode(read_req.data.as_slice()).unwrap();
        let last_msg_decoded =
            pb_msg::GetLastMessageReq::decode(last_msg_req.data.as_slice()).unwrap();

        assert_eq!(
            read_req.req_identifier,
            WsReqIdentifier::GetConvMaxReadSeq.as_i32()
        );
        assert_eq!(read_decoded.conversation_i_ds, vec!["si_u1_u2"]);
        assert!(read_decoded.return_pinned);
        assert_eq!(
            last_msg_req.req_identifier,
            WsReqIdentifier::PullConvLastMessage.as_i32()
        );
        assert_eq!(last_msg_decoded.conversation_i_ds, vec!["g_group1"]);
    }

    #[test]
    fn decode_response_data_checks_error_and_decodes_protobuf() {
        let mut data = Vec::new();
        pb_msg::SendMsgResp {
            server_msg_id: "server-1".to_string(),
            client_msg_id: "client-1".to_string(),
            send_time: 123,
            modify: None,
        }
        .encode(&mut data)
        .unwrap();

        let resp = GeneralWsResp {
            req_identifier: WsReqIdentifier::SendMsg.as_i32(),
            err_code: 0,
            err_msg: String::new(),
            msg_incr: "msg-1".to_string(),
            operation_id: "op1".to_string(),
            data,
        };
        let decoded = decode_send_msg_response(&resp).unwrap();

        assert_eq!(decoded.server_msg_id, "server-1");
        assert_eq!(decoded.client_msg_id, "client-1");
        assert_eq!(decoded.send_time, 123);

        let err = GeneralWsResp {
            err_code: 100,
            err_msg: "failed".to_string(),
            ..resp
        };
        assert!(decode_send_msg_response(&err).is_err());
    }

    #[test]
    fn decode_push_messages_response_flattens_proto_payload() {
        let mut data = Vec::new();
        pb_sdkws::PushMessages {
            msgs: std::collections::HashMap::from([(
                "si_u1_u2".to_string(),
                pb_sdkws::PullMsgs {
                    msgs: vec![pb_sdkws::MsgData {
                        send_id: "u2".to_string(),
                        recv_id: "u1".to_string(),
                        client_msg_id: "client-1".to_string(),
                        server_msg_id: "server-1".to_string(),
                        session_type: 1,
                        content_type: 101,
                        content: br#"{"content":"hello"}"#.to_vec(),
                        seq: 2,
                        send_time: 20,
                        create_time: 10,
                        status: 2,
                        is_read: false,
                        attached_info: "{}".to_string(),
                        ex: String::new(),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
            )]),
            ..Default::default()
        }
        .encode(&mut data)
        .unwrap();

        let resp = GeneralWsResp {
            req_identifier: WsReqIdentifier::PushMsg.as_i32(),
            err_code: 0,
            err_msg: String::new(),
            msg_incr: String::new(),
            operation_id: "op1".to_string(),
            data,
        };

        let messages = decode_push_messages_response(&resp).unwrap();

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].send_id, "u2");
        assert_eq!(messages[0].recv_id, "u1");
        assert_eq!(messages[0].content_json, r#"{"content":"hello"}"#);
        assert_eq!(messages[0].content_type, 101);
    }

    fn test_config() -> TransportConfig {
        let mut config = TransportConfig::new("ws://example.com/msg_gateway", "u1", "token", 5);
        config.operation_id = "op1".to_string();
        config
    }
}
