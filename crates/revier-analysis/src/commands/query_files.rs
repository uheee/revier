use crate::cli::QueryFilesArgs;
use crate::error::AppError;
use crate::index::queries::QueryFilesFilter;
use crate::json::{QueryFilesOutput, QueryFilesRangeOutput};

pub fn run(args: QueryFilesArgs) -> Result<String, AppError> {
    let repo = crate::git::repository::open_repository(&args.common.repo)?;
    let identity = crate::git::repository::repository_identity(&repo)?;
    let db_path =
        args.common
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

    let conn = crate::index::connection::open_database(&db_path)?;
    crate::index::migrations::ensure_compatible_schema(&conn)?;
    let files = crate::index::queries::query_files(
        &conn,
        &QueryFilesFilter {
            base: args.base.clone(),
            head: args.head.clone(),
            authors: args.authors,
            author_query: args.author_query,
            message: args.message,
            since: args.since,
            until: args.until,
            globs: args.globs,
        },
    )?;

    let output = QueryFilesOutput {
        version: 1,
        range: QueryFilesRangeOutput {
            base_commit: args.base,
            head_commit: args.head,
        },
        files,
        warnings: Vec::new(),
    };
    crate::serialize_json(&output, args.common.pretty)
}
