pub use openim_transport_core::{
    build_get_conversations_has_read_and_max_seq_request, build_get_newest_seq_request,
    build_pull_conversation_last_message_request, build_pull_msg_by_range_request,
    build_pull_msg_by_seq_list_request, build_send_msg_request,
    decode_get_conversations_has_read_and_max_seq_response,
    decode_pull_conversation_last_message_response, decode_pull_msg_by_range_response,
    decode_pull_msg_by_seq_list_response, decode_response_data, decode_response_payload,
    decode_send_msg_response, encode_request_payload, ensure_success_response, heartbeat_ping_text,
    route_envelope, text_heartbeat_frame, text_pong_response, ClientConfig, HeartbeatConfig,
    PendingRequests, ReconnectPolicy, TextHeartbeatFrame, TransportConfig, TransportEvent,
};
pub use openim_transport_native::{NativeWsClient, OpenImWsClient};

#[cfg(target_arch = "wasm32")]
pub use openim_transport_wasm::WasmWsClient;
