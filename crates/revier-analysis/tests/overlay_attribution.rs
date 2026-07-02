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

#[test]
fn merge_conflict_resolution_attributes_new_content_to_merge_commit() {
    let fixture = fixtures::merge_conflict();
    let value = run_file_overlay(&fixture, "src/app.txt");
    let block = &value["overlay"]["blocks"].as_array().expect("blocks 数组")[0];
    let related_commits = block["relatedCommits"]
        .as_array()
        .expect("relatedCommits 数组");
    let warnings = block["attribution"]["warnings"]
        .as_array()
        .expect("warnings 数组");
    let merge_trace_commits = related_commits
        .iter()
        .filter(|commit| commit["attribution"]["method"] == "merge-trace")
        .collect::<Vec<_>>();

    assert_eq!(block["attribution"]["confidence"], "precise");
    assert!(warnings.is_empty(), "precise merge trace 不应有 warning");
    assert_eq!(
        merge_trace_commits.len(),
        1,
        "冲突解决块只应产生一个 merge-trace 归因，实际为 {merge_trace_commits:?}"
    );
    assert_eq!(merge_trace_commits[0]["subject"], "merge: resolve conflict");
    assert!(
        !merge_trace_commits[0]["attribution"]["viaMergeHashes"]
            .as_array()
            .expect("viaMergeHashes 数组")
            .is_empty(),
        "merge-trace 归因应包含 viaMergeHashes，实际为 {merge_trace_commits:?}"
    );
}

#[test]
fn ambiguous_merge_sources_are_marked_partial() {
    let fixture = fixtures::multi_parent_ambiguous();
    let value = run_file_overlay(&fixture, "src/app.txt");
    let block = &value["overlay"]["blocks"].as_array().expect("blocks 数组")[0];
    let related_commits = block["relatedCommits"]
        .as_array()
        .expect("relatedCommits 数组");
    let warnings = block["attribution"]["warnings"]
        .as_array()
        .expect("warnings 数组");
    let merge_trace_subjects = related_commits
        .iter()
        .filter(|commit| commit["attribution"]["method"] == "merge-trace")
        .map(|commit| commit["subject"].as_str().expect("提交主题"))
        .collect::<Vec<_>>();

    assert_eq!(block["attribution"]["confidence"], "partial");
    assert!(
        warnings
            .iter()
            .any(|warning| warning["code"] == "MERGE_TRACE_AMBIGUOUS"),
        "歧义 merge trace 应产生 MERGE_TRACE_AMBIGUOUS warning，实际为 {warnings:?}"
    );
    assert!(
        merge_trace_subjects.contains(&"feat: left shared")
            && merge_trace_subjects.contains(&"feat: right shared"),
        "歧义 merge trace 应指向左右父提交，实际为 {merge_trace_subjects:?}"
    );
    assert!(
        related_commits
            .iter()
            .filter(|commit| commit["attribution"]["method"] == "merge-trace")
            .all(|commit| !commit["attribution"]["viaMergeHashes"]
                .as_array()
                .expect("viaMergeHashes 数组")
                .is_empty()),
        "merge-trace 候选应包含 viaMergeHashes，实际为 {related_commits:?}"
    );
}

#[test]
fn unrelated_later_merge_does_not_pollute_merge_trace() {
    let fixture = fixtures::unrelated_merge_contains_same_block_text();
    let value = run_file_overlay(&fixture, "src/app.txt");
    let block = &value["overlay"]["blocks"].as_array().expect("blocks 数组")[0];
    let related_commits = block["relatedCommits"]
        .as_array()
        .expect("relatedCommits 数组");

    assert_eq!(block["attribution"]["confidence"], "precise");
    assert!(
        related_commits.iter().all(|commit| {
            commit["attribution"]["viaMergeHashes"]
                .as_array()
                .expect("viaMergeHashes 数组")
                .is_empty()
        }),
        "未触达当前 block 的后续 merge 不应产生 viaMergeHashes，实际为 {related_commits:?}"
    );
}

#[test]
fn parent_text_match_outside_block_window_does_not_explain_merge_source() {
    let fixture = fixtures::merge_adds_duplicate_text_with_parent_match_elsewhere();
    let value = run_file_overlay(&fixture, "src/app.txt");
    let block = &value["overlay"]["blocks"].as_array().expect("blocks 数组")[0];
    let related_commits = block["relatedCommits"]
        .as_array()
        .expect("relatedCommits 数组");
    let merge_trace_commits = related_commits
        .iter()
        .filter(|commit| commit["attribution"]["method"] == "merge-trace")
        .collect::<Vec<_>>();

    assert_eq!(block["attribution"]["confidence"], "precise");
    assert_eq!(
        merge_trace_commits.len(),
        1,
        "同文本只出现在父提交其它位置时，不应把父提交当作来源，实际为 {merge_trace_commits:?}"
    );
    assert_eq!(
        merge_trace_commits[0]["subject"],
        "merge: add duplicate shared"
    );
    assert!(
        related_commits
            .iter()
            .all(|commit| commit["subject"] != "docs: parent with same text elsewhere"),
        "父提交其它位置的相同文本不应污染 relatedCommits，实际为 {related_commits:?}"
    );
}

fn run_file_overlay(fixture: &fixtures::FixtureRepo, file: &str) -> Value {
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
            file,
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

    serde_json::from_slice(&output.stdout).expect("解析 file-overlay JSON")
}
