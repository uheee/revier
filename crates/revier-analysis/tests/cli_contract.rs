#[allow(dead_code)]
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
