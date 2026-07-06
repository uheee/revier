mod fixtures;

use revier_analysis::api::{
    list_branches, query_files_with_context, resolve_analysis_range,
    resolve_analysis_range_with_context, validate_repository, QueryFilesRequest,
};
use revier_analysis::error::AppError;
use revier_analysis::execution::AnalysisExecutionContext;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[test]
fn validates_non_git_directory_without_recoverable() {
    let dir = tempfile::tempdir().expect("创建临时目录失败");
    let result = validate_repository(dir.path()).expect("仓库校验命令失败");

    assert!(!result.valid);
    assert!(result.error.is_some());
}

#[test]
fn query_files_with_context_returns_cancelled_before_opening_repo() {
    let context = AnalysisExecutionContext::with_cancel(|| true);

    let error = query_files_with_context(
        QueryFilesRequest {
            repo: PathBuf::from("missing-repo"),
            db: None,
            base: "base".to_string(),
            head: "head".to_string(),
            branch: "main".to_string(),
            authors: Vec::new(),
            author_query: None,
            message: None,
            since: None,
            until: None,
            globs: Vec::new(),
        },
        &context,
    )
    .expect_err("已取消的查询不应继续打开仓库");

    assert!(matches!(error, AppError::Cancelled));
}

#[test]
fn resolve_analysis_range_with_context_returns_cancelled_before_discovering_repo() {
    let context = AnalysisExecutionContext::with_cancel(|| true);

    let error = resolve_analysis_range_with_context(
        PathBuf::from("missing-repo").as_path(),
        "main",
        None,
        None,
        &context,
    )
    .expect_err("已取消的范围解析不应继续读取仓库");

    assert!(matches!(error, AppError::Cancelled));
}

#[test]
fn resolve_analysis_range_with_context_preserves_cancelled_for_empty_repo() {
    let repo = tempfile::tempdir().expect("创建临时目录失败");
    gix::create::into(
        repo.path(),
        gix::create::Kind::WithWorktree,
        gix::create::Options::default(),
    )
    .expect("初始化测试仓库失败");
    let checks = Arc::new(AtomicUsize::new(0));
    let checks_for_context = Arc::clone(&checks);
    let context = AnalysisExecutionContext::with_cancel(move || {
        checks_for_context.fetch_add(1, Ordering::SeqCst) >= 1
    });

    let error = resolve_analysis_range_with_context(repo.path(), "main", None, None, &context)
        .expect_err("中途取消不应被空仓库错误覆盖");

    assert!(matches!(error, AppError::Cancelled));
}

#[test]
fn lists_branches_for_fixture_repo() {
    let repo = tempfile::tempdir().expect("创建临时目录失败");
    gix::create::into(
        repo.path(),
        gix::create::Kind::WithWorktree,
        gix::create::Options::default(),
    )
    .expect("初始化测试仓库失败");

    let branches = list_branches(repo.path()).expect("读取分支失败");
    assert!(branches.iter().any(|branch| branch.current));
}

#[test]
fn resolves_analysis_range_from_branch_and_dates() {
    let fixture = fixtures::linear_with_authors();
    let range = resolve_analysis_range(
        fixture.repo.path(),
        "main",
        Some("2026-05-02T00:00:00Z".to_string()),
        Some("2026-05-03T23:59:59Z".to_string()),
    )
    .expect("解析分析范围失败");

    assert_eq!(range.branch, "main");
    assert_eq!(range.base_commit, fixture.base);
    assert_eq!(range.head_commit, fixture.head);
    assert_eq!(range.start_at.as_deref(), Some("2026-05-02T00:00:00+00:00"));
    assert_eq!(range.end_at.as_deref(), Some("2026-05-03T23:59:59+00:00"));
}

#[test]
fn rejects_analysis_range_when_repository_has_no_commits() {
    let repo = tempfile::tempdir().expect("创建临时目录失败");
    gix::create::into(
        repo.path(),
        gix::create::Kind::WithWorktree,
        gix::create::Options::default(),
    )
    .expect("初始化测试仓库失败");

    let error = resolve_analysis_range(repo.path(), "main", None, None)
        .expect_err("空仓库不应解析出分析范围");

    assert!(error.to_string().contains("没有可用提交"));
}

#[test]
fn rejects_analysis_range_when_start_is_after_end() {
    let fixture = fixtures::linear_with_authors();

    let error = resolve_analysis_range(
        fixture.repo.path(),
        "main",
        Some("2026-05-04T00:00:00Z".to_string()),
        Some("2026-05-03T00:00:00Z".to_string()),
    )
    .expect_err("开始时间晚于结束时间时不应解析出分析范围");

    assert!(error.to_string().contains("start_at 不能晚于 end_at"));
}

#[test]
fn resolved_analysis_range_times_filter_query_files_boundaries() {
    let fixture = fixtures::linear_with_authors();
    let dir = tempfile::tempdir().expect("创建临时目录失败");
    let db_path = dir.path().join("index.duckdb");
    run_index_build(&fixture, &db_path);

    let range = resolve_analysis_range(
        fixture.repo.path(),
        "main",
        Some("2026-05-02T00:00:00Z".to_string()),
        Some("2026-05-03T23:59:59Z".to_string()),
    )
    .expect("解析分析范围失败");

    let output = revier_analysis::api::query_files(QueryFilesRequest {
        repo: fixture.repo.path().to_path_buf(),
        db: Some(db_path),
        base: range.base_commit,
        head: range.head_commit,
        branch: range.branch,
        authors: vec!["alice@example.com".to_string()],
        author_query: None,
        message: Some("alice".to_string()),
        since: range.start_at,
        until: range.end_at,
        globs: vec!["src/**/*.txt".to_string()],
    })
    .expect("查询变更文件失败");

    assert_eq!(output.files.len(), 1);
    assert_eq!(output.files[0].path, "src/app.txt");
}

#[test]
fn subsecond_range_is_normalized_before_querying() {
    let fixture = fixtures::linear_with_authors();
    let dir = tempfile::tempdir().expect("创建临时目录失败");
    let db_path = dir.path().join("index.duckdb");
    run_index_build(&fixture, &db_path);

    let range = resolve_analysis_range(
        fixture.repo.path(),
        "main",
        Some("2026-05-02T00:00:00.500Z".to_string()),
        Some("2026-05-02T00:00:00.900Z".to_string()),
    )
    .expect("解析分析范围失败");

    assert_eq!(range.base_commit, fixture.base);
    assert_eq!(range.start_at.as_deref(), Some("2026-05-02T00:00:00+00:00"));
    assert_eq!(range.end_at.as_deref(), Some("2026-05-02T00:00:00+00:00"));

    let output = revier_analysis::api::query_files(QueryFilesRequest {
        repo: fixture.repo.path().to_path_buf(),
        db: Some(db_path),
        base: range.base_commit,
        head: range.head_commit,
        branch: range.branch,
        authors: vec!["alice@example.com".to_string()],
        author_query: None,
        message: Some("alice".to_string()),
        since: range.start_at,
        until: range.end_at,
        globs: vec!["src/**/*.txt".to_string()],
    })
    .expect("查询变更文件失败");

    assert_eq!(output.files.len(), 1);
    assert_eq!(output.files[0].path, "src/app.txt");
}

fn run_index_build(fixture: &fixtures::FixtureRepo, db_path: &std::path::Path) {
    let output = Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
        .args([
            "index",
            "build",
            "--repo",
            fixture.repo.path().to_str().expect("repo path"),
            "--db",
            db_path.to_str().expect("db path"),
            "--branch",
            "main",
            "--format",
            "json",
        ])
        .output()
        .expect("运行 index build");
    assert!(
        output.status.success(),
        "index build 应成功，stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}
