use std::io::{Read, Write};

use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use openim_errors::{OpenImError, Result};

use crate::envelope::{GeneralWsReq, GeneralWsResp};

pub type ProtocolError = OpenImError;

pub fn encode_json_request(req: &GeneralWsReq) -> Result<Vec<u8>> {
    serde_json::to_vec(req).map_err(|err| OpenImError::sdk_internal(err.to_string()))
}

pub fn decode_json_response(data: &[u8]) -> Result<GeneralWsResp> {
    serde_json::from_slice(data).map_err(|err| OpenImError::msg_decode_binary_ws(err.to_string()))
}

pub fn gzip_compress(data: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(data)
        .map_err(|err| OpenImError::sdk_internal(err.to_string()))?;
    encoder
        .finish()
        .map_err(|err| OpenImError::sdk_internal(err.to_string()))
}

pub fn gzip_decompress(data: &[u8]) -> Result<Vec<u8>> {
    let mut decoder = GzDecoder::new(data);
    let mut decoded = Vec::new();
    decoder
        .read_to_end(&mut decoded)
        .map_err(|err| OpenImError::msg_decompression(err.to_string()))?;
    Ok(decoded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gzip_round_trips_json_payload() {
        let payload = br#"{"reqIdentifier":1001,"data":"CgJ1MQ=="}"#;

        let compressed = gzip_compress(payload).unwrap();
        let decompressed = gzip_decompress(&compressed).unwrap();

        assert_eq!(decompressed, payload);
    }
}
