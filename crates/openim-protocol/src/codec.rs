use std::io::{Read, Write};

use flate2::{read::GzDecoder, write::GzEncoder, Compression};
use thiserror::Error;

use crate::envelope::{GeneralWsReq, GeneralWsResp};

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("json encode/decode failed: {0}")]
    Json(#[from] serde_json::Error),
    #[error("gzip compression failed: {0}")]
    Gzip(#[from] std::io::Error),
}

pub fn encode_json_request(req: &GeneralWsReq) -> Result<Vec<u8>, ProtocolError> {
    Ok(serde_json::to_vec(req)?)
}

pub fn decode_json_response(data: &[u8]) -> Result<GeneralWsResp, ProtocolError> {
    Ok(serde_json::from_slice(data)?)
}

pub fn gzip_compress(data: &[u8]) -> Result<Vec<u8>, ProtocolError> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(data)?;
    Ok(encoder.finish()?)
}

pub fn gzip_decompress(data: &[u8]) -> Result<Vec<u8>, ProtocolError> {
    let mut decoder = GzDecoder::new(data);
    let mut decoded = Vec::new();
    decoder.read_to_end(&mut decoded)?;
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
