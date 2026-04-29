use std::path::Path;

use openim_domain::{
    conversation::{ConversationInfo, ConversationRepository},
    message::{ChatMessage, MessageContent, MessageRepository, MessageSnapshot},
};
use openim_errors::{OpenImError, Result};
use openim_storage_core::{
    AppSdkVersion, AppVersionStore, StorageMigrator, VersionRecord, VersionStore,
    APP_SDK_VERSION_TABLE, VERSION_SYNC_TABLE,
};
use openim_types::{MessageContentType, MessageStatus, Pagination, SessionType};
use rusqlite::types::Value;
use rusqlite::{params, Connection, OptionalExtension};

pub const LOCAL_CONVERSATIONS_TABLE: &str = "local_conversations";
pub const CHAT_LOGS_TABLE_PREFIX: &str = "chat_logs_";

pub struct SqliteStorage {
    conn: Connection,
}

impl SqliteStorage {
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        Ok(Self {
            conn: Connection::open(path).map_err(sqlite_error)?,
        })
    }

    pub fn open_in_memory() -> Result<Self> {
        Ok(Self {
            conn: Connection::open_in_memory().map_err(sqlite_error)?,
        })
    }

    pub fn connection(&self) -> &Connection {
        &self.conn
    }
}

impl StorageMigrator for SqliteStorage {
    fn migrate(&self) -> Result<()> {
        let sql = format!(
            r#"
            CREATE TABLE IF NOT EXISTS {APP_SDK_VERSION_TABLE} (
                version varchar(255) PRIMARY KEY NOT NULL,
                installed boolean
            );

            CREATE TABLE IF NOT EXISTS {VERSION_SYNC_TABLE} (
                table_name varchar(255) NOT NULL,
                entity_id varchar(255) NOT NULL,
                version_id text,
                version integer,
                create_time integer,
                id_list text,
                PRIMARY KEY (table_name, entity_id)
            );

            CREATE TABLE IF NOT EXISTS {LOCAL_CONVERSATIONS_TABLE} (
                owner_user_id varchar(64) NOT NULL DEFAULT '',
                conversation_id char(128) PRIMARY KEY NOT NULL,
                conversation_type integer,
                user_id char(64),
                group_id char(128),
                show_name varchar(255),
                face_url varchar(255),
                recv_msg_opt integer,
                unread_count integer,
                latest_msg text,
                latest_msg_send_time integer,
                draft_text text,
                draft_text_time integer,
                is_pinned boolean,
                max_seq integer,
                min_seq integer,
                ex varchar(1024)
            );

            CREATE INDEX IF NOT EXISTS index_local_conversations_owner
            ON {LOCAL_CONVERSATIONS_TABLE} (owner_user_id);

            CREATE INDEX IF NOT EXISTS index_local_conversations_latest_msg_send_time
            ON {LOCAL_CONVERSATIONS_TABLE} (latest_msg_send_time);
            "#
        );

        self.conn.execute_batch(&sql).map_err(sqlite_error)
    }
}

impl AppVersionStore for SqliteStorage {
    fn get_app_sdk_version(&self) -> Result<Option<AppSdkVersion>> {
        let sql = format!("SELECT version, installed FROM {APP_SDK_VERSION_TABLE} LIMIT 1");
        self.conn
            .query_row(&sql, [], |row| {
                Ok(AppSdkVersion {
                    version: row.get(0)?,
                    installed: row.get(1)?,
                })
            })
            .optional()
            .map_err(sqlite_error)
    }

    fn set_app_sdk_version(&self, version: &AppSdkVersion) -> Result<()> {
        let select_sql = format!("SELECT version FROM {APP_SDK_VERSION_TABLE} LIMIT 1");
        let existing = self
            .conn
            .query_row(&select_sql, [], |row| row.get::<_, String>(0))
            .optional()
            .map_err(sqlite_error)?;

        if let Some(existing_version) = existing {
            let update_sql = format!(
                "UPDATE {APP_SDK_VERSION_TABLE} SET version = ?1, installed = ?2 WHERE version = ?3"
            );
            self.conn
                .execute(
                    &update_sql,
                    params![&version.version, version.installed, existing_version],
                )
                .map_err(sqlite_error)?;
        } else {
            let insert_sql =
                format!("INSERT INTO {APP_SDK_VERSION_TABLE} (version, installed) VALUES (?1, ?2)");
            self.conn
                .execute(&insert_sql, params![&version.version, version.installed])
                .map_err(sqlite_error)?;
        }

        Ok(())
    }
}

impl VersionStore for SqliteStorage {
    fn get_version_sync(&self, table_name: &str, entity_id: &str) -> Result<Option<VersionRecord>> {
        let sql = format!(
            "SELECT table_name, entity_id, version_id, version, create_time, id_list \
             FROM {VERSION_SYNC_TABLE} WHERE table_name = ?1 AND entity_id = ?2 LIMIT 1"
        );

        let raw = self
            .conn
            .query_row(&sql, params![table_name, entity_id], |row| {
                Ok(RawVersionRecord {
                    table_name: row.get(0)?,
                    entity_id: row.get(1)?,
                    version_id: row.get(2)?,
                    version: row.get(3)?,
                    create_time: row.get(4)?,
                    uid_list: row.get(5)?,
                })
            })
            .optional()
            .map_err(sqlite_error)?;

        raw.map(VersionRecord::try_from).transpose()
    }

    fn set_version_sync(&self, record: &VersionRecord) -> Result<()> {
        let version = i64::try_from(record.version)
            .map_err(|_| OpenImError::args("version exceeds sqlite integer range"))?;
        let uid_list = serde_json::to_string(&record.uid_list).map_err(json_error)?;
        let sql = format!(
            "INSERT INTO {VERSION_SYNC_TABLE} \
             (table_name, entity_id, version_id, version, create_time, id_list) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
             ON CONFLICT(table_name, entity_id) DO UPDATE SET \
             version_id = excluded.version_id, \
             version = excluded.version, \
             create_time = excluded.create_time, \
             id_list = excluded.id_list"
        );

        self.conn
            .execute(
                &sql,
                params![
                    &record.table_name,
                    &record.entity_id,
                    &record.version_id,
                    version,
                    record.create_time,
                    uid_list
                ],
            )
            .map_err(sqlite_error)?;

        Ok(())
    }

    fn delete_version_sync(&self, table_name: &str, entity_id: &str) -> Result<()> {
        let sql =
            format!("DELETE FROM {VERSION_SYNC_TABLE} WHERE table_name = ?1 AND entity_id = ?2");

        self.conn
            .execute(&sql, params![table_name, entity_id])
            .map_err(sqlite_error)?;

        Ok(())
    }
}

impl MessageRepository for SqliteStorage {
    fn save_message(&mut self, message: ChatMessage) -> Result<()> {
        let table_name = self.ensure_chat_logs_table(&message.conversation_id)?;
        let table = quoted_identifier(&table_name);
        let content = serde_json::to_string(&message.content).map_err(json_error)?;
        let server_msg_id = message.server_msg_id.clone().unwrap_or_default();
        let sql = format!(
            "INSERT INTO {table} \
             (client_msg_id, server_msg_id, send_id, recv_id, sender_platform_id, \
              sender_nick_name, sender_face_url, session_type, msg_from, content_type, \
              content, is_read, status, seq, send_time, create_time, attached_info, ex, local_ex) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19) \
             ON CONFLICT(client_msg_id) DO UPDATE SET \
             server_msg_id = excluded.server_msg_id, \
             send_id = excluded.send_id, \
             recv_id = excluded.recv_id, \
             session_type = excluded.session_type, \
             content_type = excluded.content_type, \
             content = excluded.content, \
             is_read = excluded.is_read, \
             status = excluded.status, \
             seq = excluded.seq, \
             send_time = excluded.send_time, \
             create_time = excluded.create_time, \
             attached_info = excluded.attached_info, \
             ex = excluded.ex, \
             local_ex = excluded.local_ex"
        );

        self.conn
            .execute(
                &sql,
                params![
                    &message.client_msg_id,
                    server_msg_id,
                    &message.send_id,
                    &message.recv_id,
                    0_i32,
                    "",
                    "",
                    message.session_type.as_i32(),
                    0_i32,
                    message.content_type.as_i32(),
                    content,
                    message.is_read,
                    message.status.as_i32(),
                    message.seq,
                    message.send_time,
                    message.create_time,
                    &message.attached_info,
                    &message.ex,
                    &message.local_ex,
                ],
            )
            .map_err(sqlite_error)?;

        Ok(())
    }

    fn load_message(
        &self,
        conversation_id: &str,
        client_msg_id: &str,
    ) -> Result<Option<ChatMessage>> {
        let table_name = chat_logs_table_name(conversation_id)?;
        if !table_exists(&self.conn, &table_name)? {
            return Ok(None);
        }

        let table = quoted_identifier(&table_name);
        let sql = format!(
            "SELECT client_msg_id, server_msg_id, send_id, recv_id, session_type, \
             content_type, content, is_read, status, seq, send_time, create_time, \
             attached_info, ex, local_ex \
             FROM {table} WHERE client_msg_id = ?1 LIMIT 1"
        );

        let raw = self
            .conn
            .query_row(&sql, params![client_msg_id], RawChatMessage::from_row)
            .optional()
            .map_err(sqlite_error)?;

        raw.map(|message| message.into_chat_message(conversation_id))
            .transpose()
    }

    fn load_history(
        &self,
        conversation_id: &str,
        pagination: Pagination,
    ) -> Result<Vec<ChatMessage>> {
        let table_name = chat_logs_table_name(conversation_id)?;
        if !table_exists(&self.conn, &table_name)? {
            return Ok(Vec::new());
        }

        let pagination = pagination.normalized();
        let limit = i64::from(pagination.show_number);
        let offset = i64::from(pagination.page_number).saturating_mul(limit);
        let table = quoted_identifier(&table_name);
        let sql = format!(
            "SELECT client_msg_id, server_msg_id, send_id, recv_id, session_type, \
             content_type, content, is_read, status, seq, send_time, create_time, \
             attached_info, ex, local_ex \
             FROM {table} \
             ORDER BY seq DESC, send_time DESC, client_msg_id DESC \
             LIMIT ?1 OFFSET ?2"
        );

        let mut stmt = self.conn.prepare(&sql).map_err(sqlite_error)?;
        let rows = stmt
            .query_map(params![limit, offset], RawChatMessage::from_row)
            .map_err(sqlite_error)?;
        let raw_messages = rows
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(sqlite_error)?;

        raw_messages
            .into_iter()
            .map(|message| message.into_chat_message(conversation_id))
            .collect()
    }
}

impl ConversationRepository for SqliteStorage {
    fn save_conversation(&mut self, conversation: ConversationInfo) -> Result<()> {
        <Self as StorageMigrator>::migrate(self)?;
        let latest_msg = match &conversation.latest_message {
            Some(snapshot) => serde_json::to_string(snapshot).map_err(json_error)?,
            None => String::new(),
        };
        let sql = format!(
            "INSERT INTO {LOCAL_CONVERSATIONS_TABLE} \
             (owner_user_id, conversation_id, conversation_type, user_id, group_id, show_name, \
              face_url, recv_msg_opt, unread_count, latest_msg, latest_msg_send_time, \
              draft_text, draft_text_time, is_pinned, max_seq, min_seq, ex) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17) \
             ON CONFLICT(conversation_id) DO UPDATE SET \
             owner_user_id = excluded.owner_user_id, \
             conversation_type = excluded.conversation_type, \
             user_id = excluded.user_id, \
             group_id = excluded.group_id, \
             show_name = excluded.show_name, \
             face_url = excluded.face_url, \
             recv_msg_opt = excluded.recv_msg_opt, \
             unread_count = excluded.unread_count, \
             latest_msg = excluded.latest_msg, \
             latest_msg_send_time = excluded.latest_msg_send_time, \
             draft_text = excluded.draft_text, \
             draft_text_time = excluded.draft_text_time, \
             is_pinned = excluded.is_pinned, \
             max_seq = excluded.max_seq, \
             min_seq = excluded.min_seq, \
             ex = excluded.ex"
        );

        self.conn
            .execute(
                &sql,
                params![
                    &conversation.owner_user_id,
                    &conversation.conversation_id,
                    conversation.conversation_type.as_i32(),
                    &conversation.user_id,
                    &conversation.group_id,
                    &conversation.show_name,
                    &conversation.face_url,
                    conversation.recv_msg_opt,
                    i64::from(conversation.unread_count),
                    latest_msg,
                    conversation.latest_msg_send_time,
                    &conversation.draft_text,
                    conversation.draft_text_time,
                    conversation.is_pinned,
                    conversation.max_seq,
                    conversation.min_seq,
                    &conversation.ex,
                ],
            )
            .map_err(sqlite_error)?;

        Ok(())
    }

    fn remove_conversation(&mut self, owner_user_id: &str, conversation_id: &str) -> Result<()> {
        let sql = format!(
            "DELETE FROM {LOCAL_CONVERSATIONS_TABLE} \
             WHERE owner_user_id = ?1 AND conversation_id = ?2"
        );
        self.conn
            .execute(&sql, params![owner_user_id, conversation_id])
            .map_err(sqlite_error)?;
        Ok(())
    }

    fn load_conversations(&self, owner_user_id: &str) -> Result<Vec<ConversationInfo>> {
        if !table_exists(&self.conn, LOCAL_CONVERSATIONS_TABLE)? {
            return Ok(Vec::new());
        }

        let sql = format!(
            "SELECT owner_user_id, conversation_id, conversation_type, user_id, group_id, \
             show_name, face_url, recv_msg_opt, unread_count, latest_msg, latest_msg_send_time, \
             draft_text, draft_text_time, is_pinned, max_seq, min_seq, ex \
             FROM {LOCAL_CONVERSATIONS_TABLE} \
             WHERE owner_user_id = ?1 \
             ORDER BY CASE WHEN is_pinned = 1 THEN 0 ELSE 1 END, \
                      max(latest_msg_send_time, draft_text_time) DESC"
        );

        let mut stmt = self.conn.prepare(&sql).map_err(sqlite_error)?;
        let rows = stmt
            .query_map(params![owner_user_id], RawConversation::from_row)
            .map_err(sqlite_error)?;
        let raw_conversations = rows
            .collect::<rusqlite::Result<Vec<_>>>()
            .map_err(sqlite_error)?;

        raw_conversations
            .into_iter()
            .map(RawConversation::into_conversation)
            .collect()
    }
}

impl SqliteStorage {
    fn ensure_chat_logs_table(&self, conversation_id: &str) -> Result<String> {
        let table_name = chat_logs_table_name(conversation_id)?;
        let table = quoted_identifier(&table_name);
        let seq_index = quoted_identifier(&format!("index_seq_{conversation_id}"));
        let send_time_index = quoted_identifier(&format!("index_send_time_{conversation_id}"));
        let sql = format!(
            r#"
            CREATE TABLE IF NOT EXISTS {table} (
                client_msg_id char(64) PRIMARY KEY NOT NULL,
                server_msg_id char(64),
                send_id char(64),
                recv_id char(64),
                sender_platform_id integer,
                sender_nick_name varchar(255),
                sender_face_url varchar(255),
                session_type integer,
                msg_from integer,
                content_type integer,
                content text,
                is_read boolean,
                status integer,
                seq integer DEFAULT 0,
                send_time integer,
                create_time integer,
                attached_info varchar(1024),
                ex varchar(1024),
                local_ex varchar(1024)
            );

            CREATE INDEX IF NOT EXISTS {seq_index} ON {table} (seq);
            CREATE INDEX IF NOT EXISTS {send_time_index} ON {table} (send_time);
            "#
        );

        self.conn.execute_batch(&sql).map_err(sqlite_error)?;
        Ok(table_name)
    }
}

struct RawChatMessage {
    client_msg_id: String,
    server_msg_id: String,
    send_id: String,
    recv_id: String,
    session_type: i32,
    content_type: i32,
    content: String,
    is_read: bool,
    status: i32,
    seq: i64,
    send_time: i64,
    create_time: i64,
    attached_info: String,
    ex: String,
    local_ex: String,
}

impl RawChatMessage {
    fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            client_msg_id: row.get(0)?,
            server_msg_id: row.get(1)?,
            send_id: row.get(2)?,
            recv_id: row.get(3)?,
            session_type: row.get(4)?,
            content_type: row.get(5)?,
            content: row.get(6)?,
            is_read: row.get(7)?,
            status: row.get(8)?,
            seq: row.get(9)?,
            send_time: row.get(10)?,
            create_time: row.get(11)?,
            attached_info: row.get(12)?,
            ex: row.get(13)?,
            local_ex: row.get(14)?,
        })
    }

    fn into_chat_message(self, conversation_id: &str) -> Result<ChatMessage> {
        let session_type = SessionType::from_i32(self.session_type)
            .ok_or_else(|| OpenImError::sdk_internal("sqlite message has invalid session_type"))?;
        let content_type = MessageContentType::from_i32(self.content_type)
            .ok_or_else(|| OpenImError::sdk_internal("sqlite message has invalid content_type"))?;
        let status = MessageStatus::from_i32(self.status)
            .ok_or_else(|| OpenImError::sdk_internal("sqlite message has invalid status"))?;
        let content = serde_json::from_str::<MessageContent>(&self.content).map_err(json_error)?;

        Ok(ChatMessage {
            client_msg_id: self.client_msg_id,
            server_msg_id: non_empty(self.server_msg_id),
            conversation_id: conversation_id.to_string(),
            send_id: self.send_id,
            recv_id: self.recv_id,
            group_id: group_id_from_conversation(conversation_id, session_type),
            session_type,
            content_type,
            content,
            is_read: self.is_read,
            status,
            seq: self.seq,
            send_time: self.send_time,
            create_time: self.create_time,
            attached_info: self.attached_info,
            ex: self.ex,
            local_ex: self.local_ex,
            revoked: status == MessageStatus::HasDeleted,
        })
    }
}

struct RawConversation {
    owner_user_id: String,
    conversation_id: String,
    conversation_type: i32,
    user_id: String,
    group_id: String,
    show_name: String,
    face_url: String,
    recv_msg_opt: i32,
    unread_count: i64,
    latest_msg: String,
    latest_msg_send_time: i64,
    draft_text: String,
    draft_text_time: i64,
    is_pinned: bool,
    max_seq: i64,
    min_seq: i64,
    ex: String,
}

impl RawConversation {
    fn from_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Self> {
        Ok(Self {
            owner_user_id: row.get(0)?,
            conversation_id: row.get(1)?,
            conversation_type: row.get(2)?,
            user_id: row.get(3)?,
            group_id: row.get(4)?,
            show_name: row.get(5)?,
            face_url: row.get(6)?,
            recv_msg_opt: row.get(7)?,
            unread_count: row.get(8)?,
            latest_msg: row.get(9)?,
            latest_msg_send_time: row.get(10)?,
            draft_text: row.get(11)?,
            draft_text_time: row.get(12)?,
            is_pinned: row.get(13)?,
            max_seq: row.get(14)?,
            min_seq: row.get(15)?,
            ex: row.get(16)?,
        })
    }

    fn into_conversation(self) -> Result<ConversationInfo> {
        let conversation_type = SessionType::from_i32(self.conversation_type).ok_or_else(|| {
            OpenImError::sdk_internal("sqlite conversation has invalid conversation_type")
        })?;
        let unread_count = u32::try_from(self.unread_count)
            .map_err(|_| OpenImError::sdk_internal("sqlite unread_count is negative"))?;
        let latest_message = if self.latest_msg.is_empty() {
            None
        } else {
            Some(serde_json::from_str::<MessageSnapshot>(&self.latest_msg).map_err(json_error)?)
        };

        Ok(ConversationInfo {
            owner_user_id: self.owner_user_id,
            conversation_id: self.conversation_id,
            conversation_type,
            user_id: self.user_id,
            group_id: self.group_id,
            show_name: self.show_name,
            face_url: self.face_url,
            recv_msg_opt: self.recv_msg_opt,
            unread_count,
            latest_message,
            latest_msg_send_time: self.latest_msg_send_time,
            draft_text: self.draft_text,
            draft_text_time: self.draft_text_time,
            is_pinned: self.is_pinned,
            max_seq: self.max_seq,
            min_seq: self.min_seq,
            ex: self.ex,
        })
    }
}

struct RawVersionRecord {
    table_name: String,
    entity_id: String,
    version_id: Option<String>,
    version: Option<i64>,
    create_time: Option<i64>,
    uid_list: Value,
}

impl TryFrom<RawVersionRecord> for VersionRecord {
    type Error = OpenImError;

    fn try_from(raw: RawVersionRecord) -> Result<Self> {
        let version = raw.version.unwrap_or_default();
        let version = u64::try_from(version)
            .map_err(|_| OpenImError::sdk_internal("sqlite version is negative"))?;

        Ok(Self {
            table_name: raw.table_name,
            entity_id: raw.entity_id,
            version_id: raw.version_id.unwrap_or_default(),
            version,
            create_time: raw.create_time.unwrap_or_default(),
            uid_list: decode_uid_list(raw.uid_list)?,
        })
    }
}

fn decode_uid_list(value: Value) -> Result<Vec<String>> {
    match value {
        Value::Null => Ok(Vec::new()),
        Value::Text(text) if text.is_empty() => Ok(Vec::new()),
        Value::Text(text) => serde_json::from_str(&text).map_err(json_error),
        Value::Blob(bytes) if bytes.is_empty() => Ok(Vec::new()),
        Value::Blob(bytes) => serde_json::from_slice(&bytes).map_err(json_error),
        _ => Err(OpenImError::sdk_internal(
            "sqlite id_list has unsupported type",
        )),
    }
}

fn chat_logs_table_name(conversation_id: &str) -> Result<String> {
    if conversation_id.is_empty() {
        return Err(OpenImError::args("conversation_id is empty"));
    }
    Ok(format!("{CHAT_LOGS_TABLE_PREFIX}{conversation_id}"))
}

fn quoted_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

fn table_exists(conn: &Connection, table_name: &str) -> Result<bool> {
    let exists = conn
        .query_row(
            "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1 LIMIT 1",
            params![table_name],
            |_| Ok(()),
        )
        .optional()
        .map_err(sqlite_error)?
        .is_some();
    Ok(exists)
}

fn non_empty(value: String) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

fn group_id_from_conversation(conversation_id: &str, session_type: SessionType) -> String {
    match session_type {
        SessionType::WriteGroup => conversation_id
            .strip_prefix("g_")
            .unwrap_or(conversation_id)
            .to_string(),
        SessionType::ReadGroup => conversation_id
            .strip_prefix("sg_")
            .unwrap_or(conversation_id)
            .to_string(),
        SessionType::Single | SessionType::Notification => String::new(),
    }
}

fn sqlite_error(err: rusqlite::Error) -> OpenImError {
    OpenImError::sdk_internal(format!("sqlite error: {err}"))
}

fn json_error(err: serde_json::Error) -> OpenImError {
    OpenImError::sdk_internal(format!("sqlite json error: {err}"))
}

#[cfg(test)]
mod tests {
    use openim_domain::conversation::ConversationInfo;
    use openim_domain::message::{ChatMessage, MessageContent, MessageSnapshot};
    use openim_storage_core::{
        AppVersionStore, StorageMigrator, VersionStore, APP_SDK_VERSION_TABLE, VERSION_SYNC_TABLE,
    };
    use openim_types::{Pagination, SessionType};
    use rusqlite::params;

    use super::*;

    #[test]
    fn migrate_creates_go_compatible_table_names() {
        let storage = SqliteStorage::open_in_memory().unwrap();
        storage.migrate().unwrap();

        let mut stmt = storage
            .connection()
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table'")
            .unwrap();
        let names = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap();

        assert!(names.contains(&APP_SDK_VERSION_TABLE.to_owned()));
        assert!(names.contains(&VERSION_SYNC_TABLE.to_owned()));
        assert!(names.contains(&LOCAL_CONVERSATIONS_TABLE.to_owned()));
    }

    #[test]
    fn app_sdk_version_round_trips_single_record() {
        let storage = SqliteStorage::open_in_memory().unwrap();
        storage.migrate().unwrap();

        assert_eq!(storage.get_app_sdk_version().unwrap(), None);

        storage
            .set_app_sdk_version(&AppSdkVersion::new("3.8.3", false))
            .unwrap();
        assert_eq!(
            storage.get_app_sdk_version().unwrap(),
            Some(AppSdkVersion::new("3.8.3", false))
        );

        storage
            .set_app_sdk_version(&AppSdkVersion::new("4.0.0", true))
            .unwrap();
        assert_eq!(
            storage.get_app_sdk_version().unwrap(),
            Some(AppSdkVersion::new("4.0.0", true))
        );
    }

    #[test]
    fn version_sync_upserts_and_deletes_by_composite_key() {
        let storage = SqliteStorage::open_in_memory().unwrap();
        storage.migrate().unwrap();

        let mut record = VersionRecord::new("local_group_entities_version", "1076204769");
        record.version_id = "667aabe3417b67f0f0d3cdee".to_owned();
        record.version = 1076204769;
        record.uid_list = vec!["8879166186".to_owned(), "1695766238".to_owned()];

        storage.set_version_sync(&record).unwrap();
        assert_eq!(
            storage
                .get_version_sync("local_group_entities_version", "1076204769")
                .unwrap(),
            Some(record.clone())
        );

        record.version = 1076204770;
        record.uid_list.push("2882899447".to_owned());
        storage.set_version_sync(&record).unwrap();
        assert_eq!(
            storage
                .get_version_sync("local_group_entities_version", "1076204769")
                .unwrap(),
            Some(record)
        );

        storage
            .delete_version_sync("local_group_entities_version", "1076204769")
            .unwrap();
        assert_eq!(
            storage
                .get_version_sync("local_group_entities_version", "1076204769")
                .unwrap(),
            None
        );
    }

    #[test]
    fn version_sync_reads_go_string_array_blob() {
        let storage = SqliteStorage::open_in_memory().unwrap();
        storage.migrate().unwrap();
        let uid_list = serde_json::to_vec(&vec!["u1", "u2"]).unwrap();
        let sql = format!(
            "INSERT INTO {VERSION_SYNC_TABLE} \
             (table_name, entity_id, version_id, version, create_time, id_list) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)"
        );

        storage
            .connection()
            .execute(
                &sql,
                params!["table", "entity", "version-id", 7_i64, 8_i64, uid_list],
            )
            .unwrap();

        assert_eq!(
            storage.get_version_sync("table", "entity").unwrap(),
            Some(VersionRecord {
                table_name: "table".to_owned(),
                entity_id: "entity".to_owned(),
                version_id: "version-id".to_owned(),
                version: 7,
                create_time: 8,
                uid_list: vec!["u1".to_owned(), "u2".to_owned()]
            })
        );
    }

    #[test]
    fn message_repository_round_trips_dynamic_chat_log_table() {
        let mut storage = SqliteStorage::open_in_memory().unwrap();
        storage.migrate().unwrap();
        let first = ChatMessage::incoming(
            "client-1",
            "server-1",
            "u2",
            "u1",
            SessionType::Single,
            MessageContent::Text {
                content: "hello".to_string(),
            },
            1,
            100,
        )
        .unwrap();
        let second = ChatMessage::incoming(
            "client-2",
            "server-2",
            "u2",
            "u1",
            SessionType::Single,
            MessageContent::Text {
                content: "world".to_string(),
            },
            2,
            200,
        )
        .unwrap();

        storage.save_message(first.clone()).unwrap();
        storage.save_message(second.clone()).unwrap();

        let loaded = storage
            .load_message(&first.conversation_id, &first.client_msg_id)
            .unwrap()
            .unwrap();
        let history = storage
            .load_history(
                &first.conversation_id,
                Pagination {
                    page_number: 0,
                    show_number: 1,
                },
            )
            .unwrap();

        assert_eq!(loaded, first);
        assert_eq!(history, vec![second]);
        assert!(table_exists(
            storage.connection(),
            &format!("{CHAT_LOGS_TABLE_PREFIX}{}", first.conversation_id)
        )
        .unwrap());
    }

    #[test]
    fn conversation_repository_round_trips_owner_scoped_rows() {
        let mut storage = SqliteStorage::open_in_memory().unwrap();
        storage.migrate().unwrap();
        let message = ChatMessage::incoming(
            "client-1",
            "server-1",
            "u2",
            "u1",
            SessionType::Single,
            MessageContent::Text {
                content: "hello".to_string(),
            },
            7,
            700,
        )
        .unwrap();
        let mut conversation = ConversationInfo::from_message("u1", &message).unwrap();
        conversation.latest_message = Some(MessageSnapshot::from(&message));
        conversation.latest_msg_send_time = message.send_time;
        conversation.unread_count = 1;
        conversation.is_pinned = true;
        conversation.max_seq = message.seq;

        storage.save_conversation(conversation.clone()).unwrap();

        assert_eq!(
            storage.load_conversations("u1").unwrap(),
            vec![conversation.clone()]
        );
        assert!(storage.load_conversations("u3").unwrap().is_empty());

        storage
            .remove_conversation("u1", &conversation.conversation_id)
            .unwrap();
        assert!(storage.load_conversations("u1").unwrap().is_empty());
    }
}
