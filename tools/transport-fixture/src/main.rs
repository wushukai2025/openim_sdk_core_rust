use anyhow::Result;
use futures_util::{SinkExt, StreamExt};
use openim_protocol::{GeneralWsReq, GeneralWsResp, WsReqIdentifier};
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{accept_async, tungstenite::Message as WsMessage};

#[tokio::main]
async fn main() -> Result<()> {
    let addr = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "127.0.0.1:19081".to_string());
    let listener = TcpListener::bind(&addr).await?;
    println!("transport fixture listening on ws://{addr}/msg_gateway");

    loop {
        let (stream, _) = listener.accept().await?;
        tokio::spawn(async move {
            if let Err(err) = serve_connection(stream).await {
                eprintln!("transport fixture connection failed: {err}");
            }
        });
    }
}

async fn serve_connection(stream: TcpStream) -> Result<()> {
    let mut ws = accept_async(stream).await?;
    eprintln!("transport fixture accepted websocket");
    ws.send(WsMessage::Text(r#"{"errCode":0}"#.into())).await?;

    while let Some(frame) = ws.next().await {
        match frame? {
            WsMessage::Binary(data) => {
                let req: GeneralWsReq = serde_json::from_slice(data.as_ref())?;
                let msg_incr = req.msg_incr.clone();
                eprintln!("transport fixture received binary msg_incr={msg_incr}");
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

                if msg_incr == "wasm-msg-1" {
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

                if msg_incr == "wasm-close-after-response" {
                    eprintln!("transport fixture closing after requested response");
                    ws.close(None).await?;
                    break;
                }
            }
            WsMessage::Text(text) => {
                eprintln!("transport fixture received text={text}");
                if openim_transport_core_ping(&text)? {
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

fn openim_transport_core_ping(text: &str) -> Result<bool> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else {
        return Ok(false);
    };

    Ok(value.get("type").and_then(serde_json::Value::as_str) == Some("ping"))
}
