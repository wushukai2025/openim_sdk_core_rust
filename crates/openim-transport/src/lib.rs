use anyhow::{anyhow, Context, Result};
use futures_util::{SinkExt, StreamExt};
use openim_protocol::{
    constants::{GZIP_COMPRESSION, PHASE1_SDK_VERSION, SDK_TYPE_JS},
    decode_json_response, encode_json_request, gen_msg_incr, gen_operation_id, gzip_compress,
    gzip_decompress, GeneralWsReq, GeneralWsResp, GetMaxSeqReq, WsReqIdentifier,
};
use prost::Message as ProstMessage;
use serde::Deserialize;
use tokio::net::TcpStream;
use tokio_tungstenite::{
    connect_async, tungstenite::Message as WsMessage, MaybeTlsStream, WebSocketStream,
};
use url::Url;

type WsStream = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[derive(Debug, Clone)]
pub struct ClientConfig {
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

impl ClientConfig {
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

pub struct OpenImWsClient {
    config: ClientConfig,
    stream: WsStream,
}

impl OpenImWsClient {
    pub async fn connect(config: ClientConfig) -> Result<Self> {
        let url = config.connect_url()?;
        let (mut stream, _) = connect_async(url.as_str())
            .await
            .with_context(|| format!("websocket connect failed: {url}"))?;

        if config.send_response {
            read_initial_response(&mut stream).await?;
        }

        Ok(Self { config, stream })
    }

    pub fn config(&self) -> &ClientConfig {
        &self.config
    }

    pub async fn send_get_newest_seq(&mut self) -> Result<String> {
        let req = GetMaxSeqReq {
            user_id: self.config.user_id.clone(),
        };
        let mut data = Vec::new();
        req.encode(&mut data)?;

        let msg_incr = gen_msg_incr(&self.config.user_id);
        let envelope = GeneralWsReq::new(
            WsReqIdentifier::GetNewestSeq,
            self.config.user_id.clone(),
            self.config.operation_id.clone(),
            msg_incr.clone(),
            data,
        );

        self.send_request(&envelope).await?;
        Ok(msg_incr)
    }

    pub async fn send_request(&mut self, req: &GeneralWsReq) -> Result<()> {
        let mut payload = encode_json_request(req)?;
        if self.config.compression {
            payload = gzip_compress(&payload)?;
        }
        self.stream.send(WsMessage::Binary(payload.into())).await?;
        Ok(())
    }

    pub async fn recv_envelope(&mut self) -> Result<GeneralWsResp> {
        loop {
            let Some(frame) = self.stream.next().await else {
                return Err(anyhow!("websocket closed"));
            };
            match frame? {
                WsMessage::Binary(data) => {
                    let mut payload = data;
                    if self.config.compression {
                        payload = gzip_decompress(&payload)?;
                    }
                    return Ok(decode_json_response(&payload)?);
                }
                WsMessage::Text(text) => {
                    if handle_text_ping(&mut self.stream, text.to_string()).await? {
                        continue;
                    }
                    return Err(anyhow!("unexpected websocket text frame"));
                }
                WsMessage::Ping(data) => {
                    self.stream.send(WsMessage::Pong(data)).await?;
                }
                WsMessage::Pong(_) => {}
                WsMessage::Close(frame) => {
                    return Err(anyhow!("websocket closed by server: {frame:?}"));
                }
                _ => {}
            }
        }
    }
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

async fn read_initial_response(stream: &mut WsStream) -> Result<()> {
    let Some(frame) = stream.next().await else {
        return Err(anyhow!("websocket closed before initial response"));
    };
    let response = match frame? {
        WsMessage::Text(text) => serde_json::from_str::<ApiResponse>(&text.to_string())?,
        WsMessage::Binary(data) => serde_json::from_slice::<ApiResponse>(&data)?,
        other => return Err(anyhow!("unexpected initial websocket frame: {other:?}")),
    };

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

async fn handle_text_ping(stream: &mut WsStream, text: String) -> Result<bool> {
    let Ok(mut message) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Ok(false);
    };

    if message.get("type").and_then(serde_json::Value::as_str) != Some("ping") {
        return Ok(false);
    }

    if let Some(obj) = message.as_object_mut() {
        obj.insert(
            "type".to_string(),
            serde_json::Value::String("pong".to_string()),
        );
    }
    stream
        .send(WsMessage::Text(serde_json::to_string(&message)?.into()))
        .await?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connect_url_uses_js_encoder_branch_and_gzip() {
        let mut config = ClientConfig::new("ws://example.com/msg_gateway", "u1", "token", 5);
        config.operation_id = "op1".to_string();

        let url = config.connect_url().unwrap();
        let query = url.query().unwrap();

        assert!(query.contains("sendID=u1"));
        assert!(query.contains("platformID=5"));
        assert!(query.contains("operationID=op1"));
        assert!(query.contains("sdkType=js"));
        assert!(query.contains("compression=gzip"));
        assert!(query.contains("isMsgResp=true"));
    }
}
