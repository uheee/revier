use crate::cli::IndexBuildArgs;
use crate::error::AppError;
use crate::json::{IndexBuildOutput, IndexRunStatus};

pub fn run(args: IndexBuildArgs) -> Result<String, AppError> {
    let output = IndexBuildOutput {
        version: 1,
        repo_id: format!("uninitialized:{}", args.common.repo.display()),
        status: IndexRunStatus::Failed,
        indexed_commit_count: 0,
        indexed_file_count: 0,
        elapsed_ms: 0,
        warnings: vec![format!("索引构建尚未接入分支 {}", args.branch)],
    };
    crate::serialize_json(&output, args.common.pretty)
}
