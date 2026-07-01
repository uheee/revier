use crate::error::AppError;

pub const CURRENT_SCHEMA_VERSION: u32 = 1;

pub fn ensure_compatible_schema(conn: &duckdb::Connection) -> Result<(), AppError> {
    match crate::index::schema::read_schema_version(conn)? {
        Some(CURRENT_SCHEMA_VERSION) | None => Ok(()),
        Some(version) => Err(AppError::SchemaIncompatible(format!(
            "当前支持 schema_version={CURRENT_SCHEMA_VERSION}，数据库为 {version}"
        ))),
    }
}
