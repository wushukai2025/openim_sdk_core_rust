use std::path::Path;

use openim_errors::{OpenImError, Result};
use openim_storage_core::{
    AppSdkVersion, AppVersionStore, StorageMigrator, VersionRecord, VersionStore,
    APP_SDK_VERSION_TABLE, VERSION_SYNC_TABLE,
};
use rusqlite::types::Value;
use rusqlite::{params, Connection, OptionalExtension};

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

fn sqlite_error(err: rusqlite::Error) -> OpenImError {
    OpenImError::sdk_internal(format!("sqlite error: {err}"))
}

fn json_error(err: serde_json::Error) -> OpenImError {
    OpenImError::sdk_internal(format!("sqlite json error: {err}"))
}

#[cfg(test)]
mod tests {
    use openim_storage_core::{
        AppVersionStore, StorageMigrator, VersionStore, APP_SDK_VERSION_TABLE, VERSION_SYNC_TABLE,
    };
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
}
