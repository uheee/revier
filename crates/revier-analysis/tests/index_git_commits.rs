mod fixtures;

use std::fs;
use std::path::Path;
use std::process::Command;

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

#[test]
fn range_commit_hashes_returns_commits_from_base_to_head() {
    let fixture = fixtures::linear_with_authors();
    let repo =
        revier_analysis::git::repository::open_repository(fixture.repo.path()).expect("打开仓库");
    let reachable =
        revier_analysis::git::commits::list_reachable_commits(&repo, "main").expect("读取提交列表");

    let hashes =
        revier_analysis::git::commits::range_commit_hashes(&repo, &fixture.base, &fixture.head)
            .expect("读取范围提交");

    assert_eq!(
        hashes,
        vec![reachable[1].hash.clone(), fixture.head],
        "范围提交应按 base 到 head 的时间方向返回，且不包含 base"
    );
}

#[test]
fn range_commit_hashes_uses_base_dotdot_head_for_diverged_commits() {
    let fixture = diverged();
    let repo =
        revier_analysis::git::repository::open_repository(fixture.repo.path()).expect("打开仓库");

    let hashes =
        revier_analysis::git::commits::range_commit_hashes(&repo, &fixture.base, &fixture.head)
            .expect("读取分叉范围提交");

    assert_eq!(hashes, vec![fixture.head]);
}

struct DivergedFixture {
    repo: tempfile::TempDir,
    base: String,
    head: String,
}

fn diverged() -> DivergedFixture {
    let repo = tempfile::Builder::new()
        .prefix("revier-diverged-")
        .tempdir()
        .expect("创建临时仓库");

    git(repo.path(), ["init", "-b", "main"]);
    git(repo.path(), ["config", "user.name", "Fixture Author"]);
    git(repo.path(), ["config", "user.email", "fixture@example.com"]);

    write_file(repo.path(), "src/app.txt", "root\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: root"]);
    let root = rev_parse(repo.path(), "HEAD");

    write_file(repo.path(), "src/app.txt", "root\nmain\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: main side"]);
    let base = rev_parse(repo.path(), "HEAD");

    git(repo.path(), ["checkout", "-b", "feature", &root]);
    write_file(repo.path(), "src/app.txt", "root\nfeature\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: feature side"]);
    let head = rev_parse(repo.path(), "HEAD");

    DivergedFixture { repo, base, head }
}

fn write_file(repo: &Path, path: &str, content: &str) {
    let full_path = repo.join(path);
    if let Some(parent) = full_path.parent() {
        fs::create_dir_all(parent).expect("创建父目录");
    }
    fs::write(full_path, content).expect("写入文件");
}

fn rev_parse(repo: &Path, rev: &str) -> String {
    let output = Command::new("git")
        .current_dir(repo)
        .args(["rev-parse", rev])
        .output()
        .expect("运行 git rev-parse");

    assert!(
        output.status.success(),
        "git rev-parse 失败: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("提交哈希应为 UTF-8")
        .trim()
        .to_string()
}

fn git<const N: usize>(repo: &Path, args: [&str; N]) {
    let output = Command::new("git")
        .current_dir(repo)
        .args(args)
        .output()
        .expect("运行 git 命令");

    assert!(
        output.status.success(),
        "git 命令失败: {}\nstdout: {}\nstderr: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
