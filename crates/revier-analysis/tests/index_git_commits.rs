mod fixtures;

#[test]
fn builds_stable_repo_identity_from_root_and_common_dir() {
    let fixture = fixtures::linear();
    let repo =
        revier_analysis::git::repository::open_repository(fixture.repo.path()).expect("打开仓库");

    let identity =
        revier_analysis::git::repository::repository_identity(&repo).expect("读取仓库标识");

    assert_eq!(identity.repo_id.len(), 64);
    assert!(identity
        .repo_root
        .ends_with(fixture.repo.path().file_name().unwrap().to_str().unwrap()));
    assert!(identity.git_common_dir.ends_with(".git"));
}

#[test]
fn lists_branch_commits_with_author_key_and_parent_hashes() {
    let fixture = fixtures::linear_with_authors();
    let repo =
        revier_analysis::git::repository::open_repository(fixture.repo.path()).expect("打开仓库");

    let commits =
        revier_analysis::git::commits::list_reachable_commits(&repo, "main").expect("读取提交列表");

    assert_eq!(commits.len(), 3);
    assert_eq!(commits[0].hash, fixture.head);
    assert_eq!(commits[0].author_key, "bob@example.com");
    assert_eq!(commits[0].parents.len(), 1);
    assert_eq!(commits[1].author_key, "alice@example.com");
}
