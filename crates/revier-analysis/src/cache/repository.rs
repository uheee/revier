use duckdb::{params, Connection, Error as DuckDbError};

use std::collections::HashMap;

use crate::cache::models::{
    CachedAnalysisFile, CachedAnalysisSnapshot, CachedBlockCommit, CachedCommitOverlay,
    CachedCommitOverlayBlock, CachedFileAnalysis, CachedFileBlock, CachedTouchedRange,
};
use crate::contracts::CacheState;
use crate::error::AppError;

pub fn branch_cache_state(
    conn: &Connection,
    repo_id: &str,
    branch: &str,
    current_head: &str,
    analysis_version: u32,
) -> Result<CacheState, AppError> {
    match conn.query_row(
        "select head_commit, analysis_version
         from analysis_snapshots where repo_id = ? and branch = ?",
        params![repo_id, branch],
        |row| Ok((row.get::<_, String>(0)?, row.get::<_, i32>(1)? as u32)),
    ) {
        Ok((cached_head, cached_version))
            if cached_head == current_head && cached_version == analysis_version =>
        {
            Ok(CacheState::Hit)
        }
        Ok(_) => Ok(CacheState::Stale),
        Err(DuckDbError::QueryReturnedNoRows) => Ok(CacheState::Miss),
        Err(error) => Err(duckdb_error(error)),
    }
}

pub fn publish_branch_snapshot(
    conn: &Connection,
    snapshot: &CachedAnalysisSnapshot,
) -> Result<(), AppError> {
    transaction(conn, || {
        delete_branch_snapshot(conn, &snapshot.repo_id, &snapshot.branch)?;
        insert_snapshot(conn, snapshot)
    })
}

pub fn publish_branch_snapshot_and_prune(
    conn: &Connection,
    snapshot: &CachedAnalysisSnapshot,
    locally_reachable_hashes: &[String],
) -> Result<u64, AppError> {
    publish_branch_snapshot_and_prune_with_completion(
        conn,
        snapshot,
        locally_reachable_hashes,
        || (snapshot.completed_at.clone(), snapshot.elapsed_ms),
    )
}

pub fn publish_branch_snapshot_and_prune_with_completion(
    conn: &Connection,
    snapshot: &CachedAnalysisSnapshot,
    locally_reachable_hashes: &[String],
    completion: impl FnOnce() -> (String, u64),
) -> Result<u64, AppError> {
    transaction(conn, || {
        delete_branch_snapshot(conn, &snapshot.repo_id, &snapshot.branch)?;
        insert_snapshot(conn, snapshot)?;
        let pruned = crate::index::writer::prune_unreferenced_commits_in_transaction(
            conn,
            locally_reachable_hashes,
        )?;
        let (completed_at, elapsed_ms) = completion();
        conn.execute(
            "update analysis_snapshots set completed_at = ?, elapsed_ms = ? where analysis_id = ?",
            params![
                completed_at,
                as_i64(elapsed_ms, "elapsed_ms")?,
                snapshot.analysis_id
            ],
        )
        .map_err(duckdb_error)?;
        Ok(pruned)
    })
}

pub fn load_branch_snapshot(
    conn: &Connection,
    repo_id: &str,
    branch: &str,
) -> Result<Option<CachedAnalysisSnapshot>, AppError> {
    let snapshot = match conn.query_row(
        "select analysis_id, repo_id, branch, base_commit, head_commit,
                case when start_at is null then null else strftime(start_at, '%Y-%m-%dT%H:%M:%SZ') end,
                case when end_at is null then null else strftime(end_at, '%Y-%m-%dT%H:%M:%SZ') end,
                author_query, message_query, filter_fingerprint, analysis_version,
                strftime(started_at, '%Y-%m-%dT%H:%M:%SZ'),
                strftime(completed_at, '%Y-%m-%dT%H:%M:%SZ'), elapsed_ms,
                last_selected_path
         from analysis_snapshots where repo_id = ? and branch = ?",
        params![repo_id, branch],
        |row| {
            Ok(CachedAnalysisSnapshot {
                analysis_id: row.get(0)?,
                repo_id: row.get(1)?,
                branch: row.get(2)?,
                base_commit: row.get(3)?,
                head_commit: row.get(4)?,
                start_at: row.get(5)?,
                end_at: row.get(6)?,
                author_query: row.get(7)?,
                message_query: row.get(8)?,
                filter_fingerprint: row.get(9)?,
                analysis_version: row.get::<_, i32>(10)? as u32,
                started_at: row.get(11)?,
                completed_at: row.get(12)?,
                elapsed_ms: row.get::<_, i64>(13)? as u64,
                last_selected_path: row.get(14)?,
                author_keys: Vec::new(),
                globs: Vec::new(),
                files: Vec::new(),
            })
        },
    ) {
        Ok(snapshot) => snapshot,
        Err(DuckDbError::QueryReturnedNoRows) => return Ok(None),
        Err(error) => return Err(AppError::DuckDb(error.to_string())),
    };

    let mut snapshot = snapshot;
    snapshot.author_keys = query_strings(
        conn,
        "select author_key from analysis_filter_authors where analysis_id = ? order by ordinal",
        &snapshot.analysis_id,
    )?;
    snapshot.globs = query_strings(
        conn,
        "select glob_rule from analysis_filter_globs where analysis_id = ? order by ordinal",
        &snapshot.analysis_id,
    )?;
    snapshot.files = load_analysis_files(conn, &snapshot.analysis_id)?;
    Ok(Some(snapshot))
}

pub fn load_branch_analysis_file(
    conn: &Connection,
    repo_id: &str,
    branch: &str,
    path: &str,
) -> Result<Option<(String, CachedAnalysisFile)>, AppError> {
    match conn.query_row(
        "select s.analysis_id, f.path, f.old_path, f.status, f.additions, f.deletions,
                f.is_binary, f.is_previewable, f.old_blob_id, f.new_blob_id
         from analysis_snapshots s
         join analysis_files f on f.analysis_id = s.analysis_id
         where s.repo_id = ? and s.branch = ? and f.path = ?",
        params![repo_id, branch, path],
        |row| {
            Ok((
                row.get(0)?,
                CachedAnalysisFile {
                    path: row.get(1)?,
                    old_path: row.get(2)?,
                    status: row.get(3)?,
                    additions: row.get::<_, i64>(4)? as u64,
                    deletions: row.get::<_, i64>(5)? as u64,
                    is_binary: row.get(6)?,
                    is_previewable: row.get(7)?,
                    old_blob_id: row.get(8)?,
                    new_blob_id: row.get(9)?,
                },
            ))
        },
    ) {
        Ok(file) => Ok(Some(file)),
        Err(DuckDbError::QueryReturnedNoRows) => Ok(None),
        Err(error) => Err(duckdb_error(error)),
    }
}

pub fn update_last_selected_path(
    conn: &Connection,
    repo_id: &str,
    branch: &str,
    file_path: Option<&str>,
) -> Result<(), AppError> {
    if let Some(path) = file_path {
        let exists: bool = conn
            .query_row(
                "select count(*) > 0
                 from analysis_snapshots s
                 join analysis_files f on f.analysis_id = s.analysis_id
                 where s.repo_id = ? and s.branch = ? and f.path = ?",
                params![repo_id, branch, path],
                |row| row.get(0),
            )
            .map_err(duckdb_error)?;
        if !exists {
            return Err(AppError::Analysis(format!("分支快照中不存在文件：{path}")));
        }
    }

    let changed = conn
        .execute(
            "update analysis_snapshots set last_selected_path = ? where repo_id = ? and branch = ?",
            params![file_path, repo_id, branch],
        )
        .map_err(duckdb_error)?;
    if changed == 0 {
        return Err(AppError::Analysis(format!("分支快照不存在：{branch}")));
    }
    Ok(())
}

pub fn replace_file_analysis(
    conn: &Connection,
    analysis: &CachedFileAnalysis,
) -> Result<(), AppError> {
    transaction(conn, || {
        delete_file_analysis(conn, &analysis.analysis_id, &analysis.path)?;
        insert_file_analysis(conn, analysis)
    })
}

pub fn load_file_analysis(
    conn: &Connection,
    analysis_id: &str,
    path: &str,
) -> Result<Option<CachedFileAnalysis>, AppError> {
    let mut analysis = match conn.query_row(
        "select file_analysis_id, analysis_id, path, resolved_encoding, block_signature,
                analysis_version, content_elapsed_ms, attribution_elapsed_ms,
                strftime(completed_at, '%Y-%m-%dT%H:%M:%SZ')
         from file_analyses where analysis_id = ? and path = ?",
        params![analysis_id, path],
        |row| {
            Ok(CachedFileAnalysis {
                file_analysis_id: row.get(0)?,
                analysis_id: row.get(1)?,
                path: row.get(2)?,
                resolved_encoding: row.get(3)?,
                block_signature: row.get(4)?,
                analysis_version: row.get::<_, i32>(5)? as u32,
                content_elapsed_ms: row.get::<_, i64>(6)? as u64,
                attribution_elapsed_ms: row.get::<_, i64>(7)? as u64,
                completed_at: row.get(8)?,
                blocks: Vec::new(),
            })
        },
    ) {
        Ok(analysis) => analysis,
        Err(DuckDbError::QueryReturnedNoRows) => return Ok(None),
        Err(error) => return Err(duckdb_error(error)),
    };
    analysis.blocks = load_file_blocks(conn, &analysis.file_analysis_id)?;
    Ok(Some(analysis))
}

pub fn replace_commit_overlay(
    conn: &Connection,
    overlay: &CachedCommitOverlay,
) -> Result<(), AppError> {
    transaction(conn, || {
        delete_commit_overlay(conn, &overlay.file_analysis_id, &overlay.commit_hash)?;
        insert_commit_overlay(conn, overlay)
    })
}

pub fn load_commit_overlay(
    conn: &Connection,
    file_analysis_id: &str,
    commit_hash: &str,
) -> Result<Option<CachedCommitOverlay>, AppError> {
    let mut overlay = match conn.query_row(
        "select commit_overlay_id, file_analysis_id, commit_hash, parent_hash, historical_path,
                old_blob_id, new_blob_id, resolved_encoding, analysis_version, elapsed_ms,
                strftime(completed_at, '%Y-%m-%dT%H:%M:%SZ')
         from commit_overlays where file_analysis_id = ? and commit_hash = ?",
        params![file_analysis_id, commit_hash],
        |row| {
            Ok(CachedCommitOverlay {
                commit_overlay_id: row.get(0)?,
                file_analysis_id: row.get(1)?,
                commit_hash: row.get(2)?,
                parent_hash: row.get(3)?,
                historical_path: row.get(4)?,
                old_blob_id: row.get(5)?,
                new_blob_id: row.get(6)?,
                resolved_encoding: row.get(7)?,
                analysis_version: row.get::<_, i32>(8)? as u32,
                elapsed_ms: row.get::<_, i64>(9)? as u64,
                completed_at: row.get(10)?,
                blocks: Vec::new(),
            })
        },
    ) {
        Ok(overlay) => overlay,
        Err(DuckDbError::QueryReturnedNoRows) => return Ok(None),
        Err(error) => return Err(duckdb_error(error)),
    };
    overlay.blocks = load_commit_overlay_blocks(conn, &overlay.commit_overlay_id)?;
    Ok(Some(overlay))
}

pub fn delete_file_commit_overlays(
    conn: &Connection,
    analysis_id: &str,
    path: &str,
) -> Result<(), AppError> {
    transaction(conn, || {
        let file_analysis_id = match conn.query_row(
            "select file_analysis_id from file_analyses where analysis_id = ? and path = ?",
            params![analysis_id, path],
            |row| row.get::<_, String>(0),
        ) {
            Ok(value) => value,
            Err(DuckDbError::QueryReturnedNoRows) => return Ok(()),
            Err(error) => return Err(duckdb_error(error)),
        };
        delete_all_commit_overlays(conn, &file_analysis_id)
    })
}

pub fn cleanup_orphaned_cache_rows(conn: &Connection) -> Result<(), AppError> {
    transaction(conn, || {
        for sql in [
            "delete from analysis_filter_authors where not exists (
               select 1 from analysis_snapshots
               where analysis_snapshots.analysis_id = analysis_filter_authors.analysis_id)",
            "delete from analysis_filter_globs where not exists (
               select 1 from analysis_snapshots
               where analysis_snapshots.analysis_id = analysis_filter_globs.analysis_id)",
            "delete from analysis_files where not exists (
               select 1 from analysis_snapshots
               where analysis_snapshots.analysis_id = analysis_files.analysis_id)",
            "delete from file_analyses where not exists (
               select 1 from analysis_files
               where analysis_files.analysis_id = file_analyses.analysis_id
                 and analysis_files.path = file_analyses.path)",
            "delete from file_block_merge_sources where not exists (
               select 1 from file_analyses
               where file_analyses.file_analysis_id = file_block_merge_sources.file_analysis_id)",
            "delete from file_block_commit_ranges where not exists (
               select 1 from file_analyses
               where file_analyses.file_analysis_id = file_block_commit_ranges.file_analysis_id)",
            "delete from file_block_commits where not exists (
               select 1 from file_analyses
               where file_analyses.file_analysis_id = file_block_commits.file_analysis_id)",
            "delete from file_blocks where not exists (
               select 1 from file_analyses
               where file_analyses.file_analysis_id = file_blocks.file_analysis_id)",
            "delete from commit_overlays where not exists (
               select 1 from file_analyses
               where file_analyses.file_analysis_id = commit_overlays.file_analysis_id)",
            "delete from commit_overlay_blocks where not exists (
               select 1 from commit_overlays
               where commit_overlays.commit_overlay_id = commit_overlay_blocks.commit_overlay_id)",
        ] {
            conn.execute(sql, []).map_err(duckdb_error)?;
        }
        Ok(())
    })
}

fn insert_snapshot(conn: &Connection, snapshot: &CachedAnalysisSnapshot) -> Result<(), AppError> {
    conn.execute(
        "insert into analysis_snapshots
         (analysis_id, repo_id, branch, base_commit, head_commit, start_at, end_at,
          author_query, message_query, filter_fingerprint, analysis_version,
          started_at, completed_at, elapsed_ms, last_selected_path)
         values (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params![
            snapshot.analysis_id,
            snapshot.repo_id,
            snapshot.branch,
            snapshot.base_commit,
            snapshot.head_commit,
            snapshot.start_at,
            snapshot.end_at,
            snapshot.author_query,
            snapshot.message_query,
            snapshot.filter_fingerprint,
            i64::from(snapshot.analysis_version),
            snapshot.started_at,
            snapshot.completed_at,
            as_i64(snapshot.elapsed_ms, "elapsed_ms")?,
            snapshot.last_selected_path
        ],
    )
    .map_err(duckdb_error)?;

    for (ordinal, author_key) in snapshot.author_keys.iter().enumerate() {
        conn.execute(
            "insert into analysis_filter_authors (analysis_id, ordinal, author_key) values (?, ?, ?)",
            params![snapshot.analysis_id, ordinal as i64, author_key],
        )
        .map_err(duckdb_error)?;
    }
    for (ordinal, glob) in snapshot.globs.iter().enumerate() {
        conn.execute(
            "insert into analysis_filter_globs (analysis_id, ordinal, glob_rule) values (?, ?, ?)",
            params![snapshot.analysis_id, ordinal as i64, glob],
        )
        .map_err(duckdb_error)?;
    }
    for file in &snapshot.files {
        insert_analysis_file(conn, &snapshot.analysis_id, file)?;
    }
    Ok(())
}

fn insert_analysis_file(
    conn: &Connection,
    analysis_id: &str,
    file: &CachedAnalysisFile,
) -> Result<(), AppError> {
    conn.execute(
        "insert into analysis_files
         (analysis_id, path, old_path, status, additions, deletions, is_binary,
          is_previewable, old_blob_id, new_blob_id)
         values (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        params![
            analysis_id,
            file.path,
            file.old_path,
            i64::from(file.status),
            as_i64(file.additions, "additions")?,
            as_i64(file.deletions, "deletions")?,
            file.is_binary,
            file.is_previewable,
            file.old_blob_id,
            file.new_blob_id
        ],
    )
    .map_err(duckdb_error)?;
    Ok(())
}

fn load_analysis_files(
    conn: &Connection,
    analysis_id: &str,
) -> Result<Vec<CachedAnalysisFile>, AppError> {
    let mut statement = conn
        .prepare(
            "select path, old_path, status, additions, deletions, is_binary,
                    is_previewable, old_blob_id, new_blob_id
             from analysis_files where analysis_id = ? order by path",
        )
        .map_err(duckdb_error)?;
    let rows = statement
        .query_map(params![analysis_id], |row| {
            Ok(CachedAnalysisFile {
                path: row.get(0)?,
                old_path: row.get(1)?,
                status: row.get::<_, i16>(2)?,
                additions: row.get::<_, i64>(3)? as u64,
                deletions: row.get::<_, i64>(4)? as u64,
                is_binary: row.get(5)?,
                is_previewable: row.get(6)?,
                old_blob_id: row.get(7)?,
                new_blob_id: row.get(8)?,
            })
        })
        .map_err(duckdb_error)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(duckdb_error)
}

fn insert_file_analysis(conn: &Connection, analysis: &CachedFileAnalysis) -> Result<(), AppError> {
    conn.execute(
        "insert into file_analyses
         (file_analysis_id, analysis_id, path, resolved_encoding, block_signature,
          analysis_version, content_elapsed_ms, attribution_elapsed_ms, completed_at)
         select ?, ?, ?, ?, ?, ?, ?, ?, ?
         where exists (select 1 from analysis_files where analysis_id = ? and path = ?)",
        params![
            analysis.file_analysis_id,
            analysis.analysis_id,
            analysis.path,
            i64::from(analysis.resolved_encoding),
            analysis.block_signature,
            i64::from(analysis.analysis_version),
            as_i64(analysis.content_elapsed_ms, "content_elapsed_ms")?,
            as_i64(analysis.attribution_elapsed_ms, "attribution_elapsed_ms")?,
            analysis.completed_at,
            analysis.analysis_id,
            analysis.path
        ],
    )
    .map_err(duckdb_error)
    .and_then(|changed| {
        if changed == 1 {
            Ok(())
        } else {
            Err(AppError::InvalidArgument(format!(
                "分析快照中不存在文件：{} / {}",
                analysis.analysis_id, analysis.path
            )))
        }
    })?;

    for block in &analysis.blocks {
        conn.execute(
            "insert into file_blocks
             (file_analysis_id, block_ordinal, old_start, old_end, new_start, new_end,
              change_type, confidence, warning_flags)
             values (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                analysis.file_analysis_id,
                i64::from(block.ordinal),
                as_i64(block.old_start, "old_start")?,
                as_i64(block.old_end, "old_end")?,
                as_i64(block.new_start, "new_start")?,
                as_i64(block.new_end, "new_end")?,
                i64::from(block.change_type),
                block.confidence.map(i64::from),
                i64::from(block.warning_flags)
            ],
        )
        .map_err(duckdb_error)?;
        for commit in &block.commits {
            insert_block_commit(conn, &analysis.file_analysis_id, block.ordinal, commit)?;
        }
    }
    Ok(())
}

fn insert_block_commit(
    conn: &Connection,
    file_analysis_id: &str,
    block_ordinal: u32,
    commit: &CachedBlockCommit,
) -> Result<(), AppError> {
    conn.execute(
        "insert into file_block_commits
         (file_analysis_id, block_ordinal, commit_hash, matched_by_filter, attribution_method)
         values (?, ?, ?, ?, ?)",
        params![
            file_analysis_id,
            i64::from(block_ordinal),
            commit.commit_hash,
            commit.matched_by_filter,
            commit.attribution_method.map(i64::from)
        ],
    )
    .map_err(duckdb_error)?;
    for (ordinal, range) in commit.touched_ranges.iter().enumerate() {
        conn.execute(
            "insert into file_block_commit_ranges
             (file_analysis_id, block_ordinal, commit_hash, range_ordinal,
              old_start, old_end, new_start, new_end)
             values (?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                file_analysis_id,
                i64::from(block_ordinal),
                commit.commit_hash,
                ordinal as i64,
                optional_i64(range.old_start, "old_start")?,
                optional_i64(range.old_end, "old_end")?,
                optional_i64(range.new_start, "new_start")?,
                optional_i64(range.new_end, "new_end")?
            ],
        )
        .map_err(duckdb_error)?;
    }
    for (ordinal, merge_hash) in commit.merge_hashes.iter().enumerate() {
        conn.execute(
            "insert into file_block_merge_sources
             (file_analysis_id, block_ordinal, commit_hash, merge_ordinal, merge_hash)
             values (?, ?, ?, ?, ?)",
            params![
                file_analysis_id,
                i64::from(block_ordinal),
                commit.commit_hash,
                ordinal as i64,
                merge_hash
            ],
        )
        .map_err(duckdb_error)?;
    }
    Ok(())
}

fn load_file_blocks(
    conn: &Connection,
    file_analysis_id: &str,
) -> Result<Vec<CachedFileBlock>, AppError> {
    let mut range_map = load_touched_ranges(conn, file_analysis_id)?;
    let mut merge_map = load_merge_hashes(conn, file_analysis_id)?;
    let mut commit_map = HashMap::<u32, Vec<CachedBlockCommit>>::new();
    let mut commit_statement = conn
        .prepare(
            "select block_ordinal, commit_hash, matched_by_filter, attribution_method
             from file_block_commits where file_analysis_id = ?
             order by block_ordinal, commit_hash",
        )
        .map_err(duckdb_error)?;
    let commits = commit_statement
        .query_map(params![file_analysis_id], |row| {
            Ok((
                row.get::<_, i32>(0)? as u32,
                row.get::<_, String>(1)?,
                row.get::<_, bool>(2)?,
                row.get::<_, Option<i16>>(3)?,
            ))
        })
        .map_err(duckdb_error)?;
    for commit in commits {
        let (block_ordinal, commit_hash, matched_by_filter, attribution_method) =
            commit.map_err(duckdb_error)?;
        let key = (block_ordinal, commit_hash.clone());
        commit_map
            .entry(block_ordinal)
            .or_default()
            .push(CachedBlockCommit {
                commit_hash,
                matched_by_filter,
                attribution_method,
                touched_ranges: range_map.remove(&key).unwrap_or_default(),
                merge_hashes: merge_map.remove(&key).unwrap_or_default(),
            });
    }

    let mut statement = conn
        .prepare(
            "select block_ordinal, old_start, old_end, new_start, new_end,
                    change_type, confidence, warning_flags
             from file_blocks where file_analysis_id = ? order by block_ordinal",
        )
        .map_err(duckdb_error)?;
    let rows = statement
        .query_map(params![file_analysis_id], |row| {
            let ordinal = row.get::<_, i32>(0)? as u32;
            Ok(CachedFileBlock {
                ordinal,
                old_start: row.get::<_, i64>(1)? as u64,
                old_end: row.get::<_, i64>(2)? as u64,
                new_start: row.get::<_, i64>(3)? as u64,
                new_end: row.get::<_, i64>(4)? as u64,
                change_type: row.get(5)?,
                confidence: row.get(6)?,
                warning_flags: row.get::<_, i32>(7)? as u32,
                commits: commit_map.remove(&ordinal).unwrap_or_default(),
            })
        })
        .map_err(duckdb_error)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(duckdb_error)
}

type BlockCommitKey = (u32, String);

fn load_touched_ranges(
    conn: &Connection,
    file_analysis_id: &str,
) -> Result<HashMap<BlockCommitKey, Vec<CachedTouchedRange>>, AppError> {
    let mut statement = conn
        .prepare(
            "select block_ordinal, commit_hash, old_start, old_end, new_start, new_end
             from file_block_commit_ranges where file_analysis_id = ?
             order by block_ordinal, commit_hash, range_ordinal",
        )
        .map_err(duckdb_error)?;
    let rows = statement
        .query_map(params![file_analysis_id], |row| {
            Ok((
                row.get::<_, i32>(0)? as u32,
                row.get::<_, String>(1)?,
                CachedTouchedRange {
                    old_start: row.get::<_, Option<i64>>(2)?.map(|value| value as u64),
                    old_end: row.get::<_, Option<i64>>(3)?.map(|value| value as u64),
                    new_start: row.get::<_, Option<i64>>(4)?.map(|value| value as u64),
                    new_end: row.get::<_, Option<i64>>(5)?.map(|value| value as u64),
                },
            ))
        })
        .map_err(duckdb_error)?;
    let mut ranges = HashMap::<BlockCommitKey, Vec<CachedTouchedRange>>::new();
    for row in rows {
        let (block_ordinal, commit_hash, range) = row.map_err(duckdb_error)?;
        ranges
            .entry((block_ordinal, commit_hash))
            .or_default()
            .push(range);
    }
    Ok(ranges)
}

fn load_merge_hashes(
    conn: &Connection,
    file_analysis_id: &str,
) -> Result<HashMap<BlockCommitKey, Vec<String>>, AppError> {
    let mut statement = conn
        .prepare(
            "select block_ordinal, commit_hash, merge_hash
             from file_block_merge_sources where file_analysis_id = ?
             order by block_ordinal, commit_hash, merge_ordinal",
        )
        .map_err(duckdb_error)?;
    let rows = statement
        .query_map(params![file_analysis_id], |row| {
            Ok((
                row.get::<_, i32>(0)? as u32,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .map_err(duckdb_error)?;
    let mut hashes = HashMap::<BlockCommitKey, Vec<String>>::new();
    for row in rows {
        let (block_ordinal, commit_hash, merge_hash) = row.map_err(duckdb_error)?;
        hashes
            .entry((block_ordinal, commit_hash))
            .or_default()
            .push(merge_hash);
    }
    Ok(hashes)
}

fn insert_commit_overlay(conn: &Connection, overlay: &CachedCommitOverlay) -> Result<(), AppError> {
    conn.execute(
        "insert into commit_overlays
         (commit_overlay_id, file_analysis_id, commit_hash, parent_hash, historical_path,
          old_blob_id, new_blob_id, resolved_encoding, analysis_version, elapsed_ms, completed_at)
         select ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?
         where exists (select 1 from file_analyses where file_analysis_id = ?)",
        params![
            overlay.commit_overlay_id,
            overlay.file_analysis_id,
            overlay.commit_hash,
            overlay.parent_hash,
            overlay.historical_path,
            overlay.old_blob_id,
            overlay.new_blob_id,
            i64::from(overlay.resolved_encoding),
            i64::from(overlay.analysis_version),
            as_i64(overlay.elapsed_ms, "elapsed_ms")?,
            overlay.completed_at,
            overlay.file_analysis_id
        ],
    )
    .map_err(duckdb_error)
    .and_then(|changed| {
        if changed == 1 {
            Ok(())
        } else {
            Err(AppError::InvalidArgument(format!(
                "文件分析不存在：{}",
                overlay.file_analysis_id
            )))
        }
    })?;
    for block in &overlay.blocks {
        conn.execute(
            "insert into commit_overlay_blocks
             (commit_overlay_id, block_ordinal, old_start, old_end, new_start, new_end, change_type)
             values (?, ?, ?, ?, ?, ?, ?)",
            params![
                overlay.commit_overlay_id,
                i64::from(block.ordinal),
                as_i64(block.old_start, "old_start")?,
                as_i64(block.old_end, "old_end")?,
                as_i64(block.new_start, "new_start")?,
                as_i64(block.new_end, "new_end")?,
                i64::from(block.change_type)
            ],
        )
        .map_err(duckdb_error)?;
    }
    Ok(())
}

fn load_commit_overlay_blocks(
    conn: &Connection,
    commit_overlay_id: &str,
) -> Result<Vec<CachedCommitOverlayBlock>, AppError> {
    let mut statement = conn
        .prepare(
            "select block_ordinal, old_start, old_end, new_start, new_end, change_type
             from commit_overlay_blocks where commit_overlay_id = ? order by block_ordinal",
        )
        .map_err(duckdb_error)?;
    let rows = statement
        .query_map(params![commit_overlay_id], |row| {
            Ok(CachedCommitOverlayBlock {
                ordinal: row.get::<_, i32>(0)? as u32,
                old_start: row.get::<_, i64>(1)? as u64,
                old_end: row.get::<_, i64>(2)? as u64,
                new_start: row.get::<_, i64>(3)? as u64,
                new_end: row.get::<_, i64>(4)? as u64,
                change_type: row.get(5)?,
            })
        })
        .map_err(duckdb_error)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(duckdb_error)
}

fn delete_file_analysis(conn: &Connection, analysis_id: &str, path: &str) -> Result<(), AppError> {
    let file_analysis_id = match conn.query_row(
        "select file_analysis_id from file_analyses where analysis_id = ? and path = ?",
        params![analysis_id, path],
        |row| row.get::<_, String>(0),
    ) {
        Ok(value) => value,
        Err(DuckDbError::QueryReturnedNoRows) => return Ok(()),
        Err(error) => return Err(duckdb_error(error)),
    };
    delete_file_analysis_by_id(conn, &file_analysis_id)
}

fn delete_file_analysis_by_id(conn: &Connection, file_analysis_id: &str) -> Result<(), AppError> {
    delete_all_commit_overlays(conn, file_analysis_id)?;
    for table in [
        "file_block_merge_sources",
        "file_block_commit_ranges",
        "file_block_commits",
        "file_blocks",
    ] {
        conn.execute(
            &format!("delete from {table} where file_analysis_id = ?"),
            params![file_analysis_id],
        )
        .map_err(duckdb_error)?;
    }
    conn.execute(
        "delete from file_analyses where file_analysis_id = ?",
        params![file_analysis_id],
    )
    .map_err(duckdb_error)?;
    Ok(())
}

fn delete_all_commit_overlays(conn: &Connection, file_analysis_id: &str) -> Result<(), AppError> {
    let overlay_ids = "select commit_overlay_id from commit_overlays where file_analysis_id = ?";
    conn.execute(
        &format!("delete from commit_overlay_blocks where commit_overlay_id in ({overlay_ids})"),
        params![file_analysis_id],
    )
    .map_err(duckdb_error)?;
    conn.execute(
        "delete from commit_overlays where file_analysis_id = ?",
        params![file_analysis_id],
    )
    .map_err(duckdb_error)?;
    Ok(())
}

fn delete_commit_overlay(
    conn: &Connection,
    file_analysis_id: &str,
    commit_hash: &str,
) -> Result<(), AppError> {
    let overlay_id = match conn.query_row(
        "select commit_overlay_id from commit_overlays where file_analysis_id = ? and commit_hash = ?",
        params![file_analysis_id, commit_hash],
        |row| row.get::<_, String>(0),
    ) {
        Ok(value) => value,
        Err(DuckDbError::QueryReturnedNoRows) => return Ok(()),
        Err(error) => return Err(duckdb_error(error)),
    };
    conn.execute(
        "delete from commit_overlay_blocks where commit_overlay_id = ?",
        params![overlay_id],
    )
    .map_err(duckdb_error)?;
    conn.execute(
        "delete from commit_overlays where commit_overlay_id = ?",
        params![overlay_id],
    )
    .map_err(duckdb_error)?;
    Ok(())
}

fn query_strings(conn: &Connection, sql: &str, analysis_id: &str) -> Result<Vec<String>, AppError> {
    let mut statement = conn.prepare(sql).map_err(duckdb_error)?;
    let rows = statement
        .query_map(params![analysis_id], |row| row.get(0))
        .map_err(duckdb_error)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(duckdb_error)
}

fn delete_branch_snapshot(conn: &Connection, repo_id: &str, branch: &str) -> Result<(), AppError> {
    let condition = "select analysis_id from analysis_snapshots where repo_id = ? and branch = ?";
    for sql in [
        format!("delete from commit_overlay_blocks where commit_overlay_id in (select commit_overlay_id from commit_overlays where file_analysis_id in (select file_analysis_id from file_analyses where analysis_id in ({condition})))"),
        format!("delete from commit_overlays where file_analysis_id in (select file_analysis_id from file_analyses where analysis_id in ({condition}))"),
        format!("delete from file_block_merge_sources where file_analysis_id in (select file_analysis_id from file_analyses where analysis_id in ({condition}))"),
        format!("delete from file_block_commit_ranges where file_analysis_id in (select file_analysis_id from file_analyses where analysis_id in ({condition}))"),
        format!("delete from file_block_commits where file_analysis_id in (select file_analysis_id from file_analyses where analysis_id in ({condition}))"),
        format!("delete from file_blocks where file_analysis_id in (select file_analysis_id from file_analyses where analysis_id in ({condition}))"),
        format!("delete from file_analyses where analysis_id in ({condition})"),
        format!("delete from analysis_files where analysis_id in ({condition})"),
        format!("delete from analysis_filter_authors where analysis_id in ({condition})"),
        format!("delete from analysis_filter_globs where analysis_id in ({condition})"),
    ] {
        conn.execute(&sql, params![repo_id, branch])
            .map_err(duckdb_error)?;
    }
    conn.execute(
        "delete from analysis_snapshots where repo_id = ? and branch = ?",
        params![repo_id, branch],
    )
    .map_err(duckdb_error)?;
    Ok(())
}

fn transaction<T>(
    conn: &Connection,
    operation: impl FnOnce() -> Result<T, AppError>,
) -> Result<T, AppError> {
    conn.execute_batch("begin transaction")
        .map_err(duckdb_error)?;
    match operation() {
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

fn optional_i64(value: Option<u64>, field: &str) -> Result<Option<i64>, AppError> {
    value.map(|value| as_i64(value, field)).transpose()
}

fn duckdb_error(error: DuckDbError) -> AppError {
    AppError::DuckDb(error.to_string())
}
