use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use openim_storage_core::{
    AppSdkVersion, AppVersionStore, StorageMigrator, VersionRecord, VersionStore,
};
use openim_storage_sqlite::SqliteStorage;

const FIXTURE_TABLE: &str = "local_group_entities_version";
const FIXTURE_ENTITY: &str = "1076204769";
const FIXTURE_VERSION_ID: &str = "667aabe3417b67f0f0d3cdee";

#[derive(Debug, Parser)]
#[command(about = "OpenIM Rust storage fixture tool")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    WriteSqlite {
        #[arg(long)]
        path: PathBuf,
        #[arg(long, default_value = "4.0.0")]
        sdk_version: String,
    },
    VerifySqlite {
        #[arg(long)]
        path: PathBuf,
    },
    MigrateSqlite {
        #[arg(long)]
        path: PathBuf,
        #[arg(long, default_value = "4.0.0")]
        sdk_version: String,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::WriteSqlite { path, sdk_version } => write_sqlite_fixture(&path, &sdk_version),
        Command::VerifySqlite { path } => verify_sqlite_fixture(&path),
        Command::MigrateSqlite { path, sdk_version } => migrate_sqlite_fixture(&path, &sdk_version),
    }
}

fn write_sqlite_fixture(path: &Path, sdk_version: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("create fixture dir failed: {}", parent.display()))?;
    }

    let storage = SqliteStorage::open(path)
        .with_context(|| format!("open sqlite fixture failed: {}", path.display()))?;
    storage.migrate().map_err(anyhow::Error::msg)?;
    storage
        .set_app_sdk_version(&AppSdkVersion::new(sdk_version, false))
        .map_err(anyhow::Error::msg)?;
    storage
        .set_version_sync(&fixture_version_record())
        .map_err(anyhow::Error::msg)?;

    Ok(())
}

fn verify_sqlite_fixture(path: &Path) -> Result<()> {
    let storage = SqliteStorage::open(path)
        .with_context(|| format!("open sqlite fixture failed: {}", path.display()))?;
    let app_version = storage
        .get_app_sdk_version()
        .map_err(anyhow::Error::msg)?
        .context("app sdk version missing")?;
    let version = storage
        .get_version_sync(FIXTURE_TABLE, FIXTURE_ENTITY)
        .map_err(anyhow::Error::msg)?
        .context("version sync fixture missing")?;

    anyhow::ensure!(!app_version.version.is_empty(), "app sdk version is empty");
    anyhow::ensure!(
        version == fixture_version_record(),
        "version sync fixture mismatch"
    );
    Ok(())
}

fn migrate_sqlite_fixture(path: &Path, sdk_version: &str) -> Result<()> {
    let storage = SqliteStorage::open(path)
        .with_context(|| format!("open sqlite fixture failed: {}", path.display()))?;
    storage.migrate().map_err(anyhow::Error::msg)?;
    storage
        .set_app_sdk_version(&AppSdkVersion::new(sdk_version, false))
        .map_err(anyhow::Error::msg)
}

fn fixture_version_record() -> VersionRecord {
    VersionRecord {
        table_name: FIXTURE_TABLE.to_owned(),
        entity_id: FIXTURE_ENTITY.to_owned(),
        version_id: FIXTURE_VERSION_ID.to_owned(),
        version: 1076204769,
        create_time: 0,
        uid_list: vec![
            "8879166186".to_owned(),
            "1695766238".to_owned(),
            "2882899447".to_owned(),
        ],
    }
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn sqlite_fixture_round_trips_and_migrates_version() {
        let path = temp_db_path("fixture-tool");

        write_sqlite_fixture(&path, "3.8.0").unwrap();
        verify_sqlite_fixture(&path).unwrap();
        migrate_sqlite_fixture(&path, "4.0.0").unwrap();

        let storage = SqliteStorage::open(&path).unwrap();
        assert_eq!(
            storage.get_app_sdk_version().unwrap(),
            Some(AppSdkVersion::new("4.0.0", false))
        );

        let _ = std::fs::remove_file(path);
    }

    fn temp_db_path(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("openim-{name}-{nanos}.db"))
    }
}
