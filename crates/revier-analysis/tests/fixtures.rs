use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

pub struct FixtureRepo {
    pub name: &'static str,
    pub repo: TempDir,
    pub base: String,
    pub head: String,
}

pub fn linear() -> FixtureRepo {
    let repo = init_repo("linear");
    write_file(repo.path(), "src/app.txt", "one\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: initial"]);
    let base = rev_parse(repo.path(), "HEAD");

    write_file(repo.path(), "src/app.txt", "one\ntwo\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: add second line"]);
    let head = rev_parse(repo.path(), "HEAD");

    FixtureRepo {
        name: "linear",
        repo,
        base,
        head,
    }
}

pub fn merge_conflict() -> FixtureRepo {
    let repo = init_repo("merge-conflict");
    write_file(repo.path(), "src/app.txt", "base\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: base"]);
    let base = rev_parse(repo.path(), "HEAD");

    git(repo.path(), ["checkout", "-b", "feature"]);
    write_file(repo.path(), "src/app.txt", "base\nfeature\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: feature line"]);

    git(repo.path(), ["checkout", "main"]);
    write_file(repo.path(), "src/app.txt", "base\nmain\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: main line"]);

    git_expect_failure(repo.path(), ["merge", "feature"]);
    write_file(repo.path(), "src/app.txt", "base\nmain\nfeature\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "merge: resolve conflict"]);
    let head = rev_parse(repo.path(), "HEAD");

    FixtureRepo {
        name: "merge-conflict",
        repo,
        base,
        head,
    }
}

pub fn rename_merge() -> FixtureRepo {
    let repo = init_repo("rename-merge");
    write_file(repo.path(), "src/old.txt", "alpha\nbeta\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: add old file"]);
    let base = rev_parse(repo.path(), "HEAD");

    git(repo.path(), ["checkout", "-b", "feature"]);
    git(repo.path(), ["mv", "src/old.txt", "src/new.txt"]);
    write_file(repo.path(), "src/new.txt", "alpha\nbeta\nfeature\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: rename and edit"]);

    git(repo.path(), ["checkout", "main"]);
    git(
        repo.path(),
        ["merge", "--no-ff", "feature", "-m", "merge: feature rename"],
    );
    let head = rev_parse(repo.path(), "HEAD");

    FixtureRepo {
        name: "rename-merge",
        repo,
        base,
        head,
    }
}

fn init_repo(name: &'static str) -> TempDir {
    let repo = tempfile::Builder::new()
        .prefix(&format!("revier-{name}-"))
        .tempdir()
        .expect("create temp repo");

    git(repo.path(), ["init", "-b", "main"]);
    git(repo.path(), ["config", "user.name", "Fixture Author"]);
    git(repo.path(), ["config", "user.email", "fixture@example.com"]);
    repo
}

fn write_file(repo: &Path, path: &str, content: &str) {
    let full_path = repo.join(path);
    if let Some(parent) = full_path.parent() {
        fs::create_dir_all(parent).expect("create parent directory");
    }
    fs::write(full_path, content).expect("write fixture file");
}

fn rev_parse(repo: &Path, rev: &str) -> String {
    let output = Command::new("git")
        .current_dir(repo)
        .args(["rev-parse", rev])
        .output()
        .expect("run git rev-parse");

    assert!(
        output.status.success(),
        "git rev-parse failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("utf8 hash")
        .trim()
        .to_string()
}

fn git<const N: usize>(repo: &Path, args: [&str; N]) {
    let output = Command::new("git")
        .current_dir(repo)
        .args(args)
        .output()
        .expect("run git command");

    assert!(
        output.status.success(),
        "git command failed: {}\nstdout: {}\nstderr: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn git_expect_failure<const N: usize>(repo: &Path, args: [&str; N]) {
    let output = Command::new("git")
        .current_dir(repo)
        .args(args)
        .output()
        .expect("run git command");

    assert!(
        !output.status.success(),
        "git command unexpectedly succeeded: {}\nstdout: {}\nstderr: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
