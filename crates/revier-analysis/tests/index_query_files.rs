mod fixtures;

use serde_json::Value;
use std::process::Command;

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
    assert_eq!(json["warnings"].as_array().expect("warnings").len(), 0);
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
