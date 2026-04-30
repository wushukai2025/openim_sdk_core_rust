use openim_domain::{conversation::ConversationInfo, message::ChatMessage};
use openim_errors::{OpenImError, Result};
use openim_storage_core::{
    AppSdkVersion, VersionRecord, APP_SDK_VERSION_KEY, APP_SDK_VERSION_TABLE, VERSION_SYNC_TABLE,
};
use openim_types::Pagination;
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    Event, IdbDatabase, IdbFactory, IdbObjectStore, IdbRequest, IdbTransactionMode,
    IdbVersionChangeEvent,
};

use crate::{
    conversation_key, message_key, CONVERSATION_STORE, MESSAGE_HISTORY_STORE, MESSAGE_STORE,
    OWNER_CONVERSATION_STORE,
};

const DB_VERSION: u32 = 2;

pub async fn migrate(db_name: &str) -> Result<()> {
    let db = open_database(db_name).await?;
    db.close();
    Ok(())
}

pub async fn get_app_sdk_version(db_name: &str) -> Result<Option<AppSdkVersion>> {
    get_json(db_name, APP_SDK_VERSION_TABLE, APP_SDK_VERSION_KEY).await
}

pub async fn set_app_sdk_version(db_name: &str, version: &AppSdkVersion) -> Result<()> {
    put_json(db_name, APP_SDK_VERSION_TABLE, APP_SDK_VERSION_KEY, version).await
}

pub async fn get_version_sync(db_name: &str, key: &str) -> Result<Option<VersionRecord>> {
    get_json(db_name, VERSION_SYNC_TABLE, key).await
}

pub async fn set_version_sync(db_name: &str, key: &str, record: &VersionRecord) -> Result<()> {
    put_json(db_name, VERSION_SYNC_TABLE, key, record).await
}

pub async fn delete_version_sync(db_name: &str, key: &str) -> Result<()> {
    delete_key(db_name, VERSION_SYNC_TABLE, key).await
}

pub async fn save_message(db_name: &str, message: ChatMessage) -> Result<()> {
    let key = message_key(&message.conversation_id, &message.client_msg_id)?;
    put_json(db_name, MESSAGE_STORE, &key, &message).await?;

    let mut history =
        get_json::<Vec<ChatMessage>>(db_name, MESSAGE_HISTORY_STORE, &message.conversation_id)
            .await?
            .unwrap_or_default();
    upsert_message(&mut history, message.clone());
    put_json(
        db_name,
        MESSAGE_HISTORY_STORE,
        &message.conversation_id,
        &history,
    )
    .await
}

pub async fn load_message(db_name: &str, key: &str) -> Result<Option<ChatMessage>> {
    get_json(db_name, MESSAGE_STORE, key).await
}

pub async fn load_history(
    db_name: &str,
    conversation_id: &str,
    pagination: Pagination,
) -> Result<Vec<ChatMessage>> {
    let mut history = get_json::<Vec<ChatMessage>>(db_name, MESSAGE_HISTORY_STORE, conversation_id)
        .await?
        .unwrap_or_default();
    history.sort_by(message_desc_order);
    Ok(paginate(history, pagination))
}

pub async fn save_conversation(db_name: &str, conversation: ConversationInfo) -> Result<()> {
    let key = conversation_key(&conversation.owner_user_id, &conversation.conversation_id)?;
    put_json(db_name, CONVERSATION_STORE, &key, &conversation).await?;

    let mut conversations = get_json::<Vec<ConversationInfo>>(
        db_name,
        OWNER_CONVERSATION_STORE,
        &conversation.owner_user_id,
    )
    .await?
    .unwrap_or_default();
    upsert_conversation(&mut conversations, conversation.clone());
    put_json(
        db_name,
        OWNER_CONVERSATION_STORE,
        &conversation.owner_user_id,
        &conversations,
    )
    .await
}

pub async fn remove_conversation(
    db_name: &str,
    key: &str,
    owner_user_id: &str,
    conversation_id: &str,
) -> Result<()> {
    delete_key(db_name, CONVERSATION_STORE, key).await?;

    let mut conversations =
        get_json::<Vec<ConversationInfo>>(db_name, OWNER_CONVERSATION_STORE, owner_user_id)
            .await?
            .unwrap_or_default();
    conversations.retain(|conversation| conversation.conversation_id != conversation_id);
    put_json(
        db_name,
        OWNER_CONVERSATION_STORE,
        owner_user_id,
        &conversations,
    )
    .await
}

pub async fn load_conversations(
    db_name: &str,
    owner_user_id: &str,
) -> Result<Vec<ConversationInfo>> {
    let mut conversations =
        get_json::<Vec<ConversationInfo>>(db_name, OWNER_CONVERSATION_STORE, owner_user_id)
            .await?
            .unwrap_or_default();
    conversations.sort_by(conversation_order);
    Ok(conversations)
}

pub async fn delete_database(db_name: &str) -> Result<()> {
    let factory = indexeddb_factory()?;
    let request = factory.delete_database(db_name).map_err(js_error)?;
    request_value(request.unchecked_into::<IdbRequest>()).await?;
    Ok(())
}

async fn get_json<T>(db_name: &str, store_name: &str, key: &str) -> Result<Option<T>>
where
    T: serde::de::DeserializeOwned,
{
    let db = open_database(db_name).await?;
    let store = object_store(&db, store_name, IdbTransactionMode::Readonly)?;
    let request = store.get(&JsValue::from_str(key)).map_err(js_error)?;
    let value = request_value(request).await?;
    db.close();

    if value.is_undefined() || value.is_null() {
        return Ok(None);
    }

    let json = value
        .as_string()
        .ok_or_else(|| OpenImError::sdk_internal("indexeddb value is not a JSON string"))?;
    serde_json::from_str(&json).map(Some).map_err(json_error)
}

async fn put_json<T>(db_name: &str, store_name: &str, key: &str, value: &T) -> Result<()>
where
    T: serde::Serialize,
{
    let db = open_database(db_name).await?;
    let store = object_store(&db, store_name, IdbTransactionMode::Readwrite)?;
    let json = serde_json::to_string(value).map_err(json_error)?;
    let request = store
        .put_with_key(&JsValue::from_str(&json), &JsValue::from_str(key))
        .map_err(js_error)?;
    request_value(request).await?;
    db.close();
    Ok(())
}

async fn delete_key(db_name: &str, store_name: &str, key: &str) -> Result<()> {
    let db = open_database(db_name).await?;
    let store = object_store(&db, store_name, IdbTransactionMode::Readwrite)?;
    let request = store.delete(&JsValue::from_str(key)).map_err(js_error)?;
    request_value(request).await?;
    db.close();
    Ok(())
}

async fn open_database(db_name: &str) -> Result<IdbDatabase> {
    let factory = indexeddb_factory()?;
    let request = factory
        .open_with_u32(db_name, DB_VERSION)
        .map_err(js_error)?;
    let upgrade_request = request.clone();
    let on_upgrade = Closure::wrap(Box::new(move |_event: IdbVersionChangeEvent| {
        let Ok(value) = upgrade_request.result() else {
            return;
        };
        let db = value.unchecked_into::<IdbDatabase>();
        let _ = ensure_store(&db, APP_SDK_VERSION_TABLE);
        let _ = ensure_store(&db, VERSION_SYNC_TABLE);
        let _ = ensure_store(&db, MESSAGE_STORE);
        let _ = ensure_store(&db, MESSAGE_HISTORY_STORE);
        let _ = ensure_store(&db, CONVERSATION_STORE);
        let _ = ensure_store(&db, OWNER_CONVERSATION_STORE);
    }) as Box<dyn FnMut(IdbVersionChangeEvent)>);

    request.set_onupgradeneeded(Some(on_upgrade.as_ref().unchecked_ref()));
    on_upgrade.forget();

    let value = request_value(request.unchecked_into::<IdbRequest>()).await?;
    value
        .dyn_into::<IdbDatabase>()
        .map_err(|_| OpenImError::sdk_internal("indexeddb open result is not a database"))
}

fn object_store(
    db: &IdbDatabase,
    store_name: &str,
    mode: IdbTransactionMode,
) -> Result<IdbObjectStore> {
    let transaction = db
        .transaction_with_str_and_mode(store_name, mode)
        .map_err(js_error)?;
    transaction.object_store(store_name).map_err(js_error)
}

fn ensure_store(db: &IdbDatabase, store_name: &str) -> std::result::Result<(), JsValue> {
    if !db.object_store_names().contains(store_name) {
        db.create_object_store(store_name)?;
    }

    Ok(())
}

fn indexeddb_factory() -> Result<IdbFactory> {
    let window = web_sys::window().ok_or_else(|| OpenImError::sdk_internal("window is missing"))?;
    window
        .indexed_db()
        .map_err(js_error)?
        .ok_or_else(|| OpenImError::sdk_internal("indexedDB is not available"))
}

async fn request_value(request: IdbRequest) -> Result<JsValue> {
    let promise = js_sys::Promise::new(&mut |resolve, reject| {
        let success_request = request.clone();
        let on_success = Closure::wrap(Box::new(move |_event: Event| {
            let value = success_request.result().unwrap_or(JsValue::UNDEFINED);
            let _ = resolve.call1(&JsValue::UNDEFINED, &value);
        }) as Box<dyn FnMut(Event)>);

        let error_request = request.clone();
        let on_error = Closure::wrap(Box::new(move |_event: Event| {
            let error = error_request
                .error()
                .ok()
                .flatten()
                .map(JsValue::from)
                .unwrap_or_else(|| JsValue::from_str("indexeddb request failed"));
            let _ = reject.call1(&JsValue::UNDEFINED, &error);
        }) as Box<dyn FnMut(Event)>);

        request.set_onsuccess(Some(on_success.as_ref().unchecked_ref()));
        request.set_onerror(Some(on_error.as_ref().unchecked_ref()));
        on_success.forget();
        on_error.forget();
    });

    JsFuture::from(promise).await.map_err(js_error)
}

fn js_error(value: JsValue) -> OpenImError {
    if let Some(message) = value.as_string() {
        return OpenImError::sdk_internal(format!("indexeddb error: {message}"));
    }

    OpenImError::sdk_internal("indexeddb error")
}

fn json_error(err: serde_json::Error) -> OpenImError {
    OpenImError::sdk_internal(format!("indexeddb json error: {err}"))
}

fn upsert_message(messages: &mut Vec<ChatMessage>, message: ChatMessage) {
    if let Some(existing) = messages
        .iter_mut()
        .find(|item| item.client_msg_id == message.client_msg_id)
    {
        *existing = message;
    } else {
        messages.push(message);
    }
}

fn upsert_conversation(conversations: &mut Vec<ConversationInfo>, conversation: ConversationInfo) {
    if let Some(existing) = conversations
        .iter_mut()
        .find(|item| item.conversation_id == conversation.conversation_id)
    {
        *existing = conversation;
    } else {
        conversations.push(conversation);
    }
}

fn message_desc_order(left: &ChatMessage, right: &ChatMessage) -> std::cmp::Ordering {
    right
        .seq
        .cmp(&left.seq)
        .then_with(|| right.send_time.cmp(&left.send_time))
        .then_with(|| right.client_msg_id.cmp(&left.client_msg_id))
}

fn conversation_order(left: &ConversationInfo, right: &ConversationInfo) -> std::cmp::Ordering {
    right
        .is_pinned
        .cmp(&left.is_pinned)
        .then_with(|| conversation_time(right).cmp(&conversation_time(left)))
        .then_with(|| left.conversation_id.cmp(&right.conversation_id))
}

fn conversation_time(conversation: &ConversationInfo) -> i64 {
    conversation
        .latest_msg_send_time
        .max(conversation.draft_text_time)
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
    use openim_domain::{
        conversation::ConversationInfo,
        message::{ChatMessage, MessageContent, MessageSnapshot},
    };
    use openim_storage_core::{
        version_sync_key, AsyncAppVersionStore, AsyncStorageMigrator, AsyncVersionStore,
    };
    use openim_types::{Pagination, SessionType};
    use wasm_bindgen_test::{wasm_bindgen_test, wasm_bindgen_test_configure};

    use crate::{delete_database, IndexedDbStorage};

    wasm_bindgen_test_configure!(run_in_browser);

    #[wasm_bindgen_test]
    async fn indexeddb_crud_round_trips_storage_records() {
        let db_name = "OpenIM_v3_wasm_crud_test";
        let _ = delete_database(db_name).await;
        let storage = IndexedDbStorage::with_db_name(db_name).unwrap();
        storage.migrate().await.unwrap();

        let version = openim_storage_core::AppSdkVersion::new("4.0.0", true);
        storage.set_app_sdk_version(&version).await.unwrap();
        assert_eq!(storage.get_app_sdk_version().await.unwrap(), Some(version));

        let mut record =
            openim_storage_core::VersionRecord::new("local_group_entities_version", "1076204769");
        record.version_id = "667aabe3417b67f0f0d3cdee".to_owned();
        record.version = 1076204769;
        record.uid_list = vec!["8879166186".to_owned(), "1695766238".to_owned()];

        storage.set_version_sync(&record).await.unwrap();
        assert_eq!(
            storage
                .get_version_sync("local_group_entities_version", "1076204769")
                .await
                .unwrap(),
            Some(record)
        );

        storage
            .delete_version_sync("local_group_entities_version", "1076204769")
            .await
            .unwrap();
        assert_eq!(
            storage
                .get_version_sync("local_group_entities_version", "1076204769")
                .await
                .unwrap(),
            None
        );
        delete_database(db_name).await.unwrap();
    }

    #[wasm_bindgen_test]
    async fn indexeddb_uses_stable_version_sync_key() {
        assert_eq!(
            version_sync_key("table", "entity").unwrap(),
            r#"["table","entity"]"#
        );
    }

    #[wasm_bindgen_test]
    async fn indexeddb_round_trips_message_and_conversation_records() {
        let db_name = "OpenIM_v3_wasm_message_conversation_test";
        let _ = delete_database(db_name).await;
        let storage = IndexedDbStorage::with_db_name(db_name).unwrap();
        storage.migrate().await.unwrap();

        let first = message("client-1", "server-1", 1, 100);
        let second = message("client-2", "server-2", 2, 200);
        storage.save_message(first.clone()).await.unwrap();
        storage.save_message(second.clone()).await.unwrap();

        assert_eq!(
            storage
                .load_message(&first.conversation_id, &first.client_msg_id)
                .await
                .unwrap(),
            Some(first.clone())
        );
        assert_eq!(
            storage
                .load_history(
                    &first.conversation_id,
                    Pagination {
                        page_number: 0,
                        show_number: 1,
                    },
                )
                .await
                .unwrap(),
            vec![second.clone()]
        );

        let mut conversation = ConversationInfo::from_message("u1", &second).unwrap();
        conversation.latest_message = Some(MessageSnapshot::from(&second));
        conversation.latest_msg_send_time = second.send_time;
        conversation.unread_count = 1;
        storage
            .save_conversation(conversation.clone())
            .await
            .unwrap();
        assert_eq!(
            storage.load_conversations("u1").await.unwrap(),
            vec![conversation.clone()]
        );

        storage
            .remove_conversation("u1", &conversation.conversation_id)
            .await
            .unwrap();
        assert!(storage.load_conversations("u1").await.unwrap().is_empty());
        delete_database(db_name).await.unwrap();
    }

    fn message(client_msg_id: &str, server_msg_id: &str, seq: i64, send_time: i64) -> ChatMessage {
        ChatMessage::incoming(
            client_msg_id,
            server_msg_id,
            "u2",
            "u1",
            SessionType::Single,
            MessageContent::Text {
                content: format!("hello-{seq}"),
            },
            seq,
            send_time,
        )
        .unwrap()
    }
}
