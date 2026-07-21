use crate::error::AppError;

pub const CURRENT_SCHEMA_VERSION: u32 = 3;

pub fn ensure_compatible_schema(conn: &duckdb::Connection) -> Result<(), AppError> {
    match crate::index::schema::read_schema_version(conn)? {
        Some(CURRENT_SCHEMA_VERSION) | None => Ok(()),
        Some(1) => {
            migrate_version_one_to_two(conn)?;
            migrate_version_two_to_three(conn)
        }
        Some(2) => migrate_version_two_to_three(conn),
        Some(version) => Err(AppError::SchemaIncompatible(format!(
            "当前支持 schema_version={CURRENT_SCHEMA_VERSION}，数据库为 {version}"
        ))),
    }
}

fn migrate_version_two_to_three(conn: &duckdb::Connection) -> Result<(), AppError> {
    conn.execute_batch("begin transaction")
        .map_err(|error| AppError::DuckDb(error.to_string()))?;
    let result = (|| {
        conn.execute_batch(
            "alter table analysis_snapshots add column if not exists last_selected_path text",
        )
        .map_err(|error| AppError::DuckDb(error.to_string()))?;
        conn.execute(
            "insert or replace into metadata (key, value) values ('schema_version', '3')",
            [],
        )
        .map_err(|error| AppError::DuckDb(error.to_string()))?;
        Ok(())
    })();

    match result {
        Ok(()) => {
            if let Err(error) = conn.execute_batch("commit") {
                let _ = conn.execute_batch("rollback");
                return Err(AppError::DuckDb(error.to_string()));
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
        .map_err(|error| AppError::DuckDb(error.to_string()))?;
    let result = (|| {
        crate::index::schema::initialize_cache_schema(conn)?;
        conn.execute(
            "insert or replace into metadata (key, value) values ('schema_version', '2')",
            [],
        )
        .map_err(|error| AppError::DuckDb(error.to_string()))?;
        Ok(())
    })();

    match result {
        Ok(()) => {
            if let Err(error) = conn.execute_batch("commit") {
                let _ = conn.execute_batch("rollback");
                return Err(AppError::DuckDb(error.to_string()));
            }
            Ok(())
        }
        Err(error) => {
            let _ = conn.execute_batch("rollback");
            Err(error)
        }
    }
}
