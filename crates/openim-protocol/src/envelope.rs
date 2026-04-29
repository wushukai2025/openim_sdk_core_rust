use serde::{Deserialize, Serialize};

use crate::constants::WsReqIdentifier;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneralWsReq {
    #[serde(rename = "reqIdentifier")]
    pub req_identifier: i32,
    pub token: String,
    #[serde(rename = "sendID")]
    pub send_id: String,
    #[serde(rename = "operationID")]
    pub operation_id: String,
    #[serde(rename = "msgIncr")]
    pub msg_incr: String,
    #[serde(default, with = "base64_bytes")]
    pub data: Vec<u8>,
}

impl GeneralWsReq {
    pub fn new(
        req_identifier: WsReqIdentifier,
        send_id: impl Into<String>,
        operation_id: impl Into<String>,
        msg_incr: impl Into<String>,
        data: Vec<u8>,
    ) -> Self {
        Self {
            req_identifier: req_identifier.as_i32(),
            token: String::new(),
            send_id: send_id.into(),
            operation_id: operation_id.into(),
            msg_incr: msg_incr.into(),
            data,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeneralWsResp {
    #[serde(rename = "reqIdentifier")]
    pub req_identifier: i32,
    #[serde(default, rename = "errCode")]
    pub err_code: i32,
    #[serde(default, rename = "errMsg")]
    pub err_msg: String,
    #[serde(default, rename = "msgIncr")]
    pub msg_incr: String,
    #[serde(default, rename = "operationID")]
    pub operation_id: String,
    #[serde(default, with = "base64_bytes")]
    pub data: Vec<u8>,
}

mod base64_bytes {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&STANDARD.encode(bytes))
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let encoded = Option::<String>::deserialize(deserializer)?;
        match encoded {
            Some(encoded) => STANDARD.decode(encoded).map_err(serde::de::Error::custom),
            None => Ok(Vec::new()),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn request_data_serializes_like_go_json_byte_slice() {
        let req = GeneralWsReq::new(
            WsReqIdentifier::GetNewestSeq,
            "u1",
            "op1",
            "u1_op1",
            vec![1, 2, 3, 4],
        );

        let value = serde_json::to_value(req).unwrap();

        assert_eq!(value["data"], json!("AQIDBA=="));
        assert_eq!(value["reqIdentifier"], json!(1001));
        assert_eq!(value["sendID"], json!("u1"));
        assert_eq!(value["operationID"], json!("op1"));
        assert_eq!(value["msgIncr"], json!("u1_op1"));
    }

    #[test]
    fn response_accepts_null_data_from_go_json() {
        let resp: GeneralWsResp = serde_json::from_str(
            r#"{"reqIdentifier":2001,"errCode":0,"errMsg":"","msgIncr":"","operationID":"","data":null}"#,
        )
        .unwrap();

        assert!(resp.data.is_empty());
    }
}
