use crate::attribution::commit_lookup;
use crate::attribution::context::AttributionContext;
use crate::attribution::patch_inference::FilterMatcher;
use crate::error::AppError;
use crate::git::blame::BlameLine;
use crate::git::commits::{author_key, IndexedCommit};
use crate::json::{
    AttributionWarningOutput, AuthorOutput, BlockAttributionOutput, DiffBlockOutput,
    RelatedCommitAttributionOutput, RelatedCommitOutput,
};
use std::collections::{HashMap, HashSet};

const BLAME_METHOD: &str = "blame";
const MERGE_TRACE_METHOD: &str = "merge-trace";
const PRECISE_CONFIDENCE: &str = "precise";
const INFERRED_CONFIDENCE: &str = "inferred";
const BLAME_UNAVAILABLE_CODE: &str = "BLAME_UNAVAILABLE";
const BLAME_UNAVAILABLE_MESSAGE: &str = "无法使用 blame 结果，已根据提交差异推断";
const DELETION_TRACE_INCOMPLETE_CODE: &str = "DELETION_TRACE_INCOMPLETE";
const DELETION_TRACE_INCOMPLETE_MESSAGE: &str = "删除块无法通过最终内容 blame，已根据提交差异推断";

pub fn attach_blame_attribution(
    context: &AttributionContext<'_>,
    head: &str,
    path: &str,
    blocks: Vec<DiffBlockOutput>,
    authors: &[String],
    author_query: Option<&str>,
    message: Option<&str>,
) -> Result<Vec<DiffBlockOutput>, AppError> {
    let filter = FilterMatcher::new(authors, author_query, message);

    blocks
        .into_iter()
        .map(|block| attach_block_blame_attribution(context, head, path, block, &filter))
        .collect()
}

fn attach_block_blame_attribution(
    context: &AttributionContext<'_>,
    head: &str,
    path: &str,
    block: DiffBlockOutput,
    filter: &FilterMatcher,
) -> Result<DiffBlockOutput, AppError> {
    attach_block_blame_attribution_with_resolver(block, |block| {
        related_commits_from_blame(context, head, path, block, filter)
    })
}

fn attach_block_blame_attribution_with_resolver<F>(
    mut block: DiffBlockOutput,
    resolve_blame: F,
) -> Result<DiffBlockOutput, AppError>
where
    F: FnOnce(&DiffBlockOutput) -> Result<BlameResolution, AppError>,
{
    if block.new_start == 0 || block.new_end == 0 {
        block.attribution = Some(inferred_attribution(
            DELETION_TRACE_INCOMPLETE_CODE,
            DELETION_TRACE_INCOMPLETE_MESSAGE,
        ));
        return Ok(block);
    }

    match resolve_blame(&block)? {
        BlameResolution::Resolved(related_commits) => {
            if related_commits.is_empty() {
                block.attribution = Some(inferred_attribution(
                    BLAME_UNAVAILABLE_CODE,
                    BLAME_UNAVAILABLE_MESSAGE,
                ));
            } else {
                block.related_commits = related_commits;
                block.authors = authors_from_related_commits(&block.related_commits);
                block.attribution = Some(precise_attribution());
            }
        }
        BlameResolution::Unavailable => {
            block.attribution = Some(inferred_attribution(
                BLAME_UNAVAILABLE_CODE,
                BLAME_UNAVAILABLE_MESSAGE,
            ));
        }
    }

    Ok(block)
}

fn related_commits_from_blame(
    context: &AttributionContext<'_>,
    head: &str,
    path: &str,
    block: &DiffBlockOutput,
    filter: &FilterMatcher,
) -> Result<BlameResolution, AppError> {
    let blame_lines =
        crate::git::blame::blame_range(context.repo, head, path, block.new_start, block.new_end);

    resolve_blame_range_result(block, blame_lines, filter, |hash| {
        commit_lookup::get_commit(context, hash)
    })
}

#[derive(Debug)]
enum BlameResolution {
    Resolved(Vec<RelatedCommitOutput>),
    Unavailable,
}

fn resolve_blame_range_result<F>(
    block: &DiffBlockOutput,
    blame_lines: Result<Vec<BlameLine>, AppError>,
    filter: &FilterMatcher,
    lookup_commit: F,
) -> Result<BlameResolution, AppError>
where
    F: FnMut(&str) -> Result<IndexedCommit, AppError>,
{
    resolve_related_commits_from_blame_lines(block, blame_lines?, filter, lookup_commit)
}

fn resolve_related_commits_from_blame_lines<F>(
    block: &DiffBlockOutput,
    blame_lines: Vec<BlameLine>,
    filter: &FilterMatcher,
    mut lookup_commit: F,
) -> Result<BlameResolution, AppError>
where
    F: FnMut(&str) -> Result<IndexedCommit, AppError>,
{
    if blame_lines.is_empty() {
        return Ok(BlameResolution::Unavailable);
    }

    let previous_by_hash = block
        .related_commits
        .iter()
        .map(|commit| (commit.hash.clone(), commit.clone()))
        .collect::<HashMap<_, _>>();
    let mut seen_hashes = HashSet::new();
    let mut related_commits = Vec::new();

    for blame_line in blame_lines {
        if !seen_hashes.insert(blame_line.commit_hash.clone()) {
            continue;
        }

        let commit = lookup_commit(&blame_line.commit_hash)?;
        let previous = previous_by_hash.get(&commit.hash);
        let matched_by_filter = matched_by_filter(previous, &commit, filter);
        let touched_ranges = previous
            .map(|commit| commit.touched_ranges.clone())
            .unwrap_or_default();
        let (method, via_merge_hashes) = if commit.is_merge {
            (MERGE_TRACE_METHOD.to_string(), vec![commit.hash.clone()])
        } else {
            (BLAME_METHOD.to_string(), Vec::new())
        };

        related_commits.push(RelatedCommitOutput {
            hash: commit.hash,
            short_hash: commit.short_hash,
            author_name: commit.author_name,
            author_email: commit.author_email,
            committed_at: commit.committed_at,
            subject: commit.subject,
            matched_by_filter,
            touched_ranges,
            attribution: Some(RelatedCommitAttributionOutput {
                method,
                via_merge_hashes,
            }),
        });
    }

    if related_commits.is_empty() {
        Ok(BlameResolution::Unavailable)
    } else {
        Ok(BlameResolution::Resolved(related_commits))
    }
}

fn matched_by_filter(
    previous: Option<&RelatedCommitOutput>,
    commit: &IndexedCommit,
    filter: &FilterMatcher,
) -> bool {
    previous
        .map(|commit| commit.matched_by_filter)
        .unwrap_or_else(|| filter.matches(commit))
}

fn precise_attribution() -> BlockAttributionOutput {
    BlockAttributionOutput {
        confidence: PRECISE_CONFIDENCE.to_string(),
        warnings: Vec::new(),
    }
}

fn inferred_attribution(code: &str, message: &str) -> BlockAttributionOutput {
    BlockAttributionOutput {
        confidence: INFERRED_CONFIDENCE.to_string(),
        warnings: vec![AttributionWarningOutput {
            code: code.to_string(),
            message: message.to_string(),
        }],
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attribution::patch_inference::FilterMatcher;
    use crate::git::blame::BlameLine;
    use crate::git::commits::IndexedCommit;
    use crate::json::{SideBySideDiffRowOutput, TouchedRangeOutput};

    #[test]
    fn missing_previous_blame_commit_matches_when_filter_is_inactive() {
        let commit = indexed_commit("Alice", Some("alice@example.com"), "feat: alice change");
        let filter = FilterMatcher::new(&[], None, None);

        assert!(matched_by_filter(None, &commit, &filter));
    }

    #[test]
    fn missing_previous_blame_commit_uses_active_filter() {
        let commit = indexed_commit("Alice", Some("alice@example.com"), "feat: alice change");
        let filter = FilterMatcher::new(&[], Some("bob@example.com"), None);

        assert!(!matched_by_filter(None, &commit, &filter));
    }

    #[test]
    fn previous_blame_candidate_match_result_has_priority() {
        let commit = indexed_commit("Alice", Some("alice@example.com"), "feat: alice change");
        let previous = related_commit(&commit, false);
        let filter = FilterMatcher::new(&[], None, None);

        assert!(!matched_by_filter(Some(&previous), &commit, &filter));
    }

    #[test]
    fn metadata_lookup_error_is_returned_instead_of_inferred_warning() {
        let block = diff_block(0, 0, 2, 2, Vec::new(), Vec::new());
        let filter = FilterMatcher::new(&[], None, None);
        let result = resolve_related_commits_from_blame_lines(
            &block,
            vec![BlameLine {
                line_number: 2,
                commit_hash: "missing-metadata".to_string(),
            }],
            &filter,
            |_| Err(AppError::Repository("metadata broken".to_string())),
        );

        assert!(
            matches!(result, Err(AppError::Repository(ref message)) if message == "metadata broken"),
            "元数据读取硬错误应返回，实际为 {result:?}"
        );
    }

    #[test]
    fn resolver_error_is_returned_instead_of_inferred_warning() {
        let block = diff_block(0, 0, 2, 2, Vec::new(), Vec::new());

        let result = attach_block_blame_attribution_with_resolver(block, |_| {
            Err(AppError::Repository("resolver broken".to_string()))
        });

        assert!(
            matches!(result, Err(AppError::Repository(ref message)) if message == "resolver broken"),
            "resolver 硬错误应返回，实际为 {result:?}"
        );
    }

    #[test]
    fn blame_range_error_is_returned_instead_of_unavailable() {
        let block = diff_block(0, 0, 2, 2, Vec::new(), Vec::new());
        let filter = FilterMatcher::new(&[], None, None);

        let result = resolve_blame_range_result(
            &block,
            Err(AppError::Repository("blame broken".to_string())),
            &filter,
            |_| panic!("blame_range 失败后不应读取提交元数据"),
        );

        assert!(
            matches!(result, Err(AppError::Repository(ref message)) if message == "blame broken"),
            "blame_range 硬错误应返回，实际为 {result:?}"
        );
    }

    #[test]
    fn unavailable_blame_keeps_patch_inference_and_sets_blame_warning() {
        let commit = indexed_commit("Alice", Some("alice@example.com"), "feat: alice change");
        let related_commit = related_commit(&commit, true);
        let author = AuthorOutput {
            name: commit.author_name.clone(),
            email: commit.author_email.clone(),
        };
        let block = diff_block(
            0,
            0,
            2,
            2,
            vec![author.clone()],
            vec![related_commit.clone()],
        );

        let result = attach_block_blame_attribution_with_resolver(block, |_| {
            Ok(BlameResolution::Unavailable)
        })
        .expect("blame 不可用 fallback 应成功");

        assert_eq!(result.related_commits.len(), 1);
        assert_eq!(result.related_commits[0].hash, related_commit.hash);
        assert_eq!(result.authors.len(), 1);
        assert_eq!(result.authors[0].name, author.name);
        assert_eq!(result.authors[0].email, author.email);
        let attribution = result.attribution.expect("fallback 应设置 attribution");
        assert_eq!(attribution.confidence, INFERRED_CONFIDENCE);
        assert_eq!(attribution.warnings.len(), 1);
        assert_eq!(attribution.warnings[0].code, BLAME_UNAVAILABLE_CODE);
    }

    #[test]
    fn deleted_block_keeps_patch_inference_and_sets_deletion_warning() {
        let commit = indexed_commit("Alice", Some("alice@example.com"), "feat: alice change");
        let related_commit = related_commit(&commit, true);
        let author = AuthorOutput {
            name: commit.author_name.clone(),
            email: commit.author_email.clone(),
        };
        let block = diff_block(
            2,
            2,
            0,
            0,
            vec![author.clone()],
            vec![related_commit.clone()],
        );

        let result = attach_block_blame_attribution_with_resolver(block, |_| {
            panic!("删除块不应执行 blame resolver")
        })
        .expect("删除块归因应成功");

        assert_eq!(result.related_commits.len(), 1);
        assert_eq!(result.related_commits[0].hash, related_commit.hash);
        assert_eq!(result.authors.len(), 1);
        assert_eq!(result.authors[0].name, author.name);
        assert_eq!(result.authors[0].email, author.email);
        let attribution = result.attribution.expect("删除块应设置 attribution");
        assert_eq!(attribution.confidence, INFERRED_CONFIDENCE);
        assert_eq!(attribution.warnings.len(), 1);
        assert_eq!(attribution.warnings[0].code, DELETION_TRACE_INCOMPLETE_CODE);
    }

    fn indexed_commit(name: &str, email: Option<&str>, subject: &str) -> IndexedCommit {
        IndexedCommit {
            hash: "0123456789abcdef0123456789abcdef01234567".to_string(),
            short_hash: "01234567".to_string(),
            author_name: name.to_string(),
            author_email: email.map(ToString::to_string),
            author_key: author_key(name, email),
            committed_at: "2026-05-02T00:00:00+00:00".to_string(),
            subject: subject.to_string(),
            parents: vec!["fedcba9876543210fedcba9876543210fedcba98".to_string()],
            is_merge: false,
        }
    }

    fn related_commit(commit: &IndexedCommit, matched_by_filter: bool) -> RelatedCommitOutput {
        RelatedCommitOutput {
            hash: commit.hash.clone(),
            short_hash: commit.short_hash.clone(),
            author_name: commit.author_name.clone(),
            author_email: commit.author_email.clone(),
            committed_at: commit.committed_at.clone(),
            subject: commit.subject.clone(),
            matched_by_filter,
            touched_ranges: vec![TouchedRangeOutput {
                old_start: None,
                old_end: None,
                new_start: Some(2),
                new_end: Some(2),
            }],
            attribution: None,
        }
    }

    fn diff_block(
        old_start: usize,
        old_end: usize,
        new_start: usize,
        new_end: usize,
        authors: Vec<AuthorOutput>,
        related_commits: Vec<RelatedCommitOutput>,
    ) -> DiffBlockOutput {
        DiffBlockOutput {
            id: "block-1".to_string(),
            old_start,
            old_end,
            new_start,
            new_end,
            row_start_index: None,
            row_end_index: None,
            change_type: if new_start == 0 {
                "deleted".to_string()
            } else {
                "added".to_string()
            },
            authors,
            rows: Vec::<SideBySideDiffRowOutput>::new(),
            related_commits,
            attribution: None,
        }
    }
}
