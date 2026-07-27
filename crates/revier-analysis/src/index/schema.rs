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
    .map_err(|error| AppError::DuckDb(error))?;
    initialize_cache_schema(conn)?;

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

pub(crate) fn initialize_cache_schema(conn: &duckdb::Connection) -> Result<(), AppError> {
    conn.execute_batch(
        "
        create table if not exists analysis_snapshots (
          analysis_id text primary key,
          repo_id text not null,
          branch text not null,
          base_commit text not null,
          head_commit text not null,
          start_at timestamp,
          end_at timestamp,
          author_query text,
          message_query text,
          filter_fingerprint text not null,
          analysis_version integer not null,
          started_at timestamp not null,
          completed_at timestamp not null,
          elapsed_ms bigint not null,
          last_selected_path text,
          unique (repo_id, branch)
        );
        create table if not exists analysis_filter_authors (
          analysis_id text not null,
          ordinal integer not null,
          author_key text not null,
          primary key (analysis_id, ordinal)
        );
        create table if not exists analysis_filter_globs (
          analysis_id text not null,
          ordinal integer not null,
          glob_rule text not null,
          primary key (analysis_id, ordinal)
        );
        create table if not exists analysis_files (
          analysis_id text not null,
          path text not null,
          old_path text,
          status smallint not null,
          additions bigint not null,
          deletions bigint not null,
          is_binary boolean not null,
          is_previewable boolean not null,
          old_blob_id text,
          new_blob_id text,
          primary key (analysis_id, path)
        );
        create table if not exists file_analyses (
          file_analysis_id text primary key,
          analysis_id text not null,
          path text not null,
          resolved_encoding smallint not null,
          block_signature text not null,
          analysis_version integer not null,
          content_elapsed_ms bigint not null,
          attribution_elapsed_ms bigint not null,
          completed_at timestamp not null,
          unique (analysis_id, path)
        );
        create table if not exists file_blocks (
          file_analysis_id text not null,
          block_ordinal integer not null,
          old_start integer not null,
          old_end integer not null,
          new_start integer not null,
          new_end integer not null,
          change_type smallint not null,
          confidence smallint,
          warning_flags integer not null,
          primary key (file_analysis_id, block_ordinal)
        );
        create table if not exists file_block_commits (
          file_analysis_id text not null,
          block_ordinal integer not null,
          commit_hash text not null,
          matched_by_filter boolean not null,
          attribution_method smallint,
          primary key (file_analysis_id, block_ordinal, commit_hash)
        );
        create table if not exists file_block_commit_ranges (
          file_analysis_id text not null,
          block_ordinal integer not null,
          commit_hash text not null,
          range_ordinal integer not null,
          old_start integer,
          old_end integer,
          new_start integer,
          new_end integer,
          primary key (file_analysis_id, block_ordinal, commit_hash, range_ordinal)
        );
        create table if not exists file_block_merge_sources (
          file_analysis_id text not null,
          block_ordinal integer not null,
          commit_hash text not null,
          merge_ordinal integer not null,
          merge_hash text not null,
          primary key (file_analysis_id, block_ordinal, commit_hash, merge_ordinal)
        );
        create table if not exists commit_overlays (
          commit_overlay_id text primary key,
          file_analysis_id text not null,
          commit_hash text not null,
          parent_hash text not null,
          historical_path text not null,
          old_blob_id text,
          new_blob_id text,
          resolved_encoding smallint not null,
          analysis_version integer not null,
          elapsed_ms bigint not null,
          completed_at timestamp not null,
          unique (file_analysis_id, commit_hash)
        );
        create table if not exists commit_overlay_blocks (
          commit_overlay_id text not null,
          block_ordinal integer not null,
          old_start integer not null,
          old_end integer not null,
          new_start integer not null,
          new_end integer not null,
          change_type smallint not null,
          primary key (commit_overlay_id, block_ordinal)
        );
        create index if not exists analysis_snapshots_repo_branch_idx
          on analysis_snapshots (repo_id, branch);
        create index if not exists analysis_files_analysis_path_idx
          on analysis_files (analysis_id, path);
        create index if not exists file_analyses_analysis_path_idx
          on file_analyses (analysis_id, path);
        create index if not exists commit_overlays_file_commit_idx
          on commit_overlays (file_analysis_id, commit_hash);
        create index if not exists commit_files_path_commit_idx
          on commit_files (path, commit_hash);
        create index if not exists commit_files_old_path_commit_idx
          on commit_files (old_path, commit_hash);
        ",
    )
    .map_err(|error| AppError::DuckDb(error))?;
    Ok(())
}

pub fn read_schema_version(conn: &duckdb::Connection) -> Result<Option<u32>, AppError> {
    if !metadata_table_exists(conn)? {
        return Ok(None);
    }

    metadata_value(conn, "schema_version")?
        .map(|value| {
            value.parse::<u32>().map_err(|error| {
                AppError::SchemaIncompatible(format!("schema_version 无法解析：{error}"))
            })
        })
        .transpose()
}

pub fn list_tables(conn: &duckdb::Connection) -> Result<Vec<String>, AppError> {
    let mut stmt = conn
        .prepare("select table_name from information_schema.tables order by table_name")
        .map_err(|error| AppError::DuckDb(error))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|error| AppError::DuckDb(error))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| AppError::DuckDb(error))
}

fn metadata_table_exists(conn: &duckdb::Connection) -> Result<bool, AppError> {
    conn.query_row(
        "select count(*) > 0 from information_schema.tables where table_name = 'metadata'",
        [],
        |row| row.get(0),
    )
    .map_err(|error| AppError::DuckDb(error))
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
        Err(error) => Err(AppError::DuckDb(error)),
    }
}

fn upsert_metadata(conn: &duckdb::Connection, key: &str, value: &str) -> Result<(), AppError> {
    conn.execute("delete from metadata where key = ?", params![key])
        .map_err(|error| AppError::DuckDb(error))?;
    conn.execute(
        "insert into metadata (key, value) values (?, ?)",
        params![key, value],
    )
    .map_err(|error| AppError::DuckDb(error))?;
    Ok(())
}
