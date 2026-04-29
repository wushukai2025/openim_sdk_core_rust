pub mod protobuf {
    include!(concat!(env!("OUT_DIR"), "/openim.protobuf.rs"));
}

pub mod sdkws {
    include!(concat!(env!("OUT_DIR"), "/openim.sdkws.rs"));
}

pub mod conversation {
    include!(concat!(env!("OUT_DIR"), "/openim.conversation.rs"));
}

pub mod msg {
    include!(concat!(env!("OUT_DIR"), "/openim.msg.rs"));
}

#[cfg(test)]
mod tests {
    use prost::Message;

    use super::{msg, sdkws};

    #[test]
    fn generated_msg_data_matches_proto_field_numbers() {
        let msg = sdkws::MsgData {
            send_id: "u1".to_string(),
            recv_id: "u2".to_string(),
            client_msg_id: "client-1".to_string(),
            content: b"hello".to_vec(),
            seq: 9,
            ..Default::default()
        };
        let mut encoded = Vec::new();

        msg.encode(&mut encoded).unwrap();

        assert!(encoded.windows(2).any(|field| field == [10, 2]));
        assert!(encoded.windows(2).any(|field| field == [18, 2]));
        assert!(encoded.windows(2).any(|field| field == [34, 8]));
        assert!(encoded.windows(2).any(|field| field == [98, 5]));
        assert!(encoded.windows(2).any(|field| field == [112, 9]));
    }

    #[test]
    fn generated_send_msg_resp_round_trips_modify_message() {
        let resp = msg::SendMsgResp {
            server_msg_id: "server-1".to_string(),
            client_msg_id: "client-1".to_string(),
            send_time: 123,
            modify: Some(sdkws::MsgData {
                client_msg_id: "client-1".to_string(),
                content: b"modified".to_vec(),
                ..Default::default()
            }),
        };
        let mut encoded = Vec::new();

        resp.encode(&mut encoded).unwrap();
        let decoded = msg::SendMsgResp::decode(encoded.as_slice()).unwrap();

        assert_eq!(decoded.server_msg_id, "server-1");
        assert_eq!(decoded.client_msg_id, "client-1");
        assert_eq!(decoded.send_time, 123);
        assert_eq!(decoded.modify.unwrap().content, b"modified".to_vec());
    }

    #[test]
    fn generated_pull_request_uses_seq_ranges() {
        let req = sdkws::PullMessageBySeqsReq {
            user_id: "u1".to_string(),
            seq_ranges: vec![sdkws::SeqRange {
                conversation_id: "si_u1_u2".to_string(),
                begin: 1,
                end: 10,
                num: 20,
            }],
            order: sdkws::PullOrder::Desc as i32,
        };
        let mut encoded = Vec::new();

        req.encode(&mut encoded).unwrap();
        let decoded = sdkws::PullMessageBySeqsReq::decode(encoded.as_slice()).unwrap();

        assert_eq!(decoded.user_id, "u1");
        assert_eq!(decoded.seq_ranges[0].conversation_id, "si_u1_u2");
        assert_eq!(decoded.order, sdkws::PullOrder::Desc as i32);
    }

    #[test]
    fn generated_read_and_revoke_requests_match_msg_proto() {
        let read = msg::MarkConversationAsReadReq {
            conversation_id: "si_u1_u2".to_string(),
            user_id: "u1".to_string(),
            has_read_seq: 10,
            seqs: vec![8, 9, 10],
        };
        let revoke = msg::RevokeMsgReq {
            conversation_id: "si_u1_u2".to_string(),
            seq: 10,
            user_id: "u1".to_string(),
        };

        assert_eq!(read.conversation_id, "si_u1_u2");
        assert_eq!(read.seqs, vec![8, 9, 10]);
        assert_eq!(revoke.seq, 10);
        assert_eq!(revoke.user_id, "u1");
    }
}
