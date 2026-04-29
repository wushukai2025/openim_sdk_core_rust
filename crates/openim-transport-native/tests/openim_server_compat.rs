use std::time::Duration;

use anyhow::Result;
use openim_protocol::WsReqIdentifier;
use openim_transport_core::TransportConfig;
use openim_transport_native::NativeWsClient;
use tokio::time::timeout;

#[tokio::test]
#[ignore = "requires OPENIM_WS_ADDR, OPENIM_USER_ID and OPENIM_TOKEN"]
async fn real_openim_server_get_newest_seq_round_trips() -> Result<()> {
    let Some(config) = config_from_env()? else {
        eprintln!("skipping real OpenIM server test: required env vars are missing");
        return Ok(());
    };

    let mut client = NativeWsClient::connect(config).await?;
    let msg_incr = client.send_get_newest_seq().await?;
    let response = timeout(Duration::from_secs(10), async {
        loop {
            let resp = client.recv_envelope().await?;
            if resp.req_identifier == WsReqIdentifier::GetNewestSeq.as_i32()
                && resp.msg_incr == msg_incr
            {
                return Ok::<_, anyhow::Error>(resp);
            }
        }
    })
    .await??;

    assert_eq!(response.err_code, 0, "{}", response.err_msg);
    Ok(())
}

fn config_from_env() -> Result<Option<TransportConfig>> {
    let Ok(ws_addr) = std::env::var("OPENIM_WS_ADDR") else {
        return Ok(None);
    };
    let Ok(user_id) = std::env::var("OPENIM_USER_ID") else {
        return Ok(None);
    };
    let Ok(token) = std::env::var("OPENIM_TOKEN") else {
        return Ok(None);
    };
    let platform_id = std::env::var("OPENIM_PLATFORM_ID")
        .ok()
        .and_then(|value| value.parse::<i32>().ok())
        .unwrap_or(5);

    let mut config = TransportConfig::new(ws_addr, user_id, token, platform_id);
    if let Ok(operation_id) = std::env::var("OPENIM_OPERATION_ID") {
        config.operation_id = operation_id;
    }
    config.is_background = std::env::var("OPENIM_BACKGROUND")
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    config.compression = std::env::var("OPENIM_NO_COMPRESSION").is_err();
    Ok(Some(config))
}
