mod fixtures;

use revier_analysis::error::AppError;
use revier_analysis::execution::AnalysisExecutionContext;
use revier_analysis::index::queries::QueryFilesFilter;
use serde_json::Value;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[test]
fn query_files_filters_by_author_message_time_and_glob() {
    let fixture = fixtures::linear_with_authors();
    let dir = tempfile::tempdir().expect("创建临时目录");
    let db_path = dir.path().join("index.duckdb");

    run_index_build(&fixture, &db_path);

    let output = Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
        .args([
            "index",
            "query-files",
            "--repo",
            fixture.repo.path().to_str().expect("repo path"),
            "--db",
            db_path.to_str().expect("db path"),
            "--base",
            &fixture.base,
            "--head",
            &fixture.head,
            "--branch",
            "main",
            "--author",
            "alice@example.com",
            "--message",
            "alice",
            "--since",
            "2026-05-01T00:00:00Z",
            "--until",
            "2026-05-31T23:59:59Z",
            "--glob",
            "src/**/*.txt",
            "--format",
            "json",
        ])
        .output()
        .expect("运行 query-files");

    assert!(
        output.status.success(),
        "query-files 应成功，stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let json: Value = serde_json::from_slice(&output.stdout).expect("解析 query json");
    assert_eq!(json["version"], 1);
    assert_eq!(json["range"]["baseCommit"], fixture.base);
    assert_eq!(json["range"]["headCommit"], fixture.head);
    assert_eq!(json["files"][0]["path"], "src/app.txt");
    assert!(json["files"][0]["oldPath"].is_null());
    assert_eq!(json["files"][0]["status"], "modified");
    assert_eq!(json["files"][0]["additions"], 2);
    assert_eq!(json["files"][0]["deletions"], 0);
    assert_eq!(json["warnings"].as_array().expect("warnings").len(), 0);
}

#[test]
fn query_files_omits_files_restored_to_base_content() {
    let fixture = fixtures::changed_then_restored();
    let dir = tempfile::tempdir().expect("创建临时目录");
    let db_path = dir.path().join("index.duckdb");
    run_index_build(&fixture, &db_path);

    let output = Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
        .args([
            "index",
            "query-files",
            "--repo",
            fixture.repo.path().to_str().expect("repo path"),
            "--db",
            db_path.to_str().expect("db path"),
            "--base",
            &fixture.base,
            "--head",
            &fixture.head,
            "--branch",
            "main",
            "--format",
            "json",
        ])
        .output()
        .expect("运行 query-files");

    assert!(
        output.status.success(),
        "query-files 应成功，stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).expect("解析 query json");
    assert!(json["files"].as_array().expect("files").is_empty());
}

#[test]
fn query_files_intersects_filtered_commit_paths_with_final_changes() {
    let fixture = fixtures::separate_files_with_authors();
    let dir = tempfile::tempdir().expect("创建临时目录");
    let db_path = dir.path().join("index.duckdb");
    run_index_build(&fixture, &db_path);

    let output = Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
        .args([
            "index",
            "query-files",
            "--repo",
            fixture.repo.path().to_str().expect("repo path"),
            "--db",
            db_path.to_str().expect("db path"),
            "--base",
            &fixture.base,
            "--head",
            &fixture.head,
            "--branch",
            "main",
            "--author",
            "alice@example.com",
            "--format",
            "json",
        ])
        .output()
        .expect("运行 query-files");

    assert!(output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).expect("解析 query json");
    let files = json["files"].as_array().expect("files");
    assert_eq!(files.len(), 1);
    assert_eq!(files[0]["path"], "src/app.txt");
    assert_eq!(files[0]["additions"], 1);
    assert_eq!(files[0]["deletions"], 0);
}

#[test]
fn query_files_glob_matches_renamed_old_path() {
    let fixture = fixtures::pure_rename();
    let dir = tempfile::tempdir().expect("创建临时目录");
    let db_path = dir.path().join("index.duckdb");
    run_index_build(&fixture, &db_path);

    let output = Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
        .args([
            "index",
            "query-files",
            "--repo",
            fixture.repo.path().to_str().expect("repo path"),
            "--db",
            db_path.to_str().expect("db path"),
            "--base",
            &fixture.base,
            "--head",
            &fixture.head,
            "--branch",
            "main",
            "--glob",
            "src/old.txt",
            "--format",
            "json",
        ])
        .output()
        .expect("运行 query-files");

    assert!(output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).expect("解析 query json");
    let files = json["files"].as_array().expect("files");
    assert_eq!(files.len(), 1);
    assert_eq!(files[0]["path"], "src/new.txt");
    assert_eq!(files[0]["oldPath"], "src/old.txt");
    assert_eq!(files[0]["additions"], 0);
    assert_eq!(files[0]["deletions"], 0);
}

#[test]
fn index_query_files_with_context_returns_cancelled_before_schema_query() {
    let conn = duckdb::Connection::open_in_memory().expect("创建内存数据库");
    let context = AnalysisExecutionContext::with_cancel(|| true);

    let error = revier_analysis::index::queries::query_files_with_context(
        &conn,
        &QueryFilesFilter {
            base: "base".to_string(),
            head: "head".to_string(),
            authors: Vec::new(),
            author_query: None,
            message: None,
            since: None,
            until: None,
            globs: Vec::new(),
        },
        &context,
    )
    .expect_err("已取消的索引查询不应继续读取 DuckDB");

    assert!(matches!(error, AppError::Cancelled));
}

#[test]
fn index_query_files_with_context_returns_cancelled_during_commit_iteration() {
    let fixture = fixtures::linear_with_authors();
    let dir = tempfile::tempdir().expect("创建临时目录");
    let db_path = dir.path().join("index.duckdb");
    run_index_build(&fixture, &db_path);
    let conn = revier_analysis::index::connection::open_database(&db_path).expect("打开索引数据库");
    let checks = Arc::new(AtomicUsize::new(0));
    let checks_for_context = Arc::clone(&checks);
    let context = AnalysisExecutionContext::with_cancel(move || {
        checks_for_context.fetch_add(1, Ordering::SeqCst) >= 4
    });

    let error = revier_analysis::index::queries::query_files_with_context(
        &conn,
        &QueryFilesFilter {
            base: fixture.base,
            head: fixture.head,
            authors: Vec::new(),
            author_query: None,
            message: None,
            since: None,
            until: None,
            globs: Vec::new(),
        },
        &context,
    )
    .expect_err("提交遍历中途取消应返回取消错误");

    assert!(matches!(error, AppError::Cancelled));
}

#[test]
fn query_files_normalizes_rfc3339_time_boundaries() {
    let fixture = fixtures::linear_with_authors();
    let dir = tempfile::tempdir().expect("创建临时目录");
    let db_path = dir.path().join("index.duckdb");

    run_index_build(&fixture, &db_path);

    let output = Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
        .args([
            "index",
            "query-files",
            "--repo",
            fixture.repo.path().to_str().expect("repo path"),
            "--db",
            db_path.to_str().expect("db path"),
            "--base",
            &fixture.base,
            "--head",
            &fixture.head,
            "--branch",
            "main",
            "--author",
            "alice@example.com",
            "--message",
            "alice",
            "--since",
            "2026-05-02T08:00:00+08:00",
            "--until",
            "2026-05-02T00:00:00Z",
            "--glob",
            "src/**/*.txt",
            "--format",
            "json",
        ])
        .output()
        .expect("运行 query-files");

    assert!(
        output.status.success(),
        "query-files 应成功，stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let json: Value = serde_json::from_slice(&output.stdout).expect("解析 query json");
    assert_eq!(json["files"].as_array().expect("files").len(), 1);
    assert_eq!(json["files"][0]["path"], "src/app.txt");
}

#[test]
fn query_files_returns_exit_code_four_when_index_is_missing() {
    let fixture = fixtures::linear();
    let dir = tempfile::tempdir().expect("创建临时目录");
    let db_path = dir.path().join("missing.duckdb");

    let output = Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
        .args([
            "index",
            "query-files",
            "--repo",
            fixture.repo.path().to_str().expect("repo path"),
            "--db",
            db_path.to_str().expect("db path"),
            "--base",
            &fixture.base,
            "--head",
            &fixture.head,
            "--branch",
            "main",
            "--format",
            "json",
        ])
        .output()
        .expect("运行 query-files");

    assert_eq!(output.status.code(), Some(4));
    assert!(String::from_utf8_lossy(&output.stderr).contains("索引不可用"));
}

#[test]
fn commit_metadata_preserves_git_rfc3339_committed_at() {
    let fixture = fixtures::linear_with_authors();
    let dir = tempfile::tempdir().expect("创建临时目录");
    let db_path = dir.path().join("index.duckdb");
    run_index_build(&fixture, &db_path);
    let repo =
        revier_analysis::git::repository::open_repository(fixture.repo.path()).expect("打开仓库");
    let range_hashes =
        revier_analysis::git::commits::range_commit_hashes(&repo, &fixture.base, &fixture.head)
            .expect("读取范围提交");
    let hash = range_hashes.first().expect("范围内应有提交");

    let conn = revier_analysis::index::connection::open_database(&db_path).expect("打开索引数据库");
    let indexed = revier_analysis::index::queries::commit_metadata(&conn, hash)
        .expect("读取索引提交")
        .expect("索引提交应存在");
    let git = revier_analysis::git::commits::get_commit(&repo, hash).expect("读取 Git 提交");

    assert_eq!(indexed.committed_at, git.committed_at);
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
    assert!(output.status.success());
}
