use crate::error::AppError;
use std::path::Path;

pub fn open_repository(path: &Path) -> Result<gix::Repository, AppError> {
    gix::open(path).map_err(|error| AppError::Repository(error.to_string()))
}

pub fn parent_ids(repo: &gix::Repository, commit_hash: &str) -> Result<Vec<String>, AppError> {
    let object = repo
        .rev_parse_single(commit_hash)
        .map_err(|error| AppError::Repository(error.to_string()))?;
    let commit = object
        .object()
        .map_err(|error| AppError::Repository(error.to_string()))?
        .try_into_commit()
        .map_err(|_| AppError::Repository(format!("对象不是提交：{commit_hash}")))?;

    Ok(commit.parent_ids().map(|id| id.to_string()).collect())
}
