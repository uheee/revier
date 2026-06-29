mod fixtures;

#[test]
fn linear_fixture_has_base_and_head_commits() {
    let fixture = fixtures::linear();

    assert!(fixture.repo.path().join(".git").exists());
    assert_ne!(fixture.base, fixture.head);
    assert_eq!(fixture.name, "linear");
}

#[test]
fn gix_reads_commit_parents_without_git_process() {
    let fixture = fixtures::linear();
    let repo =
        revier_analysis::git::repository::open_repository(fixture.repo.path()).expect("open repo");
    let parents =
        revier_analysis::git::repository::parent_ids(&repo, &fixture.head).expect("read parents");

    assert_eq!(parents, vec![fixture.base]);
}

#[test]
fn gix_reads_pairwise_merge_diff_for_each_parent() {
    let fixture = fixtures::merge_conflict();
    let repo =
        revier_analysis::git::repository::open_repository(fixture.repo.path()).expect("open repo");
    let changes = revier_analysis::git::diff::pairwise_parent_changes(&repo, &fixture.head)
        .expect("pairwise changes");

    assert_eq!(changes.parent_count, 2);
    assert!(
        changes.paths.iter().any(|path| path == "src/app.txt"),
        "expected pairwise diff to include src/app.txt, got {:?}",
        changes.paths
    );
    assert!(
        !changes.paths.iter().any(|path| path == "src/absent.txt"),
        "pairwise diff should not contain unrelated paths"
    );
}

#[test]
fn gix_detects_rename_in_tree_diff() {
    let fixture = fixtures::rename_merge();
    let repo =
        revier_analysis::git::repository::open_repository(fixture.repo.path()).expect("open repo");
    let renames = revier_analysis::git::diff::rename_changes(&repo, &fixture.base, &fixture.head)
        .expect("rename changes");

    assert!(
        renames
            .iter()
            .any(|rename| rename.old_path == "src/old.txt" && rename.new_path == "src/new.txt"),
        "expected src/old.txt -> src/new.txt rename, got {:?}",
        renames
    );
}

#[test]
fn gix_blame_returns_source_commit_for_added_line() {
    let fixture = fixtures::linear();
    let repo =
        revier_analysis::git::repository::open_repository(fixture.repo.path()).expect("open repo");
    let lines = revier_analysis::git::blame::blame_range(&repo, &fixture.head, "src/app.txt", 2, 2)
        .expect("blame range");

    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].line_number, 2);
    assert_eq!(lines[0].commit_hash, fixture.head);

    let first_line =
        revier_analysis::git::blame::blame_range(&repo, &fixture.head, "src/app.txt", 1, 1)
            .expect("blame first line");
    assert_eq!(first_line[0].commit_hash, fixture.base);
}
