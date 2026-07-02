mod fixtures;

use serde_json::Value;
use std::process::Command;

#[test]
fn added_or_modified_block_uses_blame_as_precise_attribution() {
    let fixture = fixtures::linear_with_authors();
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
    let block = &value["overlay"]["blocks"].as_array().expect("blocks 数组")[0];
    let related_commits = block["relatedCommits"]
        .as_array()
        .expect("relatedCommits 数组");
    let authors = block["authors"].as_array().expect("authors 数组");

    assert_eq!(block["attribution"]["confidence"], "precise");
    assert_eq!(related_commits[0]["attribution"]["method"], "blame");
    assert!(!authors.is_empty(), "authors 应来自 blame relatedCommits");
    assert_eq!(related_commits[0]["matchedByFilter"], true);
}

#[test]
fn file_overlay_attaches_related_commit_from_patch_inference() {
    let fixture = fixtures::linear_with_authors();
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
            "--author",
            "alice@example.com",
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
    let blocks = value["overlay"]["blocks"].as_array().expect("blocks 数组");
    let related_commits = blocks[0]["relatedCommits"]
        .as_array()
        .expect("relatedCommits 数组");
    let alice_commit = related_commits
        .iter()
        .find(|commit| commit["authorEmail"] == "alice@example.com")
        .expect("relatedCommits 应包含 Alice 提交");

    assert_eq!(alice_commit["matchedByFilter"], true);
    assert_eq!(alice_commit["attribution"]["method"], "blame");
    assert!(
        !alice_commit["touchedRanges"]
            .as_array()
            .expect("touchedRanges 数组")
            .is_empty(),
        "blame relatedCommit 应复用 patch inference touchedRanges"
    );
}

#[test]
fn file_overlay_does_not_attach_unrelated_commit_after_rename() {
    let fixture = fixtures::rename_then_unrelated();
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
            "src/new.txt",
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
    let blocks = value["overlay"]["blocks"].as_array().expect("blocks 数组");
    let subjects = blocks
        .iter()
        .flat_map(|block| {
            block["relatedCommits"]
                .as_array()
                .expect("relatedCommits 数组")
        })
        .map(|commit| commit["subject"].as_str().expect("提交主题"))
        .collect::<Vec<_>>();

    assert!(
        subjects.contains(&"feat: rename and edit"),
        "relatedCommits 应包含 rename 修改提交，实际为 {subjects:?}"
    );
    assert!(
        !subjects.contains(&"chore: unrelated docs"),
        "relatedCommits 不应包含 rename 后的无关提交，实际为 {subjects:?}"
    );
}

#[test]
fn file_overlay_does_not_attach_reused_old_path_after_rename() {
    let fixture = fixtures::rename_then_reused_old_path();
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
            "src/new.txt",
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
    let blocks = value["overlay"]["blocks"].as_array().expect("blocks 数组");
    let subjects = blocks
        .iter()
        .flat_map(|block| {
            block["relatedCommits"]
                .as_array()
                .expect("relatedCommits 数组")
        })
        .map(|commit| commit["subject"].as_str().expect("提交主题"))
        .collect::<Vec<_>>();

    assert!(
        subjects.contains(&"feat: rename and edit"),
        "relatedCommits 应包含 rename 修改提交，实际为 {subjects:?}"
    );
    assert!(
        !subjects.contains(&"chore: edit recreated old path same line"),
        "relatedCommits 不应包含复用旧路径的提交，实际为 {subjects:?}"
    );
}
