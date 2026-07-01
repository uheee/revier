use crate::error::AppError;
use crate::git::commits::IndexedCommit;
use crate::git::diff::CommitFileChange;
use chrono::Utc;
use duckdb::params;

pub struct IndexWriteSummary {
    pub indexed_commit_count: u64,
    pub indexed_file_count: u64,
}

pub fn write_index(
    conn: &duckdb::Connection,
    repo_id: &str,
    commits: &[IndexedCommit],
    files: &[CommitFileChange],
    elapsed_ms: u64,
) -> Result<IndexWriteSummary, AppError> {
    let started_at = Utc::now().to_rfc3339();
    let run_id = format!(
        "{repo_id}-{}-{}",
        Utc::now().timestamp_millis(),
        std::process::id()
    );
    conn.execute(
        "insert into index_runs (run_id, repo_id, started_at, status, indexed_commit_count, indexed_file_count)
         values (?, ?, ?, 'running', 0, 0)",
        params![run_id, repo_id, started_at],
    )
    .map_err(|error| AppError::DuckDb(error.to_string()))?;

    conn.execute("delete from commit_files", [])
        .map_err(|error| AppError::DuckDb(error.to_string()))?;
    conn.execute("delete from commit_parents", [])
        .map_err(|error| AppError::DuckDb(error.to_string()))?;
    conn.execute("delete from commits", [])
        .map_err(|error| AppError::DuckDb(error.to_string()))?;

    for commit in commits {
        conn.execute(
            "insert or replace into commits
             (hash, short_hash, author_name, author_email, author_key, committed_at, subject, parent_count, is_merge)
             values (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                commit.hash,
                commit.short_hash,
                commit.author_name,
                commit.author_email,
                commit.author_key,
                commit.committed_at,
                commit.subject,
                commit.parents.len() as i64,
                commit.is_merge
            ],
        )
        .map_err(|error| AppError::DuckDb(error.to_string()))?;

        for (parent_index, parent_hash) in commit.parents.iter().enumerate() {
            conn.execute(
                "insert or replace into commit_parents (commit_hash, parent_hash, parent_index)
                 values (?, ?, ?)",
                params![commit.hash, parent_hash, parent_index as i64],
            )
            .map_err(|error| AppError::DuckDb(error.to_string()))?;
        }
    }

    for file in files {
        conn.execute(
            "insert or replace into commit_files
             (commit_hash, parent_hash, parent_index, path, old_path, status, additions, deletions, is_binary, is_previewable, similarity)
             values (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                file.commit_hash,
                file.parent_hash,
                file.parent_index as i64,
                file.path,
                file.old_path.clone().unwrap_or_default(),
                file.status,
                file.additions as i64,
                file.deletions as i64,
                file.is_binary,
                file.is_previewable,
                file.similarity
            ],
        )
        .map_err(|error| AppError::DuckDb(error.to_string()))?;
    }

    let finished_at = Utc::now().to_rfc3339();
    conn.execute(
        "update index_runs
         set finished_at = ?, status = 'completed', indexed_commit_count = ?, indexed_file_count = ?, elapsed_ms = ?
         where run_id = ?",
        params![
            finished_at,
            commits.len() as i64,
            files.len() as i64,
            elapsed_ms as i64,
            run_id
        ],
    )
    .map_err(|error| AppError::DuckDb(error.to_string()))?;
    conn.execute(
        "insert or replace into metadata (key, value) values ('updated_at', ?)",
        params![finished_at],
    )
    .map_err(|error| AppError::DuckDb(error.to_string()))?;

    Ok(IndexWriteSummary {
        indexed_commit_count: commits.len() as u64,
        indexed_file_count: files.len() as u64,
    })
}
