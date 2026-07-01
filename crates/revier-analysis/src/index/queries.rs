use crate::error::AppError;

pub struct IndexStatusRecord {
    pub indexed_commit_count: u64,
    pub indexed_file_count: u64,
    pub updated_at: Option<String>,
}

pub fn status_record(conn: &duckdb::Connection) -> Result<IndexStatusRecord, AppError> {
    let indexed_commit_count =
        conn.query_row("select count(*) from commits", [], |row| {
            row.get::<_, i64>(0)
        })
        .map_err(|error| AppError::DuckDb(error.to_string()))? as u64;
    let indexed_file_count = conn
        .query_row("select count(*) from commit_files", [], |row| {
            row.get::<_, i64>(0)
        })
        .map_err(|error| AppError::DuckDb(error.to_string()))? as u64;
    let updated_at = conn
        .query_row(
            "select value from metadata where key = 'updated_at'",
            [],
            |row| row.get::<_, String>(0),
        )
        .ok();

    Ok(IndexStatusRecord {
        indexed_commit_count,
        indexed_file_count,
        updated_at,
    })
}
