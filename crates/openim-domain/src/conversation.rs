use std::collections::HashMap;

use openim_errors::{OpenImError, Result};
use openim_sync::{diff_by, DiffOptions, SyncAction};
use openim_types::{ConversationId, GroupId, Pagination, SessionType, UserId};
use serde::{Deserialize, Serialize};

use crate::message::{ChatMessage, MessageSnapshot};
use crate::{summarize_action, DomainSyncSummary};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationInfo {
    pub owner_user_id: UserId,
    pub conversation_id: ConversationId,
    pub conversation_type: SessionType,
    pub user_id: UserId,
    pub group_id: GroupId,
    pub show_name: String,
    pub face_url: String,
    pub recv_msg_opt: i32,
    pub unread_count: u32,
    pub latest_message: Option<MessageSnapshot>,
    pub latest_msg_send_time: i64,
    pub draft_text: String,
    pub draft_text_time: i64,
    pub is_pinned: bool,
    pub max_seq: i64,
    pub min_seq: i64,
    pub ex: String,
}

impl ConversationInfo {
    pub fn from_message(owner_user_id: &str, message: &ChatMessage) -> Result<Self> {
        ensure_not_empty(owner_user_id, "owner_user_id")?;
        let (user_id, group_id, show_name) = conversation_target(owner_user_id, message)?;
        Ok(Self {
            owner_user_id: owner_user_id.to_string(),
            conversation_id: message.conversation_id.clone(),
            conversation_type: message.session_type,
            user_id,
            group_id,
            show_name,
            face_url: String::new(),
            recv_msg_opt: 0,
            unread_count: 0,
            latest_message: None,
            latest_msg_send_time: 0,
            draft_text: String::new(),
            draft_text_time: 0,
            is_pinned: false,
            max_seq: 0,
            min_seq: 0,
            ex: String::new(),
        })
    }
}

pub trait ConversationRepository {
    fn save_conversation(&mut self, conversation: ConversationInfo) -> Result<()>;
    fn remove_conversation(&mut self, owner_user_id: &str, conversation_id: &str) -> Result<()>;
    fn load_conversations(&self, owner_user_id: &str) -> Result<Vec<ConversationInfo>>;
}

#[derive(Debug, Default)]
pub struct ConversationService {
    conversations: HashMap<(UserId, ConversationId), ConversationInfo>,
}

impl ConversationService {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn upsert_conversation(&mut self, conversation: ConversationInfo) -> Result<()> {
        validate_conversation(&conversation)?;
        self.conversations
            .insert(conversation_key(&conversation), conversation);
        Ok(())
    }

    pub fn get_conversation(
        &self,
        owner_user_id: &str,
        conversation_id: &str,
    ) -> Result<Option<ConversationInfo>> {
        ensure_pair(owner_user_id, conversation_id)?;
        Ok(self
            .conversations
            .get(&(owner_user_id.to_string(), conversation_id.to_string()))
            .cloned())
    }

    pub fn delete_conversation(
        &mut self,
        owner_user_id: &str,
        conversation_id: &str,
    ) -> Result<()> {
        ensure_pair(owner_user_id, conversation_id)?;
        self.conversations
            .remove(&(owner_user_id.to_string(), conversation_id.to_string()));
        Ok(())
    }

    pub fn all_conversations(&self, owner_user_id: &str) -> Result<Vec<ConversationInfo>> {
        ensure_not_empty(owner_user_id, "owner_user_id")?;
        let mut conversations = self
            .conversations
            .values()
            .filter(|conversation| conversation.owner_user_id == owner_user_id)
            .cloned()
            .collect::<Vec<_>>();
        conversations.sort_by(conversation_order);
        Ok(conversations)
    }

    pub fn paged_conversations(
        &self,
        owner_user_id: &str,
        pagination: Pagination,
    ) -> Result<Vec<ConversationInfo>> {
        Ok(paginate(self.all_conversations(owner_user_id)?, pagination))
    }

    pub fn apply_message(
        &mut self,
        owner_user_id: &str,
        message: &ChatMessage,
    ) -> Result<ConversationInfo> {
        ensure_not_empty(owner_user_id, "owner_user_id")?;
        let key = (owner_user_id.to_string(), message.conversation_id.clone());
        let conversation = match self.conversations.get_mut(&key) {
            Some(conversation) => conversation,
            None => {
                let conversation = ConversationInfo::from_message(owner_user_id, message)?;
                self.conversations.insert(key.clone(), conversation);
                self.conversations
                    .get_mut(&key)
                    .expect("inserted conversation")
            }
        };

        if message.seq > conversation.max_seq {
            conversation.max_seq = message.seq;
        }
        if should_replace_latest(conversation, message) {
            conversation.latest_msg_send_time = message.send_time;
            conversation.latest_message = Some(MessageSnapshot::from(message));
        }
        if message.send_id != owner_user_id && !message.is_read && !message.revoked {
            conversation.unread_count = conversation.unread_count.saturating_add(1);
        }

        Ok(conversation.clone())
    }

    pub fn mark_conversation_read(
        &mut self,
        owner_user_id: &str,
        conversation_id: &str,
    ) -> Result<u32> {
        let conversation = self.conversation_mut(owner_user_id, conversation_id)?;
        let previous = conversation.unread_count;
        conversation.unread_count = 0;
        Ok(previous)
    }

    pub fn mark_all_read(&mut self, owner_user_id: &str) -> Result<u32> {
        ensure_not_empty(owner_user_id, "owner_user_id")?;
        let mut cleared = 0;
        for conversation in self.conversations.values_mut() {
            if conversation.owner_user_id == owner_user_id {
                cleared += conversation.unread_count;
                conversation.unread_count = 0;
            }
        }
        Ok(cleared)
    }

    pub fn set_draft(
        &mut self,
        owner_user_id: &str,
        conversation_id: &str,
        draft_text: impl Into<String>,
        draft_text_time: i64,
    ) -> Result<ConversationInfo> {
        let conversation = self.conversation_mut(owner_user_id, conversation_id)?;
        conversation.draft_text = draft_text.into();
        conversation.draft_text_time = draft_text_time;
        Ok(conversation.clone())
    }

    pub fn set_pinned(
        &mut self,
        owner_user_id: &str,
        conversation_id: &str,
        is_pinned: bool,
    ) -> Result<ConversationInfo> {
        let conversation = self.conversation_mut(owner_user_id, conversation_id)?;
        conversation.is_pinned = is_pinned;
        Ok(conversation.clone())
    }

    pub fn total_unread_count(&self, owner_user_id: &str) -> Result<u32> {
        ensure_not_empty(owner_user_id, "owner_user_id")?;
        Ok(self
            .conversations
            .values()
            .filter(|conversation| conversation.owner_user_id == owner_user_id)
            .map(|conversation| conversation.unread_count)
            .sum())
    }

    pub fn search_conversations(
        &self,
        owner_user_id: &str,
        query: &str,
        pagination: Pagination,
    ) -> Result<Vec<ConversationInfo>> {
        ensure_not_empty(owner_user_id, "owner_user_id")?;
        ensure_not_empty(query, "query")?;
        let query = query.to_lowercase();
        let conversations = self
            .all_conversations(owner_user_id)?
            .into_iter()
            .filter(|conversation| conversation_matches(conversation, &query))
            .collect::<Vec<_>>();
        Ok(paginate(conversations, pagination))
    }

    pub fn sync_conversations(
        &mut self,
        owner_user_id: &str,
        server_conversations: Vec<ConversationInfo>,
    ) -> Result<DomainSyncSummary> {
        ensure_not_empty(owner_user_id, "owner_user_id")?;
        for conversation in &server_conversations {
            validate_conversation(conversation)?;
            if conversation.owner_user_id != owner_user_id {
                return Err(OpenImError::args(
                    "server conversation owner_user_id mismatch",
                ));
            }
        }

        let local_conversations = self.all_conversations(owner_user_id)?;
        let plan = diff_by(
            &server_conversations,
            &local_conversations,
            |conversation| conversation.conversation_id.clone(),
            |server, local| server == local,
            DiffOptions::default(),
        );

        let mut summary = DomainSyncSummary::default();
        for action in plan.actions {
            summarize_action(&mut summary, &action);
            match action {
                SyncAction::Insert { server } | SyncAction::Update { server, .. } => {
                    self.conversations.insert(conversation_key(&server), server);
                }
                SyncAction::Delete { local } => {
                    self.conversations.remove(&conversation_key(&local));
                }
                SyncAction::Unchanged { .. } => {}
            }
        }

        Ok(summary)
    }

    pub fn len(&self) -> usize {
        self.conversations.len()
    }

    pub fn is_empty(&self) -> bool {
        self.conversations.is_empty()
    }

    fn conversation_mut(
        &mut self,
        owner_user_id: &str,
        conversation_id: &str,
    ) -> Result<&mut ConversationInfo> {
        ensure_pair(owner_user_id, conversation_id)?;
        self.conversations
            .get_mut(&(owner_user_id.to_string(), conversation_id.to_string()))
            .ok_or_else(|| OpenImError::args("conversation not found"))
    }
}

impl ConversationRepository for ConversationService {
    fn save_conversation(&mut self, conversation: ConversationInfo) -> Result<()> {
        ConversationService::upsert_conversation(self, conversation)
    }

    fn remove_conversation(&mut self, owner_user_id: &str, conversation_id: &str) -> Result<()> {
        ConversationService::delete_conversation(self, owner_user_id, conversation_id)
    }

    fn load_conversations(&self, owner_user_id: &str) -> Result<Vec<ConversationInfo>> {
        ConversationService::all_conversations(self, owner_user_id)
    }
}

fn conversation_target(
    owner_user_id: &str,
    message: &ChatMessage,
) -> Result<(UserId, GroupId, String)> {
    match message.session_type {
        SessionType::Single | SessionType::Notification => {
            let user_id = if message.send_id == owner_user_id {
                message.recv_id.clone()
            } else {
                message.send_id.clone()
            };
            ensure_not_empty(&user_id, "user_id")?;
            Ok((user_id.clone(), String::new(), user_id))
        }
        SessionType::WriteGroup | SessionType::ReadGroup => {
            ensure_not_empty(&message.group_id, "group_id")?;
            Ok((
                String::new(),
                message.group_id.clone(),
                message.group_id.clone(),
            ))
        }
    }
}

fn should_replace_latest(conversation: &ConversationInfo, message: &ChatMessage) -> bool {
    match &conversation.latest_message {
        None => true,
        Some(latest) if message.seq > 0 && latest.seq > 0 => {
            message.seq > latest.seq
                || (message.seq == latest.seq && message.send_time >= latest.send_time)
        }
        Some(latest) => message.send_time >= latest.send_time,
    }
}

fn conversation_matches(conversation: &ConversationInfo, query: &str) -> bool {
    conversation.conversation_id.to_lowercase().contains(query)
        || conversation.show_name.to_lowercase().contains(query)
        || conversation.draft_text.to_lowercase().contains(query)
        || conversation
            .latest_message
            .as_ref()
            .map(|message| message.summary.to_lowercase().contains(query))
            .unwrap_or(false)
}

fn validate_conversation(conversation: &ConversationInfo) -> Result<()> {
    ensure_pair(&conversation.owner_user_id, &conversation.conversation_id)?;
    match conversation.conversation_type {
        SessionType::Single | SessionType::Notification => {
            ensure_not_empty(&conversation.user_id, "user_id")?;
        }
        SessionType::WriteGroup | SessionType::ReadGroup => {
            ensure_not_empty(&conversation.group_id, "group_id")?;
        }
    }
    Ok(())
}

fn conversation_order(left: &ConversationInfo, right: &ConversationInfo) -> std::cmp::Ordering {
    right
        .is_pinned
        .cmp(&left.is_pinned)
        .then_with(|| right.latest_msg_send_time.cmp(&left.latest_msg_send_time))
        .then_with(|| left.conversation_id.cmp(&right.conversation_id))
}

fn conversation_key(conversation: &ConversationInfo) -> (UserId, ConversationId) {
    (
        conversation.owner_user_id.clone(),
        conversation.conversation_id.clone(),
    )
}

fn ensure_pair(owner_user_id: &str, conversation_id: &str) -> Result<()> {
    ensure_not_empty(owner_user_id, "owner_user_id")?;
    ensure_not_empty(conversation_id, "conversation_id")
}

fn ensure_not_empty(value: &str, field: &str) -> Result<()> {
    if value.is_empty() {
        Err(OpenImError::args(format!("{field} is empty")))
    } else {
        Ok(())
    }
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

#[cfg(test)]
mod tests {
    use openim_types::MessageContentType;

    use super::*;
    use crate::message::MessageContent;

    #[test]
    fn apply_messages_updates_latest_and_unread_count() {
        let mut service = ConversationService::new();
        let inbound = inbound_message("m1", "hello", 1, 10);
        let outbound = ChatMessage::outgoing(
            "m2",
            "u1",
            "u2",
            SessionType::Single,
            MessageContent::Text {
                content: "reply".to_string(),
            },
            20,
        )
        .unwrap();

        service.apply_message("u1", &inbound).unwrap();
        let conversation = service.apply_message("u1", &outbound).unwrap();

        assert_eq!(conversation.unread_count, 1);
        assert_eq!(conversation.latest_message.unwrap().client_msg_id, "m2");
        assert_eq!(service.total_unread_count("u1").unwrap(), 1);

        assert_eq!(
            service
                .mark_conversation_read("u1", &inbound.conversation_id)
                .unwrap(),
            1
        );
        assert_eq!(service.total_unread_count("u1").unwrap(), 0);
    }

    #[test]
    fn sync_conversations_is_owner_scoped_and_sorted() {
        let mut service = ConversationService::new();
        let mut old = conversation("u1", "si_old", "old", 1);
        old.is_pinned = true;
        service.upsert_conversation(old).unwrap();
        service
            .upsert_conversation(conversation("other", "si_keep", "keep", 9))
            .unwrap();

        let mut pinned = conversation("u1", "si_pin", "pin", 2);
        pinned.is_pinned = true;
        let normal = conversation("u1", "si_new", "new", 99);
        let summary = service
            .sync_conversations("u1", vec![normal.clone(), pinned.clone()])
            .unwrap();

        assert_eq!(summary.inserted, 2);
        assert_eq!(summary.deleted, 1);
        assert!(service
            .get_conversation("other", "si_keep")
            .unwrap()
            .is_some());

        let conversations = service.all_conversations("u1").unwrap();
        assert_eq!(conversations[0].conversation_id, "si_pin");
        assert_eq!(conversations[1].conversation_id, "si_new");
    }

    #[test]
    fn draft_pin_search_and_mark_all_read_work() {
        let mut service = ConversationService::new();
        let message = inbound_message("m1", "searchable latest", 1, 10);
        service.apply_message("u1", &message).unwrap();
        service
            .set_draft("u1", &message.conversation_id, "draft words", 20)
            .unwrap();
        service
            .set_pinned("u1", &message.conversation_id, true)
            .unwrap();

        let matches = service
            .search_conversations(
                "u1",
                "draft",
                Pagination {
                    page_number: 0,
                    show_number: 10,
                },
            )
            .unwrap();
        assert_eq!(matches[0].conversation_id, message.conversation_id);
        assert!(matches[0].is_pinned);

        assert_eq!(service.mark_all_read("u1").unwrap(), 1);
        assert_eq!(service.total_unread_count("u1").unwrap(), 0);
    }

    fn inbound_message(
        client_msg_id: &str,
        content: &str,
        seq: i64,
        send_time: i64,
    ) -> ChatMessage {
        ChatMessage::incoming(
            client_msg_id,
            format!("server-{client_msg_id}"),
            "u2",
            "u1",
            SessionType::Single,
            MessageContent::Text {
                content: content.to_string(),
            },
            seq,
            send_time,
        )
        .unwrap()
    }

    fn conversation(
        owner_user_id: &str,
        conversation_id: &str,
        show_name: &str,
        latest_time: i64,
    ) -> ConversationInfo {
        ConversationInfo {
            owner_user_id: owner_user_id.to_string(),
            conversation_id: conversation_id.to_string(),
            conversation_type: SessionType::Single,
            user_id: show_name.to_string(),
            group_id: String::new(),
            show_name: show_name.to_string(),
            face_url: String::new(),
            recv_msg_opt: 0,
            unread_count: 0,
            latest_message: Some(MessageSnapshot {
                client_msg_id: format!("msg-{show_name}"),
                sender_user_id: show_name.to_string(),
                content_type: MessageContentType::Text,
                summary: show_name.to_string(),
                send_time: latest_time,
                seq: latest_time,
            }),
            latest_msg_send_time: latest_time,
            draft_text: String::new(),
            draft_text_time: 0,
            is_pinned: false,
            max_seq: latest_time,
            min_seq: 0,
            ex: String::new(),
        }
    }
}
