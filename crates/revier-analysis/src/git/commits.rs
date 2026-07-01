use crate::error::AppError;
use chrono::{DateTime, Utc};
use gix::bstr::ByteSlice;

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
    let tip = repo
        .rev_parse_single(branch)
        .map_err(|error| AppError::Repository(error.to_string()))?;
    let walk = repo
        .rev_walk([tip.detach()])
        .all()
        .map_err(|error| AppError::Repository(error.to_string()))?;

    let mut commits = Vec::new();
    for item in walk {
        let info = item.map_err(|error| AppError::Repository(error.to_string()))?;
        let commit = info
            .object()
            .map_err(|error| AppError::Repository(error.to_string()))?;
        commits.push(indexed_commit_from_gix(&commit)?);
    }
    Ok(commits)
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
