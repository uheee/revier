use crate::api::QueryFilesRequest;
use crate::cli::QueryFilesArgs;
use crate::contracts::OperationStage;
use crate::error::AppError;
use crate::execution::{AnalysisExecutionContext, OperationProgressUpdate};
use crate::index::queries::QueryFilesFilter;
use crate::json::{QueryFilesOutput, QueryFilesRangeOutput};

pub fn query(args: QueryFilesArgs) -> Result<QueryFilesOutput, AppError> {
    let context = AnalysisExecutionContext::none();
    query_request_with_context(
        QueryFilesRequest {
            repo: args.common.repo,
            db: args.common.db,
            base: args.base,
            head: args.head,
            branch: args.branch,
            authors: args.authors,
            author_query: args.author_query,
            message: args.message,
            since: args.since,
            until: args.until,
            globs: args.globs,
        },
        &context,
    )
}

pub fn run(args: QueryFilesArgs) -> Result<String, AppError> {
    let pretty = args.common.pretty;
    let output = query(args)?;
    crate::serialize_json(&output, pretty)
}

pub fn query_request(args: QueryFilesRequest) -> Result<QueryFilesOutput, AppError> {
    let context = AnalysisExecutionContext::none();
    query_request_with_context(args, &context)
}

pub fn query_request_with_context(
    args: QueryFilesRequest,
    context: &AnalysisExecutionContext,
) -> Result<QueryFilesOutput, AppError> {
    context.check_cancelled()?;
    let repo = crate::git::repository::open_repository(&args.repo)?;
    let identity = crate::git::repository::repository_identity(&repo)?;
    let db_path = args
        .db
        .clone()
        .unwrap_or(crate::index::connection::default_database_path(
            &identity.repo_id,
        )?);
    if !db_path.exists() {
        return Err(AppError::IndexUnavailable(format!(
            "索引文件不存在：{}",
            db_path.display()
        )));
    }

    context.check_cancelled()?;
    let conn = crate::index::connection::open_database(&db_path)?;
    query_request_with_resources(args, repo, &conn, context)
}

pub fn query_request_with_connection(
    args: QueryFilesRequest,
    conn: &duckdb::Connection,
    context: &AnalysisExecutionContext,
) -> Result<QueryFilesOutput, AppError> {
    context.check_cancelled()?;
    let repo = crate::git::repository::open_repository(&args.repo)?;
    query_request_with_resources(args, repo, conn, context)
}

fn query_request_with_resources(
    args: QueryFilesRequest,
    repo: gix::Repository,
    conn: &duckdb::Connection,
    context: &AnalysisExecutionContext,
) -> Result<QueryFilesOutput, AppError> {
    context.check_cancelled()?;
    crate::index::migrations::ensure_compatible_schema(conn)?;
    context.check_cancelled()?;
    let range_hashes = crate::git::commits::range_commit_hashes(&repo, &args.base, &args.head)?;
    context.check_cancelled()?;
    let touched_files = crate::index::queries::query_files_for_commits_with_context(
        conn,
        &QueryFilesFilter {
            base: args.base.clone(),
            head: args.head.clone(),
            authors: args.authors,
            author_query: args.author_query,
            message: args.message,
            since: args.since,
            until: args.until,
            globs: Vec::new(),
        },
        &range_hashes,
        context,
    )?;
    context.check_cancelled()?;
    context.report_progress(OperationProgressUpdate {
        stage: OperationStage::FilterFiles,
        message: "筛选范围文件树差异".to_string(),
        completed_units: None,
        total_units: None,
    });
    let final_changes = crate::git::diff::range_file_tree_changes(&repo, &args.base, &args.head)?;
    let final_changes = coalesce_indexed_renames(final_changes, &touched_files);
    let mut selected_changes = Vec::new();
    for change in final_changes {
        context.check_cancelled()?;
        if !touched_files
            .iter()
            .any(|touched| paths_overlap(&change, touched))
            || !crate::index::queries::path_matches_globs(
                &args.globs,
                &change.path,
                change.old_path.as_deref(),
            )?
        {
            continue;
        }
        selected_changes.push(change);
    }
    let total = selected_changes.len() as u64;
    context.report_progress(OperationProgressUpdate {
        stage: OperationStage::ComputeFileStatistics,
        message: "计算最终文件行数".to_string(),
        completed_units: Some(0),
        total_units: Some(total),
    });
    let mut files = Vec::with_capacity(selected_changes.len());
    for (index, mut change) in selected_changes.into_iter().enumerate() {
        context.check_cancelled()?;
        crate::git::diff::populate_range_file_statistics(
            &repo,
            &args.base,
            &args.head,
            std::slice::from_mut(&mut change),
        )?;
        context.report_progress(OperationProgressUpdate {
            stage: OperationStage::ComputeFileStatistics,
            message: format!("计算文件统计：{}", change.path),
            completed_units: Some(index as u64 + 1),
            total_units: Some(total),
        });
        files.push(crate::json::ChangedFileOutput {
            path: change.path,
            old_path: change.old_path,
            old_blob_id: change.old_blob_id,
            new_blob_id: change.new_blob_id,
            status: if change.is_binary {
                "binary".to_string()
            } else {
                change.status
            },
            additions: change.additions,
            deletions: change.deletions,
            is_binary: change.is_binary,
            is_previewable: change.is_previewable,
        });
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));

    Ok(QueryFilesOutput {
        version: 1,
        range: QueryFilesRangeOutput {
            base_commit: args.base,
            head_commit: args.head,
        },
        files,
        warnings: Vec::new(),
    })
}

fn paths_overlap(
    change: &crate::git::diff::CommitFileChange,
    touched: &crate::json::ChangedFileOutput,
) -> bool {
    change.path == touched.path
        || change.old_path.as_deref() == Some(touched.path.as_str())
        || touched.old_path.as_deref().is_some_and(|old_path| {
            change.path == old_path || change.old_path.as_deref() == Some(old_path)
        })
}

fn coalesce_indexed_renames(
    mut changes: Vec<crate::git::diff::CommitFileChange>,
    hints: &[crate::json::ChangedFileOutput],
) -> Vec<crate::git::diff::CommitFileChange> {
    let mut removed = vec![false; changes.len()];
    for added_index in 0..changes.len() {
        if changes[added_index].status != "added" {
            continue;
        }
        let added_path = changes[added_index].path.clone();
        let Some(deleted_index) = (0..changes.len()).find(|&index| {
            !removed[index]
                && changes[index].status == "deleted"
                && rename_paths_connected(hints, &changes[index].path, &added_path)
        }) else {
            continue;
        };
        changes[added_index].status = "renamed".to_string();
        changes[added_index].old_path = Some(changes[deleted_index].path.clone());
        changes[added_index].old_blob_id = changes[deleted_index].old_blob_id.clone();
        removed[deleted_index] = true;
    }

    changes
        .into_iter()
        .enumerate()
        .filter_map(|(index, change)| (!removed[index]).then_some(change))
        .collect()
}

fn rename_paths_connected(
    hints: &[crate::json::ChangedFileOutput],
    old_path: &str,
    new_path: &str,
) -> bool {
    let mut visited = vec![old_path.to_string()];
    let mut cursor = 0;
    while cursor < visited.len() {
        let current = visited[cursor].clone();
        if current == new_path {
            return true;
        }
        for hint in hints.iter().filter(|hint| hint.status == "renamed") {
            let Some(hint_old_path) = hint.old_path.as_deref() else {
                continue;
            };
            let adjacent = if hint_old_path == current {
                Some(hint.path.as_str())
            } else if hint.path == current {
                Some(hint_old_path)
            } else {
                None
            };
            if let Some(adjacent) = adjacent {
                if !visited.iter().any(|path| path == adjacent) {
                    visited.push(adjacent.to_string());
                }
            }
        }
        cursor += 1;
    }
    false
}
