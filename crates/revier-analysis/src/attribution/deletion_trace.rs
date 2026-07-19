use crate::attribution::commit_lookup;
use crate::attribution::context::AttributionContext;
use crate::attribution::patch_inference::{AttributionOptions, FilterMatcher};
use crate::contracts::ResolvedTextEncoding;
use crate::error::AppError;
use crate::git::blob::read_text_at_commit_with_encoding;
use crate::git::commits::{author_key, IndexedCommit};
use crate::git::diff::{commit_file_changes, CommitFileChange};
use crate::json::{
    AuthorOutput, BlockAttributionOutput, DiffBlockOutput, RelatedCommitAttributionOutput,
    RelatedCommitOutput, TouchedRangeOutput,
};
use crate::overlay::diff_builder::build_overlay_diff;
use std::collections::HashSet;

const DELETION_TRACE_METHOD: &str = "deletion-trace";
const PRECISE_CONFIDENCE: &str = "precise";

pub fn attach_deletion_trace(
    context: &AttributionContext<'_>,
    blocks: Vec<DiffBlockOutput>,
    candidate_paths: &[String],
    file_path: &str,
    old_path: Option<&str>,
    options: &AttributionOptions<'_>,
) -> Result<Vec<DiffBlockOutput>, AppError> {
    let filter = FilterMatcher::new(options.authors, options.author_query, options.message);
    let scope = DeletionTraceScope {
        candidate_paths,
        file_path,
        old_path,
        filter: &filter,
        encoding: options.encoding,
    };
    let mut traced_blocks = Vec::new();

    for block in blocks {
        traced_blocks.push(attach_block_deletion_trace(context, block, &scope)?);
    }

    Ok(traced_blocks)
}

fn attach_block_deletion_trace(
    context: &AttributionContext<'_>,
    mut block: DiffBlockOutput,
    scope: &DeletionTraceScope<'_>,
) -> Result<DiffBlockOutput, AppError> {
    if !has_old_side_deletion(&block) {
        return Ok(block);
    }

    let deleted_lines = deleted_old_lines(&block);
    if deleted_lines.is_empty() {
        return Ok(block);
    }

    let related_commits = find_deletion_commits(context, &block, &deleted_lines, scope)?;
    if related_commits.is_empty() {
        return Ok(block);
    }

    block.related_commits = related_commits;
    block.authors = authors_from_related_commits(&block.related_commits);
    block.attribution = Some(BlockAttributionOutput {
        confidence: PRECISE_CONFIDENCE.to_string(),
        warnings: Vec::new(),
    });
    Ok(block)
}

fn find_deletion_commits(
    context: &AttributionContext<'_>,
    block: &DiffBlockOutput,
    deleted_lines: &[String],
    scope: &DeletionTraceScope<'_>,
) -> Result<Vec<RelatedCommitOutput>, AppError> {
    let mut candidates = Vec::new();
    let mut seen = HashSet::new();
    let candidate_paths =
        candidate_path_set(scope.candidate_paths, scope.file_path, scope.old_path);
    let mut active_paths = initial_active_paths(scope.file_path, scope.old_path, &candidate_paths);

    for hash in &context.range_hashes {
        let commit = commit_lookup::get_commit(context, hash)?;
        let changes = commit_file_changes(context.repo, &commit.hash)?;

        if commit.is_merge {
            advance_active_paths(&mut active_paths, &changes);
            continue;
        }

        for change in changes
            .iter()
            .filter(|change| change_touches_active_path(change, &active_paths))
        {
            let touched_ranges = deletion_touched_ranges(
                context,
                &commit,
                change,
                block,
                deleted_lines,
                scope.encoding,
            )?;
            if touched_ranges.len() != 1 {
                continue;
            }
            if seen.insert(commit.hash.clone()) {
                let matched_by_filter = scope.filter.matches(&commit);
                candidates.push(DeletionTraceCandidate {
                    commit,
                    matched_by_filter,
                    touched_ranges,
                });
            }
            break;
        }
        advance_active_paths(&mut active_paths, &changes);
    }

    if candidates.len() != 1 {
        return Ok(Vec::new());
    }

    Ok(candidates
        .pop()
        .map(DeletionTraceCandidate::into_related_commit)
        .into_iter()
        .collect())
}

struct DeletionTraceScope<'a> {
    candidate_paths: &'a [String],
    file_path: &'a str,
    old_path: Option<&'a str>,
    filter: &'a FilterMatcher,
    encoding: ResolvedTextEncoding,
}

fn deletion_touched_ranges(
    context: &AttributionContext<'_>,
    commit: &IndexedCommit,
    change: &CommitFileChange,
    block: &DiffBlockOutput,
    deleted_lines: &[String],
    encoding: ResolvedTextEncoding,
) -> Result<Vec<TouchedRangeOutput>, AppError> {
    let Some(old_path) = old_change_path(change) else {
        return Ok(Vec::new());
    };
    let old_text =
        read_text_at_commit_with_encoding(context.repo, &change.parent_hash, old_path, encoding)?;
    let new_text = match new_change_path(change) {
        Some(path) => {
            read_text_at_commit_with_encoding(context.repo, &commit.hash, path, encoding)?
        }
        None => String::new(),
    };
    if old_text == new_text {
        return Ok(Vec::new());
    }

    let overlay_window_matches =
        line_window_matches(&old_text, block.old_start, block.old_end, deleted_lines);
    let mut ranges = Vec::new();
    for commit_block in build_overlay_diff(&old_text, &new_text).blocks {
        if !has_old_side_deletion(&commit_block) {
            continue;
        }
        if deleted_old_lines(&commit_block) != deleted_lines {
            continue;
        }
        if !commit_block_anchors_overlay_block(block, &commit_block, overlay_window_matches) {
            continue;
        }
        ranges.push(touched_range_from_block(&commit_block));
    }

    Ok(ranges)
}

fn commit_block_anchors_overlay_block(
    overlay_block: &DiffBlockOutput,
    commit_block: &DiffBlockOutput,
    overlay_window_matches: bool,
) -> bool {
    let old_side_anchored = block_ranges_intersect(overlay_block, commit_block)
        || (overlay_window_matches
            && overlay_block.old_start == commit_block.old_start
            && overlay_block.old_end == commit_block.old_end);
    if !old_side_anchored {
        return false;
    }

    if overlay_block.change_type == "modified" {
        return new_side_lines(overlay_block) == new_side_lines(commit_block);
    }

    true
}

fn candidate_path_set(
    candidate_paths: &[String],
    file_path: &str,
    old_path: Option<&str>,
) -> HashSet<String> {
    let mut paths = candidate_paths.iter().cloned().collect::<HashSet<_>>();
    paths.insert(file_path.to_string());
    if let Some(old_path) = old_path {
        paths.insert(old_path.to_string());
    }
    paths
}

fn initial_active_paths(
    file_path: &str,
    old_path: Option<&str>,
    candidate_paths: &HashSet<String>,
) -> HashSet<String> {
    let mut paths = HashSet::new();
    let primary_path = old_path.unwrap_or(file_path);
    if candidate_paths.contains(primary_path) {
        paths.insert(primary_path.to_string());
    }
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

fn has_old_side_deletion(block: &DiffBlockOutput) -> bool {
    block.rows.iter().any(|row| row.old_line_number.is_some()) && block.change_type != "added"
}

fn deleted_old_lines(block: &DiffBlockOutput) -> Vec<String> {
    block
        .rows
        .iter()
        .filter(|row| row.old_line_number.is_some())
        .filter_map(|row| row.old_text.clone())
        .collect()
}

fn new_side_lines(block: &DiffBlockOutput) -> Vec<String> {
    block
        .rows
        .iter()
        .filter(|row| row.new_line_number.is_some())
        .filter_map(|row| row.new_text.clone())
        .collect()
}

fn line_window_matches(
    text: &str,
    start_line: usize,
    end_line: usize,
    expected_lines: &[String],
) -> bool {
    if start_line == 0 || end_line < start_line {
        return false;
    }
    let lines = split_text_lines(text);
    let start = start_line - 1;
    let end = end_line;
    end <= lines.len() && lines[start..end] == *expected_lines
}

fn split_text_lines(text: &str) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }
    let trimmed = text.strip_suffix('\n').unwrap_or(text);
    trimmed.split('\n').map(ToString::to_string).collect()
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

fn block_ranges_intersect(left: &DiffBlockOutput, right: &DiffBlockOutput) -> bool {
    ranges_intersect(left.old_start, left.old_end, right.old_start, right.old_end)
        || ranges_intersect(left.new_start, left.new_end, right.new_start, right.new_end)
}

fn ranges_intersect(
    left_start: usize,
    left_end: usize,
    right_start: usize,
    right_end: usize,
) -> bool {
    left_start > 0
        && left_end > 0
        && right_start > 0
        && right_end > 0
        && left_start <= right_end
        && right_start <= left_end
}

struct DeletionTraceCandidate {
    commit: IndexedCommit,
    matched_by_filter: bool,
    touched_ranges: Vec<TouchedRangeOutput>,
}

impl DeletionTraceCandidate {
    fn into_related_commit(self) -> RelatedCommitOutput {
        RelatedCommitOutput {
            hash: self.commit.hash,
            short_hash: self.commit.short_hash,
            author_name: self.commit.author_name,
            author_email: self.commit.author_email,
            committed_at: self.commit.committed_at,
            subject: self.commit.subject,
            matched_by_filter: self.matched_by_filter,
            touched_ranges: self.touched_ranges,
            attribution: Some(RelatedCommitAttributionOutput {
                method: DELETION_TRACE_METHOD.to_string(),
                via_merge_hashes: Vec::new(),
            }),
        }
    }
}
