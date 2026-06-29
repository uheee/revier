# gix Capability Spike Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在当前 Revier 仓库内新增 Rust Cargo workspace 和 `crates/revier-analysis` spike crate，验证 gix 是否能覆盖 blame、rename、pairwise merge diff 和多父 merge 链路。

**Architecture:** Rust spike 以独立 CLI 形式存在于本仓库内，不接 Electron UI，不写 DuckDB，不替换现有 TypeScript 分析逻辑。测试使用临时 Git 仓库构造 fixture 和生成对照数据，但 `src/` 下的 gix 读取路径不得调用 `git` 进程。

**Tech Stack:** Rust stable、Cargo workspace、clap、serde、serde_json、thiserror、gix、gix-diff、gix-blame、tempfile、insta。

---

## Scope

本计划只实现 capability spike，不实现正式 Rust CLI、DuckDB schema、Electron 接入和 libgit2 fallback。spike 的产物是：

- 当前仓库根目录新增 Cargo workspace。
- 新增 `crates/revier-analysis` Rust crate。
- 新增 spike CLI 命令，输出稳定 JSON。
- 新增 Rust 测试，构造 Git fixture 并验证 gix 能力。
- 新增 pnpm 脚本用于运行 Rust 测试。

## File Structure

- Create: `Cargo.toml`
  - Rust workspace 根配置。
- Modify: `.gitignore`
  - 忽略 Rust `target/` 目录。
- Modify: `package.json`
  - 增加 `rust:test` 和 `rust:spike` 脚本。
- Create: `crates/revier-analysis/Cargo.toml`
  - Rust spike crate 依赖配置。
- Create: `crates/revier-analysis/src/main.rs`
  - CLI 入口。
- Create: `crates/revier-analysis/src/lib.rs`
  - crate 模块导出。
- Create: `crates/revier-analysis/src/cli.rs`
  - clap 参数定义。
- Create: `crates/revier-analysis/src/error.rs`
  - spike 错误类型和退出码映射。
- Create: `crates/revier-analysis/src/json.rs`
  - spike JSON 输出结构。
- Create: `crates/revier-analysis/src/git/mod.rs`
  - gix 读取模块导出。
- Create: `crates/revier-analysis/src/git/repository.rs`
  - 仓库打开、commit 解析和 parent 读取。
- Create: `crates/revier-analysis/src/git/diff.rs`
  - pairwise tree diff 与 rename probe。
- Create: `crates/revier-analysis/src/git/blame.rs`
  - blame range probe。
- Create: `crates/revier-analysis/src/spike.rs`
  - spike 编排逻辑。
- Create: `crates/revier-analysis/tests/fixtures.rs`
  - 临时 Git fixture 构造工具。
- Create: `crates/revier-analysis/tests/cli_contract.rs`
  - CLI 和 JSON 契约测试。
- Create: `crates/revier-analysis/tests/gix_capabilities.rs`
  - gix 能力 fixture 测试。
- Create: `crates/revier-analysis/tests/no_git_process.rs`
  - 确认 `src/` 生产路径不调用 `git` 进程。

## Task 1: Cargo Workspace Scaffold

**Files:**
- Create: `Cargo.toml`
- Modify: `.gitignore`
- Create: `crates/revier-analysis/Cargo.toml`
- Create: `crates/revier-analysis/src/main.rs`
- Create: `crates/revier-analysis/src/lib.rs`
- Create: `crates/revier-analysis/src/cli.rs`
- Create: `crates/revier-analysis/src/error.rs`

- [ ] **Step 1: Write the failing CLI smoke test**

Create `crates/revier-analysis/tests/cli_contract.rs`:

```rust
use std::process::Command;

#[test]
fn help_command_exits_successfully() {
    let output = Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
        .arg("--help")
        .output()
        .expect("run revier-analysis --help");

    assert!(
        output.status.success(),
        "expected --help to succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("revier-analysis"));
    assert!(stdout.contains("spike"));
}
```

- [ ] **Step 2: Run the test and verify it fails**

Run:

```powershell
cargo test -p revier-analysis --test cli_contract
```

Expected: FAIL because the Cargo workspace and `revier-analysis` binary do not exist yet.

- [ ] **Step 3: Create the root Cargo workspace**

Create `Cargo.toml`:

```toml
[workspace]
members = ["crates/revier-analysis"]
resolver = "2"
```

- [ ] **Step 4: Ignore Rust build output**

Append to `.gitignore`:

```gitignore
target/
```

- [ ] **Step 5: Create the Rust crate manifest**

Create `crates/revier-analysis/Cargo.toml`:

```toml
[package]
name = "revier-analysis"
version = "0.1.0"
edition = "2021"
publish = false

[dependencies]
clap = { version = "4.5", features = ["derive"] }
gix = { version = "0.84", default-features = false, features = ["max-performance-safe"] }
gix-blame = "0.14"
gix-diff = { version = "0.64", features = ["blob"] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
thiserror = "2.0"

[dev-dependencies]
insta = { version = "1.42", features = ["json"] }
tempfile = "3.10"
```

- [ ] **Step 6: Add the CLI parser**

Create `crates/revier-analysis/src/cli.rs`:

```rust
use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "revier-analysis")]
#[command(about = "Revier local analysis spike CLI")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Spike(SpikeCommand),
}

#[derive(Debug, Parser)]
pub struct SpikeCommand {
    #[command(subcommand)]
    pub command: SpikeSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum SpikeSubcommand {
    Run(SpikeRunArgs),
}

#[derive(Debug, Parser)]
pub struct SpikeRunArgs {
    #[arg(long)]
    pub repo: PathBuf,

    #[arg(long)]
    pub fixture: SpikeFixture,

    #[arg(long, default_value = "json")]
    pub format: OutputFormat,

    #[arg(long)]
    pub pretty: bool,
}

#[derive(Clone, Debug, ValueEnum)]
pub enum SpikeFixture {
    Linear,
    EarlyFeatureMerge,
    MergeConflict,
    MultiParentAmbiguous,
    RenameMerge,
    DeletionMerge,
}

#[derive(Clone, Debug, ValueEnum)]
pub enum OutputFormat {
    Json,
}
```

- [ ] **Step 7: Add the error type**

Create `crates/revier-analysis/src/error.rs`:

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("仓库读取失败：{0}")]
    Repository(String),

    #[error("gix 能力验证失败：{0}")]
    Spike(String),

    #[error("JSON 输出失败：{0}")]
    Json(#[from] serde_json::Error),
}

impl AppError {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::Repository(_) => 3,
            Self::Spike(_) => 10,
            Self::Json(_) => 10,
        }
    }
}
```

- [ ] **Step 8: Add the library and binary entry points**

Create `crates/revier-analysis/src/lib.rs`:

```rust
pub mod cli;
pub mod error;

use clap::Parser;
use cli::{Cli, Command, SpikeSubcommand};
use error::AppError;

pub fn run_from<I, T>(args: I) -> Result<String, AppError>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let cli = Cli::parse_from(args);
    match cli.command {
        Command::Spike(spike) => match spike.command {
            SpikeSubcommand::Run(_args) => Ok("{\"version\":1,\"fixture\":\"linear\",\"checks\":[],\"decision\":\"algorithm-adjustment-required\"}\n".to_string()),
        },
    }
}
```

Create `crates/revier-analysis/src/main.rs`:

```rust
fn main() {
    match revier_analysis::run_from(std::env::args_os()) {
        Ok(output) => print!("{output}"),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(error.exit_code());
        }
    }
}
```

- [ ] **Step 9: Run the smoke test and verify it passes**

Run:

```powershell
cargo test -p revier-analysis --test cli_contract
```

Expected: PASS.

- [ ] **Step 10: Commit**

```powershell
git add Cargo.toml .gitignore crates/revier-analysis/Cargo.toml crates/revier-analysis/src crates/revier-analysis/tests/cli_contract.rs
git commit -m "feat: scaffold rust analysis spike"
```

## Task 2: JSON Contract

**Files:**
- Create: `crates/revier-analysis/src/json.rs`
- Modify: `crates/revier-analysis/src/lib.rs`
- Modify: `crates/revier-analysis/tests/cli_contract.rs`

- [ ] **Step 1: Write failing JSON contract tests**

Replace `crates/revier-analysis/tests/cli_contract.rs` with:

```rust
use serde_json::Value;
use std::process::Command;

#[test]
fn help_command_exits_successfully() {
    let output = Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
        .arg("--help")
        .output()
        .expect("run revier-analysis --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("revier-analysis"));
    assert!(stdout.contains("spike"));
}

#[test]
fn spike_run_outputs_stable_json_shape() {
    let output = Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
        .args([
            "spike",
            "run",
            "--repo",
            ".",
            "--fixture",
            "linear",
            "--format",
            "json",
        ])
        .output()
        .expect("run spike command");

    assert!(output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout).expect("parse spike json");
    assert_eq!(value["version"], 1);
    assert_eq!(value["fixture"], "linear");
    assert!(value["checks"].is_array());
    assert!(value["decision"].is_string());
}
```

- [ ] **Step 2: Run the test and verify it fails**

Run:

```powershell
cargo test -p revier-analysis --test cli_contract
```

Expected: FAIL because JSON structures are not defined and fixture selection is hard-coded.

- [ ] **Step 3: Implement JSON output types**

Create `crates/revier-analysis/src/json.rs`:

```rust
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SpikeDecision {
    GixOnly,
    AlgorithmAdjustmentRequired,
    FallbackRisk,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CheckStatus {
    Pass,
    Fail,
}

#[derive(Debug, Serialize)]
pub struct SpikeDifference {
    pub field: String,
    pub expected: String,
    pub actual: String,
}

#[derive(Debug, Serialize)]
pub struct SpikeCheck {
    pub name: String,
    pub status: CheckStatus,
    pub differences: Vec<SpikeDifference>,
}

#[derive(Debug, Serialize)]
pub struct SpikeOutput {
    pub version: u8,
    pub fixture: String,
    pub checks: Vec<SpikeCheck>,
    pub decision: SpikeDecision,
}

impl SpikeOutput {
    pub fn new(fixture: impl Into<String>) -> Self {
        Self {
            version: 1,
            fixture: fixture.into(),
            checks: Vec::new(),
            decision: SpikeDecision::AlgorithmAdjustmentRequired,
        }
    }
}
```

- [ ] **Step 4: Wire JSON output into `run_from`**

Replace `crates/revier-analysis/src/lib.rs` with:

```rust
pub mod cli;
pub mod error;
pub mod json;

use clap::{Parser, ValueEnum};
use cli::{Cli, Command, SpikeSubcommand};
use error::AppError;
use json::SpikeOutput;

pub fn run_from<I, T>(args: I) -> Result<String, AppError>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let cli = Cli::parse_from(args);
    match cli.command {
        Command::Spike(spike) => match spike.command {
            SpikeSubcommand::Run(args) => {
                let fixture = args.fixture.to_possible_value().expect("fixture has value").get_name().to_string();
                let output = SpikeOutput::new(fixture);
                let json = if args.pretty {
                    serde_json::to_string_pretty(&output)?
                } else {
                    serde_json::to_string(&output)?
                };
                Ok(format!("{json}\n"))
            }
        },
    }
}
```

- [ ] **Step 5: Run the JSON contract tests**

Run:

```powershell
cargo test -p revier-analysis --test cli_contract
```

Expected: PASS.

- [ ] **Step 6: Commit**

```powershell
git add crates/revier-analysis/src/json.rs crates/revier-analysis/src/lib.rs crates/revier-analysis/tests/cli_contract.rs
git commit -m "feat: define rust spike json contract"
```

## Task 3: Git Fixture Builder

**Files:**
- Create: `crates/revier-analysis/tests/fixtures.rs`
- Create: `crates/revier-analysis/tests/gix_capabilities.rs`

- [ ] **Step 1: Write the failing fixture test**

Create `crates/revier-analysis/tests/gix_capabilities.rs`:

```rust
mod fixtures;

#[test]
fn linear_fixture_has_base_and_head_commits() {
    let fixture = fixtures::linear();

    assert!(fixture.repo.path().join(".git").exists());
    assert_ne!(fixture.base, fixture.head);
    assert_eq!(fixture.name, "linear");
}
```

- [ ] **Step 2: Run the test and verify it fails**

Run:

```powershell
cargo test -p revier-analysis --test gix_capabilities
```

Expected: FAIL because `tests/fixtures.rs` does not exist.

- [ ] **Step 3: Implement fixture helpers**

Create `crates/revier-analysis/tests/fixtures.rs`:

```rust
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

    FixtureRepo { name: "linear", repo, base, head }
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
    String::from_utf8(output.stdout).expect("utf8 hash").trim().to_string()
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
```

- [ ] **Step 4: Run the fixture test**

Run:

```powershell
cargo test -p revier-analysis --test gix_capabilities
```

Expected: PASS.

- [ ] **Step 5: Commit**

```powershell
git add crates/revier-analysis/tests/fixtures.rs crates/revier-analysis/tests/gix_capabilities.rs
git commit -m "test: add git fixture builder for gix spike"
```

## Task 4: Commit Parent Probe

**Files:**
- Create: `crates/revier-analysis/src/git/mod.rs`
- Create: `crates/revier-analysis/src/git/repository.rs`
- Modify: `crates/revier-analysis/src/lib.rs`
- Modify: `crates/revier-analysis/tests/gix_capabilities.rs`

- [ ] **Step 1: Write failing parent probe test**

Append to `crates/revier-analysis/tests/gix_capabilities.rs`:

```rust
#[test]
fn gix_reads_commit_parents_without_git_process() {
    let fixture = fixtures::linear();
    let repo = revier_analysis::git::repository::open_repository(fixture.repo.path()).expect("open repo");
    let parents = revier_analysis::git::repository::parent_ids(&repo, &fixture.head).expect("read parents");

    assert_eq!(parents, vec![fixture.base]);
}
```

- [ ] **Step 2: Run the test and verify it fails**

Run:

```powershell
cargo test -p revier-analysis --test gix_capabilities gix_reads_commit_parents_without_git_process
```

Expected: FAIL because `revier_analysis::git::repository` does not exist.

- [ ] **Step 3: Implement repository helpers**

Create `crates/revier-analysis/src/git/mod.rs`:

```rust
pub mod repository;
```

Create `crates/revier-analysis/src/git/repository.rs`:

```rust
use crate::error::AppError;
use std::path::Path;

pub fn open_repository(path: &Path) -> Result<gix::Repository, AppError> {
    gix::open(path).map_err(|error| AppError::Repository(error.to_string()))
}

pub fn parent_ids(repo: &gix::Repository, commit_hash: &str) -> Result<Vec<String>, AppError> {
    let object = repo
        .rev_parse_single(commit_hash)
        .map_err(|error| AppError::Repository(error.to_string()))?;
    let commit = object
        .object()
        .map_err(|error| AppError::Repository(error.to_string()))?
        .try_into_commit()
        .map_err(|_| AppError::Repository(format!("对象不是提交：{commit_hash}")))?;

    Ok(commit.parent_ids().map(|id| id.to_string()).collect())
}
```

Modify `crates/revier-analysis/src/lib.rs` module declarations:

```rust
pub mod cli;
pub mod error;
pub mod git;
pub mod json;
```

Keep the rest of `run_from` unchanged.

- [ ] **Step 4: Run the parent probe test**

Run:

```powershell
cargo test -p revier-analysis --test gix_capabilities gix_reads_commit_parents_without_git_process
```

Expected: PASS. If the gix conversion API name differs, adjust only `src/git/repository.rs` until this test passes; do not introduce `std::process::Command` in `src/`.

- [ ] **Step 5: Commit**

```powershell
git add crates/revier-analysis/src/git crates/revier-analysis/src/lib.rs crates/revier-analysis/tests/gix_capabilities.rs
git commit -m "feat: add gix commit parent probe"
```

## Task 5: Pairwise Merge Diff Fixture and Probe

**Files:**
- Modify: `crates/revier-analysis/tests/fixtures.rs`
- Create: `crates/revier-analysis/src/git/diff.rs`
- Modify: `crates/revier-analysis/src/git/mod.rs`
- Modify: `crates/revier-analysis/tests/gix_capabilities.rs`

- [ ] **Step 1: Add failing merge fixture test**

Append to `crates/revier-analysis/tests/gix_capabilities.rs`:

```rust
#[test]
fn gix_reads_pairwise_merge_diff_for_each_parent() {
    let fixture = fixtures::merge_conflict();
    let repo = revier_analysis::git::repository::open_repository(fixture.repo.path()).expect("open repo");
    let changes = revier_analysis::git::diff::pairwise_parent_changes(&repo, &fixture.head).expect("pairwise changes");

    assert_eq!(changes.parent_count, 2);
    assert!(
        changes.paths.iter().any(|path| path == "src/app.txt"),
        "expected pairwise diff to include src/app.txt, got {:?}",
        changes.paths
    );
}
```

- [ ] **Step 2: Run the test and verify it fails**

Run:

```powershell
cargo test -p revier-analysis --test gix_capabilities gix_reads_pairwise_merge_diff_for_each_parent
```

Expected: FAIL because `fixtures::merge_conflict` and `git::diff` do not exist.

- [ ] **Step 3: Add the merge conflict fixture**

Append to `crates/revier-analysis/tests/fixtures.rs`:

```rust
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

    let output = Command::new("git")
        .current_dir(repo.path())
        .args(["merge", "feature"])
        .output()
        .expect("run conflict merge");
    assert!(!output.status.success(), "merge should conflict");

    write_file(repo.path(), "src/app.txt", "base\nmain\nfeature\nresolved\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "merge: resolve feature"]);
    let head = rev_parse(repo.path(), "HEAD");

    FixtureRepo { name: "merge-conflict", repo, base, head }
}
```

- [ ] **Step 4: Implement pairwise diff probe**

Create `crates/revier-analysis/src/git/diff.rs`:

```rust
use crate::error::AppError;
use std::collections::BTreeSet;

#[derive(Debug)]
pub struct PairwiseChanges {
    pub parent_count: usize,
    pub paths: Vec<String>,
}

pub fn pairwise_parent_changes(repo: &gix::Repository, commit_hash: &str) -> Result<PairwiseChanges, AppError> {
    let object = repo
        .rev_parse_single(commit_hash)
        .map_err(|error| AppError::Repository(error.to_string()))?;
    let commit = object
        .object()
        .map_err(|error| AppError::Repository(error.to_string()))?
        .try_into_commit()
        .map_err(|_| AppError::Repository(format!("对象不是提交：{commit_hash}")))?;

    let parent_ids: Vec<_> = commit.parent_ids().collect();
    let mut paths = BTreeSet::new();

    for parent_id in &parent_ids {
        let parent = parent_id
            .object()
            .map_err(|error| AppError::Repository(error.to_string()))?
            .try_into_commit()
            .map_err(|_| AppError::Repository(format!("父对象不是提交：{parent_id}")))?;

        collect_changed_paths(repo, &parent, &commit, &mut paths)?;
    }

    Ok(PairwiseChanges {
        parent_count: parent_ids.len(),
        paths: paths.into_iter().collect(),
    })
}

fn collect_changed_paths(
    _repo: &gix::Repository,
    _parent: &gix::Commit<'_>,
    _commit: &gix::Commit<'_>,
    paths: &mut BTreeSet<String>,
) -> Result<(), AppError> {
    paths.insert("src/app.txt".to_string());
    Ok(())
}
```

This implementation is intentionally minimal to drive the test. The next task replaces the hard-coded path with gix tree diff.

Modify `crates/revier-analysis/src/git/mod.rs`:

```rust
pub mod diff;
pub mod repository;
```

- [ ] **Step 5: Run the pairwise diff test**

Run:

```powershell
cargo test -p revier-analysis --test gix_capabilities gix_reads_pairwise_merge_diff_for_each_parent
```

Expected: PASS.

- [ ] **Step 6: Commit**

```powershell
git add crates/revier-analysis/src/git/diff.rs crates/revier-analysis/src/git/mod.rs crates/revier-analysis/tests/fixtures.rs crates/revier-analysis/tests/gix_capabilities.rs
git commit -m "feat: add pairwise merge diff spike fixture"
```

## Task 6: Replace Pairwise Diff Stub With gix Tree Diff

**Files:**
- Modify: `crates/revier-analysis/src/git/diff.rs`
- Modify: `crates/revier-analysis/tests/gix_capabilities.rs`

- [ ] **Step 1: Add a failing negative assertion**

Append to `gix_reads_pairwise_merge_diff_for_each_parent` in `crates/revier-analysis/tests/gix_capabilities.rs`:

```rust
    assert!(
        !changes.paths.iter().any(|path| path == "src/absent.txt"),
        "pairwise diff should not contain unrelated paths"
    );
```

- [ ] **Step 2: Run the test**

Run:

```powershell
cargo test -p revier-analysis --test gix_capabilities gix_reads_pairwise_merge_diff_for_each_parent
```

Expected: PASS with the current stub. This step documents the current weakness before replacing the implementation.

- [ ] **Step 3: Replace the stub with actual gix tree diff**

Replace `collect_changed_paths` in `crates/revier-analysis/src/git/diff.rs` with a gix-backed implementation:

```rust
fn collect_changed_paths(
    repo: &gix::Repository,
    parent: &gix::Commit<'_>,
    commit: &gix::Commit<'_>,
    paths: &mut BTreeSet<String>,
) -> Result<(), AppError> {
    let parent_tree = parent
        .tree()
        .map_err(|error| AppError::Repository(error.to_string()))?;
    let commit_tree = commit
        .tree()
        .map_err(|error| AppError::Repository(error.to_string()))?;

    let parent_data = parent_tree
        .detach()
        .data
        .ok_or_else(|| AppError::Repository("父 tree 数据为空".to_string()))?;
    let commit_data = commit_tree
        .detach()
        .data
        .ok_or_else(|| AppError::Repository("目标 tree 数据为空".to_string()))?;

    let parent_iter = gix_object::TreeRefIter::from_bytes(&parent_data);
    let commit_iter = gix_object::TreeRefIter::from_bytes(&commit_data);
    let mut state = gix_diff::tree::State::default();
    let objects = repo.objects.clone();
    let mut delegate = PathCollectingDelegate { paths };

    gix_diff::tree(parent_iter, commit_iter, &mut state, objects, &mut delegate)
        .map_err(|error| AppError::Repository(error.to_string()))?;

    Ok(())
}

struct PathCollectingDelegate<'a> {
    paths: &'a mut BTreeSet<String>,
}

impl gix_diff::tree::Visit for PathCollectingDelegate<'_> {
    fn pop_front_tracked_path_and_set_current(&mut self) {}

    fn push_back_tracked_path_component(&mut self, _component: &gix_diff::tree::EntryRef<'_>) {}

    fn push_path_component(&mut self, _component: &gix_diff::tree::EntryRef<'_>) {}

    fn pop_path_component(&mut self) {}

    fn visit(
        &mut self,
        change: gix_diff::tree::ChangeRef<'_>,
    ) -> std::ops::ControlFlow<gix_diff::tree::visit::Action> {
        if let Some(location) = change.location() {
            self.paths.insert(location.to_string());
        }
        std::ops::ControlFlow::Continue(())
    }
}
```

If the exact gix delegate trait differs from the installed version, keep the public function signature and test unchanged, then adapt only the delegate implementation to the installed gix API. Do not use `std::process::Command` in `src/`.

- [ ] **Step 4: Add missing dependency if needed**

If the implementation imports `gix_object`, add it to `crates/revier-analysis/Cargo.toml`:

```toml
gix-object = "0.61"
```

- [ ] **Step 5: Run the pairwise diff test**

Run:

```powershell
cargo test -p revier-analysis --test gix_capabilities gix_reads_pairwise_merge_diff_for_each_parent
```

Expected: PASS.

- [ ] **Step 6: Commit**

```powershell
git add crates/revier-analysis/Cargo.toml crates/revier-analysis/src/git/diff.rs crates/revier-analysis/tests/gix_capabilities.rs
git commit -m "feat: validate pairwise merge diff with gix"
```

## Task 7: Rename Tracking Probe

**Files:**
- Modify: `crates/revier-analysis/tests/fixtures.rs`
- Modify: `crates/revier-analysis/src/git/diff.rs`
- Modify: `crates/revier-analysis/tests/gix_capabilities.rs`

- [ ] **Step 1: Write failing rename tracking test**

Append to `crates/revier-analysis/tests/gix_capabilities.rs`:

```rust
#[test]
fn gix_detects_rename_in_tree_diff() {
    let fixture = fixtures::rename_merge();
    let repo = revier_analysis::git::repository::open_repository(fixture.repo.path()).expect("open repo");
    let renames = revier_analysis::git::diff::rename_changes(&repo, &fixture.base, &fixture.head).expect("rename changes");

    assert!(
        renames.iter().any(|rename| rename.old_path == "src/old.txt" && rename.new_path == "src/new.txt"),
        "expected src/old.txt -> src/new.txt rename, got {:?}",
        renames
    );
}
```

- [ ] **Step 2: Run the test and verify it fails**

Run:

```powershell
cargo test -p revier-analysis --test gix_capabilities gix_detects_rename_in_tree_diff
```

Expected: FAIL because `rename_merge` fixture and `rename_changes` do not exist.

- [ ] **Step 3: Add rename merge fixture**

Append to `crates/revier-analysis/tests/fixtures.rs`:

```rust
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
    git(repo.path(), ["merge", "--no-ff", "feature", "-m", "merge: feature rename"]);
    let head = rev_parse(repo.path(), "HEAD");

    FixtureRepo { name: "rename-merge", repo, base, head }
}
```

- [ ] **Step 4: Implement rename result type and function**

Append to `crates/revier-analysis/src/git/diff.rs`:

```rust
#[derive(Debug)]
pub struct RenameChange {
    pub old_path: String,
    pub new_path: String,
    pub similarity: Option<f32>,
}

pub fn rename_changes(
    repo: &gix::Repository,
    base_commit: &str,
    head_commit: &str,
) -> Result<Vec<RenameChange>, AppError> {
    let _base = repo
        .rev_parse_single(base_commit)
        .map_err(|error| AppError::Repository(error.to_string()))?;
    let _head = repo
        .rev_parse_single(head_commit)
        .map_err(|error| AppError::Repository(error.to_string()))?;

    Ok(vec![RenameChange {
        old_path: "src/old.txt".to_string(),
        new_path: "src/new.txt".to_string(),
        similarity: Some(0.5),
    }])
}
```

This stub is intentionally narrow. The next step replaces it with `gix_diff::tree_with_rewrites`.

- [ ] **Step 5: Run the rename test**

Run:

```powershell
cargo test -p revier-analysis --test gix_capabilities gix_detects_rename_in_tree_diff
```

Expected: PASS.

- [ ] **Step 6: Replace the rename stub with gix rewrite tracking**

Replace `rename_changes` with an implementation using `gix_diff::tree_with_rewrites` and `gix_diff::Rewrites::default()`. Preserve this public signature:

```rust
pub fn rename_changes(
    repo: &gix::Repository,
    base_commit: &str,
    head_commit: &str,
) -> Result<Vec<RenameChange>, AppError>
```

Implementation requirements:

- Open both commits with gix.
- Read both trees.
- Run tree diff with rewrite tracking enabled.
- For each rewrite/rename change, push `RenameChange { old_path, new_path, similarity }`.
- Keep copy tracking disabled unless the gix API requires an explicit value.
- Do not call `git` from `src/`.

- [ ] **Step 7: Run the rename test again**

Run:

```powershell
cargo test -p revier-analysis --test gix_capabilities gix_detects_rename_in_tree_diff
```

Expected: PASS.

- [ ] **Step 8: Commit**

```powershell
git add crates/revier-analysis/src/git/diff.rs crates/revier-analysis/tests/fixtures.rs crates/revier-analysis/tests/gix_capabilities.rs
git commit -m "feat: validate gix rename tracking"
```

## Task 8: Blame Probe

**Files:**
- Create: `crates/revier-analysis/src/git/blame.rs`
- Modify: `crates/revier-analysis/src/git/mod.rs`
- Modify: `crates/revier-analysis/tests/gix_capabilities.rs`

- [ ] **Step 1: Write failing blame test**

Append to `crates/revier-analysis/tests/gix_capabilities.rs`:

```rust
#[test]
fn gix_blame_returns_source_commit_for_added_line() {
    let fixture = fixtures::linear();
    let repo = revier_analysis::git::repository::open_repository(fixture.repo.path()).expect("open repo");
    let lines = revier_analysis::git::blame::blame_range(&repo, &fixture.head, "src/app.txt", 2, 2)
        .expect("blame range");

    assert_eq!(lines.len(), 1);
    assert_eq!(lines[0].line_number, 2);
    assert_eq!(lines[0].commit_hash, fixture.head);
}
```

- [ ] **Step 2: Run the test and verify it fails**

Run:

```powershell
cargo test -p revier-analysis --test gix_capabilities gix_blame_returns_source_commit_for_added_line
```

Expected: FAIL because `git::blame` does not exist.

- [ ] **Step 3: Implement blame result type and function**

Create `crates/revier-analysis/src/git/blame.rs`:

```rust
use crate::error::AppError;

#[derive(Debug)]
pub struct BlameLine {
    pub line_number: usize,
    pub commit_hash: String,
}

pub fn blame_range(
    repo: &gix::Repository,
    head_commit: &str,
    path: &str,
    start_line: usize,
    end_line: usize,
) -> Result<Vec<BlameLine>, AppError> {
    let _object = repo
        .rev_parse_single(head_commit)
        .map_err(|error| AppError::Repository(error.to_string()))?;

    if start_line == 0 || end_line < start_line {
        return Ok(Vec::new());
    }

    let _path = path;
    Ok((start_line..=end_line)
        .map(|line_number| BlameLine {
            line_number,
            commit_hash: head_commit.to_string(),
        })
        .collect())
}
```

Modify `crates/revier-analysis/src/git/mod.rs`:

```rust
pub mod blame;
pub mod diff;
pub mod repository;
```

This stub is intentionally insufficient for old lines. The next step replaces it with `gix_blame::file`.

- [ ] **Step 4: Run the blame test**

Run:

```powershell
cargo test -p revier-analysis --test gix_capabilities gix_blame_returns_source_commit_for_added_line
```

Expected: PASS.

- [ ] **Step 5: Add an old-line regression assertion**

Append to `gix_blame_returns_source_commit_for_added_line`:

```rust
    let first_line = revier_analysis::git::blame::blame_range(&repo, &fixture.head, "src/app.txt", 1, 1)
        .expect("blame first line");
    assert_eq!(first_line[0].commit_hash, fixture.base);
```

- [ ] **Step 6: Run the blame test and verify it fails**

Run:

```powershell
cargo test -p revier-analysis --test gix_capabilities gix_blame_returns_source_commit_for_added_line
```

Expected: FAIL because the stub assigns all lines to `head`.

- [ ] **Step 7: Replace the blame stub with gix blame**

Replace `blame_range` with an implementation using `gix_blame::file`. Preserve this public signature:

```rust
pub fn blame_range(
    repo: &gix::Repository,
    head_commit: &str,
    path: &str,
    start_line: usize,
    end_line: usize,
) -> Result<Vec<BlameLine>, AppError>
```

Implementation requirements:

- Parse `head_commit` to a gix object id.
- Build `gix_blame::Options` with `ranges` set to the requested range.
- Enable rewrite tracking in the blame options.
- Pass a mutable resource cache to `gix_blame::file`.
- Convert each returned blame entry into one `BlameLine` per final line.
- Do not call `git` from `src/`.

- [ ] **Step 8: Run the blame test**

Run:

```powershell
cargo test -p revier-analysis --test gix_capabilities gix_blame_returns_source_commit_for_added_line
```

Expected: PASS.

- [ ] **Step 9: Commit**

```powershell
git add crates/revier-analysis/src/git/blame.rs crates/revier-analysis/src/git/mod.rs crates/revier-analysis/tests/gix_capabilities.rs
git commit -m "feat: validate gix blame range probe"
```

## Task 9: Spike Runner

**Files:**
- Create: `crates/revier-analysis/src/spike.rs`
- Modify: `crates/revier-analysis/src/lib.rs`
- Modify: `crates/revier-analysis/tests/cli_contract.rs`

- [ ] **Step 1: Write failing CLI fixture output test**

Append to `crates/revier-analysis/tests/cli_contract.rs`:

```rust
#[test]
fn spike_run_reports_checks_for_linear_fixture() {
    let fixture = crate::fixtures::linear();
    let output = Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
        .args([
            "spike",
            "run",
            "--repo",
            fixture.repo.path().to_str().expect("repo path"),
            "--fixture",
            "linear",
            "--format",
            "json",
        ])
        .output()
        .expect("run spike command");

    assert!(output.status.success());
    let value: Value = serde_json::from_slice(&output.stdout).expect("parse spike json");
    assert_eq!(value["fixture"], "linear");
    assert!(
        value["checks"].as_array().expect("checks array").len() >= 2,
        "expected blame and parent checks"
    );
}
```

Also add the module declaration at the top of `cli_contract.rs`:

```rust
mod fixtures;
```

- [ ] **Step 2: Run the CLI output test and verify it fails**

Run:

```powershell
cargo test -p revier-analysis --test cli_contract spike_run_reports_checks_for_linear_fixture
```

Expected: FAIL because `run_from` still emits an empty checks array.

- [ ] **Step 3: Implement spike orchestration**

Create `crates/revier-analysis/src/spike.rs`:

```rust
use crate::cli::SpikeFixture;
use crate::error::AppError;
use crate::git::{blame, repository};
use crate::json::{CheckStatus, SpikeCheck, SpikeDecision, SpikeOutput};
use std::path::Path;

pub fn run(repo_path: &Path, fixture: &SpikeFixture) -> Result<SpikeOutput, AppError> {
    let fixture_name = fixture_name(fixture);
    let repo = repository::open_repository(repo_path)?;
    let head = repo
        .head_id()
        .map_err(|error| AppError::Repository(error.to_string()))?
        .to_string();

    let mut output = SpikeOutput::new(fixture_name);
    let parents = repository::parent_ids(&repo, &head)?;
    output.checks.push(SpikeCheck {
        name: "gix-parent-ids".to_string(),
        status: if parents.is_empty() { CheckStatus::Fail } else { CheckStatus::Pass },
        differences: Vec::new(),
    });

    let blame_lines = blame::blame_range(&repo, &head, "src/app.txt", 1, 1).unwrap_or_default();
    output.checks.push(SpikeCheck {
        name: "gix-blame".to_string(),
        status: if blame_lines.is_empty() { CheckStatus::Fail } else { CheckStatus::Pass },
        differences: Vec::new(),
    });

    output.decision = if output.checks.iter().all(|check| matches!(check.status, CheckStatus::Pass)) {
        SpikeDecision::GixOnly
    } else {
        SpikeDecision::AlgorithmAdjustmentRequired
    };

    Ok(output)
}

fn fixture_name(fixture: &SpikeFixture) -> &'static str {
    match fixture {
        SpikeFixture::Linear => "linear",
        SpikeFixture::EarlyFeatureMerge => "early-feature-merge",
        SpikeFixture::MergeConflict => "merge-conflict",
        SpikeFixture::MultiParentAmbiguous => "multi-parent-ambiguous",
        SpikeFixture::RenameMerge => "rename-merge",
        SpikeFixture::DeletionMerge => "deletion-merge",
    }
}
```

Modify `crates/revier-analysis/src/lib.rs`:

```rust
pub mod cli;
pub mod error;
pub mod git;
pub mod json;
pub mod spike;

use clap::Parser;
use cli::{Cli, Command, SpikeSubcommand};
use error::AppError;

pub fn run_from<I, T>(args: I) -> Result<String, AppError>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let cli = Cli::parse_from(args);
    match cli.command {
        Command::Spike(spike_command) => match spike_command.command {
            SpikeSubcommand::Run(args) => {
                let output = spike::run(&args.repo, &args.fixture)?;
                let json = if args.pretty {
                    serde_json::to_string_pretty(&output)?
                } else {
                    serde_json::to_string(&output)?
                };
                Ok(format!("{json}\n"))
            }
        },
    }
}
```

- [ ] **Step 4: Run the CLI output test**

Run:

```powershell
cargo test -p revier-analysis --test cli_contract spike_run_reports_checks_for_linear_fixture
```

Expected: PASS after the gix blame implementation is complete.

- [ ] **Step 5: Commit**

```powershell
git add crates/revier-analysis/src/spike.rs crates/revier-analysis/src/lib.rs crates/revier-analysis/tests/cli_contract.rs
git commit -m "feat: add gix spike runner"
```

## Task 10: No Git Process Guard

**Files:**
- Create: `crates/revier-analysis/tests/no_git_process.rs`

- [ ] **Step 1: Write the guard test**

Create `crates/revier-analysis/tests/no_git_process.rs`:

```rust
use std::fs;
use std::path::Path;

#[test]
fn production_source_does_not_spawn_git_process() {
    let crate_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src = crate_root.join("src");
    let mut violations = Vec::new();
    scan_rs_files(&src, &mut violations);

    assert!(
        violations.is_empty(),
        "src/ must not spawn git processes: {:?}",
        violations
    );
}

fn scan_rs_files(path: &Path, violations: &mut Vec<String>) {
    for entry in fs::read_dir(path).expect("read directory") {
        let entry = entry.expect("read directory entry");
        let path = entry.path();
        if path.is_dir() {
            scan_rs_files(&path, violations);
            continue;
        }

        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }

        let source = fs::read_to_string(&path).expect("read source file");
        if source.contains("Command::new(\"git\")")
            || source.contains("Command::new('git')")
            || source.contains("std::process::Command")
        {
            violations.push(path.display().to_string());
        }
    }
}
```

- [ ] **Step 2: Run the guard test**

Run:

```powershell
cargo test -p revier-analysis --test no_git_process
```

Expected: PASS. The fixture helper may use `git`, but it lives under `tests/` and is intentionally excluded.

- [ ] **Step 3: Commit**

```powershell
git add crates/revier-analysis/tests/no_git_process.rs
git commit -m "test: guard rust source against git process usage"
```

## Task 11: pnpm Script Integration

**Files:**
- Modify: `package.json`

- [ ] **Step 1: Add failing script expectation**

Run:

```powershell
pnpm rust:test
```

Expected: FAIL because the script does not exist.

- [ ] **Step 2: Add Rust scripts**

Modify `package.json` scripts:

```json
{
  "scripts": {
    "rust:test": "cargo test --workspace",
    "rust:spike": "cargo run -p revier-analysis -- spike run"
  }
}
```

Keep all existing scripts unchanged and only add these two keys.

- [ ] **Step 3: Run Rust tests through pnpm**

Run:

```powershell
pnpm rust:test
```

Expected: PASS with all Rust tests passing.

- [ ] **Step 4: Commit**

```powershell
git add package.json
git commit -m "chore: add rust spike scripts"
```

## Task 12: Full Verification and Spike Decision Summary

**Files:**
- Modify only if earlier tasks require a small correction discovered by verification.

- [ ] **Step 1: Run Rust formatting**

Run:

```powershell
cargo fmt --all --check
```

Expected: PASS. If it fails, run `cargo fmt --all`, then rerun the check.

- [ ] **Step 2: Run Rust tests**

Run:

```powershell
cargo test --workspace
```

Expected: PASS.

- [ ] **Step 3: Run existing TypeScript typecheck**

Run:

```powershell
pnpm typecheck
```

Expected: PASS. This confirms the Rust workspace changes did not break existing TypeScript project configuration.

- [ ] **Step 4: Run the spike command manually against a fixture repo**

Use a repo path from the fixture test output if the implementation prints one during debugging, or run against the current repository only for command-shape validation:

```powershell
cargo run -p revier-analysis -- spike run --repo e:\Projects\revier --fixture linear --format json --pretty
```

Expected: JSON with `version: 1`, `fixture: "linear"`, and a non-empty `checks` array. If the current repository does not contain `src/app.txt`, the command may report a failed blame check; that is acceptable for command-shape validation, while fixture tests remain the correctness authority.

- [ ] **Step 5: Review no-git constraint**

Run:

```powershell
cargo test -p revier-analysis --test no_git_process
```

Expected: PASS.

- [ ] **Step 6: Commit final verification corrections**

If any formatting or small verification corrections changed files:

```powershell
git add Cargo.toml .gitignore package.json crates/revier-analysis
git commit -m "chore: verify gix capability spike"
```

If no files changed, do not create an empty commit.

## Self-Review Checklist

- The Rust crate lives inside the current repository under `crates/revier-analysis`.
- No git submodule is introduced.
- The spike does not write DuckDB.
- The spike does not touch Electron UI or existing TypeScript analysis behavior.
- Tests may use `git` to construct fixture repositories.
- `src/` production Rust code does not call `git` through `std::process::Command`.
- Every CLI output is JSON on stdout.
- Errors are printed to stderr with stable exit codes.
- Commit messages use semantic commit format.

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-06-15-gix-capability-spike-implementation.md`. Two execution options:

1. **Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration.
2. **Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints.

Which approach?
