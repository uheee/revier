mod fixtures;

use duckdb::params;
use serde_json::Value;
use std::path::Path;
use std::process::Command;

#[test]
fn file_overlay_outputs_file_overlay_compatible_json_for_linear_change() {
    let fixture = fixtures::linear();
    let output = Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
        .args([
            "file-overlay",
            "--repo",
            fixture.repo.path().to_str().expect("仓库路径"),
            "--base",
            &fixture.base,
            "--head",
            &fixture.head,
            "--branch",
            "main",
            "--file",
            "src/app.txt",
            "--format",
            "json",
        ])
        .output()
        .expect("运行 file-overlay 命令");

    assert!(
        output.status.success(),
        "file-overlay 应成功，stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let value: Value = serde_json::from_slice(&output.stdout).expect("解析 file-overlay JSON");
    assert_eq!(value["version"], 1);
    assert!(value["warnings"]
        .as_array()
        .expect("顶层 warnings 数组")
        .iter()
        .any(|item| item == "索引不可用，已即时计算 overlay 结果"));

    let overlay = &value["overlay"];
    assert_eq!(overlay["mode"], "range");
    assert!(overlay["warnings"]
        .as_array()
        .expect("overlay warnings 数组")
        .iter()
        .any(|item| item == "索引不可用，已即时计算 overlay 结果"));

    assert_eq!(overlay["file"]["path"], "src/app.txt");
    assert_eq!(overlay["file"]["oldPath"], Value::Null);
    assert_eq!(overlay["file"]["status"], "modified");
    assert_eq!(overlay["file"]["isBinary"], false);
    assert_eq!(overlay["file"]["isPreviewable"], true);

    assert_eq!(overlay["range"]["branch"], "main");
    assert_eq!(overlay["range"]["baseCommit"], fixture.base);
    assert_eq!(overlay["range"]["headCommit"], fixture.head);
    assert_eq!(overlay["range"]["startAt"], Value::Null);
    assert_eq!(overlay["range"]["endAt"], Value::Null);

    let rows = overlay["rows"].as_array().expect("rows 数组");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["type"], "context");
    assert_eq!(rows[0]["oldLineNumber"], 1);
    assert_eq!(rows[0]["newLineNumber"], 1);
    assert_eq!(rows[0]["oldText"], "one");
    assert_eq!(rows[0]["newText"], "one");
    assert_eq!(rows[1]["type"], "added");
    assert_eq!(rows[1]["oldLineNumber"], Value::Null);
    assert_eq!(rows[1]["newLineNumber"], 2);
    assert_eq!(rows[1]["oldText"], Value::Null);
    assert_eq!(rows[1]["newText"], "two");

    let blocks = overlay["blocks"].as_array().expect("blocks 数组");
    assert_eq!(blocks.len(), 1);
    assert_eq!(blocks[0]["id"], "block-1");
    assert_eq!(blocks[0]["changeType"], "added");
    assert_eq!(blocks[0]["attribution"], Value::Null);
    assert!(!blocks[0]["authors"]
        .as_array()
        .expect("authors 数组")
        .is_empty());
    let related_commits = blocks[0]["relatedCommits"]
        .as_array()
        .expect("relatedCommits 数组");
    assert!(!related_commits.is_empty());
    assert_eq!(related_commits[0]["matchedByFilter"], true);
    assert_eq!(
        related_commits[0]["attribution"]["method"],
        "patch-inference"
    );
}

#[test]
fn file_overlay_returns_exit_code_four_for_binary_file() {
    let fixture = fixtures::binary_change();
    let output = Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
        .args([
            "file-overlay",
            "--repo",
            fixture.repo.path().to_str().expect("仓库路径"),
            "--base",
            &fixture.base,
            "--head",
            &fixture.head,
            "--branch",
            "main",
            "--file",
            "assets/logo.bin",
            "--format",
            "json",
        ])
        .output()
        .expect("运行 file-overlay 命令");

    assert_eq!(
        output.status.code(),
        Some(4),
        "二进制文件应返回退出码 4，stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("文件不可分析"),
        "stderr 应说明文件不可分析"
    );
}

#[test]
fn file_overlay_require_index_returns_exit_code_five_when_db_is_missing() {
    let fixture = fixtures::linear();
    let dir = tempfile::tempdir().expect("创建临时目录");
    let db_path = dir.path().join("missing.duckdb");

    let output = Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
        .args([
            "file-overlay",
            "--repo",
            fixture.repo.path().to_str().expect("仓库路径"),
            "--db",
            db_path.to_str().expect("数据库路径"),
            "--base",
            &fixture.base,
            "--head",
            &fixture.head,
            "--branch",
            "main",
            "--file",
            "src/app.txt",
            "--require-index",
            "--format",
            "json",
        ])
        .output()
        .expect("运行 file-overlay 命令");

    assert_eq!(
        output.status.code(),
        Some(5),
        "缺少必需索引应返回退出码 5，stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("索引不可用且命令要求必须使用索引"),
        "stderr 应说明必需索引不可用"
    );
}

#[test]
fn file_overlay_without_index_succeeds_and_reports_warning() {
    let fixture = fixtures::linear();
    let dir = tempfile::tempdir().expect("创建临时目录");
    let db_path = dir.path().join("missing.duckdb");

    let output = Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
        .args([
            "file-overlay",
            "--repo",
            fixture.repo.path().to_str().expect("仓库路径"),
            "--db",
            db_path.to_str().expect("数据库路径"),
            "--base",
            &fixture.base,
            "--head",
            &fixture.head,
            "--branch",
            "main",
            "--file",
            "src/app.txt",
            "--format",
            "json",
        ])
        .output()
        .expect("运行 file-overlay 命令");

    assert!(
        output.status.success(),
        "缺少非必需索引时 file-overlay 应成功，stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let value: Value = serde_json::from_slice(&output.stdout).expect("解析 file-overlay JSON");
    let warning = "索引不可用，已即时计算 overlay 结果";
    assert!(
        value["warnings"]
            .as_array()
            .expect("顶层 warnings 数组")
            .iter()
            .any(|item| item == warning),
        "顶层 warnings 应包含缺索引提示"
    );
    assert!(
        value["overlay"]["warnings"]
            .as_array()
            .expect("overlay warnings 数组")
            .iter()
            .any(|item| item == warning),
        "overlay warnings 应包含缺索引提示"
    );
}

#[test]
fn file_overlay_require_index_returns_exit_code_five_when_range_commit_is_missing() {
    let fixture = fixtures::linear_with_authors();
    let dir = tempfile::tempdir().expect("创建临时目录");
    let db_path = dir.path().join("index.duckdb");
    run_index_build(&fixture, &db_path);
    let missing_hash = range_middle_commit_hash(&fixture);
    delete_indexed_commit(&db_path, &missing_hash);

    let output = Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
        .args([
            "file-overlay",
            "--repo",
            fixture.repo.path().to_str().expect("仓库路径"),
            "--db",
            db_path.to_str().expect("数据库路径"),
            "--base",
            &fixture.base,
            "--head",
            &fixture.head,
            "--branch",
            "main",
            "--file",
            "src/app.txt",
            "--require-index",
            "--format",
            "json",
        ])
        .output()
        .expect("运行 file-overlay 命令");

    assert_eq!(
        output.status.code(),
        Some(5),
        "范围提交缺少索引时应返回退出码 5，stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(&missing_hash),
        "stderr 应包含缺失提交哈希"
    );
}

#[test]
fn file_overlay_without_complete_index_succeeds_and_reports_warning() {
    let fixture = fixtures::linear_with_authors();
    let dir = tempfile::tempdir().expect("创建临时目录");
    let db_path = dir.path().join("index.duckdb");
    run_index_build(&fixture, &db_path);
    let missing_hash = range_middle_commit_hash(&fixture);
    delete_indexed_commit(&db_path, &missing_hash);

    let output = Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
        .args([
            "file-overlay",
            "--repo",
            fixture.repo.path().to_str().expect("仓库路径"),
            "--db",
            db_path.to_str().expect("数据库路径"),
            "--base",
            &fixture.base,
            "--head",
            &fixture.head,
            "--branch",
            "main",
            "--file",
            "src/app.txt",
            "--format",
            "json",
        ])
        .output()
        .expect("运行 file-overlay 命令");

    assert!(
        output.status.success(),
        "范围提交缺少非必需索引时 file-overlay 应成功，stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).expect("解析 file-overlay JSON");
    let warning = "索引不可用，已即时计算 overlay 结果";
    assert!(value["warnings"]
        .as_array()
        .expect("顶层 warnings 数组")
        .iter()
        .any(|item| item == warning));
    assert!(value["overlay"]["warnings"]
        .as_array()
        .expect("overlay warnings 数组")
        .iter()
        .any(|item| item == warning));
}

#[test]
fn file_overlay_returns_exit_code_four_for_file_outside_analysis_range() {
    let fixture = fixtures::linear();
    let output = Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
        .args([
            "file-overlay",
            "--repo",
            fixture.repo.path().to_str().expect("仓库路径"),
            "--base",
            &fixture.base,
            "--head",
            &fixture.head,
            "--branch",
            "main",
            "--file",
            "src/missing.txt",
            "--format",
            "json",
        ])
        .output()
        .expect("运行 file-overlay 命令");

    assert_eq!(
        output.status.code(),
        Some(4),
        "不在分析范围的文件应返回退出码 4，stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("文件不可分析"),
        "stderr 应说明文件不可分析"
    );
}

fn run_index_build(fixture: &fixtures::FixtureRepo, db_path: &Path) {
    let output = Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
        .args([
            "index",
            "build",
            "--repo",
            fixture.repo.path().to_str().expect("仓库路径"),
            "--db",
            db_path.to_str().expect("数据库路径"),
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

fn range_middle_commit_hash(fixture: &fixtures::FixtureRepo) -> String {
    let repo =
        revier_analysis::git::repository::open_repository(fixture.repo.path()).expect("打开仓库");
    let hashes =
        revier_analysis::git::commits::range_commit_hashes(&repo, &fixture.base, &fixture.head)
            .expect("读取范围提交");
    hashes.first().expect("范围内应有中间提交").clone()
}

fn delete_indexed_commit(db_path: &Path, commit_hash: &str) {
    let conn = revier_analysis::index::connection::open_database(db_path).expect("打开索引数据库");
    conn.execute("delete from commits where hash = ?", params![commit_hash])
        .expect("删除索引提交");
}
