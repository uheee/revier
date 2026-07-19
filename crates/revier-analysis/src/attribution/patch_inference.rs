use crate::attribution::commit_lookup;
use crate::attribution::context::AttributionContext;
use crate::contracts::ResolvedTextEncoding;
use crate::error::AppError;
use crate::git::blob::read_text_at_commit_with_encoding;
use crate::git::commits::{author_key, IndexedCommit};
use crate::git::diff::{commit_file_changes, CommitFileChange};
use crate::json::{
    AuthorOutput, DiffBlockOutput, RelatedCommitAttributionOutput, RelatedCommitOutput,
    TouchedRangeOutput,
};
use crate::overlay::diff_builder::build_overlay_diff;
use std::collections::HashSet;

const PATCH_INFERENCE_METHOD: &str = "patch-inference";

pub struct AttributionOptions<'a> {
    pub encoding: ResolvedTextEncoding,
    pub authors: &'a [String],
    pub author_query: Option<&'a str>,
    pub message: Option<&'a str>,
}

pub fn attach_patch_inference(
    context: &AttributionContext<'_>,
    blocks: Vec<DiffBlockOutput>,
    file_path: &str,
    old_path: Option<&str>,
    options: &AttributionOptions<'_>,
) -> Result<Vec<DiffBlockOutput>, AppError> {
    let filter = FilterMatcher::new(options.authors, options.author_query, options.message);
    let mut candidates = Vec::new();
    let mut active_paths = initial_active_paths(file_path, old_path);

    for hash in &context.range_hashes {
        let commit = commit_lookup::get_commit(context, hash)?;
        let changes = commit_file_changes(context.repo, &commit.hash)?;
        let touched_ranges =
            touched_ranges_for_commit(context, &commit, &changes, &active_paths, options.encoding)?;
        advance_active_paths(&mut active_paths, &changes);
        if touched_ranges.is_empty() {
            continue;
        }
        candidates.push(RelatedCommitCandidate {
            matched_by_filter: filter.matches(&commit),
            commit,
            touched_ranges,
        });
    }

    Ok(blocks
        .into_iter()
        .map(|mut block| {
            block.related_commits = candidates
                .iter()
                .filter_map(|candidate| candidate.related_for_block(&block))
                .collect();
            block.authors = authors_from_related_commits(&block.related_commits);
            block
        })
        .collect())
}

fn touched_ranges_for_commit(
    context: &AttributionContext<'_>,
    commit: &IndexedCommit,
    changes: &[CommitFileChange],
    active_paths: &HashSet<String>,
    encoding: ResolvedTextEncoding,
) -> Result<Vec<TouchedRangeOutput>, AppError> {
    let mut touched_ranges = Vec::new();
    let mut seen = HashSet::new();

    for change in changes
        .iter()
        .filter(|change| change_touches_active_path(change, active_paths))
    {
        let old_text = match old_change_path(change) {
            Some(path) => read_text_at_commit_with_encoding(
                context.repo,
                &change.parent_hash,
                path,
                encoding,
            )?,
            None => String::new(),
        };
        let new_text = match new_change_path(change) {
            Some(path) => {
                read_text_at_commit_with_encoding(context.repo, &commit.hash, path, encoding)?
            }
            None => String::new(),
        };
        if old_text == new_text {
            continue;
        }

        for block in build_overlay_diff(&old_text, &new_text).blocks {
            let range = touched_range_from_block(&block);
            if seen.insert(range_key(&range)) {
                touched_ranges.push(range);
            }
        }
    }

    Ok(touched_ranges)
}

fn initial_active_paths(file_path: &str, old_path: Option<&str>) -> HashSet<String> {
    let mut paths = HashSet::new();
    paths.insert(old_path.unwrap_or(file_path).to_string());
    paths
}

fn change_touches_active_path(change: &CommitFileChange, active_paths: &HashSet<String>) -> bool {
    active_paths.contains(change.path.as_str())
        || change
            .old_path
            .as_deref()
            .is_some_and(|path| active_paths.contains(path))
}

fn advance_active_paths(active_paths: &mut HashSet<String>, changes: &[CommitFileChange]) {
    for change in changes {
        if let Some(old_path) = change.old_path.as_deref() {
            if active_paths.remove(old_path) {
                if change.status != "deleted" {
                    active_paths.insert(change.path.clone());
                }
                continue;
            }
        }

        if change.status == "deleted" {
            active_paths.remove(change.path.as_str());
        }
    }
}

fn old_change_path(change: &CommitFileChange) -> Option<&str> {
    match change.status.as_str() {
        "added" => None,
        "renamed" => Some(change.old_path.as_deref().unwrap_or(change.path.as_str())),
        _ => Some(change.path.as_str()),
    }
}

fn new_change_path(change: &CommitFileChange) -> Option<&str> {
    match change.status.as_str() {
        "deleted" => None,
        _ => Some(change.path.as_str()),
    }
}

fn touched_range_from_block(block: &DiffBlockOutput) -> TouchedRangeOutput {
    TouchedRangeOutput {
        old_start: non_zero(block.old_start),
        old_end: non_zero(block.old_end),
        new_start: non_zero(block.new_start),
        new_end: non_zero(block.new_end),
    }
}

fn non_zero(value: usize) -> Option<usize> {
    (value > 0).then_some(value)
}

fn range_key(
    range: &TouchedRangeOutput,
) -> (Option<usize>, Option<usize>, Option<usize>, Option<usize>) {
    (
        range.old_start,
        range.old_end,
        range.new_start,
        range.new_end,
    )
}

fn block_intersects_touched(block: &DiffBlockOutput, touched: &TouchedRangeOutput) -> bool {
    ranges_intersect(
        block.old_start,
        block.old_end,
        touched.old_start,
        touched.old_end,
    ) || ranges_intersect(
        block.new_start,
        block.new_end,
        touched.new_start,
        touched.new_end,
    )
}

fn ranges_intersect(
    block_start: usize,
    block_end: usize,
    touched_start: Option<usize>,
    touched_end: Option<usize>,
) -> bool {
    let Some(touched_start) = touched_start else {
        return false;
    };
    let Some(touched_end) = touched_end else {
        return false;
    };
    block_start > 0 && block_end > 0 && block_start <= touched_end && touched_start <= block_end
}

fn authors_from_related_commits(related_commits: &[RelatedCommitOutput]) -> Vec<AuthorOutput> {
    let mut authors = Vec::new();
    let mut seen = HashSet::new();
    for commit in related_commits {
        let key = author_key(&commit.author_name, commit.author_email.as_deref());
        if seen.insert(key) {
            authors.push(AuthorOutput {
                name: commit.author_name.clone(),
                email: commit.author_email.clone(),
            });
        }
    }
    authors
}

struct RelatedCommitCandidate {
    commit: IndexedCommit,
    matched_by_filter: bool,
    touched_ranges: Vec<TouchedRangeOutput>,
}

impl RelatedCommitCandidate {
    fn related_for_block(&self, block: &DiffBlockOutput) -> Option<RelatedCommitOutput> {
        let touched_ranges = self
            .touched_ranges
            .iter()
            .filter(|range| block_intersects_touched(block, range))
            .cloned()
            .collect::<Vec<_>>();
        if touched_ranges.is_empty() {
            return None;
        }

        Some(RelatedCommitOutput {
            hash: self.commit.hash.clone(),
            short_hash: self.commit.short_hash.clone(),
            author_name: self.commit.author_name.clone(),
            author_email: self.commit.author_email.clone(),
            committed_at: self.commit.committed_at.clone(),
            subject: self.commit.subject.clone(),
            matched_by_filter: self.matched_by_filter,
            touched_ranges,
            attribution: Some(RelatedCommitAttributionOutput {
                method: PATCH_INFERENCE_METHOD.to_string(),
                via_merge_hashes: Vec::new(),
            }),
        })
    }
}

pub(crate) struct FilterMatcher {
    authors: Vec<String>,
    author_query: Option<String>,
    message: Option<String>,
}

impl FilterMatcher {
    pub(crate) fn new(
        authors: &[String],
        author_query: Option<&str>,
        message: Option<&str>,
    ) -> Self {
        Self {
            authors: authors
                .iter()
                .map(|value| value.trim().to_lowercase())
                .filter(|value| !value.is_empty())
                .collect(),
            author_query: normalized_query(author_query),
            message: normalized_query(message),
        }
    }

    pub(crate) fn matches(&self, commit: &IndexedCommit) -> bool {
        if !self.is_active() {
            return true;
        }
        if !self.authors.is_empty() && !self.authors.contains(&commit.author_key) {
            return false;
        }
        if let Some(query) = &self.author_query {
            let author_text = format!(
                "{} {}",
                commit.author_name,
                commit.author_email.as_deref().unwrap_or_default()
            )
            .to_lowercase();
            if !author_text.contains(query) {
                return false;
            }
        }
        if let Some(query) = &self.message {
            if !commit.subject.to_lowercase().contains(query) {
                return false;
            }
        }
        true
    }

    fn is_active(&self) -> bool {
        !self.authors.is_empty() || self.author_query.is_some() || self.message.is_some()
    }
}

fn normalized_query(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_lowercase())
}
