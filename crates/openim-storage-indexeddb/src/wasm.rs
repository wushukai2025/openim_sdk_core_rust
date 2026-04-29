use openim_errors::{OpenImError, Result};
use openim_storage_core::{
    AppSdkVersion, VersionRecord, APP_SDK_VERSION_KEY, APP_SDK_VERSION_TABLE, VERSION_SYNC_TABLE,
};
use wasm_bindgen::closure::Closure;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    Event, IdbDatabase, IdbFactory, IdbObjectStore, IdbRequest, IdbTransactionMode,
    IdbVersionChangeEvent,
};

const DB_VERSION: u32 = 1;

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

#[cfg(test)]
mod tests {
    use openim_storage_core::{
        version_sync_key, AsyncAppVersionStore, AsyncStorageMigrator, AsyncVersionStore,
    };
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
}
