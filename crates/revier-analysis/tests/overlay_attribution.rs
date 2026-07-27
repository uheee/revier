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
fn merge_source_followed_by_unrelated_commit_keeps_real_source() {
    let fixture = fixtures::merge_source_followed_by_unrelated_commit();
    let value = run_file_overlay(&fixture, "src/app.txt");
    let block = &value["overlay"]["blocks"].as_array().expect("blocks 数组")[0];
    let subjects = related_subjects(block);
    let source = related_commit_with_subject(block, "feat: add feature line");
    let source_method = source["attribution"]["method"].as_str().expect("归因方法");

    assert!(
        subjects.contains(&"feat: add feature line"),
        "应保留真实来源提交，实际为 {subjects:?}"
    );
    assert!(
        !subjects.contains(&"chore: unrelated docs change"),
        "不应保留与目标块无关的提交，实际为 {subjects:?}"
    );
    assert!(
        source_method == "merge-trace" || source_method == "blame",
        "真实来源应可归因到真实提交，实际为 {source_method}"
    );
}

#[test]
fn nested_merge_source_tracks_real_source_and_chain() {
    let fixture = fixtures::nested_merge_source();
    let value = run_file_overlay(&fixture, "src/app.txt");
    let block = &value["overlay"]["blocks"].as_array().expect("blocks 数组")[0];
    let subjects = related_subjects(block);
    let source = related_commit_with_subject(block, "feat: add A line");
    let via_hashes = source["attribution"]["viaMergeHashes"]
        .as_array()
        .expect("viaMergeHashes 数组")
        .iter()
        .filter_map(|item| item.as_str())
        .collect::<Vec<_>>();

    assert!(
        subjects.contains(&"feat: add A line"),
        "应最终归因到原始来源提交，实际为 {subjects:?}"
    );
    assert!(
        !subjects.contains(&"merge: feature into integration"),
        "链路 merge 提交不应作为最终来源，实际为 {subjects:?}"
    );
    assert!(
        !subjects.contains(&"merge: integrate feature"),
        "链路 merge 提交不应作为最终来源，实际为 {subjects:?}"
    );
    assert_eq!(
        via_hashes.len(),
        2,
        "多层 merge 的 viaMergeHashes 应保留完整链路，实际为 {via_hashes:?}"
    );
}

#[test]
fn merge_only_second_parent_change_keeps_real_source_not_merge_commit() {
    let fixture = fixtures::merge_only_differs_from_second_parent();
    let value = run_file_overlay(&fixture, "src/app.txt");
    let block = &value["overlay"]["blocks"].as_array().expect("blocks 数组")[0];
    let subjects = related_subjects(block);
    let methods = block["relatedCommits"]
        .as_array()
        .expect("relatedCommits 数组")
        .iter()
        .map(|commit| {
            (
                commit["subject"].as_str().expect("提交主题"),
                commit["attribution"]["method"].as_str().expect("归因方法"),
            )
        })
        .collect::<Vec<_>>();

    assert!(
        subjects.contains(&"feat: add feature line"),
        "应追踪到真实修改提交，实际为 {subjects:?}"
    );
    assert!(
        !subjects.contains(&"merge: second parent only change"),
        "仅第二父有差异的 merge 不应作为来源提交，实际为 {subjects:?}"
    );
    assert!(
        methods.iter().all(
            |(subject, method)| *subject != "merge: second parent only change"
                || *method != "merge-trace"
        ),
        "不应把 merge 提交作为 final 来源，实际为 {methods:?}"
    );
}

#[test]
fn ambiguous_nested_merge_sources_mark_partial_and_keep_branch_sources_only() {
    let fixture = fixtures::ambiguous_nested_merge_sources();
    let value = run_file_overlay(&fixture, "src/app.txt");
    let block = &value["overlay"]["blocks"].as_array().expect("blocks 数组")[0];
    let subjects = related_subjects(block);
    let warnings = block["attribution"]["warnings"]
        .as_array()
        .expect("warnings 数组");

    assert_eq!(block["attribution"]["confidence"], "partial");
    assert!(
        warnings
            .iter()
            .any(|warning| warning["code"] == "MERGE_TRACE_AMBIGUOUS"),
        "歧义 merge 应产生 MERGE_TRACE_AMBIGUOUS，实际为 {warnings:?}"
    );
    assert!(
        subjects.contains(&"feat: left path") && subjects.contains(&"feat: right path"),
        "多层歧义应保留左右真实来源，实际为 {subjects:?}"
    );
    assert!(
        !subjects.contains(&"merge: ambiguous nested sources"),
        "链路 merge 不应作为最终来源，实际为 {subjects:?}"
    );
}

#[test]
fn merge_deletion_source_not_polluted_by_unrelated_and_merge() {
    let fixture = fixtures::merge_deletion_source_followed_by_unrelated_commit();
    let value = run_file_overlay(&fixture, "src/app.txt");
    let block = &value["overlay"]["blocks"].as_array().expect("blocks 数组")[0];
    let subjects = related_subjects(block);
    let related_commit = related_commit_with_subject(block, "fix: delete line");

    assert_eq!(block["changeType"], "deleted");
    assert!(
        subjects.contains(&"fix: delete line"),
        "应保留真正的删除来源，实际为 {subjects:?}"
    );
    assert_eq!(related_commit["attribution"]["method"], "deletion-trace");
    assert!(
        !subjects.contains(&"chore: unrelated docs"),
        "不应包含无关提交，实际为 {subjects:?}"
    );
    assert!(
        !subjects.contains(&"merge: delete from feature"),
        "普通 merge 不应成为删除块主要来源，实际为 {subjects:?}"
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

#[test]
fn linear_deleted_block_uses_precise_deletion_trace() {
    let fixture = fixtures::linear_deletion();
    let value = run_file_overlay(&fixture, "src/app.txt");
    let block = &value["overlay"]["blocks"].as_array().expect("blocks 数组")[0];
    let related_commit = related_commit_with_subject(block, "fix: delete line");
    let warnings = block["attribution"]["warnings"]
        .as_array()
        .expect("warnings 数组");

    assert_eq!(block["changeType"], "deleted");
    assert_eq!(block["attribution"]["confidence"], "precise");
    assert_eq!(related_commit["attribution"]["method"], "deletion-trace");
    assert!(
        !warnings
            .iter()
            .any(|warning| warning["code"] == "DELETION_TRACE_INCOMPLETE"),
        "明确 deletion-trace 不应保留 deletion incomplete warning，实际为 {warnings:?}"
    );
}

#[test]
fn replacement_hunk_old_side_deletion_uses_deletion_trace() {
    let fixture = fixtures::replacement_hunk_deletion();
    let value = run_file_overlay(&fixture, "src/app.txt");
    let block = &value["overlay"]["blocks"].as_array().expect("blocks 数组")[0];
    let related_commit = related_commit_with_subject(block, "fix: replace deleted line");

    assert_eq!(block["changeType"], "modified");
    assert_eq!(block["attribution"]["confidence"], "precise");
    assert_eq!(related_commit["attribution"]["method"], "deletion-trace");
}

#[test]
fn stale_modified_deletion_does_not_override_final_change_blame() {
    let fixture = fixtures::stale_modified_deletion_then_final_change();
    let value = run_file_overlay(&fixture, "src/app.txt");
    let block = &value["overlay"]["blocks"].as_array().expect("blocks 数组")[0];
    let related_commits = block["relatedCommits"]
        .as_array()
        .expect("relatedCommits 数组");

    assert_eq!(block["changeType"], "modified");
    assert!(
        related_commits
            .iter()
            .any(|commit| commit["subject"] == "fix: bar to baz"),
        "最终 modified block 应保留最终新侧提交归因，实际为 {related_commits:?}"
    );
    assert!(
        related_commits.iter().all(|commit| {
            !(commit["subject"] == "fix: foo to bar"
                && commit["attribution"]["method"] == "deletion-trace")
        }),
        "旧侧删除提交不能作为 precise deletion-trace 覆盖最终 modified block，实际为 {related_commits:?}"
    );
}

#[test]
fn deleted_block_uses_deletion_trace_or_inference() {
    let fixture = fixtures::deletion_merge();
    let value = run_file_overlay(&fixture, "src/app.txt");
    let block = &value["overlay"]["blocks"].as_array().expect("blocks 数组")[0];
    let confidence = block["attribution"]["confidence"]
        .as_str()
        .expect("归因 confidence");
    let related_commits = block["relatedCommits"]
        .as_array()
        .expect("relatedCommits 数组");
    let warnings = block["attribution"]["warnings"]
        .as_array()
        .expect("warnings 数组");
    let subjects = related_commits
        .iter()
        .map(|commit| commit["subject"].as_str().expect("提交主题"))
        .collect::<Vec<_>>();

    assert_eq!(block["changeType"], "deleted");
    assert!(
        ["precise", "inferred", "partial"].contains(&confidence),
        "删除块 confidence 应为 precise/inferred/partial 之一，实际为 {confidence}"
    );
    assert!(
        subjects
            .iter()
            .any(|subject| subject.contains("delete line")),
        "relatedCommits 应包含删除提交，实际为 {subjects:?}"
    );
    assert!(
        !subjects.contains(&"merge: delete feature"),
        "普通 merge 不应作为删除块主要来源，实际为 {subjects:?}"
    );
    assert!(
        !warnings
            .iter()
            .any(|warning| warning["code"] == "DELETION_TRACE_INCOMPLETE"),
        "找到删除提交后不应继续标记删除追踪不完整，实际为 {warnings:?}"
    );
}

#[test]
fn repeated_deleted_text_does_not_attribute_other_location_to_current_block() {
    let fixture = fixtures::duplicate_deletions_same_text();
    let value = run_file_overlay(&fixture, "src/app.txt");
    let blocks = value["overlay"]["blocks"].as_array().expect("blocks 数组");
    let first_block = blocks
        .iter()
        .find(|block| block["oldStart"] == 1)
        .expect("应存在第一处删除 block");
    let subjects = related_subjects(first_block);

    assert!(
        subjects.contains(&"fix: delete first dup"),
        "第一处删除 block 应包含实际删除提交，实际为 {subjects:?}"
    );
    assert!(
        !subjects.contains(&"fix: delete second dup"),
        "同文件其它位置的相同删除文本不应污染当前 block，实际为 {subjects:?}"
    );
}

#[test]
fn deletion_trace_does_not_use_reused_old_path_after_rename() {
    let fixture = fixtures::rename_delete_then_reused_old_path_delete();
    let value = run_file_overlay(&fixture, "src/new.txt");
    let block = &value["overlay"]["blocks"].as_array().expect("blocks 数组")[0];
    let subjects = related_subjects(block);

    assert_eq!(block["changeType"], "deleted");
    assert!(
        subjects.contains(&"fix: rename and delete tracked line"),
        "删除追踪应保留当前文件路径上的真实删除提交，实际为 {subjects:?}"
    );
    let related_commit = related_commit_with_subject(block, "fix: rename and delete tracked line");
    assert_eq!(related_commit["attribution"]["method"], "deletion-trace");
    assert!(
        !subjects.contains(&"chore: delete reused old path"),
        "rename 后复用旧路径的删除不应污染当前文件，实际为 {subjects:?}"
    );
}

#[test]
fn deletion_trace_follows_multi_hop_rename_middle_path() {
    let fixture = fixtures::multi_hop_rename_delete();
    let value = run_file_overlay(&fixture, "src/c.txt");
    let block = &value["overlay"]["blocks"].as_array().expect("blocks 数组")[0];
    let related_commit = related_commit_with_subject(block, "fix: delete line on b");

    assert_eq!(value["overlay"]["file"]["oldPath"], "src/a.txt");
    assert_eq!(block["changeType"], "deleted");
    assert_eq!(block["attribution"]["confidence"], "precise");
    assert_eq!(related_commit["attribution"]["method"], "deletion-trace");
}

#[test]
fn rename_overlay_uses_old_path_and_does_not_drop_blocks() {
    let fixture = fixtures::rename_merge();
    let value = run_file_overlay(&fixture, "src/new.txt");
    let blocks = value["overlay"]["blocks"].as_array().expect("blocks 数组");

    assert_eq!(value["overlay"]["file"]["oldPath"], "src/old.txt");
    assert!(!blocks.is_empty(), "rename overlay 不应丢失 blocks");
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

fn related_subjects(block: &Value) -> Vec<&str> {
    block["relatedCommits"]
        .as_array()
        .expect("relatedCommits 数组")
        .iter()
        .map(|commit| commit["subject"].as_str().expect("提交主题"))
        .collect()
}

fn related_commit_with_subject<'a>(block: &'a Value, subject: &str) -> &'a Value {
    block["relatedCommits"]
        .as_array()
        .expect("relatedCommits 数组")
        .iter()
        .find(|commit| commit["subject"] == subject)
        .unwrap_or_else(|| {
            panic!(
                "relatedCommits 应包含 {subject}，实际为 {:?}",
                block["relatedCommits"]
            )
        })
}
