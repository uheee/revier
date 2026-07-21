mod fixtures;

use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use duckdb::params;
use revier_analysis::execution::AnalysisExecutionContext;
use revier_analysis::git::commits::IndexedCommit;
use revier_analysis::git::diff::CommitFileChange;
use serde_json::Value;

#[test]
fn appends_only_one_new_commit_after_initial_build() {
    let fixture = fixtures::linear();
    let dir = tempfile::tempdir().expect("创建临时目录");
    let db_path = dir.path().join("index.duckdb");
    let first = run_index_build(fixture.repo.path(), &db_path, "main");
    let first_total = count_rows(&db_path, "commits");

    std::fs::write(fixture.repo.path().join("src/app.txt"), "one\ntwo\nthree\n")
        .expect("写入新增提交");
    git(fixture.repo.path(), &["add", "."]);
    git(
        fixture.repo.path(),
        &["commit", "-m", "feat: add third line"],
    );
    let second = run_index_build(fixture.repo.path(), &db_path, "main");

    eprintln!(
        "首次索引={}ms，增量索引={}ms，增量提交数={}",
        first["elapsedMs"].as_u64().expect("首次索引耗时"),
        second["elapsedMs"].as_u64().expect("增量索引耗时"),
        second["indexedCommitCount"]
    );
    assert!(first["indexedCommitCount"].as_u64().expect("首次提交数") >= 2);
    assert_eq!(second["indexedCommitCount"], 1);
    assert_eq!(count_rows(&db_path, "commits"), first_total + 1);
}

#[test]
fn shared_commits_are_not_rewritten_when_indexing_another_branch() {
    let fixture = fixtures::linear();
    let dir = tempfile::tempdir().expect("创建临时目录");
    let db_path = dir.path().join("index.duckdb");
    run_index_build(fixture.repo.path(), &db_path, "main");
    let main_total = count_rows(&db_path, "commits");

    git(
        fixture.repo.path(),
        &["switch", "-c", "feature", &fixture.base],
    );
    std::fs::write(fixture.repo.path().join("feature.txt"), "feature\n")
        .expect("写入 feature 文件");
    git(fixture.repo.path(), &["add", "."]);
    git(fixture.repo.path(), &["commit", "-m", "feat: feature only"]);
    let feature = run_index_build(fixture.repo.path(), &db_path, "feature");

    assert_eq!(feature["indexedCommitCount"], 1);
    assert_eq!(count_rows(&db_path, "commits"), main_total + 1);
}

#[test]
fn failed_incremental_write_rolls_back_commits_files_and_run() {
    let (_dir, conn) = database();
    let commits = vec![commit("commit-1"), commit("commit-2")];
    let duplicate = file_change("commit-1", "src/lib.rs");
    let files = vec![duplicate.clone(), duplicate];

    let result = revier_analysis::index::writer::write_incremental_index(
        &conn,
        "repo-1",
        "2026-07-22T00:00:00Z",
        &commits,
        &files,
        &AnalysisExecutionContext::none(),
    );

    assert!(result.is_err());
    assert_eq!(count_rows_in_connection(&conn, "commits"), 0);
    assert_eq!(count_rows_in_connection(&conn, "commit_files"), 0);
    assert_eq!(count_rows_in_connection(&conn, "index_runs"), 0);
}

#[test]
fn cancelled_incremental_write_rolls_back_partial_rows() {
    let (_dir, conn) = database();
    let checks = Arc::new(AtomicUsize::new(0));
    let cancellation_checks = Arc::clone(&checks);
    let context = AnalysisExecutionContext::with_cancel(move || {
        cancellation_checks.fetch_add(1, Ordering::SeqCst) >= 2
    });

    let result = revier_analysis::index::writer::write_incremental_index(
        &conn,
        "repo-1",
        "2026-07-22T00:00:00Z",
        &[commit("commit-1"), commit("commit-2")],
        &[],
        &context,
    );

    assert!(matches!(
        result,
        Err(revier_analysis::error::AppError::Cancelled)
    ));
    assert_eq!(count_rows_in_connection(&conn, "commits"), 0);
    assert_eq!(count_rows_in_connection(&conn, "index_runs"), 0);
}

#[test]
fn pruning_keeps_local_reachable_and_snapshot_referenced_commits() {
    let (_dir, conn) = database();
    for hash in ["root", "live", "abandoned", "snapshot-head"] {
        insert_commit(&conn, hash);
    }
    conn.execute(
        "insert into commit_files
         (commit_hash, parent_hash, parent_index, path, old_path, status, additions, deletions,
          is_binary, is_previewable, similarity)
         values ('abandoned', 'root', 0, 'old.txt', '', 'modified', 0, 0, false, true, null)",
        [],
    )
    .expect("写入废弃提交文件");
    conn.execute(
        "insert into analysis_snapshots
         (analysis_id, repo_id, branch, base_commit, head_commit, filter_fingerprint,
          analysis_version, started_at, completed_at, elapsed_ms)
         values ('analysis-old', 'repo-1', 'old', 'root', 'snapshot-head', 'filter', 1,
                 '2026-07-22T00:00:00Z', '2026-07-22T00:00:01Z', 1000)",
        [],
    )
    .expect("写入旧快照");

    let pruned = revier_analysis::index::writer::prune_unreferenced_commits(
        &conn,
        &["root".to_string(), "live".to_string()],
    )
    .expect("清理不可达提交");

    assert_eq!(pruned, 1);
    assert_eq!(commit_hashes(&conn), vec!["live", "root", "snapshot-head"]);
    assert_eq!(count_rows_in_connection(&conn, "commit_files"), 0);
}

fn run_index_build(repo: &Path, db_path: &Path, branch: &str) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
        .args([
            "index",
            "build",
            "--repo",
            repo.to_str().expect("repo path"),
            "--db",
            db_path.to_str().expect("db path"),
            "--branch",
            branch,
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
    serde_json::from_slice(&output.stdout).expect("解析 index build 输出")
}

fn database() -> (tempfile::TempDir, duckdb::Connection) {
    let dir = tempfile::tempdir().expect("创建临时目录");
    let db_path = dir.path().join("index.duckdb");
    let conn = revier_analysis::index::connection::open_database(&db_path).expect("打开 DuckDB");
    revier_analysis::index::schema::initialize_schema(
        &conn,
        "repo-1",
        "E:/repo/app",
        "E:/repo/app/.git",
    )
    .expect("初始化 schema");
    (dir, conn)
}

fn commit(hash: &str) -> IndexedCommit {
    IndexedCommit {
        hash: hash.to_string(),
        short_hash: hash.to_string(),
        author_name: "测试作者".to_string(),
        author_email: Some("author@example.com".to_string()),
        author_key: "author@example.com".to_string(),
        committed_at: "2026-07-22T00:00:00Z".to_string(),
        subject: "测试提交".to_string(),
        parents: Vec::new(),
        is_merge: false,
    }
}

fn file_change(commit_hash: &str, path: &str) -> CommitFileChange {
    CommitFileChange {
        commit_hash: commit_hash.to_string(),
        parent_hash: "parent".to_string(),
        parent_index: 0,
        path: path.to_string(),
        old_path: None,
        status: "modified".to_string(),
        additions: 1,
        deletions: 0,
        is_binary: false,
        is_previewable: true,
        similarity: None,
        old_blob_id: Some("old-blob".to_string()),
        new_blob_id: Some("new-blob".to_string()),
    }
}

fn insert_commit(conn: &duckdb::Connection, hash: &str) {
    conn.execute(
        "insert into commits
         (hash, short_hash, author_name, author_email, author_key, committed_at,
          subject, parent_count, is_merge)
         values (?, ?, '测试作者', null, '测试作者', '2026-07-22T00:00:00Z', '测试提交', 0, false)",
        params![hash, hash],
    )
    .expect("写入提交");
}

fn count_rows(db_path: &Path, table: &str) -> i64 {
    let conn = revier_analysis::index::connection::open_database(db_path).expect("打开 DuckDB");
    count_rows_in_connection(&conn, table)
}

fn count_rows_in_connection(conn: &duckdb::Connection, table: &str) -> i64 {
    conn.query_row(&format!("select count(*) from {table}"), [], |row| {
        row.get(0)
    })
    .expect("读取表行数")
}

fn commit_hashes(conn: &duckdb::Connection) -> Vec<String> {
    let mut statement = conn
        .prepare("select hash from commits order by hash")
        .expect("准备提交查询");
    statement
        .query_map([], |row| row.get(0))
        .expect("查询提交")
        .collect::<Result<Vec<_>, _>>()
        .expect("读取提交")
}

fn git(repo: &Path, args: &[&str]) {
    let output = Command::new("git")
        .current_dir(repo)
        .args(args)
        .output()
        .expect("运行 git");
    assert!(
        output.status.success(),
        "git 命令失败：{}\n{}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
}
