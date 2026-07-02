#![allow(dead_code)]

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

pub fn linear_with_authors() -> FixtureRepo {
    let repo = init_repo("linear-authors");
    write_file(repo.path(), "src/app.txt", "base\n");
    git_with_author(repo.path(), ["add", "."], "Base", "base@example.com", None);
    git_with_author(
        repo.path(),
        ["commit", "-m", "feat: base"],
        "Base",
        "base@example.com",
        Some("2026-05-01T00:00:00Z"),
    );
    let base = rev_parse(repo.path(), "HEAD");

    write_file(repo.path(), "src/app.txt", "base\nalice\n");
    git_with_author(
        repo.path(),
        ["add", "."],
        "Alice",
        "alice@example.com",
        None,
    );
    git_with_author(
        repo.path(),
        ["commit", "-m", "feat: alice change"],
        "Alice",
        "alice@example.com",
        Some("2026-05-02T00:00:00Z"),
    );

    write_file(repo.path(), "src/app.txt", "base\nalice\nbob\n");
    git_with_author(repo.path(), ["add", "."], "Bob", "bob@example.com", None);
    git_with_author(
        repo.path(),
        ["commit", "-m", "fix: bob change"],
        "Bob",
        "bob@example.com",
        Some("2026-05-03T00:00:00Z"),
    );
    let head = rev_parse(repo.path(), "HEAD");

    FixtureRepo {
        name: "linear-authors",
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

pub fn rename_then_unrelated() -> FixtureRepo {
    let repo = init_repo("rename-unrelated");
    write_file(repo.path(), "src/old.txt", "alpha\nbeta\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: add old file"]);
    let base = rev_parse(repo.path(), "HEAD");

    git(repo.path(), ["mv", "src/old.txt", "src/new.txt"]);
    write_file(repo.path(), "src/new.txt", "alpha\nbeta\nfeature\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: rename and edit"]);

    write_file(repo.path(), "docs/notes.txt", "unrelated\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "chore: unrelated docs"]);
    let head = rev_parse(repo.path(), "HEAD");

    FixtureRepo {
        name: "rename-unrelated",
        repo,
        base,
        head,
    }
}

pub fn rename_then_reused_old_path() -> FixtureRepo {
    let repo = init_repo("rename-reused-old-path");
    write_file(repo.path(), "src/old.txt", "alpha\nbeta\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: add old file"]);
    let base = rev_parse(repo.path(), "HEAD");

    git(repo.path(), ["mv", "src/old.txt", "src/new.txt"]);
    write_file(repo.path(), "src/new.txt", "alpha\nbeta\nfeature\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: rename and edit"]);

    write_file(repo.path(), "src/old.txt", "one\ntwo\nunrelated\n");
    git(repo.path(), ["add", "."]);
    git(
        repo.path(),
        ["commit", "-m", "chore: edit recreated old path same line"],
    );

    fs::remove_file(repo.path().join("src/old.txt")).expect("删除复用旧路径文件");
    git(repo.path(), ["add", "."]);
    git(
        repo.path(),
        ["commit", "-m", "chore: remove recreated old path"],
    );
    let head = rev_parse(repo.path(), "HEAD");

    FixtureRepo {
        name: "rename-reused-old-path",
        repo,
        base,
        head,
    }
}

pub fn binary_change() -> FixtureRepo {
    let repo = init_repo("binary-change");
    write_file(repo.path(), "README.md", "base\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: base"]);
    let base = rev_parse(repo.path(), "HEAD");

    let full_path = repo.path().join("assets/logo.bin");
    fs::create_dir_all(full_path.parent().expect("binary parent")).expect("创建 binary 目录");
    fs::write(&full_path, [0_u8, 159, 146, 150, 0, 1, 2, 3]).expect("写入 binary");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: add binary"]);
    let head = rev_parse(repo.path(), "HEAD");

    FixtureRepo {
        name: "binary-change",
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

fn git_with_author<const N: usize>(
    repo: &Path,
    args: [&str; N],
    name: &str,
    email: &str,
    date: Option<&str>,
) {
    let mut command = Command::new("git");
    command
        .current_dir(repo)
        .env("GIT_AUTHOR_NAME", name)
        .env("GIT_AUTHOR_EMAIL", email)
        .env("GIT_COMMITTER_NAME", name)
        .env("GIT_COMMITTER_EMAIL", email);
    if let Some(date) = date {
        command
            .env("GIT_AUTHOR_DATE", date)
            .env("GIT_COMMITTER_DATE", date);
    }
    let output = command.args(args).output().expect("运行 git 命令");
    assert!(
        output.status.success(),
        "git 命令失败: {}\nstdout: {}\nstderr: {}",
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
