use crate::error::AppError;
use crate::index::migrations::CURRENT_SCHEMA_VERSION;
use chrono::Utc;
use duckdb::params;

pub fn initialize_schema(
    conn: &duckdb::Connection,
    repo_id: &str,
    repo_root: &str,
    git_common_dir: &str,
) -> Result<(), AppError> {
    conn.execute_batch(
        "
        create table if not exists metadata (
          key text primary key,
          value text not null
        );
        create table if not exists commits (
          hash text primary key,
          short_hash text not null,
          author_name text not null,
          author_email text,
          author_key text not null,
          committed_at timestamp not null,
          subject text not null,
          parent_count integer not null,
          is_merge boolean not null
        );
        create table if not exists commit_parents (
          commit_hash text not null,
          parent_hash text not null,
          parent_index integer not null,
          primary key (commit_hash, parent_index)
        );
        create table if not exists commit_files (
          commit_hash text not null,
          parent_hash text not null,
          parent_index integer not null,
          path text not null,
          old_path text not null,
          status text not null,
          additions integer not null,
          deletions integer not null,
          is_binary boolean not null,
          is_previewable boolean not null,
          similarity real,
          primary key (commit_hash, parent_index, path, old_path)
        );
        create table if not exists index_runs (
          run_id text primary key,
          repo_id text not null,
          started_at timestamp not null,
          finished_at timestamp,
          status text not null,
          indexed_commit_count integer not null,
          indexed_file_count integer not null,
          elapsed_ms integer,
          error_message text
        );
        ",
    )
    .map_err(|error| AppError::DuckDb(error.to_string()))?;

    let now = Utc::now().to_rfc3339();
    upsert_metadata(conn, "schema_version", &CURRENT_SCHEMA_VERSION.to_string())?;
    upsert_metadata(conn, "repo_id", repo_id)?;
    upsert_metadata(conn, "repo_root", repo_root)?;
    upsert_metadata(conn, "git_common_dir", git_common_dir)?;
    if metadata_value(conn, "created_at")?.is_none() {
        upsert_metadata(conn, "created_at", &now)?;
    }
    upsert_metadata(conn, "updated_at", &now)?;
    Ok(())
}

pub fn read_schema_version(conn: &duckdb::Connection) -> Result<Option<u32>, AppError> {
    if !metadata_table_exists(conn)? {
        return Ok(None);
    }

    metadata_value(conn, "schema_version")?
        .map(|value| {
            value
                .parse::<u32>()
                .map_err(|error| AppError::DuckDb(error.to_string()))
        })
        .transpose()
}

pub fn list_tables(conn: &duckdb::Connection) -> Result<Vec<String>, AppError> {
    let mut stmt = conn
        .prepare("select table_name from information_schema.tables order by table_name")
        .map_err(|error| AppError::DuckDb(error.to_string()))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|error| AppError::DuckDb(error.to_string()))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| AppError::DuckDb(error.to_string()))
}

fn metadata_table_exists(conn: &duckdb::Connection) -> Result<bool, AppError> {
    conn.query_row(
        "select count(*) > 0 from information_schema.tables where table_name = 'metadata'",
        [],
        |row| row.get(0),
    )
    .map_err(|error| AppError::DuckDb(error.to_string()))
}

fn metadata_value(conn: &duckdb::Connection, key: &str) -> Result<Option<String>, AppError> {
    if !metadata_table_exists(conn)? {
        return Ok(None);
    }

    match conn.query_row(
        "select value from metadata where key = ?",
        params![key],
        |row| row.get::<_, String>(0),
    ) {
        Ok(value) => Ok(Some(value)),
        Err(duckdb::Error::QueryReturnedNoRows) => Ok(None),
        Err(error) => Err(AppError::DuckDb(error.to_string())),
    }
}

fn upsert_metadata(conn: &duckdb::Connection, key: &str, value: &str) -> Result<(), AppError> {
    conn.execute("delete from metadata where key = ?", params![key])
        .map_err(|error| AppError::DuckDb(error.to_string()))?;
    conn.execute(
        "insert into metadata (key, value) values (?, ?)",
        params![key, value],
    )
    .map_err(|error| AppError::DuckDb(error.to_string()))?;
    Ok(())
}
