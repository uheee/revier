use crate::attribution::commit_lookup;
use crate::attribution::context::AttributionContext;
use crate::attribution::patch_inference::FilterMatcher;
use crate::error::AppError;
use crate::git::blob::read_text_at_commit;
use crate::git::commits::IndexedCommit;
use crate::json::{
    AttributionWarningOutput, BlockAttributionOutput, DiffBlockOutput,
    RelatedCommitAttributionOutput, RelatedCommitOutput,
};
use std::collections::HashMap;

const MERGE_TRACE_METHOD: &str = "merge-trace";
const PARTIAL_CONFIDENCE: &str = "partial";
const MERGE_TRACE_AMBIGUOUS_CODE: &str = "MERGE_TRACE_AMBIGUOUS";
const MERGE_TRACE_AMBIGUOUS_MESSAGE: &str = "多个父提交都能解释 merge 结果中的块内容";

#[derive(Debug)]
pub struct MergeTraceOutcome {
    pub related_commits: Vec<RelatedCommitOutput>,
    pub ambiguous: bool,
}

#[derive(Default)]
pub(crate) struct MergeTraceTextCache {
    texts: HashMap<(String, String), String>,
}

impl MergeTraceTextCache {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    fn read_text(
        &mut self,
        context: &AttributionContext<'_>,
        commit_hash: &str,
        file_path: &str,
    ) -> Result<&str, AppError> {
        let key = (commit_hash.to_string(), file_path.to_string());
        if !self.texts.contains_key(&key) {
            let text = read_text_at_commit(context.repo, commit_hash, file_path)?;
            self.texts.insert(key.clone(), text);
        }
        Ok(self.texts.get(&key).expect("缓存条目刚写入").as_str())
    }
}

pub fn partial_attribution() -> BlockAttributionOutput {
    BlockAttributionOutput {
        confidence: PARTIAL_CONFIDENCE.to_string(),
        warnings: vec![AttributionWarningOutput {
            code: MERGE_TRACE_AMBIGUOUS_CODE.to_string(),
            message: MERGE_TRACE_AMBIGUOUS_MESSAGE.to_string(),
        }],
    }
}

pub(crate) fn resolve_merge_commit(
    context: &AttributionContext<'_>,
    merge_commit: &IndexedCommit,
    file_path: &str,
    block: &DiffBlockOutput,
    filter: &FilterMatcher,
    previous_related_commits: &[RelatedCommitOutput],
    text_cache: &mut MergeTraceTextCache,
) -> Result<Option<MergeTraceOutcome>, AppError> {
    if !merge_commit.is_merge {
        return Ok(None);
    }

    let block_lines = block_new_lines(block);
    if block_lines.is_empty() {
        return Ok(None);
    }

    let merge_text = text_cache.read_text(context, &merge_commit.hash, file_path)?;
    if !text_window_matches(merge_text, block, &block_lines) {
        return Ok(None);
    }

    let mut parent_commits = Vec::new();
    for parent_hash in &merge_commit.parents {
        let parent_text = text_cache.read_text(context, parent_hash, file_path)?;
        if text_window_matches(parent_text, block, &block_lines) {
            parent_commits.push(commit_lookup::get_commit(context, parent_hash)?);
        }
    }

    let ambiguous = parent_commits.len() > 1;
    let commits = if parent_commits.is_empty() {
        vec![merge_commit.clone()]
    } else {
        parent_commits
    };
    let related_commits = commits
        .iter()
        .map(|commit| {
            related_commit_for_merge_trace(
                commit,
                &merge_commit.hash,
                filter,
                previous_related_commits,
            )
        })
        .collect();

    Ok(Some(MergeTraceOutcome {
        related_commits,
        ambiguous,
    }))
}

fn related_commit_for_merge_trace(
    commit: &IndexedCommit,
    merge_hash: &str,
    filter: &FilterMatcher,
    previous_related_commits: &[RelatedCommitOutput],
) -> RelatedCommitOutput {
    let previous = previous_related_commits
        .iter()
        .find(|previous| previous.hash == commit.hash);

    RelatedCommitOutput {
        hash: commit.hash.clone(),
        short_hash: commit.short_hash.clone(),
        author_name: commit.author_name.clone(),
        author_email: commit.author_email.clone(),
        committed_at: commit.committed_at.clone(),
        subject: commit.subject.clone(),
        matched_by_filter: previous
            .map(|commit| commit.matched_by_filter)
            .unwrap_or_else(|| filter.matches(commit)),
        touched_ranges: previous
            .map(|commit| commit.touched_ranges.clone())
            .unwrap_or_default(),
        attribution: Some(RelatedCommitAttributionOutput {
            method: MERGE_TRACE_METHOD.to_string(),
            via_merge_hashes: vec![merge_hash.to_string()],
        }),
    }
}

fn block_new_lines(block: &DiffBlockOutput) -> Vec<String> {
    block
        .rows
        .iter()
        .filter_map(|row| {
            row.new_line_number
                .map(|_| row.new_text.clone().unwrap_or_default())
        })
        .collect()
}

fn text_window_matches(text: &str, block: &DiffBlockOutput, expected_lines: &[String]) -> bool {
    if expected_lines.is_empty() {
        return false;
    }
    let Some(window) = line_window(text, block.new_start, block.new_end) else {
        return false;
    };
    window == expected_lines
}

fn line_window(text: &str, start_line: usize, end_line: usize) -> Option<Vec<String>> {
    if start_line == 0 || end_line < start_line {
        return None;
    }

    let lines = split_text_lines(text);
    let start = start_line - 1;
    let end = end_line;
    (end <= lines.len()).then(|| lines[start..end].to_vec())
}

fn split_text_lines(text: &str) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }
    let trimmed = text.strip_suffix('\n').unwrap_or(text);
    trimmed.split('\n').map(ToString::to_string).collect()
}
