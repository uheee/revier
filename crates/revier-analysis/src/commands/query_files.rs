use crate::cli::QueryFilesArgs;
use crate::error::AppError;
use crate::json::{QueryFilesOutput, QueryFilesRangeOutput};

pub fn run(args: QueryFilesArgs) -> Result<String, AppError> {
    let output = QueryFilesOutput {
        version: 1,
        range: QueryFilesRangeOutput {
            base_commit: args.base,
            head_commit: args.head,
        },
        files: Vec::new(),
        warnings: vec!["索引查询尚未接入 DuckDB".to_string()],
    };
    crate::serialize_json(&output, args.common.pretty)
}
