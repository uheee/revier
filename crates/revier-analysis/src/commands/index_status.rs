use crate::cli::IndexStatusArgs;
use crate::error::AppError;
use crate::json::{IndexStatusKind, IndexStatusOutput};

pub fn run(args: IndexStatusArgs) -> Result<String, AppError> {
    let output = IndexStatusOutput {
        version: 1,
        repo_id: format!("uninitialized:{}", args.common.repo.display()),
        schema_version: 0,
        status: IndexStatusKind::Missing,
        indexed_commit_count: 0,
        indexed_file_count: 0,
        updated_at: None,
        warnings: vec!["索引尚未初始化".to_string()],
    };
    crate::serialize_json(&output, args.common.pretty)
}
