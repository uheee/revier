use crate::cli::IndexStatusArgs;
use crate::error::AppError;
use crate::json::{IndexStatusKind, IndexStatusOutput};

pub fn run(args: IndexStatusArgs) -> Result<String, AppError> {
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
        let output = IndexStatusOutput {
            version: 1,
            repo_id: identity.repo_id,
            schema_version: 0,
            status: IndexStatusKind::Missing,
            indexed_commit_count: 0,
            indexed_file_count: 0,
            updated_at: None,
            warnings: vec!["索引文件不存在".to_string()],
        };
        return crate::serialize_json(&output, args.common.pretty);
    }

    let conn = crate::index::connection::open_database(&db_path)?;
    crate::index::migrations::ensure_compatible_schema(&conn)?;
    let record = crate::index::queries::status_record(&conn)?;
    let output = IndexStatusOutput {
        version: 1,
        repo_id: identity.repo_id,
        schema_version: crate::index::migrations::CURRENT_SCHEMA_VERSION,
        status: IndexStatusKind::Ready,
        indexed_commit_count: record.indexed_commit_count,
        indexed_file_count: record.indexed_file_count,
        updated_at: record.updated_at,
        warnings: Vec::new(),
    };
    crate::serialize_json(&output, args.common.pretty)
}
