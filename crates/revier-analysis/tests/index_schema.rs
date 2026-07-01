mod fixtures;

use serde_json::Value;
use std::process::Command;
use tempfile::tempdir;

#[test]
fn initializes_schema_version_one_tables() {
    let dir = tempdir().expect("创建临时目录");
    let db_path = dir.path().join("index.duckdb");
    let conn = revier_analysis::index::connection::open_database(&db_path).expect("打开 DuckDB");

    revier_analysis::index::schema::initialize_schema(
        &conn,
        "repo-1",
        "E:/repo/app",
        "E:/repo/app/.git",
    )
    .expect("初始化 schema");

    let version =
        revier_analysis::index::schema::read_schema_version(&conn).expect("读取 schema version");
    assert_eq!(version, Some(1));

    let tables = revier_analysis::index::schema::list_tables(&conn).expect("读取表列表");
    assert!(tables.contains(&"metadata".to_string()));
    assert!(tables.contains(&"commits".to_string()));
    assert!(tables.contains(&"commit_parents".to_string()));
    assert!(tables.contains(&"commit_files".to_string()));
    assert!(tables.contains(&"index_runs".to_string()));
}

#[test]
fn reports_incompatible_schema_version() {
    let dir = tempdir().expect("创建临时目录");
    let db_path = dir.path().join("index.duckdb");
    let conn = revier_analysis::index::connection::open_database(&db_path).expect("打开 DuckDB");

    conn.execute_batch(
        "create table metadata (key text primary key, value text not null);
         insert into metadata values ('schema_version', '999');",
    )
    .expect("写入不兼容版本");

    let result = revier_analysis::index::migrations::ensure_compatible_schema(&conn);
    assert!(matches!(
        result,
        Err(revier_analysis::error::AppError::SchemaIncompatible(_))
    ));
}

#[test]
fn index_build_then_status_returns_ready() {
    let fixture = fixtures::linear_with_authors();
    let dir = tempfile::tempdir().expect("创建临时目录");
    let db_path = dir.path().join("index.duckdb");

    let build = Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
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
        build.status.success(),
        "index build 应成功，stderr: {}",
        String::from_utf8_lossy(&build.stderr)
    );

    let build_json: Value = serde_json::from_slice(&build.stdout).expect("解析 build json");
    assert_eq!(build_json["version"], 1);
    assert_eq!(build_json["status"], "completed");
    assert!(
        build_json["indexedCommitCount"]
            .as_u64()
            .expect("commit count")
            >= 3
    );

    let status = Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
        .args([
            "index",
            "status",
            "--repo",
            fixture.repo.path().to_str().expect("repo path"),
            "--db",
            db_path.to_str().expect("db path"),
            "--format",
            "json",
        ])
        .output()
        .expect("运行 index status");

    assert!(status.status.success());
    let status_json: Value = serde_json::from_slice(&status.stdout).expect("解析 status json");
    assert_eq!(status_json["status"], "ready");
    assert!(
        status_json["indexedFileCount"]
            .as_u64()
            .expect("file count")
            >= 1
    );
}

#[test]
fn index_build_replaces_stale_file_rows() {
    let fixture = fixtures::linear_with_authors();
    let dir = tempfile::tempdir().expect("创建临时目录");
    let db_path = dir.path().join("index.duckdb");

    run_index_build(&fixture, &db_path);
    let conn = revier_analysis::index::connection::open_database(&db_path).expect("打开 DuckDB");
    let first_count = count_commit_files(&conn);
    conn.execute(
        "insert into commit_files
         (commit_hash, parent_hash, parent_index, path, old_path, status, additions, deletions, is_binary, is_previewable, similarity)
         values (?, ?, 0, 'stale.txt', '', 'modified', 0, 0, false, true, null)",
        duckdb::params![fixture.head, fixture.base],
    )
    .expect("写入 stale 文件行");
    drop(conn);

    run_index_build(&fixture, &db_path);

    let conn = revier_analysis::index::connection::open_database(&db_path).expect("打开 DuckDB");
    let second_count = count_commit_files(&conn);
    assert_eq!(second_count, first_count);
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

fn count_commit_files(conn: &duckdb::Connection) -> i64 {
    conn.query_row("select count(*) from commit_files", [], |row| row.get(0))
        .expect("读取 commit_files 数量")
}
