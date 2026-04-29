use std::collections::HashMap;

use prost::Message;

#[derive(Clone, PartialEq, Eq, Message)]
pub struct GetMaxSeqReq {
    #[prost(string, tag = "1")]
    pub user_id: String,
}

#[derive(Clone, PartialEq, Eq, Message)]
pub struct GetMaxSeqResp {
    #[prost(map = "string, int64", tag = "1")]
    pub max_seqs: HashMap<String, i64>,
    #[prost(map = "string, int64", tag = "2")]
    pub min_seqs: HashMap<String, i64>,
}

#[derive(Clone, PartialEq, Eq, Message)]
pub struct RequestPagination {
    #[prost(int32, tag = "1")]
    pub page_number: i32,
    #[prost(int32, tag = "2")]
    pub show_number: i32,
}

#[cfg(test)]
mod tests {
    use prost::Message;

    use super::*;

    #[test]
    fn get_max_seq_req_matches_proto_field_number() {
        let req = GetMaxSeqReq {
            user_id: "u1".to_string(),
        };
        let mut encoded = Vec::new();

        req.encode(&mut encoded).unwrap();

        assert_eq!(encoded, vec![10, 2, b'u', b'1']);
    }

    #[test]
    fn get_max_seq_resp_map_round_trips() {
        let mut resp = GetMaxSeqResp::default();
        resp.max_seqs.insert("single_u1".to_string(), 9);
        resp.min_seqs.insert("single_u1".to_string(), 1);
        let mut encoded = Vec::new();

        resp.encode(&mut encoded).unwrap();
        let decoded = GetMaxSeqResp::decode(encoded.as_slice()).unwrap();

        assert_eq!(decoded.max_seqs["single_u1"], 9);
        assert_eq!(decoded.min_seqs["single_u1"], 1);
    }

    #[test]
    fn request_pagination_matches_proto_field_numbers() {
        let req = RequestPagination {
            page_number: 1,
            show_number: 200,
        };
        let mut encoded = Vec::new();

        req.encode(&mut encoded).unwrap();

        assert_eq!(encoded, vec![8, 1, 16, 200, 1]);
    }
}
