use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;

use openim_errors::{OpenImError, Result};
use serde::{Deserialize, Serialize};

pub const BIG_VERSION: &str = "v3";
pub const APP_SDK_VERSION_TABLE: &str = "local_app_sdk_version";
pub const VERSION_SYNC_TABLE: &str = "local_sync_version";
pub const APP_SDK_VERSION_KEY: &str = "app_sdk_version";

pub type LocalBoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppSdkVersion {
    pub version: String,
    pub installed: bool,
}

impl AppSdkVersion {
    pub fn new(version: impl Into<String>, installed: bool) -> Self {
        Self {
            version: version.into(),
            installed,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionRecord {
    #[serde(rename = "tableName")]
    pub table_name: String,
    #[serde(rename = "entityID")]
    pub entity_id: String,
    #[serde(rename = "versionID")]
    pub version_id: String,
    pub version: u64,
    #[serde(rename = "createTime")]
    pub create_time: i64,
    #[serde(rename = "uidList")]
    pub uid_list: Vec<String>,
}

impl VersionRecord {
    pub fn new(table_name: impl Into<String>, entity_id: impl Into<String>) -> Self {
        Self {
            table_name: table_name.into(),
            entity_id: entity_id.into(),
            version_id: String::new(),
            version: 0,
            create_time: 0,
            uid_list: Vec::new(),
        }
    }
}

pub trait StorageMigrator {
    fn migrate(&self) -> Result<()>;
}

pub trait AppVersionStore {
    fn get_app_sdk_version(&self) -> Result<Option<AppSdkVersion>>;
    fn set_app_sdk_version(&self, version: &AppSdkVersion) -> Result<()>;
}

pub trait VersionStore {
    fn get_version_sync(&self, table_name: &str, entity_id: &str) -> Result<Option<VersionRecord>>;
    fn set_version_sync(&self, record: &VersionRecord) -> Result<()>;
    fn delete_version_sync(&self, table_name: &str, entity_id: &str) -> Result<()>;
}

pub trait AsyncStorageMigrator {
    fn migrate(&self) -> LocalBoxFuture<'_, Result<()>>;
}

pub trait AsyncAppVersionStore {
    fn get_app_sdk_version(&self) -> LocalBoxFuture<'_, Result<Option<AppSdkVersion>>>;
    fn set_app_sdk_version<'a>(
        &'a self,
        version: &'a AppSdkVersion,
    ) -> LocalBoxFuture<'a, Result<()>>;
}

pub trait AsyncVersionStore {
    fn get_version_sync<'a>(
        &'a self,
        table_name: &'a str,
        entity_id: &'a str,
    ) -> LocalBoxFuture<'a, Result<Option<VersionRecord>>>;
    fn set_version_sync<'a>(&'a self, record: &'a VersionRecord) -> LocalBoxFuture<'a, Result<()>>;
    fn delete_version_sync<'a>(
        &'a self,
        table_name: &'a str,
        entity_id: &'a str,
    ) -> LocalBoxFuture<'a, Result<()>>;
}

pub fn openim_db_file(db_dir: impl AsRef<Path>, login_user_id: &str) -> Result<PathBuf> {
    if login_user_id.is_empty() {
        return Err(OpenImError::args("login_user_id is empty"));
    }

    let path = db_dir
        .as_ref()
        .join(format!("OpenIM_{BIG_VERSION}_{login_user_id}.db"));
    if path.is_absolute() {
        return Ok(path);
    }

    let cwd = std::env::current_dir()
        .map_err(|err| OpenImError::sdk_internal(format!("read current dir failed: {err}")))?;
    Ok(cwd.join(path))
}

pub fn openim_indexeddb_name(login_user_id: &str) -> Result<String> {
    if login_user_id.is_empty() {
        return Err(OpenImError::args("login_user_id is empty"));
    }

    Ok(format!("OpenIM_{BIG_VERSION}_{login_user_id}"))
}

pub fn version_sync_key(table_name: &str, entity_id: &str) -> Result<String> {
    if table_name.is_empty() {
        return Err(OpenImError::args("table_name is empty"));
    }
    if entity_id.is_empty() {
        return Err(OpenImError::args("entity_id is empty"));
    }

    serde_json::to_string(&(table_name, entity_id))
        .map_err(|err| OpenImError::sdk_internal(format!("encode version sync key failed: {err}")))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn db_file_matches_go_layout() {
        let cwd = std::env::current_dir().unwrap();
        assert_eq!(
            openim_db_file("db", "1695766238").unwrap(),
            cwd.join(PathBuf::from("db").join("OpenIM_v3_1695766238.db"))
        );
    }

    #[test]
    fn indexeddb_name_matches_openim_user_scope() {
        assert_eq!(
            openim_indexeddb_name("1695766238").unwrap(),
            "OpenIM_v3_1695766238"
        );
        assert!(openim_indexeddb_name("").is_err());
    }

    #[test]
    fn version_sync_key_is_stable_json_tuple() {
        assert_eq!(
            version_sync_key("local_group_entities_version", "1076204769").unwrap(),
            r#"["local_group_entities_version","1076204769"]"#
        );
    }

    #[test]
    fn version_record_json_matches_go_tags() {
        let record = VersionRecord {
            table_name: "local_group_entities_version".to_owned(),
            entity_id: "1076204769".to_owned(),
            version_id: "667aabe3417b67f0f0d3cdee".to_owned(),
            version: 1076204769,
            create_time: 0,
            uid_list: vec!["8879166186".to_owned(), "1695766238".to_owned()],
        };

        let json = serde_json::to_string(&record).unwrap();

        assert!(json.contains("\"tableName\""));
        assert!(json.contains("\"entityID\""));
        assert!(json.contains("\"versionID\""));
        assert!(json.contains("\"createTime\""));
        assert!(json.contains("\"uidList\""));
    }
}
