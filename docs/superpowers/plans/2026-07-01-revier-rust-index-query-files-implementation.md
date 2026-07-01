# Revier Rust 索引文件查询实施计划

> **面向自动化执行代理：** 必须使用 `superpowers:subagent-driven-development`（推荐）或 `superpowers:executing-plans` 按任务逐项执行本计划。步骤使用复选框（`- [ ]`）语法跟踪。

**目标：** 在现有 `crates/revier-analysis` Rust CLI 中实现 DuckDB 本地索引和 `index query-files`，并让 Electron 主进程在显示提交筛选场景优先使用 Rust 查询文件列表。

**架构：** Rust 侧新增 `commands/index_*`、`git/commits.rs`、`index/*` 模块，使用 gix 读取本地 Git 对象并使用 DuckDB 存储提交、父提交、提交触碰文件和索引运行状态。Electron 侧新增 `src/main/analysis/rustAnalysisClient.ts` 作为 CLI 适配层，`reviewIpc` 只负责在可降级边界中编排 Rust 查询和现有 TypeScript 路径。

**技术栈：** Rust stable、Cargo workspace、clap、serde、serde_json、thiserror、gix 0.84、DuckDB Rust 绑定候选 `duckdb 1.10504.0` + `bundled` 特性、chrono、sha2、globset、TypeScript、Electron、Vitest、pnpm、fnm、pwsh。

---

## 来源规约

- 设计文档：`docs/superpowers/specs/2026-07-01-revier-rust-index-query-files-design.md`
- DuckDB Rust 绑定文档：<https://docs.rs/duckdb/latest/duckdb/>
- duckdb-rs README：<https://github.com/duckdb/duckdb-rs>

## 范围

本计划只实现 Rust 索引与文件列表查询一期：

- 实现 `revier-analysis index status`、`revier-analysis index build`、`revier-analysis index query-files`。
- 生产 Rust 路径只使用 gix 读取 Git 仓库，不调用 `git` 进程。
- DuckDB 保存 `metadata`、`commits`、`commit_parents`、`commit_files`、`index_runs`。
- `query-files` 输出与 TypeScript `ChangedFile[]` 兼容的 JSON。
- Electron 主进程只在存在作者、作者搜索或提交信息筛选时尝试 Rust 查询。
- Rust 查询不可用时降级到现有 TypeScript 文件列表路径，并记录可恢复中文 warning。

本计划不实现：

- overlay、trace-block、blame、merge trace、UI 渲染迁移。
- 后台常驻服务。
- `query-files` 中隐式冷索引构建。
- 远程 Git 平台 API。

## 执行前确认点

在开始任务 2 之前，执行者必须向用户提交以下依赖清单并获得书面确认后再修改 `crates/revier-analysis/Cargo.toml`：

```toml
duckdb = { version = "1.10504.0", features = ["bundled"] }
chrono = { version = "0.4", default-features = false, features = ["clock", "serde", "std"] }
sha2 = "0.10"
hex = "0.4"
globset = "0.4"
```

确认理由：

- `duckdb 1.10504.0` 是当前 docs.rs 文档显示的版本，duckdb-rs README 建议使用 `cargo add duckdb -F bundled`。
- `bundled` 让 Windows 开发环境不依赖系统级 DuckDB 动态库，但会增加首次编译耗时。
- `chrono` 用于 ISO 时间解析、格式化和 DuckDB 时间参数。
- `sha2` 与 `hex` 用于稳定生成 `repo_id`。
- `globset` 用于在 Rust 中复刻 include/exclude glob 规则。

若用户拒绝 `duckdb` 的 `bundled` 特性，执行者必须暂停并提交新的依赖方案，不得改用未确认绑定。

## 文件结构

- 修改：`crates/revier-analysis/Cargo.toml`
  - 增加已确认的 DuckDB、时间、hash 和 glob 依赖。
- 修改：`crates/revier-analysis/src/cli.rs`
  - 增加 `index status`、`index build`、`index query-files` 参数。
- 修改：`crates/revier-analysis/src/error.rs`
  - 增加索引、schema、DuckDB、参数错误类型和退出码。
- 修改：`crates/revier-analysis/src/json.rs`
  - 增加 index status、build、query-files JSON 契约类型。
- 修改：`crates/revier-analysis/src/lib.rs`
  - 导出新模块并分发 index 子命令。
- 新建：`crates/revier-analysis/src/git/commits.rs`
  - 使用 gix 遍历提交范围，读取提交元数据、父提交和树。
- 修改：`crates/revier-analysis/src/git/diff.rs`
  - 新增 pairwise 文件变更记录输出，包含状态、old path、binary、additions、deletions。
- 修改：`crates/revier-analysis/src/git/mod.rs`
  - 导出 `commits`。
- 新建：`crates/revier-analysis/src/index/connection.rs`
  - 解析默认 DuckDB 路径、打开连接。
- 新建：`crates/revier-analysis/src/index/schema.rs`
  - 创建表结构和读取 schema version。
- 新建：`crates/revier-analysis/src/index/migrations.rs`
  - 管理当前 schema 版本，本期只支持 version 1。
- 新建：`crates/revier-analysis/src/index/writer.rs`
  - 写入 metadata、commits、commit_parents、commit_files、index_runs。
- 新建：`crates/revier-analysis/src/index/queries.rs`
  - 查询 status、stale 范围和 filtered files。
- 新建：`crates/revier-analysis/src/index/mod.rs`
  - 汇总导出 index 模块。
- 新建：`crates/revier-analysis/src/commands/index_status.rs`
  - `index status` 编排。
- 新建：`crates/revier-analysis/src/commands/index_build.rs`
  - `index build` 编排。
- 新建：`crates/revier-analysis/src/commands/query_files.rs`
  - `index query-files` 编排。
- 新建：`crates/revier-analysis/src/commands/mod.rs`
  - 汇总导出命令模块。
- 修改：`crates/revier-analysis/tests/fixtures.rs`
  - 增加作者、时间、rename、binary、merge fixture 工具。
- 修改：`crates/revier-analysis/tests/cli_contract.rs`
  - 增加 index CLI 契约测试。
- 新建：`crates/revier-analysis/tests/index_schema.rs`
  - 验证 schema 初始化、status、schema 不兼容。
- 新建：`crates/revier-analysis/tests/index_query_files.rs`
  - 验证 build 后按作者、作者搜索、提交信息、时间和 glob 查询文件。
- 修改：`crates/revier-analysis/tests/no_git_process.rs`
  - 保持生产源码不调用 Git 进程约束。
- 新建：`src/main/analysis/rustAnalysisClient.ts`
  - Electron 主进程 Rust CLI 适配层。
- 修改：`src/main/ipc/reviewIpc.ts`
  - 在显示提交筛选分支尝试 Rust query-files，并降级到现有逻辑。
- 修改：`tests/unit/reviewIpc.test.ts`
  - 增加 Rust 成功、Rust 不可用降级、无显示提交筛选不调用 Rust 的测试。
- 新建：`tests/unit/rustAnalysisClient.test.ts`
  - 验证参数组装、JSON 解析、错误码映射。
- 修改：`package.json`
  - 增加已确认的 Rust index 调试脚本。

## 任务 1：扩展 CLI、JSON 和错误契约

**文件：**
- 修改：`crates/revier-analysis/src/cli.rs`
- 修改：`crates/revier-analysis/src/json.rs`
- 修改：`crates/revier-analysis/src/error.rs`
- 修改：`crates/revier-analysis/src/lib.rs`
- 修改：`crates/revier-analysis/tests/cli_contract.rs`

- [ ] **步骤 1：写失败的 index CLI help 测试**

修改 `crates/revier-analysis/tests/cli_contract.rs`，追加：

```rust
#[test]
fn index_help_lists_status_build_and_query_files_commands() {
    let output = Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
        .args(["index", "--help"])
        .output()
        .expect("运行 revier-analysis index --help");

    assert!(
        output.status.success(),
        "index --help 应成功，stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("status"));
    assert!(stdout.contains("build"));
    assert!(stdout.contains("query-files"));
}
```

- [ ] **步骤 2：运行测试确认失败**

运行：

```powershell
cargo test -p revier-analysis --test cli_contract index_help_lists_status_build_and_query_files_commands
```

预期： 失败，`index` 子命令不存在。

- [ ] **步骤 3：扩展 `cli.rs`**

修改 `crates/revier-analysis/src/cli.rs`，保留现有 `Spike`，增加 index 参数结构：

```rust
#[derive(Debug, Subcommand)]
pub enum Command {
    Spike(SpikeCommand),
    Index(IndexCommand),
}

#[derive(Debug, Parser)]
pub struct IndexCommand {
    #[command(subcommand)]
    pub command: IndexSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum IndexSubcommand {
    Status(IndexStatusArgs),
    Build(IndexBuildArgs),
    QueryFiles(QueryFilesArgs),
}

#[derive(Debug, Parser)]
pub struct IndexCommonArgs {
    #[arg(long)]
    pub repo: PathBuf,

    #[arg(long)]
    pub db: Option<PathBuf>,

    #[arg(long, default_value = "json")]
    pub format: OutputFormat,

    #[arg(long)]
    pub pretty: bool,
}

#[derive(Debug, Parser)]
pub struct IndexStatusArgs {
    #[command(flatten)]
    pub common: IndexCommonArgs,
}

#[derive(Debug, Parser)]
pub struct IndexBuildArgs {
    #[command(flatten)]
    pub common: IndexCommonArgs,

    #[arg(long)]
    pub branch: String,
}

#[derive(Debug, Parser)]
pub struct QueryFilesArgs {
    #[command(flatten)]
    pub common: IndexCommonArgs,

    #[arg(long)]
    pub base: String,

    #[arg(long)]
    pub head: String,

    #[arg(long)]
    pub branch: String,

    #[arg(long = "author")]
    pub authors: Vec<String>,

    #[arg(long = "author-query")]
    pub author_query: Option<String>,

    #[arg(long = "message")]
    pub message: Option<String>,

    #[arg(long = "since")]
    pub since: Option<String>,

    #[arg(long = "until")]
    pub until: Option<String>,

    #[arg(long = "glob")]
    pub globs: Vec<String>,
}
```

- [ ] **步骤 4：扩展 JSON 契约类型**

修改 `crates/revier-analysis/src/json.rs`，追加：

```rust
#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum IndexStatusKind {
    Missing,
    Building,
    Ready,
    Stale,
    Incompatible,
}

#[derive(Debug, Serialize)]
pub struct IndexStatusOutput {
    pub version: u8,
    pub repo_id: String,
    pub schema_version: u32,
    pub status: IndexStatusKind,
    pub indexed_commit_count: u64,
    pub indexed_file_count: u64,
    pub updated_at: Option<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum IndexRunStatus {
    Completed,
    Failed,
}

#[derive(Debug, Serialize)]
pub struct IndexBuildOutput {
    pub version: u8,
    pub repo_id: String,
    pub status: IndexRunStatus,
    pub indexed_commit_count: u64,
    pub indexed_file_count: u64,
    pub elapsed_ms: u64,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct QueryFilesRangeOutput {
    pub base_commit: String,
    pub head_commit: String,
}

#[derive(Debug, Serialize)]
pub struct ChangedFileOutput {
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub old_path: Option<String>,
    pub status: String,
    pub additions: u64,
    pub deletions: u64,
    pub is_binary: bool,
    pub is_previewable: bool,
}

#[derive(Debug, Serialize)]
pub struct QueryFilesOutput {
    pub version: u8,
    pub range: QueryFilesRangeOutput,
    pub files: Vec<ChangedFileOutput>,
    pub warnings: Vec<String>,
}
```

在输出 JSON 时使用 `serde_json` 的默认 snake_case 会不满足设计要求，执行者必须在任务 6 把外层输出转换为 camelCase。任务 1 先建立 Rust 内部类型，任务 6 用 snapshot 测试锁定最终字段。

- [ ] **步骤 5：扩展错误类型和退出码**

修改 `crates/revier-analysis/src/error.rs`：

```rust
#[derive(Debug, Error)]
pub enum AppError {
    #[error("参数错误：{0}")]
    InvalidArgument(String),

    #[error("仓库读取失败：{0}")]
    Repository(String),

    #[error("索引不可用：{0}")]
    IndexUnavailable(String),

    #[error("索引 schema 不兼容：{0}")]
    SchemaIncompatible(String),

    #[error("DuckDB 读写失败：{0}")]
    DuckDb(String),

    #[error("gix 能力验证失败：{0}")]
    Spike(String),

    #[error("JSON 输出失败：{0}")]
    Json(#[from] serde_json::Error),
}

impl AppError {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::InvalidArgument(_) => 2,
            Self::Repository(_) => 3,
            Self::IndexUnavailable(_) => 4,
            Self::SchemaIncompatible(_) => 5,
            Self::DuckDb(_) => 6,
            Self::Spike(_) | Self::Json(_) => 10,
        }
    }
}
```

- [ ] **步骤 6：在 `lib.rs` 中分发 index 子命令**

修改 `crates/revier-analysis/src/lib.rs`，先返回可编译的固定 JSON：

```rust
pub mod cli;
pub mod commands;
pub mod error;
pub mod git;
pub mod index;
pub mod json;
pub mod spike;

use clap::Parser;
use cli::{Cli, Command, IndexSubcommand, SpikeSubcommand};
use error::AppError;

pub fn run_from<I, T>(args: I) -> Result<String, AppError>
where
    I: IntoIterator<Item = T>,
    T: Into<std::ffi::OsString> + Clone,
{
    let cli = Cli::parse_from(args);
    match cli.command {
        Command::Spike(spike) => match spike.command {
            SpikeSubcommand::Run(args) => {
                let output = spike::run(&args.repo, &args.fixture)?;
                serialize_json(&output, args.pretty)
            }
        },
        Command::Index(index) => match index.command {
            IndexSubcommand::Status(args) => commands::index_status::run(args),
            IndexSubcommand::Build(args) => commands::index_build::run(args),
            IndexSubcommand::QueryFiles(args) => commands::query_files::run(args),
        },
    }
}

pub(crate) fn serialize_json<T: serde::Serialize>(
    output: &T,
    pretty: bool,
) -> Result<String, AppError> {
    let json = if pretty {
        serde_json::to_string_pretty(output)?
    } else {
        serde_json::to_string(output)?
    };
    Ok(format!("{json}\n"))
}
```

新建 `crates/revier-analysis/src/commands/mod.rs`：

```rust
pub mod index_build;
pub mod index_status;
pub mod query_files;
```

新建 `crates/revier-analysis/src/commands/index_status.rs`：

```rust
use crate::cli::IndexStatusArgs;
use crate::error::AppError;
use crate::json::{IndexStatusKind, IndexStatusOutput};

pub fn run(args: IndexStatusArgs) -> Result<String, AppError> {
    let output = IndexStatusOutput {
        version: 1,
        repo_id: format!("uninitialized:{}", args.common.repo.display()),
        schema_version: 0,
        status: IndexStatusKind::Missing,
        indexed_commit_count: 0,
        indexed_file_count: 0,
        updated_at: None,
        warnings: vec!["索引尚未初始化".to_string()],
    };
    crate::serialize_json(&output, args.common.pretty)
}
```

新建 `crates/revier-analysis/src/commands/index_build.rs`：

```rust
use crate::cli::IndexBuildArgs;
use crate::error::AppError;
use crate::json::{IndexBuildOutput, IndexRunStatus};

pub fn run(args: IndexBuildArgs) -> Result<String, AppError> {
    let output = IndexBuildOutput {
        version: 1,
        repo_id: format!("uninitialized:{}", args.common.repo.display()),
        status: IndexRunStatus::Failed,
        indexed_commit_count: 0,
        indexed_file_count: 0,
        elapsed_ms: 0,
        warnings: vec![format!("索引构建尚未接入分支 {}", args.branch)],
    };
    crate::serialize_json(&output, args.common.pretty)
}
```

新建 `crates/revier-analysis/src/commands/query_files.rs`：

```rust
use crate::cli::QueryFilesArgs;
use crate::error::AppError;
use crate::json::{QueryFilesOutput, QueryFilesRangeOutput};

pub fn run(args: QueryFilesArgs) -> Result<String, AppError> {
    let output = QueryFilesOutput {
        version: 1,
        range: QueryFilesRangeOutput {
            base_commit: args.base,
            head_commit: args.head,
        },
        files: Vec::new(),
        warnings: vec!["索引查询尚未接入 DuckDB".to_string()],
    };
    crate::serialize_json(&output, args.common.pretty)
}
```

新建 `crates/revier-analysis/src/index/mod.rs`：

```rust
pub mod connection;
pub mod migrations;
pub mod queries;
pub mod schema;
pub mod writer;
```

为上述空模块各创建一个文件，内容为：

```rust
pub const INDEX_MODULE_READY: bool = true;
```

- [ ] **步骤 7：运行测试确认通过**

运行：

```powershell
cargo test -p revier-analysis --test cli_contract
```

预期： 通过，现有 spike 契约和新增 index help 契约都通过。

- [ ] **步骤 8：提交**

运行：

```powershell
git add crates/revier-analysis/src crates/revier-analysis/tests/cli_contract.rs
git commit -m "feat: 添加 Rust index CLI 契约"
```

## 任务 2：确认并接入 DuckDB schema

**文件：**
- 修改：`crates/revier-analysis/Cargo.toml`
- 修改：`crates/revier-analysis/src/index/connection.rs`
- 修改：`crates/revier-analysis/src/index/schema.rs`
- 修改：`crates/revier-analysis/src/index/migrations.rs`
- 新建：`crates/revier-analysis/tests/index_schema.rs`

- [ ] **步骤 1：提交依赖确认请求**

向用户展示“执行前确认点”中的依赖清单，等待明确确认。未确认前不得执行步骤 2。

- [ ] **步骤 2：写失败的 schema 初始化测试**

创建 `crates/revier-analysis/tests/index_schema.rs`：

```rust
use tempfile::tempdir;

#[test]
fn initializes_schema_version_one_tables() {
    let dir = tempdir().expect("创建临时目录");
    let db_path = dir.path().join("index.duckdb");
    let conn = revier_analysis::index::connection::open_database(&db_path)
        .expect("打开 DuckDB");

    revier_analysis::index::schema::initialize_schema(&conn, "repo-1", "E:/repo/app", "E:/repo/app/.git")
        .expect("初始化 schema");

    let version = revier_analysis::index::schema::read_schema_version(&conn)
        .expect("读取 schema version");
    assert_eq!(version, Some(1));

    let tables = revier_analysis::index::schema::list_tables(&conn).expect("读取表列表");
    assert!(tables.contains(&"metadata".to_string()));
    assert!(tables.contains(&"commits".to_string()));
    assert!(tables.contains(&"commit_parents".to_string()));
    assert!(tables.contains(&"commit_files".to_string()));
    assert!(tables.contains(&"index_runs".to_string()));
}

#[test]
fn reports_incompatible_schema_version() {
    let dir = tempdir().expect("创建临时目录");
    let db_path = dir.path().join("index.duckdb");
    let conn = revier_analysis::index::connection::open_database(&db_path)
        .expect("打开 DuckDB");

    conn.execute_batch(
        "create table metadata (key text primary key, value text not null);
         insert into metadata values ('schema_version', '999');"
    )
    .expect("写入不兼容版本");

    let result = revier_analysis::index::migrations::ensure_compatible_schema(&conn);
    assert!(matches!(
        result,
        Err(revier_analysis::error::AppError::SchemaIncompatible(_))
    ));
}
```

- [ ] **步骤 3：运行测试确认失败**

运行：

```powershell
cargo test -p revier-analysis --test index_schema
```

预期： 失败，`duckdb` 依赖和 index schema 函数尚不存在。

- [ ] **步骤 4：添加已确认依赖**

修改 `crates/revier-analysis/Cargo.toml`：

```toml
chrono = { version = "0.4", default-features = false, features = ["clock", "serde", "std"] }
duckdb = { version = "1.10504.0", features = ["bundled"] }
globset = "0.4"
hex = "0.4"
sha2 = "0.10"
```

- [ ] **步骤 5：实现连接模块**

替换 `crates/revier-analysis/src/index/connection.rs`：

```rust
use crate::error::AppError;
use std::path::{Path, PathBuf};

pub fn open_database(path: &Path) -> Result<duckdb::Connection, AppError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| AppError::DuckDb(error.to_string()))?;
    }
    duckdb::Connection::open(path).map_err(|error| AppError::DuckDb(error.to_string()))
}

pub fn default_database_path(repo_id: &str) -> Result<PathBuf, AppError> {
    let base = default_app_data_dir()?;
    Ok(base.join("revier").join("indexes").join(format!("{repo_id}.duckdb")))
}

fn default_app_data_dir() -> Result<PathBuf, AppError> {
    if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .ok_or_else(|| AppError::IndexUnavailable("无法读取 APPDATA 环境变量".to_string()))
    } else if cfg!(target_os = "macos") {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .ok_or_else(|| AppError::IndexUnavailable("无法读取 HOME 环境变量".to_string()))?;
        Ok(home.join("Library").join("Application Support"))
    } else {
        Ok(std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local").join("share")))
            .ok_or_else(|| AppError::IndexUnavailable("无法读取 XDG_DATA_HOME 或 HOME 环境变量".to_string()))?)
    }
}
```

- [ ] **步骤 6：实现 schema 与 migration**

替换 `crates/revier-analysis/src/index/migrations.rs`：

```rust
use crate::error::AppError;

pub const CURRENT_SCHEMA_VERSION: u32 = 1;

pub fn ensure_compatible_schema(conn: &duckdb::Connection) -> Result<(), AppError> {
    match crate::index::schema::read_schema_version(conn)? {
        Some(CURRENT_SCHEMA_VERSION) | None => Ok(()),
        Some(version) => Err(AppError::SchemaIncompatible(format!(
            "当前支持 schema_version={CURRENT_SCHEMA_VERSION}，数据库为 {version}"
        ))),
    }
}
```

替换 `crates/revier-analysis/src/index/schema.rs`：

```rust
use crate::error::AppError;
use crate::index::migrations::CURRENT_SCHEMA_VERSION;
use chrono::Utc;
use duckdb::params;

pub fn initialize_schema(
    conn: &duckdb::Connection,
    repo_id: &str,
    repo_root: &str,
    git_common_dir: &str,
) -> Result<(), AppError> {
    conn.execute_batch(
        "
        create table if not exists metadata (
          key text primary key,
          value text not null
        );
        create table if not exists commits (
          hash text primary key,
          short_hash text not null,
          author_name text not null,
          author_email text,
          author_key text not null,
          committed_at timestamp not null,
          subject text not null,
          parent_count integer not null,
          is_merge boolean not null
        );
        create table if not exists commit_parents (
          commit_hash text not null,
          parent_hash text not null,
          parent_index integer not null,
          primary key (commit_hash, parent_index)
        );
        create table if not exists commit_files (
          commit_hash text not null,
          parent_hash text not null,
          parent_index integer not null,
          path text not null,
          old_path text not null,
          status text not null,
          additions integer not null,
          deletions integer not null,
          is_binary boolean not null,
          is_previewable boolean not null,
          similarity real,
          primary key (commit_hash, parent_index, path, old_path)
        );
        create table if not exists index_runs (
          run_id text primary key,
          repo_id text not null,
          started_at timestamp not null,
          finished_at timestamp,
          status text not null,
          indexed_commit_count integer not null,
          indexed_file_count integer not null,
          elapsed_ms integer,
          error_message text
        );
        ",
    )
    .map_err(|error| AppError::DuckDb(error.to_string()))?;

    let now = Utc::now().to_rfc3339();
    upsert_metadata(conn, "schema_version", &CURRENT_SCHEMA_VERSION.to_string())?;
    upsert_metadata(conn, "repo_id", repo_id)?;
    upsert_metadata(conn, "repo_root", repo_root)?;
    upsert_metadata(conn, "git_common_dir", git_common_dir)?;
    upsert_metadata(conn, "created_at", &now)?;
    upsert_metadata(conn, "updated_at", &now)?;
    Ok(())
}

pub fn read_schema_version(conn: &duckdb::Connection) -> Result<Option<u32>, AppError> {
    let exists: bool = conn
        .query_row(
            "select count(*) > 0 from information_schema.tables where table_name = 'metadata'",
            [],
            |row| row.get(0),
        )
        .map_err(|error| AppError::DuckDb(error.to_string()))?;
    if !exists {
        return Ok(None);
    }

    let value = conn
        .query_row("select value from metadata where key = 'schema_version'", [], |row| {
            row.get::<_, String>(0)
        })
        .ok();
    value
        .map(|item| item.parse::<u32>().map_err(|error| AppError::DuckDb(error.to_string())))
        .transpose()
}

pub fn list_tables(conn: &duckdb::Connection) -> Result<Vec<String>, AppError> {
    let mut stmt = conn
        .prepare("select table_name from information_schema.tables order by table_name")
        .map_err(|error| AppError::DuckDb(error.to_string()))?;
    let rows = stmt
        .query_map([], |row| row.get::<_, String>(0))
        .map_err(|error| AppError::DuckDb(error.to_string()))?;

    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|error| AppError::DuckDb(error.to_string()))
}

fn upsert_metadata(conn: &duckdb::Connection, key: &str, value: &str) -> Result<(), AppError> {
    conn.execute(
        "insert or replace into metadata (key, value) values (?, ?)",
        params![key, value],
    )
    .map_err(|error| AppError::DuckDb(error.to_string()))?;
    Ok(())
}
```

- [ ] **步骤 7：运行 schema 测试确认通过**

运行：

```powershell
cargo test -p revier-analysis --test index_schema
```

预期： 通过。

- [ ] **步骤 8：提交**

运行：

```powershell
git add crates/revier-analysis/Cargo.toml crates/revier-analysis/src/index crates/revier-analysis/tests/index_schema.rs Cargo.lock
git commit -m "feat: 添加 DuckDB 索引 schema"
```

## 任务 3：实现仓库标识和提交遍历

**文件：**
- 新建：`crates/revier-analysis/src/git/commits.rs`
- 修改：`crates/revier-analysis/src/git/repository.rs`
- 修改：`crates/revier-analysis/src/git/mod.rs`
- 修改：`crates/revier-analysis/tests/fixtures.rs`
- 新建：`crates/revier-analysis/tests/index_git_commits.rs`

- [ ] **步骤 1：写提交遍历测试**

创建 `crates/revier-analysis/tests/index_git_commits.rs`：

```rust
mod fixtures;

#[test]
fn builds_stable_repo_identity_from_root_and_common_dir() {
    let fixture = fixtures::linear();
    let repo = revier_analysis::git::repository::open_repository(fixture.repo.path())
        .expect("打开仓库");

    let identity = revier_analysis::git::repository::repository_identity(&repo)
        .expect("读取仓库标识");

    assert_eq!(identity.repo_id.len(), 64);
    assert!(identity.repo_root.ends_with(fixture.repo.path().file_name().unwrap().to_str().unwrap()));
    assert!(identity.git_common_dir.ends_with(".git"));
}

#[test]
fn lists_branch_commits_with_author_key_and_parent_hashes() {
    let fixture = fixtures::linear_with_authors();
    let repo = revier_analysis::git::repository::open_repository(fixture.repo.path())
        .expect("打开仓库");

    let commits = revier_analysis::git::commits::list_reachable_commits(&repo, "main")
        .expect("读取提交列表");

    assert_eq!(commits.len(), 3);
    assert_eq!(commits[0].hash, fixture.head);
    assert_eq!(commits[0].author_key, "bob@example.com");
    assert_eq!(commits[0].parents.len(), 1);
    assert_eq!(commits[1].author_key, "alice@example.com");
}
```

- [ ] **步骤 2：运行测试确认失败**

运行：

```powershell
cargo test -p revier-analysis --test index_git_commits
```

预期： 失败，`repository_identity`、`list_reachable_commits` 和 fixture 不存在。

- [ ] **步骤 3：扩展 fixture**

修改 `crates/revier-analysis/tests/fixtures.rs`，新增：

```rust
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
    git_with_author(repo.path(), ["add", "."], "Alice", "alice@example.com", None);
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
```

- [ ] **步骤 4：实现仓库标识**

修改 `crates/revier-analysis/src/git/repository.rs`：

```rust
use crate::error::AppError;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct RepositoryIdentity {
    pub repo_id: String,
    pub repo_root: String,
    pub git_common_dir: String,
}

pub fn repository_identity(repo: &gix::Repository) -> Result<RepositoryIdentity, AppError> {
    let repo_root = normalize_path(repo.work_dir().unwrap_or_else(|| repo.path()));
    let git_common_dir = normalize_path(repo.common_dir());
    let mut hasher = Sha256::new();
    hasher.update(repo_root.as_bytes());
    hasher.update(b"\0");
    hasher.update(git_common_dir.as_bytes());
    let repo_id = hex::encode(hasher.finalize());

    Ok(RepositoryIdentity {
        repo_id,
        repo_root,
        git_common_dir,
    })
}

fn normalize_path(path: &Path) -> String {
    path.components()
        .collect::<PathBuf>()
        .to_string_lossy()
        .replace('\\', "/")
        .trim_end_matches('/')
        .to_string()
}
```

保留原有 `open_repository` 和 `parent_ids`。

- [ ] **步骤 5：实现提交遍历**

新建 `crates/revier-analysis/src/git/commits.rs`：

```rust
use crate::error::AppError;
use chrono::{DateTime, Utc};
use gix::bstr::ByteSlice;

#[derive(Debug, Clone)]
pub struct IndexedCommit {
    pub hash: String,
    pub short_hash: String,
    pub author_name: String,
    pub author_email: Option<String>,
    pub author_key: String,
    pub committed_at: String,
    pub subject: String,
    pub parents: Vec<String>,
    pub is_merge: bool,
}

pub fn list_reachable_commits(
    repo: &gix::Repository,
    branch: &str,
) -> Result<Vec<IndexedCommit>, AppError> {
    let tip = repo
        .rev_parse_single(branch)
        .map_err(|error| AppError::Repository(error.to_string()))?;
    let walk = repo
        .rev_walk([tip.detach()])
        .all()
        .map_err(|error| AppError::Repository(error.to_string()))?;

    let mut commits = Vec::new();
    for item in walk {
        let info = item.map_err(|error| AppError::Repository(error.to_string()))?;
        let commit = info.object().map_err(|error| AppError::Repository(error.to_string()))?;
        commits.push(indexed_commit_from_gix(&commit)?);
    }
    Ok(commits)
}

pub fn indexed_commit_from_gix(commit: &gix::Commit<'_>) -> Result<IndexedCommit, AppError> {
    let author = commit.author().map_err(|error| AppError::Repository(error.to_string()))?;
    let author_name = author.name.to_str_lossy().trim().to_string();
    let author_email = author.email.to_str_lossy().trim().to_string();
    let author_email = (!author_email.is_empty()).then_some(author_email);
    let author_key = author_key(&author_name, author_email.as_deref());
    let parents = commit.parent_ids().map(|id| id.to_string()).collect::<Vec<_>>();
    let committed_at = gix_time_to_iso(commit.time().map_err(|error| AppError::Repository(error.to_string()))?);
    let subject = commit
        .message_raw()
        .map_err(|error| AppError::Repository(error.to_string()))?
        .lines()
        .next()
        .map(|line| line.to_str_lossy().to_string())
        .unwrap_or_default();

    Ok(IndexedCommit {
        hash: commit.id.to_string(),
        short_hash: commit.id.to_string().chars().take(8).collect(),
        author_name,
        author_email,
        author_key,
        committed_at,
        subject,
        is_merge: parents.len() > 1,
        parents,
    })
}

pub fn author_key(name: &str, email: Option<&str>) -> String {
    email
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(name)
        .trim()
        .to_lowercase()
}

fn gix_time_to_iso(time: gix::date::Time) -> String {
    let seconds = time.seconds;
    DateTime::<Utc>::from_timestamp(seconds, 0)
        .expect("gix 提交时间必须可转换为 UTC")
        .to_rfc3339()
}
```

修改 `crates/revier-analysis/src/git/mod.rs`：

```rust
pub mod blame;
pub mod commits;
pub mod diff;
pub mod repository;
```

- [ ] **步骤 6：运行测试确认通过**

运行：

```powershell
cargo test -p revier-analysis --test index_git_commits
cargo test -p revier-analysis --test no_git_process
```

预期： 通过，且 `no_git_process` 确认 `src/` 下没有 Git 进程调用。

- [ ] **步骤 7：提交**

运行：

```powershell
git add crates/revier-analysis/src/git crates/revier-analysis/tests/fixtures.rs crates/revier-analysis/tests/index_git_commits.rs
git commit -m "feat: 添加 gix 提交索引读取"
```

## 任务 4：实现 pairwise 文件变更记录

**文件：**
- 修改：`crates/revier-analysis/src/git/diff.rs`
- 新建：`crates/revier-analysis/tests/index_git_diff.rs`

- [ ] **步骤 1：写文件变更测试**

创建 `crates/revier-analysis/tests/index_git_diff.rs`：

```rust
mod fixtures;

#[test]
fn reads_pairwise_file_changes_with_status_and_paths() {
    let fixture = fixtures::rename_merge();
    let repo = revier_analysis::git::repository::open_repository(fixture.repo.path())
        .expect("打开仓库");

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
    let repo = revier_analysis::git::repository::open_repository(fixture.repo.path())
        .expect("打开仓库");

    let changes = revier_analysis::git::diff::commit_file_changes(&repo, &fixture.head)
        .expect("读取提交文件变更");

    let binary = changes.iter().find(|change| change.path == "assets/logo.bin").expect("存在二进制文件");
    assert_eq!(binary.status, "binary");
    assert!(binary.is_binary);
    assert!(!binary.is_previewable);
}
```

- [ ] **步骤 2：运行测试确认失败**

运行：

```powershell
cargo test -p revier-analysis --test index_git_diff
```

预期： 失败，`commit_file_changes` 和 `binary_change` fixture 不存在。

- [ ] **步骤 3：新增 binary fixture**

修改 `crates/revier-analysis/tests/fixtures.rs`，新增：

```rust
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
```

- [ ] **步骤 4：实现文件变更结构**

修改 `crates/revier-analysis/src/git/diff.rs`，追加：

```rust
#[derive(Debug, Clone)]
pub struct CommitFileChange {
    pub commit_hash: String,
    pub parent_hash: String,
    pub parent_index: usize,
    pub path: String,
    pub old_path: Option<String>,
    pub status: String,
    pub additions: u64,
    pub deletions: u64,
    pub is_binary: bool,
    pub is_previewable: bool,
    pub similarity: Option<f32>,
}

pub fn commit_file_changes(
    repo: &gix::Repository,
    commit_hash: &str,
) -> Result<Vec<CommitFileChange>, AppError> {
    let commit = resolve_commit(repo, commit_hash)?;
    let parent_ids = commit
        .parent_ids()
        .map(|parent_id| parent_id.detach())
        .collect::<Vec<_>>();

    if parent_ids.is_empty() {
        return Ok(Vec::new());
    }

    let mut changes = Vec::new();
    for (parent_index, parent_id) in parent_ids.iter().enumerate() {
        let parent = repo
            .find_commit(*parent_id)
            .map_err(|error| AppError::Repository(error.to_string()))?;
        changes.extend(pairwise_file_changes(repo, &parent, &commit, parent_index)?);
    }
    Ok(changes)
}
```

- [ ] **步骤 5：实现 pairwise tree diff 转换**

继续修改 `crates/revier-analysis/src/git/diff.rs`，追加：

```rust
fn pairwise_file_changes(
    repo: &gix::Repository,
    parent: &gix::Commit<'_>,
    commit: &gix::Commit<'_>,
    parent_index: usize,
) -> Result<Vec<CommitFileChange>, AppError> {
    let parent_tree = parent
        .tree()
        .map_err(|error| AppError::Repository(error.to_string()))?;
    let commit_tree = commit
        .tree()
        .map_err(|error| AppError::Repository(error.to_string()))?;
    let parent_iter = gix_object::TreeRefIter::from_bytes(&parent_tree.data, parent_tree.id.kind());
    let commit_iter = gix_object::TreeRefIter::from_bytes(&commit_tree.data, commit_tree.id.kind());
    let mut resource_cache = diff_resource_cache()?;
    let mut state = gix_diff::tree::State::default();
    let mut changes = Vec::new();

    gix_diff::tree_with_rewrites(
        parent_iter,
        commit_iter,
        &mut resource_cache,
        &mut state,
        &repo.objects,
        |change| {
            if let Some(file_change) = map_tree_change(parent, commit, parent_index, change) {
                changes.push(file_change);
            }
            Ok::<gix_diff::tree_with_rewrites::Action, std::io::Error>(
                std::ops::ControlFlow::Continue(()),
            )
        },
        gix_diff::tree_with_rewrites::Options {
            location: Some(gix_diff::tree::recorder::Location::Path),
            rewrites: Some(gix_diff::Rewrites::default()),
        },
    )
    .map_err(|error| AppError::Repository(error.to_string()))?;

    Ok(changes)
}
```

添加 `map_tree_change`：

```rust
fn map_tree_change(
    parent: &gix::Commit<'_>,
    commit: &gix::Commit<'_>,
    parent_index: usize,
    change: gix_diff::tree_with_rewrites::ChangeRef<'_>,
) -> Option<CommitFileChange> {
    match change {
        gix_diff::tree_with_rewrites::ChangeRef::Addition { location, .. } => Some(file_change(
            commit,
            parent,
            parent_index,
            location,
            None,
            "added",
            None,
        )),
        gix_diff::tree_with_rewrites::ChangeRef::Deletion { location, .. } => Some(file_change(
            commit,
            parent,
            parent_index,
            location,
            None,
            "deleted",
            None,
        )),
        gix_diff::tree_with_rewrites::ChangeRef::Modification { location, entry_mode, .. } => {
            let binary = matches!(entry_mode.kind(), gix_object::tree::EntryKind::BlobExecutable);
            Some(file_change(
                commit,
                parent,
                parent_index,
                location,
                None,
                if binary { "binary" } else { "modified" },
                None,
            ))
        }
        gix_diff::tree_with_rewrites::ChangeRef::Rewrite {
            source_location,
            location,
            copy: false,
            diff,
            ..
        } => Some(file_change(
            commit,
            parent,
            parent_index,
            location,
            Some(source_location),
            "renamed",
            diff.map(|stats| stats.similarity),
        )),
        _ => None,
    }
}

fn file_change(
    commit: &gix::Commit<'_>,
    parent: &gix::Commit<'_>,
    parent_index: usize,
    path: &gix::path::BStr,
    old_path: Option<&gix::path::BStr>,
    status: &str,
    similarity: Option<f32>,
) -> CommitFileChange {
    let is_binary = status == "binary";
    CommitFileChange {
        commit_hash: commit.id.to_string(),
        parent_hash: parent.id.to_string(),
        parent_index,
        path: String::from_utf8_lossy(path.as_ref()).into_owned(),
        old_path: old_path.map(|value| String::from_utf8_lossy(value.as_ref()).into_owned()),
        status: status.to_string(),
        additions: 0,
        deletions: 0,
        is_binary,
        is_previewable: !is_binary,
        similarity,
    }
}
```

如果步骤 5 中 gix `ChangeRef` 字段名与当前 `0.84.0` 编译结果不一致，执行者必须以 `cargo test` 的编译错误为准微调字段名，并保持测试语义不变；不得改用 Git CLI。

- [ ] **步骤 6：运行测试确认通过**

运行：

```powershell
cargo test -p revier-analysis --test index_git_diff
cargo test -p revier-analysis --test no_git_process
```

预期： 通过。

- [ ] **步骤 7：提交**

运行：

```powershell
git add crates/revier-analysis/src/git/diff.rs crates/revier-analysis/tests/fixtures.rs crates/revier-analysis/tests/index_git_diff.rs
git commit -m "feat: 添加 gix 文件变更索引读取"
```

## 任务 5：实现索引写入、build 和 status

**文件：**
- 修改：`crates/revier-analysis/src/index/writer.rs`
- 修改：`crates/revier-analysis/src/index/queries.rs`
- 修改：`crates/revier-analysis/src/commands/index_build.rs`
- 修改：`crates/revier-analysis/src/commands/index_status.rs`
- 修改：`crates/revier-analysis/tests/index_schema.rs`

- [ ] **步骤 1：写 build/status 集成测试**

修改 `crates/revier-analysis/tests/index_schema.rs`，追加：

```rust
mod fixtures;

use serde_json::Value;
use std::process::Command;

#[test]
fn index_build_then_status_returns_ready() {
    let fixture = fixtures::linear_with_authors();
    let dir = tempfile::tempdir().expect("创建临时目录");
    let db_path = dir.path().join("index.duckdb");

    let build = Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
        .args([
            "index",
            "build",
            "--repo",
            fixture.repo.path().to_str().expect("repo path"),
            "--db",
            db_path.to_str().expect("db path"),
            "--branch",
            "main",
            "--format",
            "json",
        ])
        .output()
        .expect("运行 index build");

    assert!(
        build.status.success(),
        "index build 应成功，stderr: {}",
        String::from_utf8_lossy(&build.stderr)
    );

    let build_json: Value = serde_json::from_slice(&build.stdout).expect("解析 build json");
    assert_eq!(build_json["version"], 1);
    assert_eq!(build_json["status"], "completed");
    assert!(build_json["indexedCommitCount"].as_u64().expect("commit count") >= 3);

    let status = Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
        .args([
            "index",
            "status",
            "--repo",
            fixture.repo.path().to_str().expect("repo path"),
            "--db",
            db_path.to_str().expect("db path"),
            "--format",
            "json",
        ])
        .output()
        .expect("运行 index status");

    assert!(status.status.success());
    let status_json: Value = serde_json::from_slice(&status.stdout).expect("解析 status json");
    assert_eq!(status_json["status"], "ready");
    assert!(status_json["indexedFileCount"].as_u64().expect("file count") >= 1);
}
```

- [ ] **步骤 2：运行测试确认失败**

运行：

```powershell
cargo test -p revier-analysis --test index_schema index_build_then_status_returns_ready
```

预期： 失败，build/status 仍返回固定 JSON。

- [ ] **步骤 3：实现 writer**

替换 `crates/revier-analysis/src/index/writer.rs`：

```rust
use crate::error::AppError;
use crate::git::commits::IndexedCommit;
use crate::git::diff::CommitFileChange;
use chrono::Utc;
use duckdb::params;

pub struct IndexWriteSummary {
    pub indexed_commit_count: u64,
    pub indexed_file_count: u64,
}

pub fn write_index(
    conn: &duckdb::Connection,
    repo_id: &str,
    commits: &[IndexedCommit],
    files: &[CommitFileChange],
    elapsed_ms: u64,
) -> Result<IndexWriteSummary, AppError> {
    let started_at = Utc::now().to_rfc3339();
    let run_id = format!("{repo_id}-{}-{}", Utc::now().timestamp_millis(), std::process::id());
    conn.execute(
        "insert into index_runs (run_id, repo_id, started_at, status, indexed_commit_count, indexed_file_count)
         values (?, ?, ?, 'running', 0, 0)",
        params![run_id, repo_id, started_at],
    )
    .map_err(|error| AppError::DuckDb(error.to_string()))?;

    for commit in commits {
        conn.execute(
            "insert or replace into commits
             (hash, short_hash, author_name, author_email, author_key, committed_at, subject, parent_count, is_merge)
             values (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                commit.hash,
                commit.short_hash,
                commit.author_name,
                commit.author_email,
                commit.author_key,
                commit.committed_at,
                commit.subject,
                commit.parents.len() as i64,
                commit.is_merge
            ],
        )
        .map_err(|error| AppError::DuckDb(error.to_string()))?;

        for (parent_index, parent_hash) in commit.parents.iter().enumerate() {
            conn.execute(
                "insert or replace into commit_parents (commit_hash, parent_hash, parent_index)
                 values (?, ?, ?)",
                params![commit.hash, parent_hash, parent_index as i64],
            )
            .map_err(|error| AppError::DuckDb(error.to_string()))?;
        }
    }

    for file in files {
        conn.execute(
            "insert or replace into commit_files
             (commit_hash, parent_hash, parent_index, path, old_path, status, additions, deletions, is_binary, is_previewable, similarity)
             values (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            params![
                file.commit_hash,
                file.parent_hash,
                file.parent_index as i64,
                file.path,
                file.old_path.clone().unwrap_or_default(),
                file.status,
                file.additions as i64,
                file.deletions as i64,
                file.is_binary,
                file.is_previewable,
                file.similarity
            ],
        )
        .map_err(|error| AppError::DuckDb(error.to_string()))?;
    }

    let finished_at = Utc::now().to_rfc3339();
    conn.execute(
        "update index_runs
         set finished_at = ?, status = 'completed', indexed_commit_count = ?, indexed_file_count = ?, elapsed_ms = ?
         where run_id = ?",
        params![finished_at, commits.len() as i64, files.len() as i64, elapsed_ms as i64, run_id],
    )
    .map_err(|error| AppError::DuckDb(error.to_string()))?;
    conn.execute(
        "insert or replace into metadata (key, value) values ('updated_at', ?)",
        params![finished_at],
    )
    .map_err(|error| AppError::DuckDb(error.to_string()))?;

    Ok(IndexWriteSummary {
        indexed_commit_count: commits.len() as u64,
        indexed_file_count: files.len() as u64,
    })
}
```

- [ ] **步骤 4：实现 status 查询**

替换 `crates/revier-analysis/src/index/queries.rs`：

```rust
use crate::error::AppError;

pub struct IndexStatusRecord {
    pub indexed_commit_count: u64,
    pub indexed_file_count: u64,
    pub updated_at: Option<String>,
}

pub fn status_record(conn: &duckdb::Connection) -> Result<IndexStatusRecord, AppError> {
    let indexed_commit_count = conn
        .query_row("select count(*) from commits", [], |row| row.get::<_, i64>(0))
        .map_err(|error| AppError::DuckDb(error.to_string()))? as u64;
    let indexed_file_count = conn
        .query_row("select count(*) from commit_files", [], |row| row.get::<_, i64>(0))
        .map_err(|error| AppError::DuckDb(error.to_string()))? as u64;
    let updated_at = conn
        .query_row("select value from metadata where key = 'updated_at'", [], |row| {
            row.get::<_, String>(0)
        })
        .ok();

    Ok(IndexStatusRecord {
        indexed_commit_count,
        indexed_file_count,
        updated_at,
    })
}
```

- [ ] **步骤 5：接入 build/status 命令**

替换 `crates/revier-analysis/src/commands/index_build.rs`：

```rust
use crate::cli::IndexBuildArgs;
use crate::error::AppError;
use crate::git::diff::CommitFileChange;
use crate::json::{IndexBuildOutput, IndexRunStatus};
use std::time::Instant;

pub fn run(args: IndexBuildArgs) -> Result<String, AppError> {
    let started = Instant::now();
    let repo = crate::git::repository::open_repository(&args.common.repo)?;
    let identity = crate::git::repository::repository_identity(&repo)?;
    let db_path = args
        .common
        .db
        .clone()
        .unwrap_or(crate::index::connection::default_database_path(&identity.repo_id)?);
    let conn = crate::index::connection::open_database(&db_path)?;
    crate::index::migrations::ensure_compatible_schema(&conn)?;
    crate::index::schema::initialize_schema(
        &conn,
        &identity.repo_id,
        &identity.repo_root,
        &identity.git_common_dir,
    )?;

    let commits = crate::git::commits::list_reachable_commits(&repo, &args.branch)?;
    let mut files = Vec::<CommitFileChange>::new();
    for commit in &commits {
        files.extend(crate::git::diff::commit_file_changes(&repo, &commit.hash)?);
    }

    let elapsed_ms = started.elapsed().as_millis() as u64;
    let summary = crate::index::writer::write_index(
        &conn,
        &identity.repo_id,
        &commits,
        &files,
        elapsed_ms,
    )?;
    let output = IndexBuildOutput {
        version: 1,
        repo_id: identity.repo_id,
        status: IndexRunStatus::Completed,
        indexed_commit_count: summary.indexed_commit_count,
        indexed_file_count: summary.indexed_file_count,
        elapsed_ms,
        warnings: Vec::new(),
    };
    crate::serialize_json(&output, args.common.pretty)
}
```

替换 `crates/revier-analysis/src/commands/index_status.rs`：

```rust
use crate::cli::IndexStatusArgs;
use crate::error::AppError;
use crate::json::{IndexStatusKind, IndexStatusOutput};

pub fn run(args: IndexStatusArgs) -> Result<String, AppError> {
    let repo = crate::git::repository::open_repository(&args.common.repo)?;
    let identity = crate::git::repository::repository_identity(&repo)?;
    let db_path = args
        .common
        .db
        .clone()
        .unwrap_or(crate::index::connection::default_database_path(&identity.repo_id)?);
    if !db_path.exists() {
        let output = IndexStatusOutput {
            version: 1,
            repo_id: identity.repo_id,
            schema_version: 0,
            status: IndexStatusKind::Missing,
            indexed_commit_count: 0,
            indexed_file_count: 0,
            updated_at: None,
            warnings: vec!["索引文件不存在".to_string()],
        };
        return crate::serialize_json(&output, args.common.pretty);
    }

    let conn = crate::index::connection::open_database(&db_path)?;
    crate::index::migrations::ensure_compatible_schema(&conn)?;
    let record = crate::index::queries::status_record(&conn)?;
    let output = IndexStatusOutput {
        version: 1,
        repo_id: identity.repo_id,
        schema_version: crate::index::migrations::CURRENT_SCHEMA_VERSION,
        status: IndexStatusKind::Ready,
        indexed_commit_count: record.indexed_commit_count,
        indexed_file_count: record.indexed_file_count,
        updated_at: record.updated_at,
        warnings: Vec::new(),
    };
    crate::serialize_json(&output, args.common.pretty)
}
```

- [ ] **步骤 6：运行 build/status 测试**

运行：

```powershell
cargo test -p revier-analysis --test index_schema
```

预期： 通过。

- [ ] **步骤 7：提交**

运行：

```powershell
git add crates/revier-analysis/src/index crates/revier-analysis/src/commands crates/revier-analysis/tests/index_schema.rs
git commit -m "feat: 实现 Rust 索引构建和状态查询"
```

## 任务 6：实现 query-files 筛选语义

**文件：**
- 修改：`crates/revier-analysis/src/index/queries.rs`
- 修改：`crates/revier-analysis/src/commands/query_files.rs`
- 修改：`crates/revier-analysis/src/json.rs`
- 新建：`crates/revier-analysis/tests/index_query_files.rs`

- [ ] **步骤 1：写 query-files 集成测试**

创建 `crates/revier-analysis/tests/index_query_files.rs`：

```rust
mod fixtures;

use serde_json::Value;
use std::process::Command;

#[test]
fn query_files_filters_by_author_message_time_and_glob() {
    let fixture = fixtures::linear_with_authors();
    let dir = tempfile::tempdir().expect("创建临时目录");
    let db_path = dir.path().join("index.duckdb");

    run_index_build(&fixture, &db_path);

    let output = Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
        .args([
            "index",
            "query-files",
            "--repo",
            fixture.repo.path().to_str().expect("repo path"),
            "--db",
            db_path.to_str().expect("db path"),
            "--base",
            &fixture.base,
            "--head",
            &fixture.head,
            "--branch",
            "main",
            "--author",
            "alice@example.com",
            "--message",
            "alice",
            "--since",
            "2026-05-01T00:00:00Z",
            "--until",
            "2026-05-31T23:59:59Z",
            "--glob",
            "src/**/*.txt",
            "--format",
            "json",
        ])
        .output()
        .expect("运行 query-files");

    assert!(
        output.status.success(),
        "query-files 应成功，stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let json: Value = serde_json::from_slice(&output.stdout).expect("解析 query json");
    assert_eq!(json["version"], 1);
    assert_eq!(json["range"]["baseCommit"], fixture.base);
    assert_eq!(json["range"]["headCommit"], fixture.head);
    assert_eq!(json["files"][0]["path"], "src/app.txt");
    assert!(json["files"][0]["oldPath"].is_null());
    assert_eq!(json["files"][0]["status"], "modified");
    assert_eq!(json["warnings"].as_array().expect("warnings").len(), 0);
}

#[test]
fn query_files_returns_exit_code_four_when_index_is_missing() {
    let fixture = fixtures::linear();
    let dir = tempfile::tempdir().expect("创建临时目录");
    let db_path = dir.path().join("missing.duckdb");

    let output = Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
        .args([
            "index",
            "query-files",
            "--repo",
            fixture.repo.path().to_str().expect("repo path"),
            "--db",
            db_path.to_str().expect("db path"),
            "--base",
            &fixture.base,
            "--head",
            &fixture.head,
            "--branch",
            "main",
            "--format",
            "json",
        ])
        .output()
        .expect("运行 query-files");

    assert_eq!(output.status.code(), Some(4));
    assert!(String::from_utf8_lossy(&output.stderr).contains("索引不可用"));
}

fn run_index_build(fixture: &fixtures::FixtureRepo, db_path: &std::path::Path) {
    let output = Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
        .args([
            "index",
            "build",
            "--repo",
            fixture.repo.path().to_str().expect("repo path"),
            "--db",
            db_path.to_str().expect("db path"),
            "--branch",
            "main",
            "--format",
            "json",
        ])
        .output()
        .expect("运行 index build");
    assert!(output.status.success());
}
```

- [ ] **步骤 2：运行测试确认失败**

运行：

```powershell
cargo test -p revier-analysis --test index_query_files
```

预期： 失败，query-files 尚未查询 DuckDB，JSON 字段仍不是 camelCase。

- [ ] **步骤 3：调整 JSON 为 camelCase**

修改 `crates/revier-analysis/src/json.rs`，为 index 输出结构加 `rename_all = "camelCase"`：

```rust
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexStatusOutput {
    pub version: u8,
    pub repo_id: String,
    pub schema_version: u32,
    pub status: IndexStatusKind,
    pub indexed_commit_count: u64,
    pub indexed_file_count: u64,
    pub updated_at: Option<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexBuildOutput {
    pub version: u8,
    pub repo_id: String,
    pub status: IndexRunStatus,
    pub indexed_commit_count: u64,
    pub indexed_file_count: u64,
    pub elapsed_ms: u64,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryFilesRangeOutput {
    pub base_commit: String,
    pub head_commit: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangedFileOutput {
    pub path: String,
    pub old_path: Option<String>,
    pub status: String,
    pub additions: u64,
    pub deletions: u64,
    pub is_binary: bool,
    pub is_previewable: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryFilesOutput {
    pub version: u8,
    pub range: QueryFilesRangeOutput,
    pub files: Vec<ChangedFileOutput>,
    pub warnings: Vec<String>,
}
```

- [ ] **步骤 4：实现筛选查询结构**

修改 `crates/revier-analysis/src/index/queries.rs`，追加：

```rust
use crate::json::ChangedFileOutput;
use globset::{Glob, GlobSet, GlobSetBuilder};

pub struct QueryFilesFilter {
    pub base: String,
    pub head: String,
    pub authors: Vec<String>,
    pub author_query: Option<String>,
    pub message: Option<String>,
    pub since: Option<String>,
    pub until: Option<String>,
    pub globs: Vec<String>,
}

pub fn query_files(
    conn: &duckdb::Connection,
    filter: &QueryFilesFilter,
) -> Result<Vec<ChangedFileOutput>, AppError> {
    ensure_range_indexed(conn, &filter.base, &filter.head)?;
    let matcher = build_glob_matcher(&filter.globs)?;
    let commits = matching_commits(conn, filter)?;
    if commits.is_empty() {
        return Ok(Vec::new());
    }

    let mut files = Vec::new();
    for commit_hash in commits {
        let mut stmt = conn
            .prepare(
                "select path, old_path, status, additions, deletions, is_binary, is_previewable
                 from commit_files
                 where commit_hash = ?
                 order by path",
            )
            .map_err(|error| AppError::DuckDb(error.to_string()))?;
        let rows = stmt
            .query_map([commit_hash], |row| {
                let old_path: String = row.get(1)?;
                Ok(ChangedFileOutput {
                    path: row.get(0)?,
                    old_path: (!old_path.is_empty()).then_some(old_path),
                    status: row.get(2)?,
                    additions: row.get::<_, i64>(3)? as u64,
                    deletions: row.get::<_, i64>(4)? as u64,
                    is_binary: row.get(5)?,
                    is_previewable: row.get(6)?,
                })
            })
            .map_err(|error| AppError::DuckDb(error.to_string()))?;

        for file in rows {
            let file = file.map_err(|error| AppError::DuckDb(error.to_string()))?;
            if path_matches(&matcher, &file.path, file.old_path.as_deref()) {
                files.push(file);
            }
        }
    }

    files.sort_by(|left, right| left.path.cmp(&right.path));
    files.dedup_by(|left, right| left.path == right.path && left.old_path == right.old_path);
    Ok(files)
}
```

- [ ] **步骤 5：实现提交筛选和 glob**

继续修改 `crates/revier-analysis/src/index/queries.rs`：

```rust
fn ensure_range_indexed(
    conn: &duckdb::Connection,
    base: &str,
    head: &str,
) -> Result<(), AppError> {
    let indexed_head: i64 = conn
        .query_row("select count(*) from commits where hash = ?", [head], |row| row.get(0))
        .map_err(|error| AppError::DuckDb(error.to_string()))?;
    if indexed_head == 0 {
        return Err(AppError::IndexUnavailable(format!(
            "查询范围 head {head} 未被索引，请先执行 index build"
        )));
    }

    let indexed_base: i64 = conn
        .query_row("select count(*) from commits where hash = ?", [base], |row| row.get(0))
        .map_err(|error| AppError::DuckDb(error.to_string()))?;
    if indexed_base == 0 {
        return Err(AppError::IndexUnavailable(format!(
            "查询范围 base {base} 未被索引，请先执行 index build"
        )));
    }
    Ok(())
}

fn matching_commits(
    conn: &duckdb::Connection,
    filter: &QueryFilesFilter,
) -> Result<Vec<String>, AppError> {
    let mut stmt = conn
        .prepare(
            "select hash, author_key, author_name, coalesce(author_email, ''), subject, cast(committed_at as varchar)
             from commits",
        )
        .map_err(|error| AppError::DuckDb(error.to_string()))?;
    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
            ))
        })
        .map_err(|error| AppError::DuckDb(error.to_string()))?;

    let normalized_authors = filter
        .authors
        .iter()
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    let author_query = normalized_query(filter.author_query.as_deref());
    let message_query = normalized_query(filter.message.as_deref());

    let mut hashes = Vec::new();
    for row in rows {
        let (hash, author_key, author_name, author_email, subject, committed_at) =
            row.map_err(|error| AppError::DuckDb(error.to_string()))?;
        if !normalized_authors.is_empty() && !normalized_authors.contains(&author_key) {
            continue;
        }
        let author_text = format!("{author_name} {author_email}").to_lowercase();
        if let Some(query) = &author_query {
            if !author_text.contains(query) {
                continue;
            }
        }
        if let Some(query) = &message_query {
            if !subject.to_lowercase().contains(query) {
                continue;
            }
        }
        if let Some(since) = &filter.since {
            if committed_at < *since {
                continue;
            }
        }
        if let Some(until) = &filter.until {
            if committed_at > *until {
                continue;
            }
        }
        hashes.push(hash);
    }
    Ok(hashes)
}

fn normalized_query(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_lowercase())
}

struct GlobMatcher {
    include: Option<GlobSet>,
    exclude: Option<GlobSet>,
}

fn build_glob_matcher(rules: &[String]) -> Result<GlobMatcher, AppError> {
    let mut include = GlobSetBuilder::new();
    let mut exclude = GlobSetBuilder::new();
    let mut include_count = 0;
    let mut exclude_count = 0;
    for rule in rules {
        if let Some(stripped) = rule.strip_prefix('!') {
            exclude.add(Glob::new(stripped).map_err(|error| AppError::InvalidArgument(error.to_string()))?);
            exclude_count += 1;
        } else {
            include.add(Glob::new(rule).map_err(|error| AppError::InvalidArgument(error.to_string()))?);
            include_count += 1;
        }
    }
    Ok(GlobMatcher {
        include: (include_count > 0)
            .then(|| include.build().map_err(|error| AppError::InvalidArgument(error.to_string())))
            .transpose()?,
        exclude: (exclude_count > 0)
            .then(|| exclude.build().map_err(|error| AppError::InvalidArgument(error.to_string())))
            .transpose()?,
    })
}

fn path_matches(matcher: &GlobMatcher, path: &str, old_path: Option<&str>) -> bool {
    let candidates = old_path.map_or_else(|| vec![path], |old| vec![path, old]);
    let included = matcher
        .include
        .as_ref()
        .map_or(true, |set| candidates.iter().any(|candidate| set.is_match(candidate)));
    let excluded = matcher
        .exclude
        .as_ref()
        .map_or(false, |set| candidates.iter().any(|candidate| set.is_match(candidate)));
    included && !excluded
}
```

- [ ] **步骤 6：接入 query-files 命令**

替换 `crates/revier-analysis/src/commands/query_files.rs`：

```rust
use crate::cli::QueryFilesArgs;
use crate::error::AppError;
use crate::index::queries::QueryFilesFilter;
use crate::json::{QueryFilesOutput, QueryFilesRangeOutput};

pub fn run(args: QueryFilesArgs) -> Result<String, AppError> {
    let repo = crate::git::repository::open_repository(&args.common.repo)?;
    let identity = crate::git::repository::repository_identity(&repo)?;
    let db_path = args
        .common
        .db
        .clone()
        .unwrap_or(crate::index::connection::default_database_path(&identity.repo_id)?);
    if !db_path.exists() {
        return Err(AppError::IndexUnavailable(format!(
            "索引文件不存在：{}",
            db_path.display()
        )));
    }

    let conn = crate::index::connection::open_database(&db_path)?;
    crate::index::migrations::ensure_compatible_schema(&conn)?;
    let files = crate::index::queries::query_files(
        &conn,
        &QueryFilesFilter {
            base: args.base.clone(),
            head: args.head.clone(),
            authors: args.authors,
            author_query: args.author_query,
            message: args.message,
            since: args.since,
            until: args.until,
            globs: args.globs,
        },
    )?;

    let output = QueryFilesOutput {
        version: 1,
        range: QueryFilesRangeOutput {
            base_commit: args.base,
            head_commit: args.head,
        },
        files,
        warnings: Vec::new(),
    };
    crate::serialize_json(&output, args.common.pretty)
}
```

- [ ] **步骤 7：运行 query-files 测试**

运行：

```powershell
cargo test -p revier-analysis --test index_query_files
cargo test -p revier-analysis --test cli_contract
cargo test -p revier-analysis --test no_git_process
```

预期： 通过。

- [ ] **步骤 8：提交**

运行：

```powershell
git add crates/revier-analysis/src/index/queries.rs crates/revier-analysis/src/commands/query_files.rs crates/revier-analysis/src/json.rs crates/revier-analysis/tests/index_query_files.rs
git commit -m "feat: 实现 Rust 索引文件查询"
```

## 任务 7：新增 Electron RustAnalysisClient

**文件：**
- 新建：`src/main/analysis/rustAnalysisClient.ts`
- 新建：`tests/unit/rustAnalysisClient.test.ts`

- [ ] **步骤 1：写客户端单元测试**

创建 `tests/unit/rustAnalysisClient.test.ts`：

```ts
import { RustAnalysisClient, type RustAnalysisExecutor } from '../../src/main/analysis/rustAnalysisClient';

describe('RustAnalysisClient', () => {
  it('assembles query-files arguments and parses changed files', async () => {
    const executor: RustAnalysisExecutor = vi.fn(async () => ({
      exitCode: 0,
      stdout: JSON.stringify({
        version: 1,
        range: { baseCommit: 'base', headCommit: 'head' },
        files: [
          {
            path: 'src/app.ts',
            oldPath: null,
            status: 'modified',
            additions: 2,
            deletions: 1,
            isBinary: false,
            isPreviewable: true
          }
        ],
        warnings: []
      }),
      stderr: ''
    }));
    const client = new RustAnalysisClient({ binaryPath: 'revier-analysis', executor });

    const result = await client.queryFiles({
      repoPath: 'E:/repo/app',
      baseCommit: 'base',
      headCommit: 'head',
      branch: 'main',
      authorKeys: ['alice@example.com'],
      messageQuery: 'fix',
      globRules: ['src/**/*.ts']
    });

    expect(executor).toHaveBeenCalledWith('revier-analysis', [
      'index',
      'query-files',
      '--repo',
      'E:/repo/app',
      '--base',
      'base',
      '--head',
      'head',
      '--branch',
      'main',
      '--author',
      'alice@example.com',
      '--message',
      'fix',
      '--glob',
      'src/**/*.ts',
      '--format',
      'json'
    ]);
    expect(result.files).toEqual([
      {
        path: 'src/app.ts',
        oldPath: undefined,
        status: 'modified',
        additions: 2,
        deletions: 1,
        isBinary: false,
        isPreviewable: true
      }
    ]);
  });

  it('marks index exit codes as recoverable fallback errors', async () => {
    const executor: RustAnalysisExecutor = vi.fn(async () => ({
      exitCode: 4,
      stdout: '',
      stderr: '索引不可用：索引文件不存在'
    }));
    const client = new RustAnalysisClient({ binaryPath: 'revier-analysis', executor });

    await expect(
      client.queryFiles({
        repoPath: 'E:/repo/app',
        baseCommit: 'base',
        headCommit: 'head',
        branch: 'main',
        globRules: []
      })
    ).rejects.toMatchObject({
      recoverable: true,
      code: 'RUST_INDEX_UNAVAILABLE'
    });
  });
});
```

- [ ] **步骤 2：运行测试确认失败**

运行：

```powershell
pnpm test -- tests/unit/rustAnalysisClient.test.ts
```

预期： 失败，`RustAnalysisClient` 不存在。

- [ ] **步骤 3：实现客户端**

创建 `src/main/analysis/rustAnalysisClient.ts`：

```ts
import { spawn } from 'node:child_process';
import { join } from 'node:path';
import type { ChangedFile, ReviewFilters } from '../../shared/reviewTypes';

export interface RustAnalysisExecutorResult {
  exitCode: number;
  stdout: string;
  stderr: string;
}

export type RustAnalysisExecutor = (
  binaryPath: string,
  args: string[]
) => Promise<RustAnalysisExecutorResult>;

export interface QueryFilesRequest {
  repoPath: string;
  baseCommit: string;
  headCommit: string;
  branch: string;
  startAt?: string;
  endAt?: string;
  authorKeys?: string[];
  authorQuery?: string;
  messageQuery?: string;
  globRules: string[];
}

export interface QueryFilesResult {
  files: ChangedFile[];
  warnings: string[];
}

export class RustAnalysisError extends Error {
  constructor(
    public readonly code: string,
    message: string,
    public readonly recoverable: boolean
  ) {
    super(message);
  }
}

export class RustAnalysisClient {
  private readonly binaryPath: string;
  private readonly executor: RustAnalysisExecutor;

  constructor(options: { binaryPath?: string; executor?: RustAnalysisExecutor } = {}) {
    this.binaryPath = options.binaryPath ?? defaultBinaryPath();
    this.executor = options.executor ?? spawnExecutor;
  }

  async queryFiles(request: QueryFilesRequest): Promise<QueryFilesResult> {
    const args = buildQueryFilesArgs(request);
    const result = await this.executor(this.binaryPath, args);
    if (result.exitCode !== 0) {
      throw mapRustError(result.exitCode, result.stderr);
    }

    const parsed = JSON.parse(result.stdout) as {
      files: Array<ChangedFile & { oldPath?: string | null }>;
      warnings?: string[];
    };
    return {
      files: parsed.files.map((file) => ({
        ...file,
        oldPath: file.oldPath ?? undefined
      })),
      warnings: parsed.warnings ?? []
    };
  }
}

export function buildQueryFilesArgs(request: QueryFilesRequest): string[] {
  const args = [
    'index',
    'query-files',
    '--repo',
    request.repoPath,
    '--base',
    request.baseCommit,
    '--head',
    request.headCommit,
    '--branch',
    request.branch
  ];

  for (const author of request.authorKeys ?? []) {
    args.push('--author', author);
  }
  if (request.authorQuery?.trim()) {
    args.push('--author-query', request.authorQuery.trim());
  }
  if (request.messageQuery?.trim()) {
    args.push('--message', request.messageQuery.trim());
  }
  if (request.startAt) {
    args.push('--since', request.startAt);
  }
  if (request.endAt) {
    args.push('--until', request.endAt);
  }
  for (const glob of request.globRules) {
    args.push('--glob', glob);
  }
  args.push('--format', 'json');
  return args;
}

function mapRustError(exitCode: number, stderr: string): RustAnalysisError {
  const message = stderr.trim() || `Rust 分析进程退出码 ${exitCode}`;
  if ([4, 5, 6].includes(exitCode)) {
    return new RustAnalysisError('RUST_INDEX_UNAVAILABLE', message, true);
  }
  if (exitCode === 2 || exitCode === 3) {
    return new RustAnalysisError('RUST_QUERY_FAILED', message, true);
  }
  return new RustAnalysisError('RUST_INTERNAL_ERROR', message, false);
}

function defaultBinaryPath(): string {
  return process.env.REVIER_ANALYSIS_BIN ?? join(process.cwd(), 'target', 'debug', process.platform === 'win32' ? 'revier-analysis.exe' : 'revier-analysis');
}

function spawnExecutor(binaryPath: string, args: string[]): Promise<RustAnalysisExecutorResult> {
  return new Promise((resolve) => {
    const child = spawn(binaryPath, args, { windowsHide: true });
    const stdout: Buffer[] = [];
    const stderr: Buffer[] = [];
    child.stdout.on('data', (chunk: Buffer) => stdout.push(chunk));
    child.stderr.on('data', (chunk: Buffer) => stderr.push(chunk));
    child.on('error', (error) => {
      resolve({ exitCode: 4, stdout: '', stderr: error.message });
    });
    child.on('close', (exitCode) => {
      resolve({
        exitCode: exitCode ?? 10,
        stdout: Buffer.concat(stdout).toString('utf8'),
        stderr: Buffer.concat(stderr).toString('utf8')
      });
    });
  });
}
```

- [ ] **步骤 4：运行测试确认通过**

运行：

```powershell
pnpm test -- tests/unit/rustAnalysisClient.test.ts
```

预期： 通过。

- [ ] **步骤 5：提交**

运行：

```powershell
git add src/main/analysis/rustAnalysisClient.ts tests/unit/rustAnalysisClient.test.ts
git commit -m "feat: 添加 Electron Rust 分析客户端"
```

## 任务 8：接入 `resolveAnalysisScope()` 并保留降级路径

**文件：**
- 修改：`src/main/ipc/reviewIpc.ts`
- 修改：`tests/unit/reviewIpc.test.ts`

- [ ] **步骤 1：写 Rust 查询成功和降级测试**

修改 `tests/unit/reviewIpc.test.ts`，在现有 `describe('reviewIpc')` 内追加：

```ts
  it('uses Rust query-files when display commit filters are present', async () => {
    const filters: ReviewFilters = {
      projectId: project.id,
      branch: 'main',
      startAt: '2026-05-01T00:00:00.000Z',
      endAt: '2026-05-31T00:00:00.000Z',
      authorKeys: ['alice@example.com'],
      globRules: ['src/**/*.ts']
    };
    const rustQueryFiles = vi.fn(async () => ({
      files: [modifiedFile],
      warnings: []
    }));
    const listChangedFiles = vi.fn(async () => [readmeFile]);

    const result = await resolveAnalysisScope({
      project,
      filters,
      git: {
        listCommits: vi.fn(async () => [
          commit('head', '2026-05-20T00:00:00.000Z', 'Alice', 'feature'),
          commit('base', '2026-05-01T00:00:00.000Z', 'Base', 'base')
        ]),
        listChangedFiles,
        readFileAtCommit: vi.fn(),
        showFilePatch: vi.fn()
      },
      rust: {
        queryFiles: rustQueryFiles
      }
    });

    expect(rustQueryFiles).toHaveBeenCalledWith({
      repoPath: project.repoPath,
      baseCommit: 'base',
      headCommit: 'head',
      branch: 'main',
      startAt: '2026-05-01T00:00:00.000Z',
      endAt: '2026-05-31T00:00:00.000Z',
      authorKeys: ['alice@example.com'],
      authorQuery: undefined,
      messageQuery: undefined,
      globRules: ['src/**/*.ts']
    });
    expect(listChangedFiles).not.toHaveBeenCalled();
    expect(result.files).toEqual([modifiedFile]);
  });

  it('falls back to TypeScript filtering when Rust query-files is recoverable', async () => {
    const filters: ReviewFilters = {
      projectId: project.id,
      branch: 'main',
      startAt: '2026-05-01T00:00:00.000Z',
      endAt: '2026-05-31T00:00:00.000Z',
      authorKeys: ['a@example.com'],
      globRules: ['src/**/*.ts']
    };
    const rustError = Object.assign(new Error('索引不可用'), { recoverable: true });

    const result = await resolveAnalysisScope({
      project,
      filters,
      git: {
        listCommits: vi.fn(async () => [
          commit('head', '2026-05-20T00:00:00.000Z', 'Bob', 'fix bug'),
          commit('alice', '2026-05-10T00:00:00.000Z', 'Alice', 'feature: update app'),
          commit('base', '2026-05-01T00:00:00.000Z', 'Base', 'base')
        ]),
        listChangedFiles: vi.fn(async () => [modifiedFile]),
        readFileAtCommit: vi.fn(),
        showFilePatch: vi.fn(async () => '@@ -1 +1 @@\n-old\n+new\n')
      },
      rust: {
        queryFiles: vi.fn(async () => {
          throw rustError;
        })
      }
    });

    expect(result.files).toEqual([modifiedFile]);
  });
```

- [ ] **步骤 2：运行测试确认失败**

运行：

```powershell
pnpm test -- tests/unit/reviewIpc.test.ts
```

预期： 失败，`resolveAnalysisScope` 尚不接收 `rust` 参数。

- [ ] **步骤 3：修改 `reviewIpc.ts` 接口和接入点**

修改 `src/main/ipc/reviewIpc.ts`：

```ts
import { RustAnalysisClient, type QueryFilesResult } from '../analysis/rustAnalysisClient';
```

新增接口：

```ts
interface RustAnalysisQueryClient {
  queryFiles(request: {
    repoPath: string;
    baseCommit: string;
    headCommit: string;
    branch: string;
    startAt?: string;
    endAt?: string;
    authorKeys?: string[];
    authorQuery?: string;
    messageQuery?: string;
    globRules: string[];
  }): Promise<QueryFilesResult>;
}
```

修改 `ResolveAnalysisScopeInput`：

```ts
interface ResolveAnalysisScopeInput {
  project: ReviewProject;
  filters: ReviewFilters;
  git: AnalysisGitClient;
  rust?: RustAnalysisQueryClient;
}
```

替换 `resolveAnalysisScope()` 的文件加载逻辑：

```ts
export async function resolveAnalysisScope({
  project,
  filters,
  git,
  rust
}: ResolveAnalysisScopeInput): Promise<ResolvedAnalysisScope> {
  const commits = await git.listCommits(project.repoPath, filters.branch);
  const range = selectRangeCommits(commits, filters);
  const matcher = createGlobMatcher(filters.globRules);
  const rangeCommits = mapRangeCommits(commits, range);

  if (hasDisplayCommitFilters(filters) && rust) {
    try {
      const result = await rust.queryFiles({
        repoPath: project.repoPath,
        baseCommit: range.baseCommit,
        headCommit: range.headCommit,
        branch: filters.branch,
        startAt: range.startAt,
        endAt: range.endAt,
        authorKeys: filters.authorKeys,
        authorQuery: filters.authorQuery,
        messageQuery: filters.messageQuery,
        globRules: filters.globRules
      });
      return { range, files: result.files, rangeCommits };
    } catch (error) {
      if (!isRecoverableRustError(error)) {
        throw error;
      }
    }
  }

  let files = (await git.listChangedFiles(project.repoPath, range.baseCommit, range.headCommit)).filter(
    (file) => matcher(file.path)
  );

  if (hasDisplayCommitFilters(filters)) {
    files = await filterFilesByMatchingCommits(project.repoPath, files, rangeCommits, filters, git);
  }

  return { range, files, rangeCommits };
}
```

追加辅助函数：

```ts
function isRecoverableRustError(error: unknown): boolean {
  return Boolean(error && typeof error === 'object' && 'recoverable' in error && error.recoverable);
}
```

在 `registerReviewIpc()` 中创建客户端并传入：

```ts
const rust = new RustAnalysisClient();
```

替换调用：

```ts
const scope = await resolveAnalysisScope({ project, filters, git, rust });
```

- [ ] **步骤 4：运行测试确认通过**

运行：

```powershell
pnpm test -- tests/unit/reviewIpc.test.ts tests/unit/rustAnalysisClient.test.ts
```

预期： 通过。

- [ ] **步骤 5：提交**

运行：

```powershell
git add src/main/ipc/reviewIpc.ts tests/unit/reviewIpc.test.ts
git commit -m "feat: 接入 Rust 文件列表查询降级路径"
```

## 任务 9：添加调试脚本和端到端验证命令

**文件：**
- 修改：`package.json`
- 修改：`crates/revier-analysis/tests/cli_contract.rs`

- [ ] **步骤 1：写 package 脚本测试**

修改 `tests/unit/scaffold.test.ts`，追加：

```ts
import { readFileSync } from 'node:fs';
import { join } from 'node:path';

it('exposes Rust index debug scripts', () => {
  const pkg = JSON.parse(readFileSync(join(process.cwd(), 'package.json'), 'utf8')) as {
    scripts: Record<string, string>;
  };

  expect(pkg.scripts['rust:index:status']).toBe('cargo run -p revier-analysis -- index status');
  expect(pkg.scripts['rust:index:build']).toBe('cargo run -p revier-analysis -- index build');
  expect(pkg.scripts['rust:index:query-files']).toBe('cargo run -p revier-analysis -- index query-files');
});
```

- [ ] **步骤 2：运行测试确认失败**

运行：

```powershell
pnpm test -- tests/unit/scaffold.test.ts
```

预期： 失败，脚本尚未添加。

- [ ] **步骤 3：添加 package 脚本**

修改 `package.json` 的 `scripts`：

```json
{
  "rust:index:status": "cargo run -p revier-analysis -- index status",
  "rust:index:build": "cargo run -p revier-analysis -- index build",
  "rust:index:query-files": "cargo run -p revier-analysis -- index query-files"
}
```

保留现有脚本，不调整顺序以外的字段。

- [ ] **步骤 4：运行脚本测试确认通过**

运行：

```powershell
pnpm test -- tests/unit/scaffold.test.ts
```

预期： 通过。

- [ ] **步骤 5：运行全量验证**

运行：

```powershell
cargo test --workspace
pnpm typecheck
pnpm test
```

预期： 三个命令全部 通过。

- [ ] **步骤 6：提交**

运行：

```powershell
git add package.json tests/unit/scaffold.test.ts
git commit -m "chore: 添加 Rust 索引调试脚本"
```

## 任务 10：性能记录和最终验收

**文件：**
- 新建：`docs/superpowers/verification/2026-07-01-rust-index-query-files.md`

- [ ] **步骤 1：运行最终验证命令**

运行：

```powershell
cargo test --workspace
pnpm typecheck
pnpm test
```

预期： 三个命令全部 通过。

- [ ] **步骤 2：执行一次真实 CLI smoke**

在当前仓库上运行：

```powershell
$Base = git -C E:/Projects/revier rev-parse HEAD~1
$Head = git -C E:/Projects/revier rev-parse HEAD
cargo run -p revier-analysis -- index status --repo E:/Projects/revier --format json
cargo run -p revier-analysis -- index build --repo E:/Projects/revier --branch main --format json
cargo run -p revier-analysis -- index query-files --repo E:/Projects/revier --base $Base --head $Head --branch main --format json
```

预期：

- `status` 在未 build 前返回 `missing` 或 build 后返回 `ready`。
- `build` 返回 `status: "completed"`。
- `query-files` 返回 `version: 1`、`range.baseCommit`、`range.headCommit` 和 `files` 数组。

- [ ] **步骤 3：创建验证记录文件**

创建 `docs/superpowers/verification/2026-07-01-rust-index-query-files.md`，写入步骤 1 和步骤 2 的命令、退出状态、关键 stdout 摘要和耗时。文件必须包含以下章节：

- `# Rust 索引文件查询验证记录`
- `## 环境`：日期、Shell、Node、pnpm、Rust。
- `## 功能验证`：`cargo test --workspace`、`pnpm typecheck`、`pnpm test` 的退出状态和关键输出。
- `## CLI 冒烟验证`：`index status`、`index build`、`index query-files` 的命令和 JSON 摘要。
- `## 性能记录`：TypeScript 原筛选、Rust 冷索引、Rust 已建索引查询、Rust CLI 启动与 JSON 解析耗时。
- `## 结论`：说明已建索引查询是否达到 500ms 参考目标；若未达到，写明慢点位于 CLI 启动、SQL 查询还是 Electron JSON 解析。

不得把失败命令记录为通过；若有失败，先修复代码并重新运行对应命令。

- [ ] **步骤 4：提交**

运行：

```powershell
git add docs/superpowers/verification/2026-07-01-rust-index-query-files.md
git commit -m "docs: 记录 Rust 索引文件查询验证结果"
```

## 最终验收清单

- [ ] `cargo run -p revier-analysis -- index status --repo E:/Projects/revier --format json` 可运行。
- [ ] `cargo run -p revier-analysis -- index build --repo E:/Projects/revier --branch main --format json` 可运行。
- [ ] 使用 `$Base = git -C E:/Projects/revier rev-parse HEAD~1` 和 `$Head = git -C E:/Projects/revier rev-parse HEAD` 后，`cargo run -p revier-analysis -- index query-files --repo E:/Projects/revier --base $Base --head $Head --branch main --format json` 可运行。
- [ ] Rust 生产源码通过 `tests/no_git_process.rs`，未调用 Git 进程。
- [ ] DuckDB 中存在 `metadata`、`commits`、`commit_parents`、`commit_files`、`index_runs`。
- [ ] `query-files` 输出字段为 camelCase，文件对象兼容 TypeScript `ChangedFile`。
- [ ] Electron 在作者、作者搜索或提交信息筛选存在时调用 Rust 查询。
- [ ] Rust 索引不可用、schema 不兼容、DuckDB 打开失败、二进制不可用时降级到 TypeScript 路径。
- [ ] overlay 打开路径未迁移，仍使用现有 TypeScript overlay 和归因逻辑。
- [ ] `cargo test --workspace` 通过。
- [ ] `pnpm typecheck` 通过。
- [ ] `pnpm test` 通过。

## 自审记录

- 规约目标 1 到 7 分别覆盖在任务 1 到任务 8。
- 非目标均保留：本计划没有实现 overlay、trace-block、blame、merge trace、后台服务或远程 API。
- DuckDB 绑定风险通过“执行前确认点”和任务 2 前置确认控制。
- 生产路径不调用 Git 进程由任务 3、任务 4、任务 6 的 `no_git_process` 验证控制。
- JSON camelCase 契约由任务 6 的 `index_query_files` 集成测试控制。
- Electron 降级行为由任务 8 的 `reviewIpc` 单元测试控制。
