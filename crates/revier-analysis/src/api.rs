use std::path::Path;

use crate::cli::{FileOverlayArgs, QueryFilesArgs};
use crate::contracts::{AnalysisRange, GitBranch, RepositoryValidation};
use crate::error::AppError;
use crate::git::commits::IndexedCommit;
use crate::json::{FileOverlayCommandOutput, QueryFilesOutput};
use chrono::{DateTime, Duration, SecondsFormat, Timelike, Utc};

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

pub fn resolve_analysis_range(
    repo_path: &Path,
    branch: &str,
    start_at: Option<String>,
    end_at: Option<String>,
) -> Result<AnalysisRange, AppError> {
    let repo = gix::discover(repo_path).map_err(|error| AppError::Repository(error.to_string()))?;
    let end = normalize_utc_second(parse_or_default_time(
        end_at.as_deref(),
        Utc::now,
        "end_at",
    )?);
    let start = normalize_utc_second(match start_at.as_deref() {
        Some(value) => parse_time(value, "start_at")?,
        None => end - Duration::days(30),
    });
    if start > end {
        return Err(AppError::InvalidArgument(
            "start_at 不能晚于 end_at".to_string(),
        ));
    }

    let commits = crate::git::commits::list_reachable_commits(&repo, branch)
        .map_err(|error| normalize_range_commit_error(&repo, branch, error))?;
    if commits.is_empty() {
        return Err(no_available_commits_error(branch));
    }

    let mut commits = commits
        .into_iter()
        .map(|commit| parse_commit_time(&commit).map(|committed_at| (committed_at, commit)))
        .collect::<Result<Vec<_>, _>>()?;
    commits.sort_by_key(|(committed_at, _)| *committed_at);

    let base_commit = commits
        .iter()
        .rev()
        .find(|(committed_at, _)| *committed_at < start)
        .or_else(|| commits.first())
        .map(|(_, commit)| commit)
        .ok_or_else(|| no_available_commits_error(branch))?;
    let head_commit = commits
        .iter()
        .rev()
        .find(|(committed_at, _)| *committed_at <= end)
        .or_else(|| commits.last())
        .map(|(_, commit)| commit)
        .ok_or_else(|| no_available_commits_error(branch))?;

    Ok(AnalysisRange {
        branch: branch.to_string(),
        base_commit: base_commit.hash.clone(),
        head_commit: head_commit.hash.clone(),
        start_at: Some(format_utc_second(start)),
        end_at: Some(format_utc_second(end)),
    })
}

pub fn file_overlay(args: FileOverlayArgs) -> Result<FileOverlayCommandOutput, AppError> {
    crate::commands::file_overlay::analyze(args)
}

fn parse_or_default_time(
    value: Option<&str>,
    default: impl FnOnce() -> DateTime<Utc>,
    field: &str,
) -> Result<DateTime<Utc>, AppError> {
    match value {
        Some(value) => parse_time(value, field),
        None => Ok(default()),
    }
}

fn parse_time(value: &str, field: &str) -> Result<DateTime<Utc>, AppError> {
    DateTime::parse_from_rfc3339(value)
        .map(|time| time.with_timezone(&Utc))
        .map_err(|error| AppError::InvalidArgument(format!("{field} 时间格式无效：{error}")))
}

fn parse_commit_time(commit: &IndexedCommit) -> Result<DateTime<Utc>, AppError> {
    DateTime::parse_from_rfc3339(&commit.committed_at)
        .map(|time| time.with_timezone(&Utc))
        .map_err(|error| {
            AppError::Repository(format!(
                "提交 {} 的提交时间格式无效：{}",
                commit.hash, error
            ))
        })
}

fn format_utc_second(time: DateTime<Utc>) -> String {
    time.to_rfc3339_opts(SecondsFormat::Secs, false)
}

fn normalize_utc_second(time: DateTime<Utc>) -> DateTime<Utc> {
    time.with_nanosecond(0).expect("UTC 时间应支持设置纳秒为零")
}

fn normalize_range_commit_error(repo: &gix::Repository, branch: &str, error: AppError) -> AppError {
    if repo.head_id().is_err() {
        return no_available_commits_error(branch);
    }

    match error {
        AppError::Repository(message) if message.contains("Spec could not be found") => {
            no_available_commits_error(branch)
        }
        other => other,
    }
}

fn no_available_commits_error(branch: &str) -> AppError {
    AppError::InvalidArgument(format!("分支 {branch} 没有可用提交，无法解析分析范围"))
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_string()
}
