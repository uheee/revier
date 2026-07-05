use std::path::Path;

use crate::cli::{FileOverlayArgs, QueryFilesArgs};
use crate::contracts::{GitBranch, RepositoryValidation};
use crate::error::AppError;
use crate::json::{FileOverlayCommandOutput, QueryFilesOutput};

pub fn validate_repository(repo_path: &Path) -> Result<RepositoryValidation, AppError> {
    let repo_path_string = normalize_path(repo_path);
    match gix::discover(repo_path) {
        Ok(repo) => {
            let workdir = repo
                .workdir()
                .map(normalize_path)
                .unwrap_or_else(|| repo_path_string.clone());
            let current_branch = repo
                .head_name()
                .ok()
                .flatten()
                .map(|name| name.shorten().to_string());
            Ok(RepositoryValidation {
                valid: true,
                repo_path: workdir,
                current_branch,
                error: None,
            })
        }
        Err(_) => Ok(RepositoryValidation {
            valid: false,
            repo_path: repo_path_string,
            current_branch: None,
            error: Some("请选择一个 Git 仓库目录".to_string()),
        }),
    }
}

pub fn list_branches(repo_path: &Path) -> Result<Vec<GitBranch>, AppError> {
    let repo = gix::discover(repo_path).map_err(|error| AppError::Repository(error.to_string()))?;
    let current = repo
        .head_name()
        .ok()
        .flatten()
        .map(|name| name.shorten().to_string());
    let mut branches = Vec::new();

    let references = repo
        .references()
        .map_err(|error| AppError::Repository(error.to_string()))?;
    for reference in references
        .local_branches()
        .map_err(|error| AppError::Repository(error.to_string()))?
    {
        let reference = reference.map_err(|error| AppError::Repository(error.to_string()))?;
        let name = reference.name().shorten().to_string();
        branches.push(GitBranch {
            current: current.as_deref() == Some(name.as_str()),
            name,
        });
    }

    if let Some(current) = current {
        if !branches.iter().any(|branch| branch.name == current) {
            branches.push(GitBranch {
                name: current,
                current: true,
            });
        }
    }

    Ok(branches)
}

pub fn query_files(args: QueryFilesArgs) -> Result<QueryFilesOutput, AppError> {
    crate::commands::query_files::query(args)
}

pub fn file_overlay(args: FileOverlayArgs) -> Result<FileOverlayCommandOutput, AppError> {
    crate::commands::file_overlay::analyze(args)
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_string()
}
