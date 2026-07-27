use crate::attribution::commit_lookup;
use crate::attribution::context::AttributionContext;
use crate::attribution::patch_inference::FilterMatcher;
use crate::attribution::path_history;
use crate::contracts::ResolvedTextEncoding;
use crate::error::AppError;
use crate::git::blob::read_text_at_commit_with_encoding;
use crate::git::commits::IndexedCommit;
use crate::json::{
    AttributionWarningOutput, BlockAttributionOutput, DiffBlockOutput,
    RelatedCommitAttributionOutput, RelatedCommitOutput,
};
use std::collections::{HashMap, HashSet};

const MERGE_TRACE_METHOD: &str = "merge-trace";
const PARTIAL_CONFIDENCE: &str = "partial";
const MERGE_TRACE_AMBIGUOUS_CODE: &str = "MERGE_TRACE_AMBIGUOUS";
const MERGE_TRACE_AMBIGUOUS_MESSAGE: &str = "多个父提交都能解释 merge 结果中的块内容";

#[derive(Debug)]
pub struct MergeTraceOutcome {
    pub related_commits: Vec<RelatedCommitOutput>,
    pub ambiguous: bool,
    pub replaced_merge_commit: bool,
}

pub(crate) struct MergeTraceTextCache {
    encoding: ResolvedTextEncoding,
    texts: HashMap<(String, String), String>,
}

impl MergeTraceTextCache {
    pub(crate) fn new(encoding: ResolvedTextEncoding) -> Self {
        Self {
            encoding,
            texts: HashMap::new(),
        }
    }

    fn read_text(
        &mut self,
        context: &AttributionContext<'_>,
        commit_hash: &str,
        file_path: &str,
    ) -> Result<&str, AppError> {
        let key = (commit_hash.to_string(), file_path.to_string());
        if !self.texts.contains_key(&key) {
            let text = read_text_at_commit_with_encoding(
                context.repo,
                commit_hash,
                file_path,
                self.encoding,
            )?;
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

    let mut state = MergeTraceState::new();
    state.resolve(
        context,
        merge_commit,
        file_path,
        block,
        filter,
        previous_related_commits,
        text_cache,
        Vec::new(),
        false,
    )
}

#[derive(Debug)]
struct MergeTraceState {
    visited: HashSet<(String, String, String)>,
}

impl MergeTraceState {
    fn new() -> Self {
        Self {
            visited: HashSet::new(),
        }
    }

    fn resolve(
        &mut self,
        context: &AttributionContext<'_>,
        commit: &IndexedCommit,
        file_path: &str,
        block: &DiffBlockOutput,
        filter: &FilterMatcher,
        previous_related_commits: &[RelatedCommitOutput],
        text_cache: &mut MergeTraceTextCache,
        via_merge_hashes: Vec<String>,
        allow_direct_merge: bool,
    ) -> Result<Option<MergeTraceOutcome>, AppError> {
        let mut via_merge_hashes = via_merge_hashes;
        if commit.is_merge && !via_merge_hashes.iter().any(|hash| hash == &commit.hash) {
            via_merge_hashes.push(commit.hash.clone());
        }

        let block_signature = build_block_signature(file_path, block);
        if !self
            .visited
            .insert((commit.hash.clone(), file_path.to_string(), block_signature))
        {
            return Ok(None);
        }

        if !commit.is_merge {
            return Ok(Some(MergeTraceOutcome {
                related_commits: vec![related_commit_for_merge_trace(
                    commit,
                    &via_merge_hashes,
                    filter,
                    previous_related_commits,
                )],
                ambiguous: false,
                replaced_merge_commit: false,
            }));
        }

        let block_lines = block_new_lines(block);
        let commit_text = text_cache.read_text(context, &commit.hash, file_path)?;
        if !text_window_matches(commit_text, block, &block_lines) {
            return Ok(None);
        }

        let mut related_commits = Vec::new();
        let mut ambiguous = false;
        let mut has_parent_candidate = false;
        let mut replaced_merge_commit = false;

        for parent_hash in &commit.parents {
            let parent_commit = commit_lookup::get_commit(context, parent_hash)?;
            let candidates = parent_candidates(
                context,
                text_cache,
                &parent_commit,
                commit,
                file_path,
                block,
                &block_lines,
            )?;

            if candidates.is_empty() {
                continue;
            }
            has_parent_candidate = true;

            for (path, mapped_block, has_ambiguous_match) in candidates {
                if has_ambiguous_match {
                    ambiguous = true;
                    continue;
                }

                let next_via = via_merge_hashes.clone();
                if let Some(outcome) = self.resolve(
                    context,
                    &parent_commit,
                    &path,
                    &mapped_block,
                    filter,
                    previous_related_commits,
                    text_cache,
                    next_via,
                    true,
                )? {
                    related_commits.extend(outcome.related_commits);
                    ambiguous |= outcome.ambiguous;
                    replaced_merge_commit |=
                        outcome.replaced_merge_commit || !parent_commit.is_merge;
                }
            }
        }

        if has_parent_candidate {
            if related_commits.is_empty() {
                return Ok(None);
            }
            deduplicate_related_commits(&mut related_commits);
            ambiguous |= related_commits.len() > 1;
            return Ok(Some(MergeTraceOutcome {
                related_commits,
                ambiguous,
                replaced_merge_commit: true,
            }));
        }

        if allow_direct_merge {
            return Ok(Some(MergeTraceOutcome {
                related_commits: vec![related_commit_for_merge_trace(
                    commit,
                    &via_merge_hashes,
                    filter,
                    previous_related_commits,
                )],
                ambiguous,
                replaced_merge_commit: false,
            }));
        }

        Ok(Some(MergeTraceOutcome {
            related_commits: vec![related_commit_for_merge_trace(
                commit,
                &via_merge_hashes,
                filter,
                previous_related_commits,
            )],
            ambiguous,
            replaced_merge_commit,
        }))
    }
}

fn parent_candidates(
    context: &AttributionContext<'_>,
    text_cache: &mut MergeTraceTextCache,
    parent: &IndexedCommit,
    commit: &IndexedCommit,
    file_path: &str,
    block: &DiffBlockOutput,
    block_lines: &[String],
) -> Result<Vec<(String, DiffBlockOutput, bool)>, AppError> {
    let paths =
        path_history::parent_path_candidates(context, &parent.hash, &commit.hash, file_path)?;
    let mut matches = Vec::new();

    for path in paths.paths {
        let parent_text = text_cache.read_text(context, &parent.hash, &path)?;
        if text_window_matches(parent_text, block, block_lines) {
            let mapped_block = mapped_block_from_window(block, block_lines);
            matches.push((path, mapped_block, false));
            continue;
        }

        match full_window_matches(parent_text, block_lines, block.new_start) {
            WindowMatch::Unique { start, end } => {
                let mapped_block = remap_block_for_window(block, start + 1, end);
                matches.push((path, mapped_block, false));
            }
            WindowMatch::Multiple => {
                matches.push((path, block.clone(), true));
            }
            WindowMatch::None => {}
        }
    }

    Ok(matches)
}

#[derive(Debug, PartialEq, Eq)]
enum WindowMatch {
    None,
    Unique { start: usize, end: usize },
    Multiple,
}

fn full_window_matches(
    text: &str,
    target_lines: &[String],
    preferred_start_line: usize,
) -> WindowMatch {
    if target_lines.is_empty() {
        return WindowMatch::None;
    }

    let lines = split_text_lines(text);
    if lines.len() < target_lines.len() || preferred_start_line == 0 {
        return WindowMatch::None;
    }
    let target = target_lines;
    let mut positions = Vec::new();
    let max_start = lines.len() - target.len();

    for start in 0..=max_start {
        let end = start + target.len();
        if lines[start..end] == *target {
            positions.push((start, end));
        }
    }

    match positions.len() {
        0 => WindowMatch::None,
        1 => {
            let (start, end) = positions[0];
            let candidate_start_line = start + 1;
            let min_start = preferred_start_line.saturating_sub(1).max(1);
            let max_start = preferred_start_line + 1;
            if candidate_start_line < min_start || candidate_start_line > max_start {
                WindowMatch::None
            } else {
                WindowMatch::Unique { start, end }
            }
        }
        _ => WindowMatch::Multiple,
    }
}

fn mapped_block_from_window(block: &DiffBlockOutput, block_lines: &[String]) -> DiffBlockOutput {
    let mut mapped_block = block.clone();
    let start = block.new_start;
    let end = start + block_lines.len().saturating_sub(1);
    if block.new_start > 0 {
        mapped_block.old_start = block.old_start;
        mapped_block.old_end = block.old_end;
        mapped_block.new_start = start;
        mapped_block.new_end = end.max(start);
    }
    mapped_block
}

fn remap_block_for_window(
    block: &DiffBlockOutput,
    start_line: usize,
    end_line: usize,
) -> DiffBlockOutput {
    let mut mapped_block = block.clone();
    mapped_block.new_start = start_line;
    mapped_block.new_end = end_line;
    mapped_block
}

fn related_commit_for_merge_trace(
    commit: &IndexedCommit,
    via_merge_hashes: &[String],
    filter: &FilterMatcher,
    previous_related_commits: &[RelatedCommitOutput],
) -> RelatedCommitOutput {
    let via_merge_hashes = {
        let mut seen = HashSet::new();
        let mut deduped = Vec::new();
        for hash in via_merge_hashes.iter() {
            if seen.insert(hash.clone()) {
                deduped.push(hash.clone());
            }
        }
        deduped
    };
    let previous = previous_related_commits
        .iter()
        .find(|previous| previous.hash == commit.hash);
    let touched_ranges = previous
        .map(|commit| commit.touched_ranges.clone())
        .unwrap_or_default();

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
        touched_ranges,
        attribution: Some(RelatedCommitAttributionOutput {
            method: MERGE_TRACE_METHOD.to_string(),
            via_merge_hashes: via_merge_hashes.to_vec(),
        }),
    }
}

fn deduplicate_related_commits(commits: &mut Vec<RelatedCommitOutput>) {
    let mut seen: HashSet<String> = HashSet::new();
    commits.retain(|commit| seen.insert(commit.hash.clone()));
}

fn build_block_signature(path: &str, block: &DiffBlockOutput) -> String {
    format!(
        "{path}|{}|{}|{}|{}|{}",
        block.old_start,
        block.old_end,
        block.new_start,
        block.new_end,
        block_new_lines(block).join("\0"),
    )
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

#[cfg(test)]
mod tests {
    use super::{
        deduplicate_related_commits, full_window_matches, resolve_merge_commit,
        MergeTraceTextCache, WindowMatch,
    };
    use crate::attribution::context::AttributionContext;
    use crate::attribution::patch_inference::FilterMatcher;
    use crate::cli::{OutputFormat, OverlayCommonArgs};
    use crate::contracts::ResolvedTextEncoding;
    use crate::git::blob::read_text_at_commit_with_encoding;
    use crate::git::commits;
    use crate::git_test_support::{run_git, run_git_output, run_git_status};
    use crate::json::{
        DiffBlockOutput, RelatedCommitAttributionOutput, RelatedCommitOutput, TouchedRangeOutput,
    };
    use crate::overlay::diff_builder::build_overlay_diff;
    use std::collections::HashSet;
    use std::fs;
    use std::path::Path;
    use tempfile::TempDir;

    #[test]
    fn resolve_merge_trace_keeps_merge_commit_when_conflict_merge_has_no_parent_match() {
        let fixture = create_conflict_merge_fixture();
        let repo = gix::discover(fixture.repo.path()).expect("打开仓库");
        let mut cache = MergeTraceTextCache::new(ResolvedTextEncoding::Utf8);
        let context = analysis_context(&repo, fixture.base.as_str(), fixture.head.as_str());
        let filter = FilterMatcher::new(&[], None, None);

        let merge_commit =
            commits::get_commit(&repo, fixture.head.as_str()).expect("解析 merge 提交");
        let block = merge_result_block(&repo, &fixture.base, &fixture.head, "src/app.txt");
        let outcome = resolve_merge_commit(
            &context,
            &merge_commit,
            "src/app.txt",
            &block,
            &filter,
            &[],
            &mut cache,
        )
        .expect("解析 merge trace")
        .expect("应返回归因结果");

        assert_eq!(outcome.related_commits.len(), 1);
        let commit = &outcome.related_commits[0];
        assert_eq!(commit.subject, "merge: resolve conflict");
        assert_eq!(
            commit
                .attribution
                .as_ref()
                .map(|attribution| attribution.via_merge_hashes.clone()),
            Some(vec![fixture.head]),
            "冲突未解析到父提交时应保留 merge 且带自身 via"
        );
        assert!(!outcome.ambiguous);
        assert!(!outcome.replaced_merge_commit);
    }

    #[test]
    fn resolve_merge_trace_recurses_nested_merge_and_keeps_only_real_sources() {
        let fixture = create_nested_merge_fixture();
        let repo = gix::discover(fixture.repo.path()).expect("打开仓库");
        let mut cache = MergeTraceTextCache::new(ResolvedTextEncoding::Utf8);
        let context = analysis_context(&repo, fixture.base.as_str(), fixture.head.as_str());
        let filter = FilterMatcher::new(&[], None, None);

        let merge_commit =
            commits::get_commit(&repo, fixture.head.as_str()).expect("解析 merge 提交");
        let block = block_containing(&repo, &fixture.base, &fixture.head, "src/app.txt", "A");
        let outcome = resolve_merge_commit(
            &context,
            &merge_commit,
            "src/app.txt",
            &block,
            &filter,
            &[],
            &mut cache,
        )
        .expect("解析 merge trace")
        .expect("应返回归因结果");

        let commit_subjects = outcome
            .related_commits
            .iter()
            .map(|commit| commit.subject.as_str())
            .collect::<HashSet<_>>();
        assert!(commit_subjects.contains(&"feat: add A line"));
        assert!(!commit_subjects.contains(&"merge: feature into integration"));
        assert!(!commit_subjects.contains(&"merge: integrate feature"));
        assert!(!outcome.ambiguous);
        assert_eq!(outcome.related_commits.len(), 1);

        let commit = &outcome.related_commits[0];
        let via_hashes = commit
            .attribution
            .as_ref()
            .expect("归因存在")
            .via_merge_hashes
            .as_slice();
        assert!(via_hashes.contains(&fixture.head), "应保留 head merge");
    }

    #[test]
    fn resolve_merge_trace_tracks_ambiguous_merge_with_multiple_valid_sources() {
        let fixture = create_ambiguous_merge_fixture();
        let repo = gix::discover(fixture.repo.path()).expect("打开仓库");
        let mut cache = MergeTraceTextCache::new(ResolvedTextEncoding::Utf8);
        let context = analysis_context(&repo, fixture.base.as_str(), fixture.head.as_str());
        let filter = FilterMatcher::new(&[], None, None);

        let merge_commit =
            commits::get_commit(&repo, fixture.head.as_str()).expect("解析 merge 提交");
        let block = merge_result_block(&repo, &fixture.base, &fixture.head, "src/app.txt");
        let outcome = resolve_merge_commit(
            &context,
            &merge_commit,
            "src/app.txt",
            &block,
            &filter,
            &[],
            &mut cache,
        )
        .expect("解析 merge trace")
        .expect("应返回归因结果");

        assert!(outcome.ambiguous);
        let commit_subjects = outcome
            .related_commits
            .iter()
            .map(|commit| commit.subject.as_str())
            .collect::<Vec<_>>();
        assert_eq!(commit_subjects.len(), 2);
        assert!(commit_subjects.contains(&"feat: left path"));
        assert!(commit_subjects.contains(&"feat: right path"));
        assert_eq!(outcome.replaced_merge_commit, true);
    }

    #[test]
    fn deduplicates_related_commits_by_hash() {
        let mut commits = vec![
            related_commit_stub("A"),
            related_commit_stub("A"),
            related_commit_stub("B"),
            related_commit_stub("A"),
        ];

        deduplicate_related_commits(&mut commits);

        assert_eq!(commits.len(), 2);
        assert_eq!(
            commits
                .iter()
                .map(|commit| commit.hash.as_str())
                .collect::<Vec<_>>(),
            vec!["A", "B"]
        );
    }

    #[test]
    fn full_window_matches_handles_multiple_and_none() {
        assert_eq!(
            full_window_matches("a\nb\nc\n", &["x".to_string()], 2),
            WindowMatch::None
        );
        assert_eq!(
            full_window_matches("a\nx\nb\nx\n", &["x".to_string()], 2),
            WindowMatch::Multiple
        );
        assert_eq!(
            full_window_matches("a\nx\nb\n", &["x".to_string()], 4),
            WindowMatch::None
        );
    }

    struct RepoFixture {
        repo: TempDir,
        base: String,
        head: String,
    }

    fn analysis_context<'repo>(
        repo: &'repo gix::Repository,
        base: &str,
        head: &str,
    ) -> AttributionContext<'repo> {
        let args = OverlayCommonArgs {
            repo: repo.path().to_path_buf(),
            db: None,
            base: base.to_string(),
            head: head.to_string(),
            branch: "main".to_string(),
            globs: Vec::new(),
            authors: Vec::new(),
            author_query: None,
            message: None,
            require_index: false,
            format: OutputFormat::Json,
            pretty: false,
        };
        AttributionContext::open(repo, &args).expect("构建上下文")
    }

    fn merge_result_block(
        repo: &gix::Repository,
        base: &str,
        head: &str,
        path: &str,
    ) -> DiffBlockOutput {
        let overlay = build_overlay_diff(
            read_text_at_commit_with_encoding(repo, base, path, ResolvedTextEncoding::Utf8)
                .expect("读取 base 文件内容")
                .as_str(),
            read_text_at_commit_with_encoding(repo, head, path, ResolvedTextEncoding::Utf8)
                .expect("读取 head 文件内容")
                .as_str(),
        );
        overlay.blocks.into_iter().next().expect("至少有一个 block")
    }

    fn block_containing(
        repo: &gix::Repository,
        base: &str,
        head: &str,
        path: &str,
        needle: &str,
    ) -> DiffBlockOutput {
        let overlay = build_overlay_diff(
            read_text_at_commit_with_encoding(repo, base, path, ResolvedTextEncoding::Utf8)
                .expect("读取 base 文件内容")
                .as_str(),
            read_text_at_commit_with_encoding(repo, head, path, ResolvedTextEncoding::Utf8)
                .expect("读取 head 文件内容")
                .as_str(),
        );
        overlay
            .blocks
            .into_iter()
            .find(|block| {
                block.rows.iter().any(|row| {
                    row.new_text
                        .as_ref()
                        .is_some_and(|text| text.contains(needle))
                })
            })
            .expect("应找到包含目标文本的 block")
    }

    fn write_file(repo: &Path, path: &str, content: &str) {
        let path = repo.join(path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("创建目录");
        }
        fs::write(path, content).expect("写入文件");
    }

    fn init_repo() -> TempDir {
        let repo = TempDir::new().expect("临时目录");
        run_git(repo.path(), &["init", "--initial-branch", "main"]);
        repo
    }

    fn commit_current(repo: &Path, message: &str) {
        run_git(repo, &["add", "."]);
        run_git(repo, &["commit", "-m", message]);
    }

    fn rev_parse(repo: &Path, revision: &str) -> String {
        run_git_output(repo, &["rev-parse", revision])
    }

    fn create_conflict_merge_fixture() -> RepoFixture {
        let repo = init_repo();
        write_file(repo.path(), "src/app.txt", "base\n");
        commit_current(repo.path(), "feat: base");
        let base = rev_parse(repo.path(), "HEAD");

        run_git(repo.path(), &["checkout", "-b", "feature"]);
        write_file(repo.path(), "src/app.txt", "base\nfeature\n");
        commit_current(repo.path(), "feat: feature line");

        run_git(repo.path(), &["checkout", "main"]);
        write_file(repo.path(), "src/app.txt", "base\nmain\n");
        commit_current(repo.path(), "feat: main line");

        assert!(!run_git_status(repo.path(), &["merge", "feature"]));
        write_file(repo.path(), "src/app.txt", "base\nmain\nfeature\n");
        commit_current(repo.path(), "merge: resolve conflict");
        let head = rev_parse(repo.path(), "HEAD");

        RepoFixture { repo, base, head }
    }

    fn create_nested_merge_fixture() -> RepoFixture {
        let repo = init_repo();
        write_file(repo.path(), "src/app.txt", "base\n");
        commit_current(repo.path(), "feat: base");
        let base = rev_parse(repo.path(), "HEAD");

        run_git(repo.path(), &["checkout", "-b", "feature"]);
        write_file(repo.path(), "src/app.txt", "base\nA\n");
        commit_current(repo.path(), "feat: add A line");

        run_git(repo.path(), &["checkout", "main"]);
        run_git(repo.path(), &["checkout", "-b", "integration", "main"]);
        run_git(
            repo.path(),
            &[
                "merge",
                "--no-ff",
                "feature",
                "-m",
                "merge: feature into integration",
            ],
        );
        run_git(repo.path(), &["checkout", "main"]);
        run_git(
            repo.path(),
            &[
                "merge",
                "--no-ff",
                "integration",
                "-m",
                "merge: integrate feature",
            ],
        );
        let head = rev_parse(repo.path(), "HEAD");

        RepoFixture { repo, base, head }
    }

    fn create_ambiguous_merge_fixture() -> RepoFixture {
        let repo = init_repo();
        write_file(repo.path(), "src/app.txt", "base\n");
        commit_current(repo.path(), "feat: base");
        let base = rev_parse(repo.path(), "HEAD");

        run_git(repo.path(), &["checkout", "-b", "left"]);
        write_file(repo.path(), "src/app.txt", "base\nshared\n");
        commit_current(repo.path(), "feat: left path");
        let left = rev_parse(repo.path(), "HEAD");

        run_git(repo.path(), &["checkout", "main"]);
        run_git(repo.path(), &["checkout", "-b", "right", "main"]);
        write_file(repo.path(), "src/app.txt", "base\nshared\n");
        commit_current(repo.path(), "feat: right path");
        let right = rev_parse(repo.path(), "HEAD");

        run_git(repo.path(), &["checkout", "main"]);
        let tree = run_git_output(repo.path(), &["rev-parse", &format!("{}^{{tree}}", right)]);
        let merge = run_git_output(
            repo.path(),
            &[
                "commit-tree",
                &tree,
                "-p",
                &left,
                "-p",
                &right,
                "-m",
                "merge: ambiguous nested sources",
            ],
        );
        run_git(repo.path(), &["reset", "--hard", &merge]);
        let head = rev_parse(repo.path(), "HEAD");

        RepoFixture { repo, base, head }
    }

    fn related_commit_stub(hash: &str) -> RelatedCommitOutput {
        RelatedCommitOutput {
            hash: hash.to_string(),
            short_hash: hash.to_string(),
            author_name: "fixture".to_string(),
            author_email: Some("fixture@example.com".to_string()),
            committed_at: "2026-01-01T00:00:00+00:00".to_string(),
            subject: format!("subject {hash}"),
            matched_by_filter: true,
            touched_ranges: vec![TouchedRangeOutput {
                old_start: Some(1),
                old_end: Some(1),
                new_start: Some(1),
                new_end: Some(1),
            }],
            attribution: Some(RelatedCommitAttributionOutput {
                method: "merge-trace".to_string(),
                via_merge_hashes: vec![hash.to_string()],
            }),
        }
    }
}
