mod fixtures;

use serde_json::Value;
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
        .is_empty());

    let overlay = &value["overlay"];
    assert_eq!(overlay["mode"], "range");
    assert!(overlay["warnings"]
        .as_array()
        .expect("overlay warnings 数组")
        .is_empty());

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
    assert!(blocks[0]["authors"]
        .as_array()
        .expect("authors 数组")
        .is_empty());
    assert!(blocks[0]["relatedCommits"]
        .as_array()
        .expect("relatedCommits 数组")
        .is_empty());
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
