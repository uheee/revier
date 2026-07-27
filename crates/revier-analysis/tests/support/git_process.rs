use std::path::Path;
use std::process::Command;

pub(crate) fn run_git(repo: &Path, args: &[&str]) {
    let status = git_command(repo)
        .args(args)
        .status()
        .expect("执行 git 命令");
    assert!(status.success(), "git {:?} 执行失败", args);
}

pub(crate) fn run_git_status(repo: &Path, args: &[&str]) -> bool {
    git_command(repo)
        .args(args)
        .status()
        .expect("执行 git 命令")
        .success()
}

pub(crate) fn run_git_output(repo: &Path, args: &[&str]) -> String {
    let output = git_command(repo)
        .args(args)
        .output()
        .expect("执行 git 命令");
    assert!(
        output.status.success(),
        "git {:?} 执行失败：{}",
        args,
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("git 输出应为 UTF-8")
        .trim()
        .to_string()
}

fn git_command(repo: &Path) -> Command {
    let mut command = Command::new("git");
    command
        .current_dir(repo)
        .env("GIT_AUTHOR_NAME", "Fixture Author")
        .env("GIT_AUTHOR_EMAIL", "fixture@example.com")
        .env("GIT_COMMITTER_NAME", "Fixture Author")
        .env("GIT_COMMITTER_EMAIL", "fixture@example.com");
    command
}
