mod fixtures;

use serde_json::Value;
use std::process::Command;

#[test]
fn help_command_exits_successfully() {
    let output = Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
        .arg("--help")
        .output()
        .expect("run revier-analysis --help");

    assert!(
        output.status.success(),
        "expected --help to succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("revier-analysis"));
    assert!(stdout.contains("spike"));
}

#[test]
fn index_help_lists_status_build_and_query_files_commands() {
    let output = Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
        .args(["index", "--help"])
        .output()
        .expect("运行 revier-analysis index --help");

    assert!(
        output.status.success(),
        "index --help 应成功，stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("status"));
    assert!(stdout.contains("build"));
    assert!(stdout.contains("query-files"));
}

#[test]
fn spike_run_outputs_stable_json_shape_for_requested_fixture() {
    let fixture = crate::fixtures::rename_merge();
    let output = Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
        .args([
            "spike",
            "run",
            "--repo",
            fixture.repo.path().to_str().expect("repo path"),
            "--fixture",
            "rename-merge",
            "--format",
            "json",
        ])
        .output()
        .expect("run spike command");

    assert!(output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout).expect("parse spike json");
    assert_eq!(value["version"], 1);
    assert_eq!(value["fixture"], "rename-merge");
    assert!(value["checks"].is_array());
    assert!(value["decision"].is_string());
}

#[test]
fn spike_run_reports_checks_for_linear_fixture() {
    let fixture = crate::fixtures::linear();
    let output = Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
        .args([
            "spike",
            "run",
            "--repo",
            fixture.repo.path().to_str().expect("repo path"),
            "--fixture",
            "linear",
            "--format",
            "json",
        ])
        .output()
        .expect("run spike command");

    assert!(output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout).expect("parse spike json");
    assert_eq!(value["fixture"], "linear");
    assert!(
        value["checks"].as_array().expect("checks array").len() >= 2,
        "expected blame and parent checks"
    );
}

#[test]
fn top_level_help_lists_file_overlay_and_trace_block_commands() {
    let output = Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
        .arg("--help")
        .output()
        .expect("运行 revier-analysis --help");

    assert!(
        output.status.success(),
        "--help 应成功，stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("file-overlay"));
    assert!(stdout.contains("trace-block"));
}

#[test]
fn file_overlay_help_lists_required_arguments() {
    let output = Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
        .args(["file-overlay", "--help"])
        .output()
        .expect("运行 revier-analysis file-overlay --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--repo"));
    assert!(stdout.contains("--base"));
    assert!(stdout.contains("--head"));
    assert!(stdout.contains("--branch"));
    assert!(stdout.contains("--file"));
    assert!(stdout.contains("--require-index"));
}

#[test]
fn trace_block_help_lists_block_and_line_arguments() {
    let output = Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
        .args(["trace-block", "--help"])
        .output()
        .expect("运行 revier-analysis trace-block --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--block-id"));
    assert!(stdout.contains("--old-start"));
    assert!(stdout.contains("--new-start"));
}
