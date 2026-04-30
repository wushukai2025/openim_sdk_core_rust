use openim_domain::{conversation::ConversationInfo, message::ChatMessage};
#[cfg(not(target_arch = "wasm32"))]
use openim_errors::ErrorCode;
use openim_errors::{OpenImError, Result};
use openim_storage_core::{
    openim_indexeddb_name, version_sync_key, AppSdkVersion, AsyncAppVersionStore,
    AsyncStorageMigrator, AsyncVersionStore, LocalBoxFuture, VersionRecord,
};
use openim_types::Pagination;

pub const MESSAGE_STORE: &str = "local_messages";
pub const MESSAGE_HISTORY_STORE: &str = "local_message_histories";
pub const CONVERSATION_STORE: &str = "local_conversations";
pub const OWNER_CONVERSATION_STORE: &str = "local_owner_conversations";

#[cfg(target_arch = "wasm32")]
mod wasm;

#[cfg(target_arch = "wasm32")]
pub use wasm::delete_database;

pub struct IndexedDbStorage {
    db_name: String,
}

impl IndexedDbStorage {
    pub fn new(login_user_id: &str) -> Result<Self> {
        Ok(Self {
            db_name: openim_indexeddb_name(login_user_id)?,
        })
    }

    pub fn with_db_name(db_name: impl Into<String>) -> Result<Self> {
        let db_name = db_name.into();
        if db_name.is_empty() {
            return Err(OpenImError::args("db_name is empty"));
        }

        Ok(Self { db_name })
    }

    pub fn db_name(&self) -> &str {
        &self.db_name
    }

    pub fn save_message(&self, message: ChatMessage) -> LocalBoxFuture<'_, Result<()>> {
        #[cfg(target_arch = "wasm32")]
        {
            Box::pin(async move { wasm::save_message(&self.db_name, message).await })
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = message;
            Box::pin(async { Err(indexeddb_unsupported()) })
        }
    }

    pub fn load_message<'a>(
        &'a self,
        conversation_id: &'a str,
        client_msg_id: &'a str,
    ) -> LocalBoxFuture<'a, Result<Option<ChatMessage>>> {
        Box::pin(async move {
            let key = message_key(conversation_id, client_msg_id)?;
            load_message_by_key(&self.db_name, &key).await
        })
    }

    pub fn load_history<'a>(
        &'a self,
        conversation_id: &'a str,
        pagination: Pagination,
    ) -> LocalBoxFuture<'a, Result<Vec<ChatMessage>>> {
        #[cfg(target_arch = "wasm32")]
        {
            Box::pin(
                async move { wasm::load_history(&self.db_name, conversation_id, pagination).await },
            )
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = (conversation_id, pagination);
            Box::pin(async { Err(indexeddb_unsupported()) })
        }
    }

    pub fn save_conversation(
        &self,
        conversation: ConversationInfo,
    ) -> LocalBoxFuture<'_, Result<()>> {
        #[cfg(target_arch = "wasm32")]
        {
            Box::pin(async move { wasm::save_conversation(&self.db_name, conversation).await })
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = conversation;
            Box::pin(async { Err(indexeddb_unsupported()) })
        }
    }

    pub fn remove_conversation<'a>(
        &'a self,
        owner_user_id: &'a str,
        conversation_id: &'a str,
    ) -> LocalBoxFuture<'a, Result<()>> {
        Box::pin(async move {
            let key = conversation_key(owner_user_id, conversation_id)?;
            delete_conversation_by_key(&self.db_name, &key, owner_user_id, conversation_id).await
        })
    }

    pub fn load_conversations<'a>(
        &'a self,
        owner_user_id: &'a str,
    ) -> LocalBoxFuture<'a, Result<Vec<ConversationInfo>>> {
        #[cfg(target_arch = "wasm32")]
        {
            Box::pin(async move { wasm::load_conversations(&self.db_name, owner_user_id).await })
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = owner_user_id;
            Box::pin(async { Err(indexeddb_unsupported()) })
        }
    }
}

impl AsyncStorageMigrator for IndexedDbStorage {
    fn migrate(&self) -> LocalBoxFuture<'_, Result<()>> {
        #[cfg(target_arch = "wasm32")]
        {
            Box::pin(async move { wasm::migrate(&self.db_name).await })
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            Box::pin(async { Err(indexeddb_unsupported()) })
        }
    }
}

impl AsyncAppVersionStore for IndexedDbStorage {
    fn get_app_sdk_version(&self) -> LocalBoxFuture<'_, Result<Option<AppSdkVersion>>> {
        #[cfg(target_arch = "wasm32")]
        {
            Box::pin(async move { wasm::get_app_sdk_version(&self.db_name).await })
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            Box::pin(async { Err(indexeddb_unsupported()) })
        }
    }

    fn set_app_sdk_version<'a>(
        &'a self,
        version: &'a AppSdkVersion,
    ) -> LocalBoxFuture<'a, Result<()>> {
        #[cfg(target_arch = "wasm32")]
        {
            Box::pin(async move { wasm::set_app_sdk_version(&self.db_name, version).await })
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = version;
            Box::pin(async { Err(indexeddb_unsupported()) })
        }
    }
}

impl AsyncVersionStore for IndexedDbStorage {
    fn get_version_sync<'a>(
        &'a self,
        table_name: &'a str,
        entity_id: &'a str,
    ) -> LocalBoxFuture<'a, Result<Option<VersionRecord>>> {
        Box::pin(async move {
            let key = version_sync_key(table_name, entity_id)?;
            get_version_sync_by_key(&self.db_name, &key).await
        })
    }

    fn set_version_sync<'a>(&'a self, record: &'a VersionRecord) -> LocalBoxFuture<'a, Result<()>> {
        Box::pin(async move {
            let key = version_sync_key(&record.table_name, &record.entity_id)?;
            set_version_sync_by_key(&self.db_name, &key, record).await
        })
    }

    fn delete_version_sync<'a>(
        &'a self,
        table_name: &'a str,
        entity_id: &'a str,
    ) -> LocalBoxFuture<'a, Result<()>> {
        Box::pin(async move {
            let key = version_sync_key(table_name, entity_id)?;
            delete_version_sync_by_key(&self.db_name, &key).await
        })
    }
}

async fn get_version_sync_by_key(db_name: &str, key: &str) -> Result<Option<VersionRecord>> {
    #[cfg(target_arch = "wasm32")]
    {
        wasm::get_version_sync(db_name, key).await
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (db_name, key);
        Err(indexeddb_unsupported())
    }
}

async fn set_version_sync_by_key(db_name: &str, key: &str, record: &VersionRecord) -> Result<()> {
    #[cfg(target_arch = "wasm32")]
    {
        wasm::set_version_sync(db_name, key, record).await
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (db_name, key, record);
        Err(indexeddb_unsupported())
    }
}

async fn delete_version_sync_by_key(db_name: &str, key: &str) -> Result<()> {
    #[cfg(target_arch = "wasm32")]
    {
        wasm::delete_version_sync(db_name, key).await
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (db_name, key);
        Err(indexeddb_unsupported())
    }
}

async fn load_message_by_key(db_name: &str, key: &str) -> Result<Option<ChatMessage>> {
    #[cfg(target_arch = "wasm32")]
    {
        wasm::load_message(db_name, key).await
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (db_name, key);
        Err(indexeddb_unsupported())
    }
}

async fn delete_conversation_by_key(
    db_name: &str,
    key: &str,
    owner_user_id: &str,
    conversation_id: &str,
) -> Result<()> {
    #[cfg(target_arch = "wasm32")]
    {
        wasm::remove_conversation(db_name, key, owner_user_id, conversation_id).await
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (db_name, key, owner_user_id, conversation_id);
        Err(indexeddb_unsupported())
    }
}

fn message_key(conversation_id: &str, client_msg_id: &str) -> Result<String> {
    ensure_not_empty(conversation_id, "conversation_id")?;
    ensure_not_empty(client_msg_id, "client_msg_id")?;
    serde_json::to_string(&(conversation_id, client_msg_id))
        .map_err(|err| OpenImError::sdk_internal(format!("encode message key failed: {err}")))
}

fn conversation_key(owner_user_id: &str, conversation_id: &str) -> Result<String> {
    ensure_not_empty(owner_user_id, "owner_user_id")?;
    ensure_not_empty(conversation_id, "conversation_id")?;
    serde_json::to_string(&(owner_user_id, conversation_id))
        .map_err(|err| OpenImError::sdk_internal(format!("encode conversation key failed: {err}")))
}

fn ensure_not_empty(value: &str, field: &str) -> Result<()> {
    if value.is_empty() {
        Err(OpenImError::args(format!("{field} is empty")))
    } else {
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn indexeddb_unsupported() -> OpenImError {
    OpenImError::new(
        ErrorCode::NOT_SUPPORT_OPT,
        "IndexedDB storage is only available on wasm32",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storage_name_is_scoped_by_login_user() {
        let storage = IndexedDbStorage::new("1695766238").unwrap();

        assert_eq!(storage.db_name(), "OpenIM_v3_1695766238");
    }

    #[test]
    fn empty_database_name_is_rejected() {
        assert!(IndexedDbStorage::with_db_name("").is_err());
        assert!(IndexedDbStorage::new("").is_err());
    }

    #[test]
    fn message_and_conversation_keys_are_stable_json_tuples() {
        assert_eq!(
            message_key("si_u1_u2", "client-1").unwrap(),
            r#"["si_u1_u2","client-1"]"#
        );
        assert_eq!(
            conversation_key("u1", "si_u1_u2").unwrap(),
            r#"["u1","si_u1_u2"]"#
        );
        assert!(message_key("", "client-1").is_err());
        assert!(conversation_key("u1", "").is_err());
    }
}
