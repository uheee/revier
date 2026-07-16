mod fixtures;

#[test]
fn reads_pairwise_file_changes_with_status_and_paths() {
    let fixture = fixtures::rename_merge();
    let repo =
        revier_analysis::git::repository::open_repository(fixture.repo.path()).expect("打开仓库");

    let changes = revier_analysis::git::diff::commit_file_changes(&repo, &fixture.head)
        .expect("读取提交文件变更");

    assert!(changes.iter().any(|change| {
        change.path == "src/new.txt"
            && change.old_path.as_deref() == Some("src/old.txt")
            && change.status == "renamed"
    }));
}

#[test]
fn counts_final_line_changes_between_range_endpoints() {
    let fixture = fixtures::linear();
    let repo =
        revier_analysis::git::repository::open_repository(fixture.repo.path()).expect("打开仓库");

    let changes =
        revier_analysis::git::diff::range_file_changes(&repo, &fixture.base, &fixture.head)
            .expect("读取范围文件变更");

    let file = changes
        .iter()
        .find(|change| change.path == "src/app.txt")
        .expect("存在文本文件变更");
    assert_eq!(file.additions, 1);
    assert_eq!(file.deletions, 0);
}

#[test]
fn counts_final_deleted_lines_between_range_endpoints() {
    let fixture = fixtures::linear_deletion();
    let repo =
        revier_analysis::git::repository::open_repository(fixture.repo.path()).expect("打开仓库");

    let changes =
        revier_analysis::git::diff::range_file_changes(&repo, &fixture.base, &fixture.head)
            .expect("读取范围文件变更");

    let file = changes
        .iter()
        .find(|change| change.path == "src/app.txt")
        .expect("存在文本文件变更");
    assert_eq!(file.additions, 0);
    assert_eq!(file.deletions, 1);
}

#[test]
fn counts_added_and_deleted_text_files_between_range_endpoints() {
    let fixture = fixtures::added_and_deleted_text_files();
    let repo =
        revier_analysis::git::repository::open_repository(fixture.repo.path()).expect("打开仓库");

    let changes =
        revier_analysis::git::diff::range_file_changes(&repo, &fixture.base, &fixture.head)
            .expect("读取范围文件变更");

    let added = changes
        .iter()
        .find(|change| change.path == "src/added.txt")
        .expect("存在新增文本文件");
    assert_eq!(added.status, "added");
    assert_eq!(added.additions, 3);
    assert_eq!(added.deletions, 0);

    let deleted = changes
        .iter()
        .find(|change| change.path == "src/deleted.txt")
        .expect("存在删除文本文件");
    assert_eq!(deleted.status, "deleted");
    assert_eq!(deleted.additions, 0);
    assert_eq!(deleted.deletions, 2);
}

#[test]
fn counts_renamed_file_content_changes_between_range_endpoints() {
    let fixture = fixtures::rename_merge();
    let repo =
        revier_analysis::git::repository::open_repository(fixture.repo.path()).expect("打开仓库");

    let changes =
        revier_analysis::git::diff::range_file_changes(&repo, &fixture.base, &fixture.head)
            .expect("读取范围文件变更");

    let file = changes
        .iter()
        .find(|change| change.path == "src/new.txt")
        .expect("存在重命名文件变更");
    assert_eq!(file.old_path.as_deref(), Some("src/old.txt"));
    assert_eq!(file.additions, 1);
    assert_eq!(file.deletions, 0);
}

#[test]
fn keeps_pure_rename_line_counts_at_zero() {
    let fixture = fixtures::pure_rename();
    let repo =
        revier_analysis::git::repository::open_repository(fixture.repo.path()).expect("打开仓库");

    let changes =
        revier_analysis::git::diff::range_file_changes(&repo, &fixture.base, &fixture.head)
            .expect("读取范围文件变更");

    let file = changes
        .iter()
        .find(|change| change.path == "src/new.txt")
        .expect("存在纯重命名文件");
    assert_eq!(file.status, "renamed");
    assert_eq!(file.old_path.as_deref(), Some("src/old.txt"));
    assert_eq!(file.additions, 0);
    assert_eq!(file.deletions, 0);
}

#[test]
fn marks_binary_files_as_not_previewable() {
    let fixture = fixtures::binary_change();
    let repo =
        revier_analysis::git::repository::open_repository(fixture.repo.path()).expect("打开仓库");

    let changes = revier_analysis::git::diff::commit_file_changes(&repo, &fixture.head)
        .expect("读取提交文件变更");

    let binary = changes
        .iter()
        .find(|change| change.path == "assets/logo.bin")
        .expect("存在二进制文件");
    assert_eq!(binary.status, "added");
    assert!(binary.is_binary);
    assert!(!binary.is_previewable);

    let range_changes =
        revier_analysis::git::diff::range_file_changes(&repo, &fixture.base, &fixture.head)
            .expect("读取范围文件变更");
    let range_binary = range_changes
        .iter()
        .find(|change| change.path == "assets/logo.bin")
        .expect("存在范围二进制文件变更");
    assert_eq!(range_binary.additions, 0);
    assert_eq!(range_binary.deletions, 0);
}

#[test]
fn omits_tree_entries_from_file_changes() {
    let fixture = fixtures::binary_change();
    let repo =
        revier_analysis::git::repository::open_repository(fixture.repo.path()).expect("打开仓库");

    let changes = revier_analysis::git::diff::commit_file_changes(&repo, &fixture.head)
        .expect("读取提交文件变更");

    assert!(changes
        .iter()
        .any(|change| change.path == "assets/logo.bin"));
    assert!(!changes.iter().any(|change| change.path == "assets"));
}
