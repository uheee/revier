use crate::cli::OverlayCommonArgs;
use crate::error::AppError;
use std::path::Path;

const INDEX_UNAVAILABLE_WARNING: &str = "索引不可用，已即时计算 overlay 结果";

pub struct AttributionContext<'repo> {
    pub repo: &'repo gix::Repository,
    pub db: Option<duckdb::Connection>,
    pub warnings: Vec<String>,
    pub range_hashes: Vec<String>,
}

impl<'repo> AttributionContext<'repo> {
    pub fn open(
        repo: &'repo gix::Repository,
        args: &OverlayCommonArgs,
    ) -> Result<AttributionContext<'repo>, AppError> {
        let identity = crate::git::repository::repository_identity(repo)?;
        let db_path = args
            .db
            .clone()
            .unwrap_or(crate::index::connection::default_database_path(
                &identity.repo_id,
            )?);
        let range_hashes = crate::git::commits::range_commit_hashes(repo, &args.base, &args.head)?;

        if !db_path.exists() {
            return Self::without_index(
                repo,
                args.require_index,
                range_hashes,
                format!("索引文件不存在：{}", display_path(&db_path)),
            );
        }

        let conn = crate::index::connection::open_database(&db_path)?;
        crate::index::migrations::ensure_compatible_schema(&conn)?;
        if !crate::index::queries::commit_exists(&conn, &args.head)? {
            return Self::without_index(
                repo,
                args.require_index,
                range_hashes,
                format!("查询范围 head {} 未被索引，请先执行 index build", args.head),
            );
        }
        if let Some(missing_hash) = first_missing_commit(&conn, &range_hashes)? {
            return Self::without_index(
                repo,
                args.require_index,
                range_hashes,
                format!("查询范围提交 {missing_hash} 未被索引，请先执行 index build"),
            );
        }

        Ok(Self {
            repo,
            db: Some(conn),
            warnings: Vec::new(),
            range_hashes,
        })
    }

    fn without_index(
        repo: &'repo gix::Repository,
        require_index: bool,
        range_hashes: Vec<String>,
        message: String,
    ) -> Result<Self, AppError> {
        if require_index {
            return Err(AppError::RequiredIndexUnavailable(message));
        }

        Ok(Self {
            repo,
            db: None,
            warnings: vec![INDEX_UNAVAILABLE_WARNING.to_string()],
            range_hashes,
        })
    }
}

fn display_path(path: &Path) -> String {
    path.display().to_string()
}

fn first_missing_commit(
    conn: &duckdb::Connection,
    hashes: &[String],
) -> Result<Option<String>, AppError> {
    for hash in hashes {
        if !crate::index::queries::commit_exists(conn, hash)? {
            return Ok(Some(hash.clone()));
        }
    }
    Ok(None)
}
