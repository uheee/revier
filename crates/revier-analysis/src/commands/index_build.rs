use crate::cli::IndexBuildArgs;
use crate::contracts::OperationStage;
use crate::error::AppError;
use crate::execution::{AnalysisExecutionContext, OperationProgressUpdate};
use crate::git::diff::CommitFileChange;
use crate::json::{IndexBuildOutput, IndexRunStatus};
use chrono::Utc;
use std::time::Instant;

pub fn run(args: IndexBuildArgs) -> Result<String, AppError> {
    run_with_context(args, &AnalysisExecutionContext::none())
}

pub fn run_with_context(
    args: IndexBuildArgs,
    context: &AnalysisExecutionContext,
) -> Result<String, AppError> {
    let started_at = Utc::now().to_rfc3339();
    let started = Instant::now();
    let repo = begin_index_build(&args, context)?;
    let identity = crate::git::repository::repository_identity(&repo)?;
    let db_path =
        args.common
            .db
            .clone()
            .unwrap_or(crate::index::connection::default_database_path(
                &identity.repo_id,
            )?);
    let conn = crate::index::connection::open_database(&db_path)?;
    run_with_resources(args, context, repo, identity, &conn, started_at, started)
}

pub fn run_with_connection(
    args: IndexBuildArgs,
    conn: &duckdb::Connection,
    context: &AnalysisExecutionContext,
) -> Result<String, AppError> {
    let started_at = Utc::now().to_rfc3339();
    let started = Instant::now();
    let repo = begin_index_build(&args, context)?;
    let identity = crate::git::repository::repository_identity(&repo)?;
    run_with_resources(args, context, repo, identity, conn, started_at, started)
}

fn begin_index_build(
    args: &IndexBuildArgs,
    context: &AnalysisExecutionContext,
) -> Result<gix::Repository, AppError> {
    report(
        context,
        OperationStage::ReadRepository,
        "读取 Git 仓库",
        None,
        None,
    );
    context.check_cancelled()?;
    crate::git::repository::open_repository(&args.common.repo)
}

fn run_with_resources(
    args: IndexBuildArgs,
    context: &AnalysisExecutionContext,
    repo: gix::Repository,
    identity: crate::git::repository::RepositoryIdentity,
    conn: &duckdb::Connection,
    started_at: String,
    started: Instant,
) -> Result<String, AppError> {
    crate::index::migrations::ensure_compatible_schema(conn)?;
    crate::index::schema::initialize_schema(
        conn,
        &identity.repo_id,
        &identity.repo_root,
        &identity.git_common_dir,
    )?;

    let commits =
        crate::git::commits::list_reachable_commits_with_context(&repo, &args.branch, context)?;
    let indexed_hashes = crate::index::queries::indexed_commit_hashes(conn)?;
    let missing_commits = commits
        .into_iter()
        .filter(|commit| !indexed_hashes.contains(&commit.hash))
        .collect::<Vec<_>>();
    let files = collect_missing_file_changes(&missing_commits, context, |hash| {
        crate::git::diff::commit_file_changes(&repo, hash)
    })?;

    report(
        context,
        OperationStage::WriteIndex,
        "写入增量提交索引",
        Some(0),
        Some(missing_commits.len() as u64),
    );
    let (summary, elapsed_ms) = elapsed_ms_after_write(started, || {
        crate::index::writer::write_incremental_index(
            conn,
            &identity.repo_id,
            &started_at,
            &missing_commits,
            &files,
            context,
        )
    })?;
    crate::index::writer::complete_index_run(conn, &summary.run_id, elapsed_ms)?;
    report(
        context,
        OperationStage::WriteIndex,
        "增量提交索引写入完成",
        Some(missing_commits.len() as u64),
        Some(missing_commits.len() as u64),
    );
    let output = IndexBuildOutput {
        version: 1,
        repo_id: identity.repo_id,
        status: IndexRunStatus::Completed,
        indexed_commit_count: summary.indexed_commit_count,
        indexed_file_count: summary.indexed_file_count,
        elapsed_ms,
        warnings: Vec::new(),
    };
    crate::serialize_json(&output, args.common.pretty)
}

fn collect_missing_file_changes(
    missing_commits: &[crate::git::commits::IndexedCommit],
    context: &AnalysisExecutionContext,
    mut diff: impl FnMut(&str) -> Result<Vec<CommitFileChange>, AppError>,
) -> Result<Vec<CommitFileChange>, AppError> {
    let total = missing_commits.len() as u64;
    report(
        context,
        OperationStage::IndexCommits,
        "计算缺失提交差异",
        Some(0),
        Some(total),
    );
    let mut files = Vec::new();
    for (index, commit) in missing_commits.iter().enumerate() {
        context.check_cancelled()?;
        files.extend(diff(&commit.hash)?);
        report(
            context,
            OperationStage::IndexCommits,
            "计算缺失提交差异",
            Some(index as u64 + 1),
            Some(total),
        );
    }
    Ok(files)
}

fn report(
    context: &AnalysisExecutionContext,
    stage: OperationStage,
    message: &str,
    completed_units: Option<u64>,
    total_units: Option<u64>,
) {
    context.report_progress(OperationProgressUpdate {
        stage,
        message: message.to_string(),
        completed_units,
        total_units,
    });
}

fn elapsed_ms_after_write<T>(
    started: Instant,
    write: impl FnOnce() -> Result<T, AppError>,
) -> Result<(T, u64), AppError> {
    let value = write()?;
    Ok((value, started.elapsed().as_millis() as u64))
}

#[cfg(test)]
mod tests {
    use crate::git::commits::IndexedCommit;
    use std::time::{Duration, Instant};

    #[test]
    fn elapsed_time_is_sampled_after_database_write() {
        let started = Instant::now();
        let (_, elapsed_ms) = super::elapsed_ms_after_write(started, || {
            std::thread::sleep(Duration::from_millis(20));
            Ok::<_, crate::error::AppError>(())
        })
        .expect("模拟数据库写入应成功");

        assert!(
            elapsed_ms >= 20,
            "耗时必须包含数据库写入，实际为 {elapsed_ms}ms"
        );
    }

    #[test]
    fn diff_is_called_once_for_each_missing_commit() {
        let commits = [indexed_commit("new-1"), indexed_commit("new-2")];
        let mut calls = Vec::new();

        let files = super::collect_missing_file_changes(
            &commits,
            &crate::execution::AnalysisExecutionContext::none(),
            |hash| {
                calls.push(hash.to_string());
                Ok(Vec::new())
            },
        )
        .expect("计算缺失提交差异失败");

        assert!(files.is_empty());
        assert_eq!(calls, vec!["new-1", "new-2"]);
    }

    fn indexed_commit(hash: &str) -> IndexedCommit {
        IndexedCommit {
            hash: hash.to_string(),
            short_hash: hash.to_string(),
            author_name: "测试作者".to_string(),
            author_email: None,
            author_key: "测试作者".to_string(),
            committed_at: "2026-07-22T00:00:00Z".to_string(),
            subject: "测试提交".to_string(),
            parents: Vec::new(),
            is_merge: false,
        }
    }
}
