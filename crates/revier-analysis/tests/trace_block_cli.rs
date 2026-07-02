mod fixtures;

use serde_json::Value;
use std::process::Command;

#[test]
fn trace_block_by_block_id_outputs_single_block_attribution() {
    let fixture = fixtures::linear();
    let output = run_trace_block(
        &fixture,
        &[
            "--file",
            "src/app.txt",
            "--block-id",
            "block-1",
            "--format",
            "json",
        ],
    );

    assert!(
        output.status.success(),
        "trace-block 应成功，stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let value = parse_stdout(&output.stdout);
    assert_eq!(value["version"], 1);
    assert_eq!(value["file"], "src/app.txt");
    assert_eq!(value["blockId"], "block-1");
    assert_eq!(value["attribution"]["confidence"], "precise");
    assert_eq!(value["relatedCommits"][0]["attribution"]["method"], "blame");
    assert!(value["warnings"]
        .as_array()
        .expect("warnings 数组")
        .iter()
        .any(|item| item == "索引不可用，已即时计算 overlay 结果"));
}

#[test]
fn trace_block_can_select_added_block_by_new_line_range() {
    let fixture = fixtures::linear();
    let output = run_trace_block(
        &fixture,
        &[
            "--file",
            "src/app.txt",
            "--new-start",
            "2",
            "--new-end",
            "2",
            "--format",
            "json",
        ],
    );

    assert!(
        output.status.success(),
        "trace-block 应通过新侧区间定位新增块，stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let value = parse_stdout(&output.stdout);
    assert_eq!(value["blockId"], "block-1");
    assert_eq!(value["relatedCommits"][0]["attribution"]["method"], "blame");
}

#[test]
fn trace_block_rejects_zero_new_line_range() {
    let fixture = fixtures::linear_deletion();
    let output = run_trace_block(
        &fixture,
        &[
            "--file",
            "src/app.txt",
            "--new-start",
            "0",
            "--new-end",
            "0",
            "--format",
            "json",
        ],
    );

    assert_eq!(
        output.status.code(),
        Some(2),
        "新侧 0 行区间应返回退出码 2，stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("0"),
        "stderr 应说明行号不能为 0"
    );
}

#[test]
fn trace_block_rejects_zero_old_line_range() {
    let fixture = fixtures::linear();
    let output = run_trace_block(
        &fixture,
        &[
            "--file",
            "src/app.txt",
            "--old-start",
            "0",
            "--old-end",
            "0",
            "--format",
            "json",
        ],
    );

    assert_eq!(
        output.status.code(),
        Some(2),
        "旧侧 0 行区间应返回退出码 2，stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("0"),
        "stderr 应说明行号不能为 0"
    );
}

#[test]
fn trace_block_returns_exit_code_two_for_incomplete_old_line_range() {
    let fixture = fixtures::linear();
    let output = run_trace_block(
        &fixture,
        &[
            "--file",
            "src/app.txt",
            "--old-start",
            "1",
            "--format",
            "json",
        ],
    );

    assert_eq!(
        output.status.code(),
        Some(2),
        "不完整旧侧区间应返回退出码 2，stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("旧侧区间"),
        "stderr 应说明旧侧区间不完整"
    );
}

#[test]
fn trace_block_prefers_block_id_over_mismatched_line_ranges() {
    let fixture = fixtures::linear();
    let output = run_trace_block(
        &fixture,
        &[
            "--file",
            "src/app.txt",
            "--block-id",
            "block-1",
            "--old-start",
            "999",
            "--old-end",
            "999",
            "--new-start",
            "999",
            "--new-end",
            "999",
            "--format",
            "json",
        ],
    );

    assert!(
        output.status.success(),
        "block-id 应优先于不匹配区间，stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let value = parse_stdout(&output.stdout);
    assert_eq!(value["blockId"], "block-1");
}

#[test]
fn trace_block_requires_block_selector() {
    let fixture = fixtures::linear();
    let output = run_trace_block(&fixture, &["--file", "src/app.txt", "--format", "json"]);

    assert_eq!(
        output.status.code(),
        Some(2),
        "缺少 selector 应返回退出码 2，stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("block")
            || String::from_utf8_lossy(&output.stderr).contains("区间"),
        "stderr 应说明缺少 block 或区间 selector"
    );
}

#[test]
fn trace_block_returns_exit_code_two_when_block_id_is_missing() {
    let fixture = fixtures::linear();
    let output = run_trace_block(
        &fixture,
        &[
            "--file",
            "src/app.txt",
            "--block-id",
            "block-missing",
            "--format",
            "json",
        ],
    );

    assert_eq!(
        output.status.code(),
        Some(2),
        "找不到 block id 应返回退出码 2，stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("block"),
        "stderr 应说明找不到 block"
    );
}

#[test]
fn trace_block_can_select_deleted_block_by_old_line_range_and_returns_deletion_trace() {
    let fixture = fixtures::linear_deletion();
    let output = run_trace_block(
        &fixture,
        &[
            "--file",
            "src/app.txt",
            "--old-start",
            "2",
            "--old-end",
            "2",
            "--format",
            "json",
        ],
    );

    assert!(
        output.status.success(),
        "trace-block 应通过旧侧区间定位删除块，stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let value = parse_stdout(&output.stdout);
    assert_eq!(value["blockId"], "block-1");
    assert_eq!(
        value["relatedCommits"][0]["attribution"]["method"],
        "deletion-trace"
    );
}

fn run_trace_block(fixture: &fixtures::FixtureRepo, extra_args: &[&str]) -> std::process::Output {
    let mut args = vec![
        "trace-block",
        "--repo",
        fixture.repo.path().to_str().expect("仓库路径"),
        "--base",
        &fixture.base,
        "--head",
        &fixture.head,
        "--branch",
        "main",
    ];
    args.extend_from_slice(extra_args);

    Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
        .args(args)
        .output()
        .expect("运行 trace-block 命令")
}

fn parse_stdout(stdout: &[u8]) -> Value {
    serde_json::from_slice(stdout).expect("解析 trace-block JSON")
}
