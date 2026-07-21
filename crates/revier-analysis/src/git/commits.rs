use crate::error::AppError;
use crate::execution::AnalysisExecutionContext;
use chrono::{DateTime, Utc};
use gix::bstr::ByteSlice;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct IndexedCommit {
    pub hash: String,
    pub short_hash: String,
    pub author_name: String,
    pub author_email: Option<String>,
    pub author_key: String,
    pub committed_at: String,
    pub subject: String,
    pub parents: Vec<String>,
    pub is_merge: bool,
}

pub fn list_reachable_commits(
    repo: &gix::Repository,
    branch: &str,
) -> Result<Vec<IndexedCommit>, AppError> {
    let context = AnalysisExecutionContext::none();
    list_reachable_commits_with_context(repo, branch, &context)
}

pub fn list_reachable_commits_with_context(
    repo: &gix::Repository,
    branch: &str,
    context: &AnalysisExecutionContext,
) -> Result<Vec<IndexedCommit>, AppError> {
    context.check_cancelled()?;
    let tip = repo
        .rev_parse_single(branch)
        .map_err(|error| AppError::Repository(error.to_string()))?;
    context.check_cancelled()?;
    let walk = repo
        .rev_walk([tip.detach()])
        .all()
        .map_err(|error| AppError::Repository(error.to_string()))?;

    let mut commits = Vec::new();
    for item in walk {
        context.check_cancelled()?;
        let info = item.map_err(|error| AppError::Repository(error.to_string()))?;
        let commit = info
            .object()
            .map_err(|error| AppError::Repository(error.to_string()))?;
        context.check_cancelled()?;
        commits.push(indexed_commit_from_gix(&commit)?);
    }
    Ok(commits)
}

pub fn all_local_branch_reachable_hashes(
    repo: &gix::Repository,
    context: &AnalysisExecutionContext,
) -> Result<Vec<String>, AppError> {
    context.check_cancelled()?;
    let references = repo
        .references()
        .map_err(|error| AppError::Repository(error.to_string()))?;
    let mut tips = Vec::new();
    for reference in references
        .local_branches()
        .map_err(|error| AppError::Repository(error.to_string()))?
    {
        context.check_cancelled()?;
        let mut reference = reference.map_err(|error| AppError::Repository(error.to_string()))?;
        tips.push(
            reference
                .peel_to_id()
                .map_err(|error| AppError::Repository(error.to_string()))?
                .detach(),
        );
    }
    if tips.is_empty() {
        return Ok(Vec::new());
    }

    let walk = repo
        .rev_walk(tips)
        .all()
        .map_err(|error| AppError::Repository(error.to_string()))?;
    let mut hashes = Vec::new();
    for item in walk {
        context.check_cancelled()?;
        hashes.push(
            item.map_err(|error| AppError::Repository(error.to_string()))?
                .id
                .to_string(),
        );
    }
    Ok(hashes)
}

pub fn get_commit(repo: &gix::Repository, commit_hash: &str) -> Result<IndexedCommit, AppError> {
    let object = repo
        .rev_parse_single(commit_hash)
        .map_err(|error| AppError::Repository(format!("无法解析提交 {commit_hash}：{error}")))?;
    let commit = object
        .object()
        .map_err(|error| AppError::Repository(error.to_string()))?
        .try_into_commit()
        .map_err(|_| AppError::Repository(format!("对象不是提交：{commit_hash}")))?;

    indexed_commit_from_gix(&commit)
}

pub fn range_commit_hashes(
    repo: &gix::Repository,
    base_commit: &str,
    head_commit: &str,
) -> Result<Vec<String>, AppError> {
    let base = repo
        .rev_parse_single(base_commit)
        .map_err(|error| {
            AppError::Repository(format!("无法解析 base 提交 {base_commit}：{error}"))
        })?
        .detach();
    let head = repo
        .rev_parse_single(head_commit)
        .map_err(|error| {
            AppError::Repository(format!("无法解析 head 提交 {head_commit}：{error}"))
        })?
        .detach();

    if base == head {
        return Ok(Vec::new());
    }

    let base_walk = repo
        .rev_walk([base])
        .all()
        .map_err(|error| AppError::Repository(error.to_string()))?;
    let mut base_reachable = HashSet::new();
    for item in base_walk {
        let info = item.map_err(|error| AppError::Repository(error.to_string()))?;
        base_reachable.insert(info.id);
    }

    let head_walk = repo
        .rev_walk([head])
        .all()
        .map_err(|error| AppError::Repository(error.to_string()))?;
    let mut hashes = Vec::new();
    for item in head_walk {
        let info = item.map_err(|error| AppError::Repository(error.to_string()))?;
        if !base_reachable.contains(&info.id) {
            hashes.push(info.id.to_string());
        }
    }

    hashes.reverse();
    Ok(hashes)
}

pub fn is_commit_reachable_from(
    repo: &gix::Repository,
    commit_hash: &str,
    head_hash: &str,
) -> Result<bool, AppError> {
    let target = repo
        .rev_parse_single(commit_hash)
        .map_err(|error| AppError::Repository(format!("无法解析目标提交 {commit_hash}：{error}")))?
        .detach();
    let head = repo
        .rev_parse_single(head_hash)
        .map_err(|error| AppError::Repository(format!("无法解析 head 提交 {head_hash}：{error}")))?
        .detach();
    let walk = repo
        .rev_walk([head])
        .all()
        .map_err(|error| AppError::Repository(error.to_string()))?;

    for item in walk {
        let info = item.map_err(|error| AppError::Repository(error.to_string()))?;
        if info.id == target {
            return Ok(true);
        }
    }
    Ok(false)
}

pub fn indexed_commit_from_gix(commit: &gix::Commit<'_>) -> Result<IndexedCommit, AppError> {
    let author = commit
        .author()
        .map_err(|error| AppError::Repository(error.to_string()))?;
    let author_name = author.name.to_str_lossy().trim().to_string();
    let author_email = author.email.to_str_lossy().trim().to_string();
    let author_email = (!author_email.is_empty()).then_some(author_email);
    let author_key = author_key(&author_name, author_email.as_deref());
    let parents = commit
        .parent_ids()
        .map(|id| id.to_string())
        .collect::<Vec<_>>();
    let committed_at = gix_time_to_iso(
        commit
            .time()
            .map_err(|error| AppError::Repository(error.to_string()))?,
    );
    let subject = commit
        .message_raw()
        .map_err(|error| AppError::Repository(error.to_string()))?
        .lines()
        .next()
        .map(|line| line.to_str_lossy().to_string())
        .unwrap_or_default();

    Ok(IndexedCommit {
        hash: commit.id.to_string(),
        short_hash: commit.id.to_string().chars().take(8).collect(),
        author_name,
        author_email,
        author_key,
        committed_at,
        subject,
        is_merge: parents.len() > 1,
        parents,
    })
}

pub fn author_key(name: &str, email: Option<&str>) -> String {
    email
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(name)
        .trim()
        .to_lowercase()
}

fn gix_time_to_iso(time: gix::date::Time) -> String {
    let seconds = time.seconds;
    DateTime::<Utc>::from_timestamp(seconds, 0)
        .expect("gix 提交时间必须可转换为 UTC")
        .to_rfc3339()
}
