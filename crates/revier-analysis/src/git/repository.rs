use crate::error::AppError;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct RepositoryIdentity {
    pub repo_id: String,
    pub repo_root: String,
    pub git_common_dir: String,
}

pub fn open_repository(path: &Path) -> Result<gix::Repository, AppError> {
    gix::open(path).map_err(|error| AppError::Repository(error.to_string()))
}

pub fn repository_identity(repo: &gix::Repository) -> Result<RepositoryIdentity, AppError> {
    let repo_root = normalize_path(repo.workdir().unwrap_or_else(|| repo.path()));
    let git_common_dir = normalize_path(repo.common_dir());
    let mut hasher = Sha256::new();
    hasher.update(repo_root.as_bytes());
    hasher.update(b"\0");
    hasher.update(git_common_dir.as_bytes());
    let repo_id = hex::encode(hasher.finalize());

    Ok(RepositoryIdentity {
        repo_id,
        repo_root,
        git_common_dir,
    })
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

fn normalize_path(path: &Path) -> String {
    path.components()
        .collect::<PathBuf>()
        .to_string_lossy()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_string()
}
