use std::time::{SystemTime, UNIX_EPOCH};

use openim_storage_core::{AppVersionStore, VersionStore};
use openim_storage_sqlite::SqliteStorage;
use rusqlite::params;

#[test]
fn reads_go_compatible_existing_version_database() {
    let path = temp_db_path("go-existing");
    create_go_compatible_db(&path);

    let storage = SqliteStorage::open(&path).unwrap();
    let app_version = storage.get_app_sdk_version().unwrap().unwrap();
    let version = storage
        .get_version_sync("local_group_entities_version", "1076204769")
        .unwrap()
        .unwrap();

    assert_eq!(app_version.version, "3.8.0");
    assert!(!app_version.installed);
    assert_eq!(version.version_id, "667aabe3417b67f0f0d3cdee");
    assert_eq!(version.version, 1076204769);
    assert_eq!(
        version.uid_list,
        vec![
            "8879166186".to_owned(),
            "1695766238".to_owned(),
            "2882899447".to_owned()
        ]
    );

    let _ = std::fs::remove_file(path);
}

fn create_go_compatible_db(path: &std::path::Path) {
    let conn = rusqlite::Connection::open(path).unwrap();
    conn.execute_batch(
        r#"
        CREATE TABLE local_app_sdk_version (
            version varchar(255) PRIMARY KEY NOT NULL,
            installed boolean
        );

        CREATE TABLE local_sync_version (
            table_name varchar(255) NOT NULL,
            entity_id varchar(255) NOT NULL,
            version_id text,
            version integer,
            create_time integer,
            id_list text,
            PRIMARY KEY (table_name, entity_id)
        );
        "#,
    )
    .unwrap();

    let uid_list = serde_json::to_string(&vec!["8879166186", "1695766238", "2882899447"]).unwrap();
    conn.execute(
        "INSERT INTO local_app_sdk_version (version, installed) VALUES (?1, ?2)",
        params!["3.8.0", false],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO local_sync_version \
         (table_name, entity_id, version_id, version, create_time, id_list) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            "local_group_entities_version",
            "1076204769",
            "667aabe3417b67f0f0d3cdee",
            1076204769_i64,
            0_i64,
            uid_list
        ],
    )
    .unwrap();
}

fn temp_db_path(name: &str) -> std::path::PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("openim-{name}-{nanos}.db"))
}
