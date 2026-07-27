use crate::attribution::context::AttributionContext;
use crate::attribution::path_history;
use crate::error::AppError;
use crate::git::blob::read_blob_at_commit;
use crate::git::commits::get_commit;
use crate::git::commits::IndexedCommit;
use crate::git::diff::commit_file_changes;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FirstParentSource {
    pub commit_hash: String,
    pub historical_path: String,
    pub first_parent_hash: String,
    pub old_blob_id: Option<String>,
    pub new_blob_id: Option<String>,
    pub change_status: String,
}

#[derive(Default)]
pub struct SourceValidationCache {
    state: HashMap<(String, String), Option<FirstParentSource>>,
}

impl SourceValidationCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn validate_first_parent_source(
        &mut self,
        context: &AttributionContext<'_>,
        commit: &IndexedCommit,
        historical_path: &str,
    ) -> Result<Option<FirstParentSource>, AppError> {
        let key = (commit.hash.clone(), historical_path.to_string());
        if let Some(cached) = self.state.get(&key) {
            return Ok(cached.clone());
        }

        let resolved = resolve_first_parent_source(context, commit, historical_path)?;
        self.state.insert(key, resolved.clone());
        Ok(resolved)
    }

    pub fn len(&self) -> usize {
        self.state.len()
    }
}

fn resolve_first_parent_source(
    context: &AttributionContext<'_>,
    commit: &IndexedCommit,
    historical_path: &str,
) -> Result<Option<FirstParentSource>, AppError> {
    let first_parent_hash = commit.parents.first().cloned().unwrap_or_default();
    let changes = commit_file_changes(context.repo, &commit.hash)?;
    let mut candidates = Vec::new();

    for change in changes {
        if change.parent_index != 0 {
            continue;
        }

        let matched =
            historical_path == change.path || change.old_path.as_deref() == Some(historical_path);
        if !matched {
            continue;
        }

        candidates.push(change);
    }

    if candidates.len() != 1 {
        return Ok(None);
    }

    let change = candidates.pop().unwrap();
    let historical_path = match change.status.as_str() {
        "renamed" => change
            .old_path
            .clone()
            .unwrap_or_else(|| change.path.clone()),
        _ => change.old_path.clone().unwrap_or(change.path.clone()),
    };

    if commit.parents.len() > 1 && !first_parent_only_change(context, commit, &change.path)? {
        return Ok(None);
    }

    Ok(Some(FirstParentSource {
        commit_hash: commit.hash.clone(),
        historical_path,
        first_parent_hash,
        old_blob_id: change.old_blob_id,
        new_blob_id: change.new_blob_id,
        change_status: change.status,
    }))
}

fn first_parent_only_change(
    context: &AttributionContext<'_>,
    commit: &IndexedCommit,
    current_path: &str,
) -> Result<bool, AppError> {
    if commit.is_merge && commit.parents.len() > 1 {
        let commit_new_blob = read_blob_at_commit(context.repo, &commit.hash, current_path)?;
        if commit_new_blob.is_none() {
            return Ok(true);
        }

        let mut matching_non_first_parent_count = 0usize;
        let mut matching_non_merge_parent_count = 0usize;

        for parent_hash in commit.parents.iter().skip(1) {
            let parent_paths = path_history::parent_path_candidates(
                context,
                parent_hash,
                &commit.hash,
                current_path,
            )?;
            let parent_commit = get_commit(context.repo, parent_hash)?;
            let mut parent_matches = false;
            for path in parent_paths.paths {
                let parent_blob = read_blob_at_commit(context.repo, parent_hash, &path)?;
                if parent_blob == commit_new_blob {
                    parent_matches = true;
                    break;
                }
            }

            if parent_matches {
                matching_non_first_parent_count += 1;
                if !parent_commit.is_merge {
                    matching_non_merge_parent_count += 1;
                }
            }
        }
        if matching_non_first_parent_count == 1 && matching_non_merge_parent_count == 1 {
            return Ok(false);
        }
        return Ok(true);
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::SourceValidationCache;
    use crate::attribution::commit_lookup;
    use crate::attribution::context::AttributionContext;
    use crate::cli::{OutputFormat, OverlayCommonArgs};
    use crate::git_test_support::{run_git as run, run_git_output};

    #[test]
    fn normal_non_merge_modification_can_be_validated() {
        let (repo, base, head) = init_linear_repo();
        let repo = gix::discover(repo.path()).expect("打开仓库");
        let context = analysis_context(&repo, &base, &head);
        let commit = commit_lookup::get_commit(&context, &head).expect("读取提交");
        let mut cache = SourceValidationCache::new();

        let source = cache
            .validate_first_parent_source(&context, &commit, "src/app.txt")
            .expect("验证成功")
            .expect("应返回有效来源");

        assert_eq!(source.change_status, "modified");
        assert_eq!(source.historical_path, "src/app.txt");
        assert_eq!(source.first_parent_hash, base);
    }

    #[test]
    fn root_commit_addition_can_be_validated() {
        let (repo, base, _) = init_root_repo();
        let repo = gix::discover(repo.path()).expect("打开仓库");
        let context = analysis_context(&repo, &base, &base);
        let commit = commit_lookup::get_commit(&context, &base).expect("读取提交");
        let mut cache = SourceValidationCache::new();

        let source = cache
            .validate_first_parent_source(&context, &commit, "src/app.txt")
            .expect("验证成功")
            .expect("应返回根提交来源");

        assert_eq!(source.change_status, "added");
        assert_eq!(source.first_parent_hash, String::new());
    }

    #[test]
    fn merge_with_second_parent_only_change_is_not_valid_first_parent_source() {
        let (repo, base, head) = init_merge_second_parent_only_repo();
        let repo = gix::discover(repo.path()).expect("打开仓库");
        let context = analysis_context(&repo, &base, &head);
        let commit = commit_lookup::get_commit(&context, &head).expect("读取 merge 提交");
        let mut cache = SourceValidationCache::new();

        let source = cache
            .validate_first_parent_source(&context, &commit, "src/app.txt")
            .expect("验证成功");

        assert!(source.is_none());
    }

    #[test]
    fn rename_current_path_maps_to_historical_path() {
        let (repo, _, head) = init_rename_repo();
        let repo = gix::discover(repo.path()).expect("打开仓库");
        let parent = commit_parent(repo.path(), &head);
        let context = analysis_context(&repo, &parent, &head);
        let commit = commit_lookup::get_commit(&context, &head).expect("读取提交");
        let mut cache = SourceValidationCache::new();

        let source = cache
            .validate_first_parent_source(&context, &commit, "src/new.txt")
            .expect("验证成功")
            .expect("应返回重命名来源");

        assert_eq!(source.change_status, "renamed");
        assert_eq!(source.historical_path, "src/old.txt");
    }

    #[test]
    fn no_change_commit_is_not_valid() {
        let (repo, base, head) = init_linear_repo_no_file_touch();
        let repo = gix::discover(repo.path()).expect("打开仓库");
        let context = analysis_context(&repo, &base, &head);
        let commit = commit_lookup::get_commit(&context, &head).expect("读取提交");
        let mut cache = SourceValidationCache::new();

        let source = cache
            .validate_first_parent_source(&context, &commit, "src/app.txt")
            .expect("验证成功");

        assert!(source.is_none());
    }

    #[test]
    fn first_parent_source_validation_reuses_cache_entry() {
        let (repo, base, head) = init_linear_repo();
        let repo = gix::discover(repo.path()).expect("打开仓库");
        let context = analysis_context(&repo, &base, &head);
        let commit = commit_lookup::get_commit(&context, &head).expect("读取提交");
        let mut cache = SourceValidationCache::new();

        let first = cache
            .validate_first_parent_source(&context, &commit, "src/app.txt")
            .expect("第一次验证成功");
        let second = cache
            .validate_first_parent_source(&context, &commit, "src/app.txt")
            .expect("第二次验证成功");

        assert_eq!(first, second);
        assert_eq!(cache.len(), 1);
    }

    fn analysis_context<'a>(
        repo: &'a gix::Repository,
        base: &str,
        head: &str,
    ) -> AttributionContext<'a> {
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

    fn init_linear_repo() -> (tempfile::TempDir, String, String) {
        let repo = tempfile::tempdir().expect("创建临时仓库");
        run(repo.path(), &["init", "--initial-branch", "main"]);
        write_file(repo.path(), "src/app.txt", "base\n");
        run(repo.path(), &["add", "."]);
        run(repo.path(), &["commit", "-m", "feat: base"]);
        let base = rev_parse(repo.path(), "HEAD");
        write_file(repo.path(), "src/app.txt", "base\nadd\n");
        run(repo.path(), &["add", "."]);
        run(repo.path(), &["commit", "-m", "feat: add"]);
        let head = rev_parse(repo.path(), "HEAD");
        (repo, base, head)
    }

    fn init_root_repo() -> (tempfile::TempDir, String, String) {
        let repo = tempfile::tempdir().expect("创建临时仓库");
        run(repo.path(), &["init", "--initial-branch", "main"]);
        write_file(repo.path(), "src/app.txt", "base\n");
        run(repo.path(), &["add", "."]);
        run(repo.path(), &["commit", "-m", "feat: initial"]);
        let base = rev_parse(repo.path(), "HEAD");
        (repo, base.clone(), base)
    }

    fn init_merge_second_parent_only_repo() -> (tempfile::TempDir, String, String) {
        let repo = tempfile::tempdir().expect("创建临时仓库");
        run(repo.path(), &["init", "--initial-branch", "main"]);
        write_file(repo.path(), "src/app.txt", "base\n");
        run(repo.path(), &["add", "."]);
        run(repo.path(), &["commit", "-m", "feat: base"]);
        let base = rev_parse(repo.path(), "HEAD");

        run(repo.path(), &["checkout", "-b", "feature"]);
        write_file(repo.path(), "src/app.txt", "base\nfeature line\n");
        run(repo.path(), &["add", "."]);
        run(repo.path(), &["commit", "-m", "feat: add feature line"]);

        run(repo.path(), &["checkout", "main"]);
        write_file(repo.path(), "docs/notes.txt", "prep\n");
        run(repo.path(), &["add", "."]);
        run(repo.path(), &["commit", "-m", "chore: prep"]);
        run(
            repo.path(),
            &[
                "merge",
                "--no-ff",
                "feature",
                "-m",
                "merge: second parent only change",
            ],
        );
        let head = rev_parse(repo.path(), "HEAD");
        (repo, base, head)
    }

    fn init_rename_repo() -> (tempfile::TempDir, String, String) {
        let repo = tempfile::tempdir().expect("创建临时仓库");
        run(repo.path(), &["init", "--initial-branch", "main"]);
        write_file(repo.path(), "src/old.txt", "alpha\n");
        run(repo.path(), &["add", "."]);
        run(repo.path(), &["commit", "-m", "feat: initial"]);
        let base = rev_parse(repo.path(), "HEAD");
        run(repo.path(), &["mv", "src/old.txt", "src/new.txt"]);
        run(repo.path(), &["add", "."]);
        run(repo.path(), &["commit", "-m", "feat: rename and edit"]);
        let head = rev_parse(repo.path(), "HEAD");
        (repo, base, head)
    }

    fn init_linear_repo_no_file_touch() -> (tempfile::TempDir, String, String) {
        let repo = tempfile::tempdir().expect("创建临时仓库");
        run(repo.path(), &["init", "--initial-branch", "main"]);
        write_file(repo.path(), "src/app.txt", "base\n");
        run(repo.path(), &["add", "."]);
        run(repo.path(), &["commit", "-m", "feat: base"]);
        let base = rev_parse(repo.path(), "HEAD");
        write_file(repo.path(), "docs/notes.txt", "notes\n");
        run(repo.path(), &["add", "."]);
        run(repo.path(), &["commit", "-m", "chore: update notes"]);
        let head = rev_parse(repo.path(), "HEAD");
        (repo, base, head)
    }

    fn rev_parse(repo: &std::path::Path, revision: &str) -> String {
        run_git_output(repo, &["rev-parse", revision])
    }

    fn commit_parent(repo: &std::path::Path, hash: &str) -> String {
        run_git_output(repo, &["rev-parse", &format!("{}^", hash)])
    }

    fn write_file(path: &std::path::Path, file: &str, content: &str) {
        let target = path.join(file);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).expect("创建目录");
        }
        std::fs::write(target, content).expect("写入文件");
    }
}
