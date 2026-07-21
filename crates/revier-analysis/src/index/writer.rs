use std::collections::{HashMap, HashSet};

use crate::error::AppError;
use crate::execution::AnalysisExecutionContext;
use crate::git::commits::IndexedCommit;
use crate::git::diff::CommitFileChange;
use chrono::Utc;
use duckdb::params;

pub struct IndexWriteSummary {
    pub run_id: String,
    pub indexed_commit_count: u64,
    pub indexed_file_count: u64,
}

pub fn write_incremental_index(
    conn: &duckdb::Connection,
    repo_id: &str,
    started_at: &str,
    commits: &[IndexedCommit],
    files: &[CommitFileChange],
    context: &AnalysisExecutionContext,
) -> Result<IndexWriteSummary, AppError> {
    context.check_cancelled()?;
    let run_id = format!(
        "{repo_id}-{}-{}",
        Utc::now().timestamp_millis(),
        std::process::id()
    );
    conn.execute_batch("begin transaction")
        .map_err(duckdb_error)?;

    let result = (|| {
        conn.execute(
            "insert into index_runs
             (run_id, repo_id, started_at, status, indexed_commit_count, indexed_file_count)
             values (?, ?, ?, 'running', 0, 0)",
            params![run_id, repo_id, started_at],
        )
        .map_err(duckdb_error)?;

        let mut commit_statement = conn
            .prepare(
                "insert into commits
                 (hash, short_hash, author_name, author_email, author_key, committed_at,
                  subject, parent_count, is_merge)
                 values (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .map_err(duckdb_error)?;
        let mut parent_statement = conn
            .prepare(
                "insert into commit_parents (commit_hash, parent_hash, parent_index)
                 values (?, ?, ?)",
            )
            .map_err(duckdb_error)?;
        for commit in commits {
            context.check_cancelled()?;
            commit_statement
                .execute(params![
                    commit.hash,
                    commit.short_hash,
                    commit.author_name,
                    commit.author_email,
                    commit.author_key,
                    commit.committed_at,
                    commit.subject,
                    commit.parents.len() as i64,
                    commit.is_merge
                ])
                .map_err(duckdb_error)?;
            for (parent_index, parent_hash) in commit.parents.iter().enumerate() {
                context.check_cancelled()?;
                parent_statement
                    .execute(params![commit.hash, parent_hash, parent_index as i64])
                    .map_err(duckdb_error)?;
            }
        }

        let mut file_statement = conn
            .prepare(
                "insert into commit_files
                 (commit_hash, parent_hash, parent_index, path, old_path, status, additions,
                  deletions, is_binary, is_previewable, similarity)
                 values (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .map_err(duckdb_error)?;
        for file in files {
            context.check_cancelled()?;
            file_statement
                .execute(params![
                    file.commit_hash,
                    file.parent_hash,
                    file.parent_index as i64,
                    file.path,
                    file.old_path.clone().unwrap_or_default(),
                    file.status,
                    as_i64(file.additions, "additions")?,
                    as_i64(file.deletions, "deletions")?,
                    file.is_binary,
                    file.is_previewable,
                    file.similarity
                ])
                .map_err(duckdb_error)?;
        }

        context.check_cancelled()?;
        conn.execute(
            "update index_runs
             set indexed_commit_count = ?, indexed_file_count = ?
             where run_id = ?",
            params![commits.len() as i64, files.len() as i64, run_id],
        )
        .map_err(duckdb_error)?;
        Ok(IndexWriteSummary {
            run_id: run_id.clone(),
            indexed_commit_count: commits.len() as u64,
            indexed_file_count: files.len() as u64,
        })
    })();

    finish_transaction(conn, result)
}

pub fn complete_index_run(
    conn: &duckdb::Connection,
    run_id: &str,
    elapsed_ms: u64,
) -> Result<(), AppError> {
    let finished_at = Utc::now().to_rfc3339();
    conn.execute_batch("begin transaction")
        .map_err(duckdb_error)?;
    let result = (|| {
        conn.execute(
            "update index_runs
             set finished_at = ?, status = 'completed', elapsed_ms = ?
             where run_id = ?",
            params![finished_at, as_i64(elapsed_ms, "elapsed_ms")?, run_id],
        )
        .map_err(duckdb_error)?;
        conn.execute(
            "insert or replace into metadata (key, value) values ('updated_at', ?)",
            params![finished_at],
        )
        .map_err(duckdb_error)?;
        Ok(())
    })();
    finish_transaction(conn, result)
}

pub fn prune_unreferenced_commits(
    conn: &duckdb::Connection,
    locally_reachable_hashes: &[String],
) -> Result<u64, AppError> {
    conn.execute_batch("begin transaction")
        .map_err(duckdb_error)?;
    let result = prune_unreferenced_commits_in_transaction(conn, locally_reachable_hashes);
    finish_transaction(conn, result)
}

pub(crate) fn prune_unreferenced_commits_in_transaction(
    conn: &duckdb::Connection,
    locally_reachable_hashes: &[String],
) -> Result<u64, AppError> {
    let mut protected = locally_reachable_hashes
        .iter()
        .cloned()
        .collect::<HashSet<_>>();
    let mut snapshot_statement = conn
        .prepare("select base_commit, head_commit from analysis_snapshots")
        .map_err(duckdb_error)?;
    let snapshot_rows = snapshot_statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(duckdb_error)?;
    for row in snapshot_rows {
        let (base, head) = row.map_err(duckdb_error)?;
        protected.insert(base);
        protected.insert(head);
    }

    let mut parents_by_commit = HashMap::<String, Vec<String>>::new();
    let mut parent_statement = conn
        .prepare("select commit_hash, parent_hash from commit_parents")
        .map_err(duckdb_error)?;
    let parent_rows = parent_statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(duckdb_error)?;
    for row in parent_rows {
        let (commit_hash, parent_hash) = row.map_err(duckdb_error)?;
        parents_by_commit
            .entry(commit_hash)
            .or_default()
            .push(parent_hash);
    }
    let mut pending = protected.iter().cloned().collect::<Vec<_>>();
    while let Some(hash) = pending.pop() {
        if let Some(parents) = parents_by_commit.get(&hash) {
            for parent in parents {
                if protected.insert(parent.clone()) {
                    pending.push(parent.clone());
                }
            }
        }
    }

    let mut hash_statement = conn
        .prepare("select hash from commits")
        .map_err(duckdb_error)?;
    let hashes = hash_statement
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(duckdb_error)?;
    let mut obsolete = Vec::new();
    for hash in hashes {
        let hash = hash.map_err(duckdb_error)?;
        if !protected.contains(&hash) {
            obsolete.push(hash);
        }
    }
    if obsolete.is_empty() {
        return Ok(0);
    }

    let mut delete_files = conn
        .prepare("delete from commit_files where commit_hash = ?")
        .map_err(duckdb_error)?;
    let mut delete_parents = conn
        .prepare("delete from commit_parents where commit_hash = ? or parent_hash = ?")
        .map_err(duckdb_error)?;
    let mut delete_commit = conn
        .prepare("delete from commits where hash = ?")
        .map_err(duckdb_error)?;
    for hash in &obsolete {
        delete_files.execute(params![hash]).map_err(duckdb_error)?;
        delete_parents
            .execute(params![hash, hash])
            .map_err(duckdb_error)?;
        delete_commit.execute(params![hash]).map_err(duckdb_error)?;
    }
    Ok(obsolete.len() as u64)
}

pub fn checkpoint(conn: &duckdb::Connection) -> Result<(), AppError> {
    conn.execute_batch("checkpoint").map_err(duckdb_error)
}

fn finish_transaction<T>(
    conn: &duckdb::Connection,
    result: Result<T, AppError>,
) -> Result<T, AppError> {
    match result {
        Ok(value) => {
            if let Err(error) = conn.execute_batch("commit") {
                let _ = conn.execute_batch("rollback");
                return Err(duckdb_error(error));
            }
            Ok(value)
        }
        Err(error) => {
            let _ = conn.execute_batch("rollback");
            Err(error)
        }
    }
}

fn as_i64(value: u64, field: &str) -> Result<i64, AppError> {
    i64::try_from(value)
        .map_err(|_| AppError::InvalidArgument(format!("{field} 超出 DuckDB BIGINT 范围")))
}

fn duckdb_error(error: duckdb::Error) -> AppError {
    AppError::DuckDb(error.to_string())
}
