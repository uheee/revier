use revier_analysis::api::{list_branches, validate_repository};

#[test]
fn validates_non_git_directory_without_recoverable() {
    let dir = tempfile::tempdir().expect("创建临时目录失败");
    let result = validate_repository(dir.path()).expect("仓库校验命令失败");

    assert!(!result.valid);
    assert!(result.error.is_some());
}

#[test]
fn lists_branches_for_fixture_repo() {
    let repo = tempfile::tempdir().expect("创建临时目录失败");
    gix::create::into(
        repo.path(),
        gix::create::Kind::WithWorktree,
        gix::create::Options::default(),
    )
    .expect("初始化测试仓库失败");

    let branches = list_branches(repo.path()).expect("读取分支失败");
    assert!(branches.iter().any(|branch| branch.current));
}
