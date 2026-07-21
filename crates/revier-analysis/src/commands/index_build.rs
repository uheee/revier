use crate::cli::IndexBuildArgs;
use crate::error::AppError;
use crate::git::diff::CommitFileChange;
use crate::json::{IndexBuildOutput, IndexRunStatus};
use chrono::Utc;
use std::time::Instant;

pub fn run(args: IndexBuildArgs) -> Result<String, AppError> {
    let started_at = Utc::now().to_rfc3339();
    let started = Instant::now();
    let repo = crate::git::repository::open_repository(&args.common.repo)?;
    let identity = crate::git::repository::repository_identity(&repo)?;
    let db_path =
        args.common
            .db
            .clone()
            .unwrap_or(crate::index::connection::default_database_path(
                &identity.repo_id,
            )?);
    let conn = crate::index::connection::open_database(&db_path)?;
    crate::index::migrations::ensure_compatible_schema(&conn)?;
    crate::index::schema::initialize_schema(
        &conn,
        &identity.repo_id,
        &identity.repo_root,
        &identity.git_common_dir,
    )?;

    let commits = crate::git::commits::list_reachable_commits(&repo, &args.branch)?;
    let mut files = Vec::<CommitFileChange>::new();
    for commit in &commits {
        files.extend(crate::git::diff::commit_file_changes(&repo, &commit.hash)?);
    }

    let (summary, elapsed_ms) = elapsed_ms_after_write(started, || {
        crate::index::writer::write_index(&conn, &identity.repo_id, &started_at, &commits, &files)
    })?;
    crate::index::writer::complete_index_run(&conn, &summary.run_id, elapsed_ms)?;
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

fn elapsed_ms_after_write<T>(
    started: Instant,
    write: impl FnOnce() -> Result<T, AppError>,
) -> Result<(T, u64), AppError> {
    let value = write()?;
    Ok((value, started.elapsed().as_millis() as u64))
}

#[cfg(test)]
mod tests {
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
}
