use std::collections::HashMap;

use openim_errors::{OpenImError, Result};
use openim_sync::{diff_by, DiffOptions, SyncAction};
use openim_types::{
    ClientMsgId, ConversationId, GroupId, MessageContentType, MessageStatus, Pagination,
    ServerMsgId, SessionType, UserId,
};
use serde::{Deserialize, Serialize};

use crate::{summarize_action, DomainSyncSummary};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PictureElem {
    pub source_url: String,
    pub snapshot_url: String,
    pub width: i32,
    pub height: i32,
    pub size: i64,
    pub image_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileElem {
    pub source_url: String,
    pub file_name: String,
    pub file_size: i64,
    pub file_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum MessageContent {
    Text { content: String },
    Picture(PictureElem),
    File(FileElem),
}

impl MessageContent {
    pub const fn content_type(&self) -> MessageContentType {
        match self {
            Self::Text { .. } => MessageContentType::Text,
            Self::Picture(_) => MessageContentType::Picture,
            Self::File(_) => MessageContentType::File,
        }
    }

    pub fn summary(&self) -> String {
        match self {
            Self::Text { content } => content.clone(),
            Self::Picture(picture) => {
                if picture.source_url.is_empty() {
                    picture.snapshot_url.clone()
                } else {
                    picture.source_url.clone()
                }
            }
            Self::File(file) => file.file_name.clone(),
        }
    }

    fn validate(&self) -> Result<()> {
        match self {
            Self::Text { content } => ensure_not_empty(content, "text_content"),
            Self::Picture(picture) => {
                if picture.source_url.is_empty() && picture.snapshot_url.is_empty() {
                    return Err(OpenImError::args("picture url is empty"));
                }
                Ok(())
            }
            Self::File(file) => {
                ensure_not_empty(&file.file_name, "file_name")?;
                if file.file_size < 0 {
                    return Err(OpenImError::args("file_size is negative"));
                }
                Ok(())
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub client_msg_id: ClientMsgId,
    pub server_msg_id: Option<ServerMsgId>,
    pub conversation_id: ConversationId,
    pub send_id: UserId,
    pub recv_id: UserId,
    pub group_id: GroupId,
    pub session_type: SessionType,
    pub content_type: MessageContentType,
    pub content: MessageContent,
    pub is_read: bool,
    pub status: MessageStatus,
    pub seq: i64,
    pub send_time: i64,
    pub create_time: i64,
    pub attached_info: String,
    pub ex: String,
    pub local_ex: String,
    pub revoked: bool,
}

impl ChatMessage {
    pub fn outgoing(
        client_msg_id: impl Into<ClientMsgId>,
        send_id: impl Into<UserId>,
        target_id: impl Into<String>,
        session_type: SessionType,
        content: MessageContent,
        send_time: i64,
    ) -> Result<Self> {
        let client_msg_id = client_msg_id.into();
        let send_id = send_id.into();
        let target_id = target_id.into();
        let (conversation_id, recv_id, group_id) =
            route_target(&send_id, &target_id, session_type)?;
        let content_type = content.content_type();

        let message = Self {
            client_msg_id,
            server_msg_id: None,
            conversation_id,
            send_id,
            recv_id,
            group_id,
            session_type,
            content_type,
            content,
            is_read: true,
            status: MessageStatus::Sending,
            seq: 0,
            send_time,
            create_time: send_time,
            attached_info: String::new(),
            ex: String::new(),
            local_ex: String::new(),
            revoked: false,
        };
        validate_message(&message)?;
        Ok(message)
    }

    pub fn incoming(
        client_msg_id: impl Into<ClientMsgId>,
        server_msg_id: impl Into<ServerMsgId>,
        send_id: impl Into<UserId>,
        target_id: impl Into<String>,
        session_type: SessionType,
        content: MessageContent,
        seq: i64,
        send_time: i64,
    ) -> Result<Self> {
        let mut message = Self::outgoing(
            client_msg_id,
            send_id,
            target_id,
            session_type,
            content,
            send_time,
        )?;
        message.server_msg_id = Some(server_msg_id.into());
        message.is_read = false;
        message.status = MessageStatus::SendSuccess;
        message.seq = seq;
        validate_message(&message)?;
        Ok(message)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageSnapshot {
    pub client_msg_id: ClientMsgId,
    pub sender_user_id: UserId,
    pub content_type: MessageContentType,
    pub summary: String,
    pub send_time: i64,
    pub seq: i64,
}

impl From<&ChatMessage> for MessageSnapshot {
    fn from(message: &ChatMessage) -> Self {
        Self {
            client_msg_id: message.client_msg_id.clone(),
            sender_user_id: message.send_id.clone(),
            content_type: message.content_type,
            summary: message.content.summary(),
            send_time: message.send_time,
            seq: message.seq,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SendMessageAck {
    pub server_msg_id: ServerMsgId,
    pub seq: i64,
    pub send_time: i64,
}

pub trait MessageSender {
    fn send_message(&mut self, message: &ChatMessage) -> Result<SendMessageAck>;
}

pub trait MessageRepository {
    fn save_message(&mut self, message: ChatMessage) -> Result<()>;
    fn load_message(
        &self,
        conversation_id: &str,
        client_msg_id: &str,
    ) -> Result<Option<ChatMessage>>;
    fn load_history(
        &self,
        conversation_id: &str,
        pagination: Pagination,
    ) -> Result<Vec<ChatMessage>>;
}

#[derive(Debug, Default)]
pub struct MessageService {
    messages: HashMap<(ConversationId, ClientMsgId), ChatMessage>,
}

impl MessageService {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn upsert_message(&mut self, message: ChatMessage) -> Result<()> {
        validate_message(&message)?;
        self.messages.insert(message_key(&message), message);
        Ok(())
    }

    pub fn send_message(
        &mut self,
        mut message: ChatMessage,
        sender: &mut dyn MessageSender,
    ) -> Result<ChatMessage> {
        validate_message(&message)?;
        message.status = MessageStatus::Sending;
        message.is_read = true;
        self.upsert_message(message.clone())?;

        match sender.send_message(&message) {
            Ok(ack) => self.ack_send_success(&message.conversation_id, &message.client_msg_id, ack),
            Err(err) => {
                self.mark_send_failed(&message.conversation_id, &message.client_msg_id)?;
                Err(err)
            }
        }
    }

    pub fn receive_message(&mut self, mut message: ChatMessage) -> Result<ChatMessage> {
        validate_message(&message)?;
        if message.status == MessageStatus::Sending {
            message.status = MessageStatus::SendSuccess;
        }
        self.upsert_message(message.clone())?;
        Ok(message)
    }

    pub fn ack_send_success(
        &mut self,
        conversation_id: &str,
        client_msg_id: &str,
        ack: SendMessageAck,
    ) -> Result<ChatMessage> {
        ensure_not_empty(&ack.server_msg_id, "server_msg_id")?;
        let message = self.message_mut(conversation_id, client_msg_id)?;
        message.server_msg_id = Some(ack.server_msg_id);
        message.seq = ack.seq;
        message.send_time = ack.send_time;
        message.status = MessageStatus::SendSuccess;
        Ok(message.clone())
    }

    pub fn mark_send_failed(
        &mut self,
        conversation_id: &str,
        client_msg_id: &str,
    ) -> Result<ChatMessage> {
        let message = self.message_mut(conversation_id, client_msg_id)?;
        message.status = MessageStatus::SendFailed;
        Ok(message.clone())
    }

    pub fn mark_read(&mut self, conversation_id: &str, client_msg_id: &str) -> Result<ChatMessage> {
        let message = self.message_mut(conversation_id, client_msg_id)?;
        message.is_read = true;
        Ok(message.clone())
    }

    pub fn revoke_message(
        &mut self,
        conversation_id: &str,
        client_msg_id: &str,
    ) -> Result<ChatMessage> {
        let message = self.message_mut(conversation_id, client_msg_id)?;
        message.revoked = true;
        message.status = MessageStatus::HasDeleted;
        Ok(message.clone())
    }

    pub fn get_message(
        &self,
        conversation_id: &str,
        client_msg_id: &str,
    ) -> Result<Option<ChatMessage>> {
        ensure_not_empty(conversation_id, "conversation_id")?;
        ensure_not_empty(client_msg_id, "client_msg_id")?;
        Ok(self
            .messages
            .get(&(conversation_id.to_string(), client_msg_id.to_string()))
            .cloned())
    }

    pub fn all_messages(&self, conversation_id: &str) -> Result<Vec<ChatMessage>> {
        ensure_not_empty(conversation_id, "conversation_id")?;
        let mut messages = self
            .messages
            .values()
            .filter(|message| message.conversation_id == conversation_id)
            .cloned()
            .collect::<Vec<_>>();
        messages.sort_by(message_order);
        Ok(messages)
    }

    pub fn history(
        &self,
        conversation_id: &str,
        pagination: Pagination,
    ) -> Result<Vec<ChatMessage>> {
        let mut messages = self.all_messages(conversation_id)?;
        messages.reverse();
        Ok(paginate(messages, pagination))
    }

    pub fn search(
        &self,
        query: &str,
        conversation_id: Option<&str>,
        pagination: Pagination,
    ) -> Result<Vec<ChatMessage>> {
        ensure_not_empty(query, "query")?;
        if let Some(conversation_id) = conversation_id {
            ensure_not_empty(conversation_id, "conversation_id")?;
        }

        let query = query.to_lowercase();
        let mut messages = self
            .messages
            .values()
            .filter(|message| {
                conversation_id
                    .map(|id| message.conversation_id == id)
                    .unwrap_or(true)
            })
            .filter(|message| message.content.summary().to_lowercase().contains(&query))
            .cloned()
            .collect::<Vec<_>>();
        messages.sort_by(message_order);
        messages.reverse();
        Ok(paginate(messages, pagination))
    }

    pub fn sync_message_range(
        &mut self,
        conversation_id: &str,
        server_messages: Vec<ChatMessage>,
    ) -> Result<DomainSyncSummary> {
        ensure_not_empty(conversation_id, "conversation_id")?;
        for message in &server_messages {
            validate_message(message)?;
            if message.conversation_id != conversation_id {
                return Err(OpenImError::args("server message conversation_id mismatch"));
            }
        }

        let local_messages = self.all_messages(conversation_id)?;
        let plan = diff_by(
            &server_messages,
            &local_messages,
            |message| message.client_msg_id.clone(),
            |server, local| server == local,
            DiffOptions {
                skip_deletion: true,
                include_unchanged: true,
            },
        );

        let mut summary = DomainSyncSummary::default();
        for action in plan.actions {
            summarize_action(&mut summary, &action);
            match action {
                SyncAction::Insert { server } | SyncAction::Update { server, .. } => {
                    self.messages.insert(message_key(&server), server);
                }
                SyncAction::Delete { .. } | SyncAction::Unchanged { .. } => {}
            }
        }

        Ok(summary)
    }

    pub fn len(&self) -> usize {
        self.messages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    fn message_mut(
        &mut self,
        conversation_id: &str,
        client_msg_id: &str,
    ) -> Result<&mut ChatMessage> {
        ensure_not_empty(conversation_id, "conversation_id")?;
        ensure_not_empty(client_msg_id, "client_msg_id")?;
        self.messages
            .get_mut(&(conversation_id.to_string(), client_msg_id.to_string()))
            .ok_or_else(|| OpenImError::args("message not found"))
    }
}

impl MessageRepository for MessageService {
    fn save_message(&mut self, message: ChatMessage) -> Result<()> {
        MessageService::upsert_message(self, message)
    }

    fn load_message(
        &self,
        conversation_id: &str,
        client_msg_id: &str,
    ) -> Result<Option<ChatMessage>> {
        MessageService::get_message(self, conversation_id, client_msg_id)
    }

    fn load_history(
        &self,
        conversation_id: &str,
        pagination: Pagination,
    ) -> Result<Vec<ChatMessage>> {
        MessageService::history(self, conversation_id, pagination)
    }
}

pub fn conversation_id_by_session_type(
    owner_or_send_id: &str,
    target_id: &str,
    session_type: SessionType,
) -> Result<ConversationId> {
    ensure_not_empty(owner_or_send_id, "owner_or_send_id")?;
    ensure_not_empty(target_id, "target_id")?;

    Ok(match session_type {
        SessionType::Single => {
            let mut members = [owner_or_send_id, target_id];
            members.sort();
            format!("si_{}_{}", members[0], members[1])
        }
        SessionType::WriteGroup => format!("g_{target_id}"),
        SessionType::ReadGroup => format!("sg_{target_id}"),
        SessionType::Notification => {
            let mut members = [owner_or_send_id, target_id];
            members.sort();
            format!("sn_{}_{}", members[0], members[1])
        }
    })
}

pub fn conversation_id_by_message(message: &ChatMessage) -> Result<ConversationId> {
    match message.session_type {
        SessionType::Single | SessionType::Notification => conversation_id_by_session_type(
            &message.send_id,
            &message.recv_id,
            message.session_type,
        ),
        SessionType::WriteGroup | SessionType::ReadGroup => conversation_id_by_session_type(
            &message.send_id,
            &message.group_id,
            message.session_type,
        ),
    }
}

fn route_target(
    send_id: &str,
    target_id: &str,
    session_type: SessionType,
) -> Result<(ConversationId, UserId, GroupId)> {
    let conversation_id = conversation_id_by_session_type(send_id, target_id, session_type)?;
    let (recv_id, group_id) = match session_type {
        SessionType::Single | SessionType::Notification => (target_id.to_string(), String::new()),
        SessionType::WriteGroup | SessionType::ReadGroup => (String::new(), target_id.to_string()),
    };
    Ok((conversation_id, recv_id, group_id))
}

fn validate_message(message: &ChatMessage) -> Result<()> {
    ensure_not_empty(&message.client_msg_id, "client_msg_id")?;
    ensure_not_empty(&message.conversation_id, "conversation_id")?;
    ensure_not_empty(&message.send_id, "send_id")?;
    if message.content_type != message.content.content_type() {
        return Err(OpenImError::args("content_type does not match content"));
    }
    match message.session_type {
        SessionType::Single | SessionType::Notification => {
            ensure_not_empty(&message.recv_id, "recv_id")?;
        }
        SessionType::WriteGroup | SessionType::ReadGroup => {
            ensure_not_empty(&message.group_id, "group_id")?;
        }
    }
    if conversation_id_by_message(message)? != message.conversation_id {
        return Err(OpenImError::args(
            "conversation_id does not match message route",
        ));
    }
    message.content.validate()
}

fn message_key(message: &ChatMessage) -> (ConversationId, ClientMsgId) {
    (
        message.conversation_id.clone(),
        message.client_msg_id.clone(),
    )
}

fn message_order(left: &ChatMessage, right: &ChatMessage) -> std::cmp::Ordering {
    left.seq
        .cmp(&right.seq)
        .then_with(|| left.send_time.cmp(&right.send_time))
        .then_with(|| left.client_msg_id.cmp(&right.client_msg_id))
}

fn paginate<T>(items: Vec<T>, pagination: Pagination) -> Vec<T> {
    let pagination = pagination.normalized();
    let start = (pagination.page_number as usize).saturating_mul(pagination.show_number as usize);
    items
        .into_iter()
        .skip(start)
        .take(pagination.show_number as usize)
        .collect()
}

fn ensure_not_empty(value: &str, field: &str) -> Result<()> {
    if value.is_empty() {
        Err(OpenImError::args(format!("{field} is empty")))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sends_text_picture_and_file_messages() {
        let mut service = MessageService::new();
        let mut sender = RecordingSender::default();
        let messages = vec![
            ChatMessage::outgoing(
                "text-1",
                "u1",
                "u2",
                SessionType::Single,
                MessageContent::Text {
                    content: "hello".to_string(),
                },
                10,
            )
            .unwrap(),
            ChatMessage::outgoing(
                "pic-1",
                "u1",
                "u2",
                SessionType::Single,
                MessageContent::Picture(PictureElem {
                    source_url: "https://img.test/a.png".to_string(),
                    snapshot_url: String::new(),
                    width: 100,
                    height: 80,
                    size: 1024,
                    image_type: "png".to_string(),
                }),
                11,
            )
            .unwrap(),
            ChatMessage::outgoing(
                "file-1",
                "u1",
                "u2",
                SessionType::Single,
                MessageContent::File(FileElem {
                    source_url: "https://file.test/a.zip".to_string(),
                    file_name: "a.zip".to_string(),
                    file_size: 2048,
                    file_type: "zip".to_string(),
                }),
                12,
            )
            .unwrap(),
        ];

        for message in messages {
            let sent = service.send_message(message, &mut sender).unwrap();
            assert_eq!(sent.status, MessageStatus::SendSuccess);
            assert!(sent.server_msg_id.unwrap().starts_with("server-"));
        }

        assert_eq!(sender.sent, vec!["text-1", "pic-1", "file-1"]);
        assert_eq!(service.len(), 3);
    }

    #[test]
    fn receives_marks_reads_revokes_and_searches_history() {
        let mut service = MessageService::new();
        let inbound = ChatMessage::incoming(
            "recv-1",
            "server-1",
            "u2",
            "u1",
            SessionType::Single,
            MessageContent::Text {
                content: "phase seven hello".to_string(),
            },
            7,
            70,
        )
        .unwrap();
        let outbound = ChatMessage::outgoing(
            "send-1",
            "u1",
            "u2",
            SessionType::Single,
            MessageContent::Text {
                content: "older".to_string(),
            },
            10,
        )
        .unwrap();
        service.upsert_message(outbound).unwrap();
        service.receive_message(inbound.clone()).unwrap();

        assert!(
            !service
                .get_message(&inbound.conversation_id, "recv-1")
                .unwrap()
                .unwrap()
                .is_read
        );
        service
            .mark_read(&inbound.conversation_id, "recv-1")
            .unwrap();
        assert!(
            service
                .get_message(&inbound.conversation_id, "recv-1")
                .unwrap()
                .unwrap()
                .is_read
        );

        let matches = service
            .search(
                "seven",
                Some(&inbound.conversation_id),
                Pagination {
                    page_number: 0,
                    show_number: 10,
                },
            )
            .unwrap();
        assert_eq!(matches[0].client_msg_id, "recv-1");

        let history = service
            .history(
                &inbound.conversation_id,
                Pagination {
                    page_number: 0,
                    show_number: 1,
                },
            )
            .unwrap();
        assert_eq!(history[0].client_msg_id, "recv-1");

        let revoked = service
            .revoke_message(&inbound.conversation_id, "recv-1")
            .unwrap();
        assert!(revoked.revoked);
        assert_eq!(revoked.status, MessageStatus::HasDeleted);
    }

    #[test]
    fn sync_message_range_merges_without_deleting_history() {
        let mut service = MessageService::new();
        let old = ChatMessage::incoming(
            "old",
            "server-old",
            "u2",
            "u1",
            SessionType::Single,
            MessageContent::Text {
                content: "old".to_string(),
            },
            1,
            1,
        )
        .unwrap();
        let mut updated = ChatMessage::incoming(
            "same",
            "server-same",
            "u2",
            "u1",
            SessionType::Single,
            MessageContent::Text {
                content: "before".to_string(),
            },
            2,
            2,
        )
        .unwrap();
        let conversation_id = old.conversation_id.clone();
        service.upsert_message(old).unwrap();
        service.upsert_message(updated.clone()).unwrap();

        updated.content = MessageContent::Text {
            content: "after".to_string(),
        };
        let new_message = ChatMessage::incoming(
            "new",
            "server-new",
            "u2",
            "u1",
            SessionType::Single,
            MessageContent::Text {
                content: "new".to_string(),
            },
            3,
            3,
        )
        .unwrap();

        let summary = service
            .sync_message_range(&conversation_id, vec![updated.clone(), new_message])
            .unwrap();

        assert_eq!(summary.inserted, 1);
        assert_eq!(summary.updated, 1);
        assert_eq!(summary.deleted, 0);
        assert_eq!(service.all_messages(&conversation_id).unwrap().len(), 3);
        assert_eq!(
            service
                .get_message(&conversation_id, "same")
                .unwrap()
                .unwrap()
                .content
                .summary(),
            "after"
        );
    }

    #[test]
    fn conversation_ids_match_go_prefixes() {
        assert_eq!(
            conversation_id_by_session_type("u2", "u1", SessionType::Single).unwrap(),
            "si_u1_u2"
        );
        assert_eq!(
            conversation_id_by_session_type("u1", "g1", SessionType::WriteGroup).unwrap(),
            "g_g1"
        );
        assert_eq!(
            conversation_id_by_session_type("u1", "g1", SessionType::ReadGroup).unwrap(),
            "sg_g1"
        );
    }

    #[derive(Default)]
    struct RecordingSender {
        sent: Vec<ClientMsgId>,
    }

    impl MessageSender for RecordingSender {
        fn send_message(&mut self, message: &ChatMessage) -> Result<SendMessageAck> {
            self.sent.push(message.client_msg_id.clone());
            Ok(SendMessageAck {
                server_msg_id: format!("server-{}", message.client_msg_id),
                seq: self.sent.len() as i64,
                send_time: message.send_time + 100,
            })
        }
    }
}
