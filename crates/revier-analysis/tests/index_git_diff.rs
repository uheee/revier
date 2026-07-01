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
    assert_eq!(binary.status, "binary");
    assert!(binary.is_binary);
    assert!(!binary.is_previewable);
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
