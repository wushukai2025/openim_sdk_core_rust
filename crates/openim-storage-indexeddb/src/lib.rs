#[cfg(not(target_arch = "wasm32"))]
use openim_errors::ErrorCode;
use openim_errors::{OpenImError, Result};
use openim_storage_core::{
    openim_indexeddb_name, version_sync_key, AppSdkVersion, AsyncAppVersionStore,
    AsyncStorageMigrator, AsyncVersionStore, LocalBoxFuture, VersionRecord,
};

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
}
