use crate::api::QueryFilesRequest;
use crate::cli::QueryFilesArgs;
use crate::error::AppError;
use crate::execution::AnalysisExecutionContext;
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
    context.check_cancelled()?;
    crate::index::migrations::ensure_compatible_schema(&conn)?;
    context.check_cancelled()?;
    let files = crate::index::queries::query_files_with_context(
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
        context,
    )?;

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
