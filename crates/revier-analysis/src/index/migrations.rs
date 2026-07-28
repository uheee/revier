use crate::error::AppError;
use chrono::Utc;
use include_dir::{include_dir, Dir};
use sha2::{Digest, Sha256};
use std::collections::HashSet;

pub const CURRENT_SCHEMA_VERSION: u32 = 4;

const LEGACY_BASELINE_VERSION: u32 = 3;
const LEGACY_BASELINE_NAME: &str = "legacy_schema_v3";
const LEGACY_BASELINE_CHECKSUM: &str = "legacy-schema-v3";
static MIGRATIONS_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/migrations");

#[derive(Debug, Clone, PartialEq, Eq)]
struct EmbeddedMigration {
    version: u32,
    name: String,
    sql: &'static [u8],
    checksum: String,
}

pub fn ensure_compatible_schema(conn: &duckdb::Connection) -> Result<(), AppError> {
    match crate::index::schema::read_schema_version(conn)? {
        Some(CURRENT_SCHEMA_VERSION) => validate_current_migration_history(conn),
        None => Ok(()),
        Some(1) => {
            migrate_version_one_to_two(conn)?;
            migrate_version_two_to_three(conn)?;
            apply_embedded_migrations(conn, LEGACY_BASELINE_VERSION)
        }
        Some(2) => {
            migrate_version_two_to_three(conn)?;
            apply_embedded_migrations(conn, LEGACY_BASELINE_VERSION)
        }
        Some(LEGACY_BASELINE_VERSION) => apply_embedded_migrations(conn, LEGACY_BASELINE_VERSION),
        Some(version) => Err(AppError::SchemaIncompatible(format!(
            "当前支持 schema_version={CURRENT_SCHEMA_VERSION}，数据库为 {version}"
        ))),
    }
}

pub(crate) fn initialize_current_migration_history(
    conn: &duckdb::Connection,
) -> Result<(), AppError> {
    let migrations = embedded_migrations()?;
    let Some(history_migration) = migrations
        .iter()
        .find(|migration| migration.version == CURRENT_SCHEMA_VERSION)
    else {
        return Err(AppError::SchemaIncompatible(format!(
            "缺少 schema_version={CURRENT_SCHEMA_VERSION} 的迁移文件"
        )));
    };

    execute_migration_sql(conn, history_migration)?;
    insert_migration_history(
        conn,
        LEGACY_BASELINE_VERSION,
        LEGACY_BASELINE_NAME,
        LEGACY_BASELINE_CHECKSUM,
        "baseline",
    )?;
    insert_migration_history(
        conn,
        history_migration.version,
        &history_migration.name,
        &history_migration.checksum,
        "migration",
    )
}

fn apply_embedded_migrations(conn: &duckdb::Connection, from_version: u32) -> Result<(), AppError> {
    let migrations = embedded_migrations()?;
    for migration in migrations
        .iter()
        .filter(|migration| migration.version > from_version)
    {
        conn.execute_batch("begin transaction")
            .map_err(AppError::DuckDb)?;
        let result = (|| {
            execute_migration_sql(conn, migration)?;
            if from_version == LEGACY_BASELINE_VERSION
                && migration.version == CURRENT_SCHEMA_VERSION
                && !migration_history_contains(conn, LEGACY_BASELINE_VERSION)?
            {
                insert_migration_history(
                    conn,
                    LEGACY_BASELINE_VERSION,
                    LEGACY_BASELINE_NAME,
                    LEGACY_BASELINE_CHECKSUM,
                    "baseline",
                )?;
            }
            validate_existing_checksum(conn, migration)?;
            insert_migration_history(
                conn,
                migration.version,
                &migration.name,
                &migration.checksum,
                "migration",
            )?;
            conn.execute(
                "insert or replace into metadata (key, value) values ('schema_version', ?)",
                [migration.version.to_string()],
            )
            .map_err(AppError::DuckDb)?;
            Ok(())
        })();

        match result {
            Ok(()) => {
                if let Err(error) = conn.execute_batch("commit") {
                    let _ = conn.execute_batch("rollback");
                    return Err(AppError::DuckDb(error));
                }
            }
            Err(error) => {
                let _ = conn.execute_batch("rollback");
                return Err(error);
            }
        }
    }
    Ok(())
}

fn validate_current_migration_history(conn: &duckdb::Connection) -> Result<(), AppError> {
    if !table_exists(conn, "schema_migrations")? {
        return Err(AppError::SchemaIncompatible(
            "缺少 schema_migrations 迁移历史表".to_string(),
        ));
    }

    if let Some(version) = max_migration_history_version(conn)? {
        if version > CURRENT_SCHEMA_VERSION {
            return Err(AppError::SchemaIncompatible(format!(
                "迁移历史包含高于当前程序的版本：{version}"
            )));
        }
    }

    validate_baseline_checksum(conn)?;
    for migration in embedded_migrations()? {
        if migration.version <= CURRENT_SCHEMA_VERSION {
            validate_existing_checksum(conn, &migration)?;
        }
    }

    if !migration_history_contains(conn, CURRENT_SCHEMA_VERSION)? {
        return Err(AppError::SchemaIncompatible(format!(
            "迁移历史缺少当前版本：{CURRENT_SCHEMA_VERSION}"
        )));
    }
    Ok(())
}

fn table_exists(conn: &duckdb::Connection, table_name: &str) -> Result<bool, AppError> {
    let count: i64 = conn
        .query_row(
            "select count(*) from information_schema.tables where table_name = ?",
            [table_name],
            |row| row.get(0),
        )
        .map_err(AppError::DuckDb)?;
    Ok(count > 0)
}

fn max_migration_history_version(conn: &duckdb::Connection) -> Result<Option<u32>, AppError> {
    let version = conn
        .query_row("select max(version) from schema_migrations", [], |row| {
            row.get::<_, Option<i64>>(0)
        })
        .map_err(AppError::DuckDb)?;
    version
        .map(|value| {
            u32::try_from(value).map_err(|error| {
                AppError::SchemaIncompatible(format!("迁移历史版本无法解析：{error}"))
            })
        })
        .transpose()
}

fn validate_baseline_checksum(conn: &duckdb::Connection) -> Result<(), AppError> {
    let Some(checksum) = migration_checksum_for_version(conn, LEGACY_BASELINE_VERSION)? else {
        return Err(AppError::SchemaIncompatible(format!(
            "迁移历史缺少 baseline 版本：{LEGACY_BASELINE_VERSION}"
        )));
    };
    if checksum != LEGACY_BASELINE_CHECKSUM {
        return Err(AppError::SchemaIncompatible(format!(
            "baseline {LEGACY_BASELINE_VERSION} checksum 不一致"
        )));
    }
    Ok(())
}

fn embedded_migrations() -> Result<Vec<EmbeddedMigration>, AppError> {
    let entries = MIGRATIONS_DIR
        .files()
        .map(|file| (file.path().to_string_lossy().to_string(), file.contents()))
        .collect::<Vec<_>>();
    migrations_from_entries(entries)
}

fn migrations_from_entries<I, P>(entries: I) -> Result<Vec<EmbeddedMigration>, AppError>
where
    I: IntoIterator<Item = (P, &'static [u8])>,
    P: AsRef<str>,
{
    let mut versions = HashSet::new();
    let mut migrations = entries
        .into_iter()
        .map(|(path, sql)| {
            let file_name = path
                .as_ref()
                .rsplit(['/', '\\'])
                .next()
                .unwrap_or(path.as_ref());
            let (version, name) = parse_migration_file_name(file_name)?;
            if !versions.insert(version) {
                return Err(AppError::SchemaIncompatible(format!(
                    "迁移版本重复：{version}"
                )));
            }
            Ok(EmbeddedMigration {
                version,
                name,
                sql,
                checksum: migration_checksum(sql),
            })
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    migrations.sort_by_key(|migration| migration.version);
    Ok(migrations)
}

fn parse_migration_file_name(file_name: &str) -> Result<(u32, String), AppError> {
    let Some(stem) = file_name.strip_suffix(".sql") else {
        return Err(AppError::SchemaIncompatible(format!(
            "迁移文件扩展名无效：{file_name}"
        )));
    };
    let Some((version, name)) = stem.split_once('_') else {
        return Err(AppError::SchemaIncompatible(format!(
            "迁移文件名格式无效：{file_name}"
        )));
    };
    if version.len() != 4 || !version.chars().all(|value| value.is_ascii_digit()) {
        return Err(AppError::SchemaIncompatible(format!(
            "迁移版本格式无效：{file_name}"
        )));
    }
    if name.is_empty()
        || !name
            .chars()
            .all(|value| value.is_ascii_lowercase() || value.is_ascii_digit() || value == '_')
    {
        return Err(AppError::SchemaIncompatible(format!(
            "迁移名称格式无效：{file_name}"
        )));
    }
    let version = version.parse::<u32>().map_err(|error| {
        AppError::SchemaIncompatible(format!("迁移版本无法解析：{file_name}：{error}"))
    })?;
    Ok((version, name.to_string()))
}

fn migration_checksum(sql: &[u8]) -> String {
    let digest = Sha256::digest(sql);
    hex::encode(digest)
}

fn execute_migration_sql(
    conn: &duckdb::Connection,
    migration: &EmbeddedMigration,
) -> Result<(), AppError> {
    let sql = std::str::from_utf8(migration.sql).map_err(|error| {
        AppError::SchemaIncompatible(format!(
            "迁移 {} 不是有效 UTF-8：{error}",
            migration.version
        ))
    })?;
    conn.execute_batch(sql).map_err(AppError::DuckDb)
}

fn insert_migration_history(
    conn: &duckdb::Connection,
    version: u32,
    name: &str,
    checksum: &str,
    kind: &str,
) -> Result<(), AppError> {
    if let Some(existing) = migration_checksum_for_version(conn, version)? {
        if existing != checksum {
            return Err(AppError::SchemaIncompatible(format!(
                "迁移 {version} checksum 不一致"
            )));
        }
        return Ok(());
    }

    conn.execute(
        "insert into schema_migrations (version, name, checksum, applied_at, kind)
         values (?, ?, ?, ?, ?)",
        duckdb::params![version, name, checksum, Utc::now().to_rfc3339(), kind],
    )
    .map_err(AppError::DuckDb)?;
    Ok(())
}

fn validate_existing_checksum(
    conn: &duckdb::Connection,
    migration: &EmbeddedMigration,
) -> Result<(), AppError> {
    let existing = migration_checksum_for_version(conn, migration.version)?;
    if existing
        .as_deref()
        .is_some_and(|value| value != migration.checksum)
    {
        return Err(AppError::SchemaIncompatible(format!(
            "迁移 {} checksum 不一致",
            migration.version
        )));
    }
    Ok(())
}

fn migration_history_contains(conn: &duckdb::Connection, version: u32) -> Result<bool, AppError> {
    Ok(migration_checksum_for_version(conn, version)?.is_some())
}

fn migration_checksum_for_version(
    conn: &duckdb::Connection,
    version: u32,
) -> Result<Option<String>, AppError> {
    match conn.query_row(
        "select checksum from schema_migrations where version = ?",
        [version],
        |row| row.get::<_, String>(0),
    ) {
        Ok(value) => Ok(Some(value)),
        Err(duckdb::Error::QueryReturnedNoRows) => Ok(None),
        Err(error) => Err(AppError::DuckDb(error)),
    }
}

#[cfg(test)]
fn migration_history(conn: &duckdb::Connection) -> Result<Vec<(u32, String)>, AppError> {
    let mut stmt = conn
        .prepare("select version, name from schema_migrations order by version")
        .map_err(AppError::DuckDb)?;
    let rows = stmt
        .query_map([], |row| {
            Ok((row.get::<_, u32>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(AppError::DuckDb)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(AppError::DuckDb)
}

fn migrate_version_two_to_three(conn: &duckdb::Connection) -> Result<(), AppError> {
    conn.execute_batch("begin transaction")
        .map_err(|error| AppError::DuckDb(error))?;
    let result = (|| {
        conn.execute_batch(
            "alter table analysis_snapshots add column if not exists last_selected_path text",
        )
        .map_err(|error| AppError::DuckDb(error))?;
        conn.execute(
            "insert or replace into metadata (key, value) values ('schema_version', '3')",
            [],
        )
        .map_err(|error| AppError::DuckDb(error))?;
        Ok(())
    })();

    match result {
        Ok(()) => {
            if let Err(error) = conn.execute_batch("commit") {
                let _ = conn.execute_batch("rollback");
                return Err(AppError::DuckDb(error));
            }
            Ok(())
        }
        Err(error) => {
            let _ = conn.execute_batch("rollback");
            Err(error)
        }
    }
}

fn migrate_version_one_to_two(conn: &duckdb::Connection) -> Result<(), AppError> {
    conn.execute_batch("begin transaction")
        .map_err(|error| AppError::DuckDb(error))?;
    let result = (|| {
        crate::index::schema::initialize_cache_schema(conn)?;
        conn.execute(
            "insert or replace into metadata (key, value) values ('schema_version', '2')",
            [],
        )
        .map_err(|error| AppError::DuckDb(error))?;
        Ok(())
    })();

    match result {
        Ok(()) => {
            if let Err(error) = conn.execute_batch("commit") {
                let _ = conn.execute_batch("rollback");
                return Err(AppError::DuckDb(error));
            }
            Ok(())
        }
        Err(error) => {
            let _ = conn.execute_batch("rollback");
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn migration_file_name_requires_numeric_version_and_snake_name() {
        let parsed =
            parse_migration_file_name("0010_add_schema_history.sql").expect("解析迁移文件名");

        assert_eq!(parsed, (10, "add_schema_history".to_string()));
        assert!(parse_migration_file_name("10_add_schema_history.sql").is_err());
        assert!(parse_migration_file_name("0010-add-schema-history.sql").is_err());
        assert!(parse_migration_file_name("0010_add_schema_history.txt").is_err());
    }

    #[test]
    fn migration_manifest_sorts_by_numeric_version_and_rejects_duplicates() {
        let migrations = migrations_from_entries(vec![
            ("0010_late.sql", b"select 10".as_slice()),
            ("0004_first.sql", b"select 4".as_slice()),
        ])
        .expect("解析迁移清单");

        assert_eq!(
            migrations
                .iter()
                .map(|migration| migration.version)
                .collect::<Vec<_>>(),
            vec![4, 10]
        );
        assert!(migrations_from_entries(vec![
            ("0004_first.sql", b"select 4".as_slice()),
            ("0004_duplicate.sql", b"select 4 again".as_slice()),
        ])
        .is_err());
    }

    #[test]
    fn migration_checksum_uses_original_bytes() {
        let checksum = migration_checksum(b"select 1\n");

        assert_eq!(checksum, migration_checksum(b"select 1\n"));
        assert_ne!(checksum, migration_checksum(b"select 1\r\n"));
    }

    #[test]
    fn version_three_database_is_adopted_with_baseline_and_migration_history() {
        let dir = tempdir().expect("创建临时目录");
        let db_path = dir.path().join("index.duckdb");
        let conn = duckdb::Connection::open(db_path).expect("打开 DuckDB");
        crate::index::schema::initialize_schema(&conn, "repo-1", "E:/repo/app", "E:/repo/app/.git")
            .expect("初始化当前 schema");

        ensure_compatible_schema(&conn).expect("确认 schema 兼容");

        assert_eq!(
            crate::index::schema::read_schema_version(&conn).expect("读取 schema 版本"),
            Some(4)
        );
        let rows: Vec<(u32, String)> = migration_history(&conn).expect("读取迁移历史");
        assert!(rows.contains(&(3, "legacy_schema_v3".to_string())));
        assert!(rows.contains(&(4, "create_migration_history".to_string())));
    }

    #[test]
    fn current_database_rejects_changed_migration_checksum() {
        let dir = tempdir().expect("创建临时目录");
        let db_path = dir.path().join("index.duckdb");
        let conn = duckdb::Connection::open(db_path).expect("打开 DuckDB");
        crate::index::schema::initialize_schema(&conn, "repo-1", "E:/repo/app", "E:/repo/app/.git")
            .expect("初始化当前 schema");
        conn.execute(
            "update schema_migrations set checksum = 'bad-checksum' where version = 4",
            [],
        )
        .expect("篡改迁移 checksum");

        let result = ensure_compatible_schema(&conn);

        assert!(matches!(result, Err(AppError::SchemaIncompatible(_))));
    }

    #[test]
    fn current_database_rejects_future_migration_history() {
        let dir = tempdir().expect("创建临时目录");
        let db_path = dir.path().join("index.duckdb");
        let conn = duckdb::Connection::open(db_path).expect("打开 DuckDB");
        crate::index::schema::initialize_schema(&conn, "repo-1", "E:/repo/app", "E:/repo/app/.git")
            .expect("初始化当前 schema");
        conn.execute(
            "insert into schema_migrations (version, name, checksum, applied_at, kind)
             values (999, 'future', 'future-checksum', '2026-07-29T00:00:00Z', 'migration')",
            [],
        )
        .expect("写入未来迁移历史");

        let result = ensure_compatible_schema(&conn);

        assert!(matches!(result, Err(AppError::SchemaIncompatible(_))));
    }
}
