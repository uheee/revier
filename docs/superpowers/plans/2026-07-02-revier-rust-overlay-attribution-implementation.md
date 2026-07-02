# Revier Rust Overlay 与归因准确性迁移实施计划

> **面向自动化执行代理：** 必须使用 `superpowers:subagent-driven-development`（推荐）或 `superpowers:executing-plans` 按任务逐项执行本计划。步骤使用复选框（`- [ ]`）语法跟踪。

**目标：** 在现有 `crates/revier-analysis` Rust CLI 中实现 `file-overlay` 与 `trace-block`，输出兼容现有 `FileOverlay`、`DiffBlock`、`RelatedCommit` 的 JSON，并让 Electron 主进程可通过显式开关逐步切到 Rust overlay。

**架构：** Rust 侧新增 `commands/file_overlay.rs`、`commands/trace_block.rs`、`overlay/*`、`attribution/*`，继续复用现有 `git/*` 与 `index/*`。Rust CLI 负责读取 Git 对象、构建 side-by-side rows、切分 blocks、计算块级归因并输出稳定 JSON；Electron 侧只扩展 `RustAnalysisClient` 与 `reviewIpc` 的可降级编排边界，默认仍走 TypeScript overlay。

**技术栈：** Rust stable、Cargo workspace、clap、serde、serde_json、thiserror、gix 0.84、gix-blame、gix-diff、DuckDB Rust 绑定、TypeScript、Electron、Vitest、pnpm、fnm、pwsh。

---

## 来源规约

- 设计文档：`docs/superpowers/specs/2026-07-01-revier-rust-overlay-attribution-design.md`
- 现有 Rust 索引计划：`docs/superpowers/plans/2026-07-01-revier-rust-index-query-files-implementation.md`
- 现有共享类型：`src/shared/reviewTypes.ts`
- 现有 TypeScript overlay：`src/main/analysis/overlayEngine.ts`
- 现有 TypeScript 归因：`src/analysis-core/attribution/attributionService.ts`
- 现有 Rust CLI：`crates/revier-analysis/src/cli.rs`

## 范围

本计划实现：

- 顶层 CLI 命令 `revier-analysis file-overlay`。
- 顶层 CLI 命令 `revier-analysis trace-block`。
- Rust 侧 `FileOverlay` 兼容 JSON 契约，字段使用 camelCase。
- Rust 侧文本文件读取、range diff、side-by-side rows 与 block 切分。
- Rust 侧新增/修改块 blame 归因、merge trace、删除 trace、patch inference、路径历史 warning。
- Rust 侧优先读取 DuckDB `commits`、`commit_parents`、`commit_files`，缺失时即时用 gix 计算。
- Electron 主进程通过 `REVIER_RUST_OVERLAY=1` 显式开关调用 Rust overlay，失败时降级 TypeScript overlay。
- `trace-block` 复用 `file-overlay` 的 diff 和归因实现，只返回单块诊断 JSON。
- Rust、TypeScript 单元测试与 Git fixture 测试。
- 最终验证记录文档。

本计划不实现：

- Renderer、Pinia store 或 Vue diff 组件重写。
- 后台常驻服务、N-API 或 worker 池。
- 语义级函数、类或 AST 归因。
- 移除 TypeScript overlay。
- 默认开启 Rust overlay。
- 完整高级归因缓存表，例如 `block_attributions`。

## 执行前确认点

开始实现前，执行者必须向用户确认以下决策。未确认前不得修改源码。

1. Rust 行 diff 不新增第三方依赖，使用 `overlay/line_diff.rs` 中的小型 LCS 实现。理由：规约要求新增 Rust 依赖需确认；本阶段只需要稳定行级 rows 与 blocks。
2. Rust `wordChanges` 第一版只在整行修改时输出两段粗粒度变化，旧文本为 `removed`、新文本为 `added`。理由：`wordChanges` 在 `SideBySideDiffRow` 中是可选字段，Rust overlay 默认不开启，可先保证契约和归因准确性。
3. Electron 开关使用环境变量 `REVIER_RUST_OVERLAY=1`。理由：现有项目已有 `REVIER_ANALYSIS_BIN` 环境变量，主进程内部开关可测试且默认不影响用户可见行为。
4. `file-overlay --require-index` 在索引不可用时返回退出码 `5`；现有 `index query-files` 缺索引退出码 `4` 保持不变。理由：避免破坏已存在索引测试，同时满足 overlay 规约对 `--require-index` 的要求。
5. 本计划不新增 Rust crates。若执行中发现 gix API 不足以构建某个 hunk 或 blame 场景，必须暂停并提交新的依赖或算法方案供确认。

## 文件结构

- 修改：`crates/revier-analysis/src/cli.rs`
  - 增加顶层 `file-overlay`、`trace-block` 参数结构。
- 修改：`crates/revier-analysis/src/lib.rs`
  - 分发两个新命令。
- 修改：`crates/revier-analysis/src/error.rs`
  - 增加文件不可分析、require-index 不可用和分析内部错误类型。
- 修改：`crates/revier-analysis/src/json.rs`
  - 增加 overlay、block、row、commit、trace-block JSON 契约类型。
- 修改：`crates/revier-analysis/src/commands/mod.rs`
  - 导出 `file_overlay`、`trace_block`。
- 新建：`crates/revier-analysis/src/commands/file_overlay.rs`
  - 编排参数、仓库、索引上下文、overlay 构建和 JSON 输出。
- 新建：`crates/revier-analysis/src/commands/trace_block.rs`
  - 编排单块定位和诊断 JSON 输出。
- 修改：`crates/revier-analysis/src/git/mod.rs`
  - 导出 `blob`。
- 新建：`crates/revier-analysis/src/git/blob.rs`
  - 读取指定 commit/path 的 blob 文本、判断二进制和缺失文件。
- 修改：`crates/revier-analysis/src/git/diff.rs`
  - 增加 base..head 目标文件状态解析和路径候选能力。
- 修改：`crates/revier-analysis/src/git/commits.rs`
  - 增加单 commit 元数据读取、base..head 范围 hash 计算。
- 修改：`crates/revier-analysis/src/index/queries.rs`
  - 增加 commit 元数据、parent、commit_files 路径候选查询。
- 新建：`crates/revier-analysis/src/overlay/mod.rs`
  - overlay 模块出口。
- 新建：`crates/revier-analysis/src/overlay/line_diff.rs`
  - 行级 LCS diff。
- 新建：`crates/revier-analysis/src/overlay/rows.rs`
  - side-by-side row 构建。
- 新建：`crates/revier-analysis/src/overlay/blocks.rs`
  - changed rows 分块和 block id。
- 新建：`crates/revier-analysis/src/overlay/diff_builder.rs`
  - 对外 diff builder。
- 新建：`crates/revier-analysis/src/overlay/file_overlay.rs`
  - 文件级 overlay 构建入口。
- 新建：`crates/revier-analysis/src/attribution/mod.rs`
  - 归因模块出口。
- 新建：`crates/revier-analysis/src/attribution/context.rs`
  - 范围提交、筛选、索引和 Git 查询上下文。
- 新建：`crates/revier-analysis/src/attribution/commit_lookup.rs`
  - 从索引优先读取提交元数据，缺失时走 gix。
- 新建：`crates/revier-analysis/src/attribution/patch_inference.rs`
  - 用行 diff 生成 touched ranges，作为 patch inference。
- 新建：`crates/revier-analysis/src/attribution/blame.rs`
  - 新增/修改块 blame 归因。
- 新建：`crates/revier-analysis/src/attribution/merge_trace.rs`
  - merge commit pairwise 追踪。
- 新建：`crates/revier-analysis/src/attribution/deletion_trace.rs`
  - 删除块追踪和内容指纹。
- 新建：`crates/revier-analysis/src/attribution/path_history.rs`
  - 当前路径、old path、索引 old_path、gix rewrite path 候选合并。
- 修改：`crates/revier-analysis/tests/fixtures.rs`
  - 增加删除 merge、多父歧义、非预览二进制、rename 删除等 fixture。
- 修改：`crates/revier-analysis/tests/cli_contract.rs`
  - 增加新命令 help 契约。
- 新建：`crates/revier-analysis/tests/overlay_rows.rs`
  - 验证 Rust rows 与 blocks。
- 新建：`crates/revier-analysis/tests/file_overlay_cli.rs`
  - 验证 `file-overlay` JSON、退出码、require-index。
- 新建：`crates/revier-analysis/tests/overlay_attribution.rs`
  - 验证 blame、merge trace、deletion trace、rename warning。
- 新建：`crates/revier-analysis/tests/trace_block_cli.rs`
  - 验证 `trace-block`。
- 修改：`crates/revier-analysis/tests/no_git_process.rs`
  - 保持生产 Rust 源码不调用 Git 进程。
- 修改：`src/main/analysis/rustAnalysisClient.ts`
  - 增加 `getFileOverlay()`、`traceBlock()`、参数构建和错误码映射。
- 修改：`src/main/ipc/reviewIpc.ts`
  - 在 `reviewGetFileOverlay` 中按开关尝试 Rust overlay，失败降级。
- 修改：`tests/unit/rustAnalysisClient.test.ts`
  - 增加 overlay 与 trace-block 客户端测试。
- 修改：`tests/unit/reviewIpc.test.ts`
  - 增加 Rust overlay 成功、失败降级、默认不开启测试。
- 修改：`tests/unit/scaffold.test.ts`
  - 增加 package script 测试。
- 修改：`package.json`
  - 增加 `rust:file-overlay`、`rust:trace-block` 调试脚本。
- 新建：`docs/superpowers/verification/2026-07-02-rust-overlay-attribution.md`
  - 最终验证记录。

## 任务 1：扩展 CLI、错误码和 JSON 契约骨架

**文件：**
- 修改：`crates/revier-analysis/src/cli.rs`
- 修改：`crates/revier-analysis/src/lib.rs`
- 修改：`crates/revier-analysis/src/error.rs`
- 修改：`crates/revier-analysis/src/json.rs`
- 修改：`crates/revier-analysis/src/commands/mod.rs`
- 新建：`crates/revier-analysis/src/commands/file_overlay.rs`
- 新建：`crates/revier-analysis/src/commands/trace_block.rs`
- 修改：`crates/revier-analysis/tests/cli_contract.rs`

- [ ] **步骤 1：写失败的 CLI help 测试**

修改 `crates/revier-analysis/tests/cli_contract.rs`，追加：

```rust
#[test]
fn top_level_help_lists_file_overlay_and_trace_block_commands() {
    let output = Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
        .arg("--help")
        .output()
        .expect("运行 revier-analysis --help");

    assert!(
        output.status.success(),
        "--help 应成功，stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("file-overlay"));
    assert!(stdout.contains("trace-block"));
}

#[test]
fn file_overlay_help_lists_required_arguments() {
    let output = Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
        .args(["file-overlay", "--help"])
        .output()
        .expect("运行 revier-analysis file-overlay --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--repo"));
    assert!(stdout.contains("--base"));
    assert!(stdout.contains("--head"));
    assert!(stdout.contains("--branch"));
    assert!(stdout.contains("--file"));
    assert!(stdout.contains("--require-index"));
}

#[test]
fn trace_block_help_lists_block_and_line_arguments() {
    let output = Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
        .args(["trace-block", "--help"])
        .output()
        .expect("运行 revier-analysis trace-block --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("--block-id"));
    assert!(stdout.contains("--old-start"));
    assert!(stdout.contains("--new-start"));
}
```

- [ ] **步骤 2：运行测试确认失败**

运行：

```powershell
cargo test -p revier-analysis --test cli_contract top_level_help_lists_file_overlay_and_trace_block_commands
cargo test -p revier-analysis --test cli_contract file_overlay_help_lists_required_arguments
cargo test -p revier-analysis --test cli_contract trace_block_help_lists_block_and_line_arguments
```

预期：失败，`file-overlay` 与 `trace-block` 尚不存在。

- [ ] **步骤 3：扩展 `cli.rs`**

修改 `crates/revier-analysis/src/cli.rs`：

```rust
#[derive(Debug, Subcommand)]
pub enum Command {
    Spike(SpikeCommand),
    Index(IndexCommand),
    FileOverlay(FileOverlayArgs),
    TraceBlock(TraceBlockArgs),
}

#[derive(Debug, Args)]
pub struct OverlayCommonArgs {
    #[arg(long)]
    pub repo: PathBuf,

    #[arg(long)]
    pub db: Option<PathBuf>,

    #[arg(long)]
    pub base: String,

    #[arg(long)]
    pub head: String,

    #[arg(long)]
    pub branch: String,

    #[arg(long = "glob")]
    pub globs: Vec<String>,

    #[arg(long = "author")]
    pub authors: Vec<String>,

    #[arg(long = "author-query")]
    pub author_query: Option<String>,

    #[arg(long = "message")]
    pub message: Option<String>,

    #[arg(long = "require-index")]
    pub require_index: bool,

    #[arg(long, default_value = "json")]
    pub format: OutputFormat,

    #[arg(long)]
    pub pretty: bool,
}

#[derive(Debug, Parser)]
pub struct FileOverlayArgs {
    #[command(flatten)]
    pub common: OverlayCommonArgs,

    #[arg(long)]
    pub file: String,
}

#[derive(Debug, Parser)]
pub struct TraceBlockArgs {
    #[command(flatten)]
    pub common: OverlayCommonArgs,

    #[arg(long)]
    pub file: String,

    #[arg(long = "block-id")]
    pub block_id: Option<String>,

    #[arg(long = "old-start")]
    pub old_start: Option<usize>,

    #[arg(long = "old-end")]
    pub old_end: Option<usize>,

    #[arg(long = "new-start")]
    pub new_start: Option<usize>,

    #[arg(long = "new-end")]
    pub new_end: Option<usize>,
}
```

- [ ] **步骤 4：扩展 `error.rs`**

修改 `crates/revier-analysis/src/error.rs`：

```rust
#[derive(Debug, Error)]
pub enum AppError {
    #[error("参数错误：{0}")]
    InvalidArgument(String),

    #[error("仓库读取失败：{0}")]
    Repository(String),

    #[error("文件不可分析：{0}")]
    FileNotAnalyzable(String),

    #[error("索引不可用：{0}")]
    IndexUnavailable(String),

    #[error("索引不可用且命令要求必须使用索引：{0}")]
    RequiredIndexUnavailable(String),

    #[error("索引 schema 不兼容：{0}")]
    SchemaIncompatible(String),

    #[error("DuckDB 读写失败：{0}")]
    DuckDb(String),

    #[error("gix 能力验证失败：{0}")]
    Spike(String),

    #[error("分析内部错误：{0}")]
    Analysis(String),

    #[error("JSON 输出失败：{0}")]
    Json(#[from] serde_json::Error),
}

impl AppError {
    pub fn exit_code(&self) -> i32 {
        match self {
            Self::InvalidArgument(_) => 2,
            Self::Repository(_) => 3,
            Self::FileNotAnalyzable(_) | Self::IndexUnavailable(_) => 4,
            Self::RequiredIndexUnavailable(_) | Self::SchemaIncompatible(_) => 5,
            Self::DuckDb(_) => 6,
            Self::Spike(_) | Self::Analysis(_) | Self::Json(_) => 10,
        }
    }
}
```

- [ ] **步骤 5：扩展 `json.rs` 契约类型**

修改 `crates/revier-analysis/src/json.rs`，追加：

```rust
#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisRangeOutput {
    pub branch: String,
    pub base_commit: String,
    pub head_commit: String,
    pub start_at: Option<String>,
    pub end_at: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AuthorOutput {
    pub name: String,
    pub email: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AttributionWarningOutput {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BlockAttributionOutput {
    pub confidence: String,
    pub warnings: Vec<AttributionWarningOutput>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RelatedCommitAttributionOutput {
    pub method: String,
    pub via_merge_hashes: Vec<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TouchedRangeOutput {
    pub old_start: Option<usize>,
    pub old_end: Option<usize>,
    pub new_start: Option<usize>,
    pub new_end: Option<usize>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RelatedCommitOutput {
    pub hash: String,
    pub short_hash: String,
    pub author_name: String,
    pub author_email: Option<String>,
    pub committed_at: String,
    pub subject: String,
    pub matched_by_filter: bool,
    pub touched_ranges: Vec<TouchedRangeOutput>,
    pub attribution: Option<RelatedCommitAttributionOutput>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WordChangeOutput {
    pub value: String,
    pub added: Option<bool>,
    pub removed: Option<bool>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SideBySideDiffRowOutput {
    pub old_line_number: Option<usize>,
    pub new_line_number: Option<usize>,
    pub old_text: Option<String>,
    pub new_text: Option<String>,
    pub r#type: String,
    pub word_changes: Option<Vec<WordChangeOutput>>,
    pub block_id: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DiffBlockOutput {
    pub id: String,
    pub old_start: usize,
    pub old_end: usize,
    pub new_start: usize,
    pub new_end: usize,
    pub row_start_index: Option<usize>,
    pub row_end_index: Option<usize>,
    pub change_type: String,
    pub authors: Vec<AuthorOutput>,
    pub rows: Vec<SideBySideDiffRowOutput>,
    pub related_commits: Vec<RelatedCommitOutput>,
    pub attribution: Option<BlockAttributionOutput>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileOverlayOutput {
    pub mode: String,
    pub file: ChangedFileOutput,
    pub range: AnalysisRangeOutput,
    pub rows: Vec<SideBySideDiffRowOutput>,
    pub blocks: Vec<DiffBlockOutput>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileOverlayCommandOutput {
    pub version: u8,
    pub overlay: FileOverlayOutput,
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceBlockOutput {
    pub version: u8,
    pub file: String,
    pub block_id: String,
    pub attribution: Option<BlockAttributionOutput>,
    pub authors: Vec<AuthorOutput>,
    pub related_commits: Vec<RelatedCommitOutput>,
    pub warnings: Vec<String>,
}
```

- [ ] **步骤 6：新增命令骨架并分发**

修改 `crates/revier-analysis/src/commands/mod.rs`：

```rust
pub mod file_overlay;
pub mod index_build;
pub mod index_status;
pub mod query_files;
pub mod trace_block;
```

新建 `crates/revier-analysis/src/commands/file_overlay.rs`：

```rust
use crate::cli::FileOverlayArgs;
use crate::error::AppError;

pub fn run(_args: FileOverlayArgs) -> Result<String, AppError> {
    Err(AppError::Analysis(
        "file-overlay 命令契约已注册，overlay 构建尚未接入".to_string(),
    ))
}
```

新建 `crates/revier-analysis/src/commands/trace_block.rs`：

```rust
use crate::cli::TraceBlockArgs;
use crate::error::AppError;

pub fn run(_args: TraceBlockArgs) -> Result<String, AppError> {
    Err(AppError::Analysis(
        "trace-block 命令契约已注册，单块追踪尚未接入".to_string(),
    ))
}
```

修改 `crates/revier-analysis/src/lib.rs` 的分发：

```rust
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
    Command::FileOverlay(args) => commands::file_overlay::run(args),
    Command::TraceBlock(args) => commands::trace_block::run(args),
}
```

- [ ] **步骤 7：运行测试确认通过**

运行：

```powershell
cargo test -p revier-analysis --test cli_contract
```

预期：通过。

- [ ] **步骤 8：提交**

运行：

```powershell
git add crates/revier-analysis/src/cli.rs crates/revier-analysis/src/lib.rs crates/revier-analysis/src/error.rs crates/revier-analysis/src/json.rs crates/revier-analysis/src/commands/mod.rs crates/revier-analysis/src/commands/file_overlay.rs crates/revier-analysis/src/commands/trace_block.rs crates/revier-analysis/tests/cli_contract.rs
git commit -m "feat: 添加 Rust overlay CLI 契约"
```

## 任务 2：实现 Git blob 读取与 Rust rows/blocks 构建

**文件：**
- 修改：`crates/revier-analysis/src/git/mod.rs`
- 新建：`crates/revier-analysis/src/git/blob.rs`
- 新建：`crates/revier-analysis/src/overlay/mod.rs`
- 新建：`crates/revier-analysis/src/overlay/line_diff.rs`
- 新建：`crates/revier-analysis/src/overlay/rows.rs`
- 新建：`crates/revier-analysis/src/overlay/blocks.rs`
- 新建：`crates/revier-analysis/src/overlay/diff_builder.rs`
- 修改：`crates/revier-analysis/src/lib.rs`
- 新建：`crates/revier-analysis/tests/overlay_rows.rs`

- [ ] **步骤 1：写失败的 rows/blocks 测试**

创建 `crates/revier-analysis/tests/overlay_rows.rs`：

```rust
use revier_analysis::overlay::diff_builder::build_overlay_diff;

#[test]
fn builds_side_by_side_rows_and_marks_changed_block_ids() {
    let diff = build_overlay_diff(
        "same before\nconst name = \"old\";\nsame after\n",
        "same before\nconst name = \"new\";\nsame after\n",
    );

    assert_eq!(diff.rows.len(), 3);
    assert_eq!(diff.blocks.len(), 1);
    assert_eq!(diff.rows[0].r#type, "context");
    assert_eq!(diff.rows[1].r#type, "modified");
    assert_eq!(diff.rows[1].block_id.as_deref(), Some("block-1"));
    assert_eq!(diff.rows[2].r#type, "context");
    assert_eq!(diff.blocks[0].row_start_index, Some(1));
    assert_eq!(diff.blocks[0].row_end_index, Some(1));
}

#[test]
fn keeps_empty_placeholders_for_added_lines() {
    let diff = build_overlay_diff("one\nthree\n", "one\ntwo\nthree\n");
    let added = diff
        .rows
        .iter()
        .find(|row| row.r#type == "added")
        .expect("存在新增行");

    assert_eq!(added.old_line_number, None);
    assert_eq!(added.new_line_number, Some(2));
    assert_eq!(added.new_text.as_deref(), Some("two"));
    assert_eq!(added.block_id.as_deref(), Some("block-1"));
}

#[test]
fn groups_adjacent_deleted_lines_into_one_block() {
    let diff = build_overlay_diff("one\ntwo\nthree\n", "one\n");

    assert_eq!(diff.blocks.len(), 1);
    assert_eq!(diff.blocks[0].change_type, "deleted");
    assert_eq!(diff.blocks[0].old_start, 2);
    assert_eq!(diff.blocks[0].old_end, 3);
    assert_eq!(diff.blocks[0].new_start, 0);
    assert_eq!(diff.blocks[0].new_end, 0);
}
```

- [ ] **步骤 2：运行测试确认失败**

运行：

```powershell
cargo test -p revier-analysis --test overlay_rows
```

预期：失败，`overlay` 模块尚不存在。

- [ ] **步骤 3：实现 blob 读取**

修改 `crates/revier-analysis/src/git/mod.rs`：

```rust
pub mod blame;
pub mod blob;
pub mod commits;
pub mod diff;
pub mod repository;
```

新建 `crates/revier-analysis/src/git/blob.rs`：

```rust
use crate::error::AppError;
use gix::bstr::ByteSlice;

pub fn read_blob_at_commit(
    repo: &gix::Repository,
    commit_hash: &str,
    path: &str,
) -> Result<Option<Vec<u8>>, AppError> {
    let object = repo
        .rev_parse_single(commit_hash)
        .map_err(|error| AppError::Repository(error.to_string()))?;
    let commit = object
        .object()
        .map_err(|error| AppError::Repository(error.to_string()))?
        .try_into_commit()
        .map_err(|_| AppError::Repository(format!("对象不是提交：{commit_hash}")))?;
    let tree = commit
        .tree()
        .map_err(|error| AppError::Repository(error.to_string()))?;
    let mut current = tree;
    let mut components = path.split('/').filter(|part| !part.is_empty()).peekable();

    while let Some(component) = components.next() {
        let entry = current
            .iter()
            .find(|entry| entry.filename.to_str_lossy() == component);
        let Some(entry) = entry else {
            return Ok(None);
        };
        if components.peek().is_none() {
            if !entry.mode.is_blob_or_symlink() {
                return Ok(None);
            }
            let blob = repo
                .find_blob(entry.oid.to_owned())
                .map_err(|error| AppError::Repository(error.to_string()))?;
            return Ok(Some(blob.data.to_vec()));
        }
        current = repo
            .find_tree(entry.oid.to_owned())
            .map_err(|error| AppError::Repository(error.to_string()))?;
    }

    Ok(None)
}

pub fn read_text_at_commit(
    repo: &gix::Repository,
    commit_hash: &str,
    path: &str,
) -> Result<String, AppError> {
    let Some(bytes) = read_blob_at_commit(repo, commit_hash, path)? else {
        return Ok(String::new());
    };
    if is_binary_bytes(&bytes) {
        return Err(AppError::FileNotAnalyzable(format!(
            "文件包含二进制内容：{path}"
        )));
    }
    String::from_utf8(bytes).map_err(|error| {
        AppError::FileNotAnalyzable(format!("文件不是 UTF-8 文本：{path}，{error}"))
    })
}

pub fn is_binary_at_commit(
    repo: &gix::Repository,
    commit_hash: &str,
    path: &str,
) -> Result<bool, AppError> {
    Ok(read_blob_at_commit(repo, commit_hash, path)?
        .as_deref()
        .map(is_binary_bytes)
        .unwrap_or(false))
}

fn is_binary_bytes(bytes: &[u8]) -> bool {
    bytes.iter().take(8000).any(|byte| *byte == 0)
}
```

- [ ] **步骤 4：实现 overlay 模块出口**

修改 `crates/revier-analysis/src/lib.rs` 增加：

```rust
pub mod overlay;
```

新建 `crates/revier-analysis/src/overlay/mod.rs`：

```rust
pub mod blocks;
pub mod diff_builder;
pub mod line_diff;
pub mod rows;
```

- [ ] **步骤 5：实现行级 LCS diff**

新建 `crates/revier-analysis/src/overlay/line_diff.rs`：

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineDiffPart {
    Equal(Vec<String>),
    Added(Vec<String>),
    Removed(Vec<String>),
}

pub fn diff_lines(old_text: &str, new_text: &str) -> Vec<LineDiffPart> {
    let old_lines = split_lines(old_text);
    let new_lines = split_lines(new_text);
    let mut table = vec![vec![0_usize; new_lines.len() + 1]; old_lines.len() + 1];

    for old_index in (0..old_lines.len()).rev() {
        for new_index in (0..new_lines.len()).rev() {
            table[old_index][new_index] = if old_lines[old_index] == new_lines[new_index] {
                table[old_index + 1][new_index + 1] + 1
            } else {
                table[old_index + 1][new_index].max(table[old_index][new_index + 1])
            };
        }
    }

    let mut parts = Vec::new();
    let mut old_index = 0;
    let mut new_index = 0;
    while old_index < old_lines.len() && new_index < new_lines.len() {
        if old_lines[old_index] == new_lines[new_index] {
            push_part(&mut parts, LineDiffPart::Equal(vec![old_lines[old_index].clone()]));
            old_index += 1;
            new_index += 1;
        } else if table[old_index + 1][new_index] >= table[old_index][new_index + 1] {
            push_part(&mut parts, LineDiffPart::Removed(vec![old_lines[old_index].clone()]));
            old_index += 1;
        } else {
            push_part(&mut parts, LineDiffPart::Added(vec![new_lines[new_index].clone()]));
            new_index += 1;
        }
    }
    while old_index < old_lines.len() {
        push_part(&mut parts, LineDiffPart::Removed(vec![old_lines[old_index].clone()]));
        old_index += 1;
    }
    while new_index < new_lines.len() {
        push_part(&mut parts, LineDiffPart::Added(vec![new_lines[new_index].clone()]));
        new_index += 1;
    }

    parts
}

fn split_lines(value: &str) -> Vec<String> {
    if value.is_empty() {
        return Vec::new();
    }
    let trimmed = value.strip_suffix('\n').unwrap_or(value);
    trimmed.split('\n').map(ToString::to_string).collect()
}

fn push_part(parts: &mut Vec<LineDiffPart>, part: LineDiffPart) {
    match (parts.last_mut(), part) {
        (Some(LineDiffPart::Equal(existing)), LineDiffPart::Equal(mut next))
        | (Some(LineDiffPart::Added(existing)), LineDiffPart::Added(mut next))
        | (Some(LineDiffPart::Removed(existing)), LineDiffPart::Removed(mut next)) => {
            existing.append(&mut next);
        }
        (_, next) => parts.push(next),
    }
}
```

- [ ] **步骤 6：实现 rows 构建**

新建 `crates/revier-analysis/src/overlay/rows.rs`：

```rust
use crate::json::{SideBySideDiffRowOutput, WordChangeOutput};
use crate::overlay::line_diff::LineDiffPart;

pub fn build_rows(parts: &[LineDiffPart]) -> Vec<SideBySideDiffRowOutput> {
    let mut rows = Vec::new();
    let mut old_line = 1_usize;
    let mut new_line = 1_usize;
    let mut index = 0_usize;

    while index < parts.len() {
        match (&parts[index], parts.get(index + 1)) {
            (LineDiffPart::Removed(old_lines), Some(LineDiffPart::Added(new_lines))) => {
                let max_len = old_lines.len().max(new_lines.len());
                for offset in 0..max_len {
                    let old_text = old_lines.get(offset).cloned();
                    let new_text = new_lines.get(offset).cloned();
                    let old_line_number = old_text.as_ref().map(|_| {
                        let current = old_line;
                        old_line += 1;
                        current
                    });
                    let new_line_number = new_text.as_ref().map(|_| {
                        let current = new_line;
                        new_line += 1;
                        current
                    });
                    rows.push(SideBySideDiffRowOutput {
                        old_line_number,
                        new_line_number,
                        old_text: old_text.clone(),
                        new_text: new_text.clone(),
                        r#type: "modified".to_string(),
                        word_changes: word_changes(old_text.as_deref(), new_text.as_deref()),
                        block_id: None,
                    });
                }
                index += 2;
            }
            (LineDiffPart::Equal(lines), _) => {
                for line in lines {
                    rows.push(SideBySideDiffRowOutput {
                        old_line_number: Some(old_line),
                        new_line_number: Some(new_line),
                        old_text: Some(line.clone()),
                        new_text: Some(line.clone()),
                        r#type: "context".to_string(),
                        word_changes: None,
                        block_id: None,
                    });
                    old_line += 1;
                    new_line += 1;
                }
                index += 1;
            }
            (LineDiffPart::Added(lines), _) => {
                for line in lines {
                    rows.push(SideBySideDiffRowOutput {
                        old_line_number: None,
                        new_line_number: Some(new_line),
                        old_text: None,
                        new_text: Some(line.clone()),
                        r#type: "added".to_string(),
                        word_changes: None,
                        block_id: None,
                    });
                    new_line += 1;
                }
                index += 1;
            }
            (LineDiffPart::Removed(lines), _) => {
                for line in lines {
                    rows.push(SideBySideDiffRowOutput {
                        old_line_number: Some(old_line),
                        new_line_number: None,
                        old_text: Some(line.clone()),
                        new_text: None,
                        r#type: "deleted".to_string(),
                        word_changes: None,
                        block_id: None,
                    });
                    old_line += 1;
                }
                index += 1;
            }
        }
    }

    rows
}

fn word_changes(old_text: Option<&str>, new_text: Option<&str>) -> Option<Vec<WordChangeOutput>> {
    match (old_text, new_text) {
        (Some(old_text), Some(new_text)) if old_text != new_text => Some(vec![
            WordChangeOutput {
                value: old_text.to_string(),
                added: None,
                removed: Some(true),
            },
            WordChangeOutput {
                value: new_text.to_string(),
                added: Some(true),
                removed: None,
            },
        ]),
        _ => None,
    }
}
```

- [ ] **步骤 7：实现 blocks 构建**

新建 `crates/revier-analysis/src/overlay/blocks.rs`：

```rust
use crate::json::{DiffBlockOutput, SideBySideDiffRowOutput};

pub fn build_blocks(rows: &mut [SideBySideDiffRowOutput]) -> Vec<DiffBlockOutput> {
    let mut blocks = Vec::new();
    let mut current: Vec<usize> = Vec::new();

    for index in 0..rows.len() {
        if rows[index].r#type == "context" {
            flush_block(rows, &mut current, &mut blocks);
        } else {
            current.push(index);
        }
    }
    flush_block(rows, &mut current, &mut blocks);
    blocks
}

fn flush_block(
    rows: &mut [SideBySideDiffRowOutput],
    current: &mut Vec<usize>,
    blocks: &mut Vec<DiffBlockOutput>,
) {
    if current.is_empty() {
        return;
    }

    let block_id = format!("block-{}", blocks.len() + 1);
    for index in current.iter().copied() {
        rows[index].block_id = Some(block_id.clone());
    }
    let block_rows = current
        .iter()
        .map(|index| rows[*index].clone())
        .collect::<Vec<_>>();
    let old_numbers = block_rows
        .iter()
        .filter_map(|row| row.old_line_number)
        .collect::<Vec<_>>();
    let new_numbers = block_rows
        .iter()
        .filter_map(|row| row.new_line_number)
        .collect::<Vec<_>>();
    let change_type = if block_rows.iter().any(|row| row.r#type == "modified") {
        "modified"
    } else if block_rows.iter().any(|row| row.r#type == "added") {
        "added"
    } else {
        "deleted"
    };

    blocks.push(DiffBlockOutput {
        id: block_id,
        old_start: old_numbers.first().copied().unwrap_or(0),
        old_end: old_numbers.last().copied().unwrap_or(0),
        new_start: new_numbers.first().copied().unwrap_or(0),
        new_end: new_numbers.last().copied().unwrap_or(0),
        row_start_index: current.first().copied(),
        row_end_index: current.last().copied(),
        change_type: change_type.to_string(),
        authors: Vec::new(),
        rows: block_rows,
        related_commits: Vec::new(),
        attribution: None,
    });
    current.clear();
}
```

- [ ] **步骤 8：实现 diff builder**

新建 `crates/revier-analysis/src/overlay/diff_builder.rs`：

```rust
use crate::json::{DiffBlockOutput, SideBySideDiffRowOutput};
use crate::overlay::blocks::build_blocks;
use crate::overlay::line_diff::diff_lines;
use crate::overlay::rows::build_rows;

pub struct BuiltOverlayDiff {
    pub rows: Vec<SideBySideDiffRowOutput>,
    pub blocks: Vec<DiffBlockOutput>,
}

pub fn build_overlay_diff(old_text: &str, new_text: &str) -> BuiltOverlayDiff {
    let parts = diff_lines(old_text, new_text);
    let mut rows = build_rows(&parts);
    let blocks = build_blocks(&mut rows);
    BuiltOverlayDiff { rows, blocks }
}
```

- [ ] **步骤 9：运行测试确认通过**

运行：

```powershell
cargo test -p revier-analysis --test overlay_rows
```

预期：通过。

- [ ] **步骤 10：提交**

运行：

```powershell
git add crates/revier-analysis/src/git/mod.rs crates/revier-analysis/src/git/blob.rs crates/revier-analysis/src/lib.rs crates/revier-analysis/src/overlay crates/revier-analysis/tests/overlay_rows.rs
git commit -m "feat: 实现 Rust overlay 行块构建"
```

## 任务 3：实现 `file-overlay` 基础 JSON 输出

**文件：**
- 修改：`crates/revier-analysis/src/git/diff.rs`
- 新建：`crates/revier-analysis/src/overlay/file_overlay.rs`
- 修改：`crates/revier-analysis/src/overlay/mod.rs`
- 修改：`crates/revier-analysis/src/commands/file_overlay.rs`
- 新建：`crates/revier-analysis/tests/file_overlay_cli.rs`

- [ ] **步骤 1：写失败的 CLI JSON 测试**

创建 `crates/revier-analysis/tests/file_overlay_cli.rs`：

```rust
mod fixtures;

use serde_json::Value;
use std::process::Command;

#[test]
fn file_overlay_outputs_file_overlay_compatible_json_for_linear_change() {
    let fixture = fixtures::linear();
    let output = Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
        .args([
            "file-overlay",
            "--repo",
            fixture.repo.path().to_str().expect("repo path"),
            "--base",
            &fixture.base,
            "--head",
            &fixture.head,
            "--branch",
            "main",
            "--file",
            "src/app.txt",
            "--format",
            "json",
        ])
        .output()
        .expect("运行 file-overlay");

    assert!(
        output.status.success(),
        "file-overlay 应成功，stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let json: Value = serde_json::from_slice(&output.stdout).expect("解析 overlay json");
    assert_eq!(json["version"], 1);
    assert_eq!(json["overlay"]["mode"], "range");
    assert_eq!(json["overlay"]["file"]["path"], "src/app.txt");
    assert_eq!(json["overlay"]["file"]["status"], "modified");
    assert_eq!(json["overlay"]["range"]["baseCommit"], fixture.base);
    assert_eq!(json["overlay"]["range"]["headCommit"], fixture.head);
    assert_eq!(json["overlay"]["blocks"][0]["id"], "block-1");
    assert_eq!(json["overlay"]["blocks"][0]["changeType"], "added");
    assert!(json["overlay"]["rows"].as_array().expect("rows").len() >= 2);
}

#[test]
fn file_overlay_returns_exit_code_four_for_binary_file() {
    let fixture = fixtures::binary_change();
    let output = Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
        .args([
            "file-overlay",
            "--repo",
            fixture.repo.path().to_str().expect("repo path"),
            "--base",
            &fixture.base,
            "--head",
            &fixture.head,
            "--branch",
            "main",
            "--file",
            "assets/logo.bin",
            "--format",
            "json",
        ])
        .output()
        .expect("运行 binary file-overlay");

    assert_eq!(output.status.code(), Some(4));
    assert!(String::from_utf8_lossy(&output.stderr).contains("文件不可分析"));
}
```

- [ ] **步骤 2：运行测试确认失败**

运行：

```powershell
cargo test -p revier-analysis --test file_overlay_cli
```

预期：失败，命令只返回骨架错误。

- [ ] **步骤 3：实现 range 文件状态解析**

修改 `crates/revier-analysis/src/git/diff.rs`，追加公开函数：

```rust
pub fn changed_file_between(
    repo: &gix::Repository,
    base_commit: &str,
    head_commit: &str,
    requested_path: &str,
) -> Result<Option<CommitFileChange>, AppError> {
    let base = resolve_commit(repo, base_commit)?;
    let head = resolve_commit(repo, head_commit)?;
    let changes = pairwise_file_changes(repo, &base, &head, 0)?;
    Ok(changes.into_iter().find(|change| {
        change.path == requested_path || change.old_path.as_deref() == Some(requested_path)
    }))
}
```

- [ ] **步骤 4：实现文件 overlay 构建入口**

修改 `crates/revier-analysis/src/overlay/mod.rs`：

```rust
pub mod blocks;
pub mod diff_builder;
pub mod file_overlay;
pub mod line_diff;
pub mod rows;
```

新建 `crates/revier-analysis/src/overlay/file_overlay.rs`：

```rust
use crate::cli::FileOverlayArgs;
use crate::error::AppError;
use crate::git::{blob, diff};
use crate::json::{
    AnalysisRangeOutput, ChangedFileOutput, FileOverlayCommandOutput, FileOverlayOutput,
};
use crate::overlay::diff_builder::build_overlay_diff;

pub fn build_file_overlay(
    repo: &gix::Repository,
    args: &FileOverlayArgs,
) -> Result<FileOverlayCommandOutput, AppError> {
    let change = diff::changed_file_between(
        repo,
        &args.common.base,
        &args.common.head,
        &args.file,
    )?
    .ok_or_else(|| AppError::FileNotAnalyzable(format!("文件不在分析范围内：{}", args.file)))?;

    if change.is_binary {
        return Err(AppError::FileNotAnalyzable(format!(
            "文件包含二进制内容：{}",
            change.path
        )));
    }

    let old_path = change.old_path.clone().unwrap_or_else(|| change.path.clone());
    let old_text = if change.status == "added" {
        String::new()
    } else {
        blob::read_text_at_commit(repo, &args.common.base, &old_path)?
    };
    let new_text = if change.status == "deleted" {
        String::new()
    } else {
        blob::read_text_at_commit(repo, &args.common.head, &change.path)?
    };
    let diff = build_overlay_diff(&old_text, &new_text);

    Ok(FileOverlayCommandOutput {
        version: 1,
        overlay: FileOverlayOutput {
            mode: "range".to_string(),
            file: ChangedFileOutput {
                path: change.path,
                old_path: change.old_path,
                status: change.status,
                additions: change.additions,
                deletions: change.deletions,
                is_binary: false,
                is_previewable: true,
            },
            range: AnalysisRangeOutput {
                branch: args.common.branch.clone(),
                base_commit: args.common.base.clone(),
                head_commit: args.common.head.clone(),
                start_at: None,
                end_at: None,
            },
            rows: diff.rows,
            blocks: diff.blocks,
            warnings: Vec::new(),
        },
        warnings: Vec::new(),
    })
}
```

- [ ] **步骤 5：接入命令**

替换 `crates/revier-analysis/src/commands/file_overlay.rs`：

```rust
use crate::cli::FileOverlayArgs;
use crate::error::AppError;

pub fn run(args: FileOverlayArgs) -> Result<String, AppError> {
    let repo = crate::git::repository::open_repository(&args.common.repo)?;
    let output = crate::overlay::file_overlay::build_file_overlay(&repo, &args)?;
    crate::serialize_json(&output, args.common.pretty)
}
```

- [ ] **步骤 6：运行测试确认通过**

运行：

```powershell
cargo test -p revier-analysis --test file_overlay_cli file_overlay_outputs_file_overlay_compatible_json_for_linear_change
cargo test -p revier-analysis --test file_overlay_cli file_overlay_returns_exit_code_four_for_binary_file
cargo test -p revier-analysis --test no_git_process
```

预期：全部通过。

- [ ] **步骤 7：提交**

运行：

```powershell
git add crates/revier-analysis/src/git/diff.rs crates/revier-analysis/src/overlay/mod.rs crates/revier-analysis/src/overlay/file_overlay.rs crates/revier-analysis/src/commands/file_overlay.rs crates/revier-analysis/tests/file_overlay_cli.rs
git commit -m "feat: 实现 Rust file-overlay 基础输出"
```

## 任务 4：实现索引优先的提交上下文与 `--require-index`

**文件：**
- 修改：`crates/revier-analysis/src/git/commits.rs`
- 修改：`crates/revier-analysis/src/index/queries.rs`
- 新建：`crates/revier-analysis/src/attribution/mod.rs`
- 新建：`crates/revier-analysis/src/attribution/context.rs`
- 新建：`crates/revier-analysis/src/attribution/commit_lookup.rs`
- 修改：`crates/revier-analysis/src/lib.rs`
- 修改：`crates/revier-analysis/src/overlay/file_overlay.rs`
- 修改：`crates/revier-analysis/tests/file_overlay_cli.rs`

- [ ] **步骤 1：写失败的 require-index 测试**

修改 `crates/revier-analysis/tests/file_overlay_cli.rs`，追加：

```rust
#[test]
fn file_overlay_require_index_returns_exit_code_five_when_db_is_missing() {
    let fixture = fixtures::linear();
    let db_path = fixture.repo.path().join("missing.duckdb");
    let output = Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
        .args([
            "file-overlay",
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
            "--file",
            "src/app.txt",
            "--require-index",
            "--format",
            "json",
        ])
        .output()
        .expect("运行 require-index file-overlay");

    assert_eq!(output.status.code(), Some(5));
    assert!(String::from_utf8_lossy(&output.stderr).contains("命令要求必须使用索引"));
}

#[test]
fn file_overlay_without_index_succeeds_and_reports_warning() {
    let fixture = fixtures::linear();
    let db_path = fixture.repo.path().join("missing.duckdb");
    let output = Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
        .args([
            "file-overlay",
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
            "--file",
            "src/app.txt",
            "--format",
            "json",
        ])
        .output()
        .expect("运行 file-overlay");

    assert!(output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).expect("解析 overlay json");
    assert!(
        json["warnings"]
            .as_array()
            .expect("warnings")
            .iter()
            .any(|warning| warning.as_str().unwrap_or("").contains("索引不可用，已即时计算"))
    );
}
```

- [ ] **步骤 2：运行测试确认失败**

运行：

```powershell
cargo test -p revier-analysis --test file_overlay_cli file_overlay_require_index_returns_exit_code_five_when_db_is_missing
cargo test -p revier-analysis --test file_overlay_cli file_overlay_without_index_succeeds_and_reports_warning
```

预期：失败，命令尚未检查索引。

- [ ] **步骤 3：扩展提交读取能力**

修改 `crates/revier-analysis/src/git/commits.rs`，追加：

```rust
pub fn get_commit(repo: &gix::Repository, commit_hash: &str) -> Result<IndexedCommit, AppError> {
    let object = repo
        .rev_parse_single(commit_hash)
        .map_err(|error| AppError::Repository(error.to_string()))?;
    let commit = object
        .object()
        .map_err(|error| AppError::Repository(error.to_string()))?
        .try_into_commit()
        .map_err(|_| AppError::Repository(format!("对象不是提交：{commit_hash}")))?;
    indexed_commit_from_gix(&commit)
}

pub fn range_commit_hashes(
    repo: &gix::Repository,
    base_commit: &str,
    head_commit: &str,
) -> Result<Vec<String>, AppError> {
    let base_id = repo
        .rev_parse_single(base_commit)
        .map_err(|error| AppError::Repository(error.to_string()))?
        .detach();
    let head_id = repo
        .rev_parse_single(head_commit)
        .map_err(|error| AppError::Repository(error.to_string()))?
        .detach();
    let walk = repo
        .rev_walk([head_id])
        .all()
        .map_err(|error| AppError::Repository(error.to_string()))?;
    let mut hashes = Vec::new();
    for item in walk {
        let info = item.map_err(|error| AppError::Repository(error.to_string()))?;
        if info.id == base_id {
            break;
        }
        hashes.push(info.id.to_string());
    }
    hashes.reverse();
    Ok(hashes)
}
```

- [ ] **步骤 4：扩展索引查询**

修改 `crates/revier-analysis/src/index/queries.rs`，追加：

```rust
pub fn commit_exists(conn: &duckdb::Connection, hash: &str) -> Result<bool, AppError> {
    let count: i64 = conn
        .query_row("select count(*) from commits where hash = ?", [hash], |row| row.get(0))
        .map_err(|error| AppError::DuckDb(error.to_string()))?;
    Ok(count > 0)
}

pub fn parent_hashes(conn: &duckdb::Connection, hash: &str) -> Result<Vec<String>, AppError> {
    let mut stmt = conn
        .prepare(
            "select parent_hash from commit_parents
             where commit_hash = ?
             order by parent_index",
        )
        .map_err(|error| AppError::DuckDb(error.to_string()))?;
    let rows = stmt
        .query_map([hash], |row| row.get::<_, String>(0))
        .map_err(|error| AppError::DuckDb(error.to_string()))?;
    rows.map(|row| row.map_err(|error| AppError::DuckDb(error.to_string())))
        .collect()
}

pub fn commit_metadata(
    conn: &duckdb::Connection,
    hash: &str,
) -> Result<Option<crate::git::commits::IndexedCommit>, AppError> {
    let mut stmt = conn
        .prepare(
            "select hash, short_hash, author_name, coalesce(author_email, ''),
                    author_key, cast(committed_at as varchar), subject, is_merge
             from commits
             where hash = ?",
        )
        .map_err(|error| AppError::DuckDb(error.to_string()))?;
    let mut rows = stmt
        .query([hash])
        .map_err(|error| AppError::DuckDb(error.to_string()))?;
    let Some(row) = rows.next().map_err(|error| AppError::DuckDb(error.to_string()))? else {
        return Ok(None);
    };
    let author_email: String = row.get(3).map_err(|error| AppError::DuckDb(error.to_string()))?;
    Ok(Some(crate::git::commits::IndexedCommit {
        hash: row.get(0).map_err(|error| AppError::DuckDb(error.to_string()))?,
        short_hash: row.get(1).map_err(|error| AppError::DuckDb(error.to_string()))?,
        author_name: row.get(2).map_err(|error| AppError::DuckDb(error.to_string()))?,
        author_email: (!author_email.is_empty()).then_some(author_email),
        author_key: row.get(4).map_err(|error| AppError::DuckDb(error.to_string()))?,
        committed_at: row.get(5).map_err(|error| AppError::DuckDb(error.to_string()))?,
        subject: row.get(6).map_err(|error| AppError::DuckDb(error.to_string()))?,
        parents: parent_hashes(conn, hash)?,
        is_merge: row.get(7).map_err(|error| AppError::DuckDb(error.to_string()))?,
    }))
}
```

- [ ] **步骤 5：新增归因上下文**

修改 `crates/revier-analysis/src/lib.rs` 增加：

```rust
pub mod attribution;
```

新建 `crates/revier-analysis/src/attribution/mod.rs`：

```rust
pub mod commit_lookup;
pub mod context;
```

新建 `crates/revier-analysis/src/attribution/context.rs`：

```rust
use crate::cli::OverlayCommonArgs;
use crate::error::AppError;

pub struct AttributionContext<'repo> {
    pub repo: &'repo gix::Repository,
    pub db: Option<duckdb::Connection>,
    pub warnings: Vec<String>,
    pub range_hashes: Vec<String>,
}

impl<'repo> AttributionContext<'repo> {
    pub fn open(repo: &'repo gix::Repository, args: &OverlayCommonArgs) -> Result<Self, AppError> {
        let identity = crate::git::repository::repository_identity(repo)?;
        let db_path = args
            .db
            .clone()
            .unwrap_or(crate::index::connection::default_database_path(&identity.repo_id)?);
        let db = if db_path.exists() {
            let conn = crate::index::connection::open_database(&db_path)?;
            crate::index::migrations::ensure_compatible_schema(&conn)?;
            if !crate::index::queries::commit_exists(&conn, &args.head)? {
                if args.require_index {
                    return Err(AppError::RequiredIndexUnavailable(format!(
                        "head {} 未被索引",
                        args.head
                    )));
                }
                None
            } else {
                Some(conn)
            }
        } else if args.require_index {
            return Err(AppError::RequiredIndexUnavailable(format!(
                "索引文件不存在：{}",
                db_path.display()
            )));
        } else {
            None
        };

        let mut warnings = Vec::new();
        if db.is_none() {
            warnings.push("索引不可用，已即时计算 overlay 结果".to_string());
        }
        let range_hashes =
            crate::git::commits::range_commit_hashes(repo, &args.base, &args.head)?;

        Ok(Self {
            repo,
            db,
            warnings,
            range_hashes,
        })
    }
}
```

新建 `crates/revier-analysis/src/attribution/commit_lookup.rs`：

```rust
use crate::attribution::context::AttributionContext;
use crate::error::AppError;
use crate::git::commits::IndexedCommit;

pub fn get_commit(
    context: &AttributionContext<'_>,
    hash: &str,
) -> Result<IndexedCommit, AppError> {
    if let Some(conn) = &context.db {
        if let Some(commit) = crate::index::queries::commit_metadata(conn, hash)? {
            return Ok(commit);
        }
    }
    crate::git::commits::get_commit(context.repo, hash)
}
```

- [ ] **步骤 6：让 file-overlay 使用上下文 warning**

修改 `crates/revier-analysis/src/overlay/file_overlay.rs`，把函数签名改为：

```rust
pub fn build_file_overlay(
    repo: &gix::Repository,
    args: &FileOverlayArgs,
) -> Result<FileOverlayCommandOutput, AppError> {
    let context = crate::attribution::context::AttributionContext::open(repo, &args.common)?;
    let change = diff::changed_file_between(
        repo,
        &args.common.base,
        &args.common.head,
        &args.file,
    )?
    .ok_or_else(|| AppError::FileNotAnalyzable(format!("文件不在分析范围内：{}", args.file)))?;
```

并把输出尾部改为：

```rust
            warnings: context.warnings.clone(),
        },
        warnings: context.warnings,
    })
}
```

- [ ] **步骤 7：运行测试确认通过**

运行：

```powershell
cargo test -p revier-analysis --test file_overlay_cli
cargo test -p revier-analysis --test index_query_files
```

预期：通过，现有 `index query-files` 缺索引退出码仍为 `4`。

- [ ] **步骤 8：提交**

运行：

```powershell
git add crates/revier-analysis/src/git/commits.rs crates/revier-analysis/src/index/queries.rs crates/revier-analysis/src/attribution crates/revier-analysis/src/lib.rs crates/revier-analysis/src/overlay/file_overlay.rs crates/revier-analysis/tests/file_overlay_cli.rs
git commit -m "feat: 添加 Rust overlay 索引上下文"
```

## 任务 5：实现 patch inference 与基础 related commits

**文件：**
- 修改：`crates/revier-analysis/src/attribution/mod.rs`
- 新建：`crates/revier-analysis/src/attribution/patch_inference.rs`
- 修改：`crates/revier-analysis/src/overlay/file_overlay.rs`
- 新建：`crates/revier-analysis/tests/overlay_attribution.rs`

- [ ] **步骤 1：写失败的 related commits 测试**

创建 `crates/revier-analysis/tests/overlay_attribution.rs`：

```rust
mod fixtures;

use serde_json::Value;
use std::process::Command;

#[test]
fn file_overlay_attaches_related_commit_from_patch_inference() {
    let fixture = fixtures::linear_with_authors();
    let output = Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
        .args([
            "file-overlay",
            "--repo",
            fixture.repo.path().to_str().expect("repo path"),
            "--base",
            &fixture.base,
            "--head",
            &fixture.head,
            "--branch",
            "main",
            "--file",
            "src/app.txt",
            "--author",
            "alice@example.com",
            "--format",
            "json",
        ])
        .output()
        .expect("运行 file-overlay");

    assert!(
        output.status.success(),
        "file-overlay 应成功，stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let json: Value = serde_json::from_slice(&output.stdout).expect("解析 overlay json");
    let related = json["overlay"]["blocks"][0]["relatedCommits"]
        .as_array()
        .expect("related commits");
    assert!(
        related
            .iter()
            .any(|commit| commit["authorName"] == "Alice"
                && commit["matchedByFilter"] == true
                && commit["attribution"]["method"] == "patch-inference")
    );
}
```

- [ ] **步骤 2：运行测试确认失败**

运行：

```powershell
cargo test -p revier-analysis --test overlay_attribution file_overlay_attaches_related_commit_from_patch_inference
```

预期：失败，blocks 还没有 related commits。

- [ ] **步骤 3：实现 patch inference**

修改 `crates/revier-analysis/src/attribution/mod.rs`：

```rust
pub mod commit_lookup;
pub mod context;
pub mod patch_inference;
```

新建 `crates/revier-analysis/src/attribution/patch_inference.rs`：

```rust
use crate::attribution::commit_lookup;
use crate::attribution::context::AttributionContext;
use crate::error::AppError;
use crate::git::blob;
use crate::json::{
    DiffBlockOutput, RelatedCommitAttributionOutput, RelatedCommitOutput, TouchedRangeOutput,
};
use crate::overlay::diff_builder::build_overlay_diff;

pub fn attach_patch_inference(
    context: &AttributionContext<'_>,
    blocks: Vec<DiffBlockOutput>,
    file_path: &str,
    old_path: Option<&str>,
    authors: &[String],
    author_query: Option<&str>,
    message: Option<&str>,
) -> Result<Vec<DiffBlockOutput>, AppError> {
    let related = related_commits_for_file(
        context,
        file_path,
        old_path,
        authors,
        author_query,
        message,
    )?;
    Ok(blocks
        .into_iter()
        .map(|mut block| {
            let block_range = TouchedRangeOutput {
                old_start: (block.old_start > 0).then_some(block.old_start),
                old_end: (block.old_end > 0).then_some(block.old_end),
                new_start: (block.new_start > 0).then_some(block.new_start),
                new_end: (block.new_end > 0).then_some(block.new_end),
            };
            block.related_commits = related
                .iter()
                .filter(|commit| {
                    commit
                        .touched_ranges
                        .iter()
                        .any(|range| ranges_intersect(&block_range, range))
                })
                .cloned()
                .collect();
            block.authors = unique_authors(&block.related_commits);
            block
        })
        .collect())
}

pub fn related_commits_for_file(
    context: &AttributionContext<'_>,
    file_path: &str,
    old_path: Option<&str>,
    authors: &[String],
    author_query: Option<&str>,
    message: Option<&str>,
) -> Result<Vec<RelatedCommitOutput>, AppError> {
    let paths = path_candidates(file_path, old_path);
    let mut commits = Vec::new();
    for hash in &context.range_hashes {
        let commit = commit_lookup::get_commit(context, hash)?;
        let touched_ranges = touched_ranges_for_commit(context, &commit.hash, &commit.parents, &paths)?;
        if touched_ranges.is_empty() {
            continue;
        }
        let matched_by_filter = commit_matches(&commit, authors, author_query, message);
        commits.push(RelatedCommitOutput {
            hash: commit.hash,
            short_hash: commit.short_hash,
            author_name: commit.author_name,
            author_email: commit.author_email,
            committed_at: commit.committed_at,
            subject: commit.subject,
            matched_by_filter,
            touched_ranges,
            attribution: Some(RelatedCommitAttributionOutput {
                method: "patch-inference".to_string(),
                via_merge_hashes: Vec::new(),
            }),
        });
    }
    Ok(commits)
}

fn touched_ranges_for_commit(
    context: &AttributionContext<'_>,
    commit_hash: &str,
    parents: &[String],
    paths: &[String],
) -> Result<Vec<TouchedRangeOutput>, AppError> {
    let mut ranges = Vec::new();
    for parent in parents {
        for path in paths {
            let old_text = blob::read_text_at_commit(context.repo, parent, path).unwrap_or_default();
            let new_text = blob::read_text_at_commit(context.repo, commit_hash, path).unwrap_or_default();
            if old_text == new_text {
                continue;
            }
            let diff = build_overlay_diff(&old_text, &new_text);
            ranges.extend(diff.blocks.into_iter().map(|block| TouchedRangeOutput {
                old_start: (block.old_start > 0).then_some(block.old_start),
                old_end: (block.old_end > 0).then_some(block.old_end),
                new_start: (block.new_start > 0).then_some(block.new_start),
                new_end: (block.new_end > 0).then_some(block.new_end),
            }));
        }
    }
    Ok(ranges)
}

fn path_candidates(file_path: &str, old_path: Option<&str>) -> Vec<String> {
    let mut paths = vec![file_path.to_string()];
    if let Some(old_path) = old_path {
        if old_path != file_path {
            paths.push(old_path.to_string());
        }
    }
    paths
}

fn ranges_intersect(left: &TouchedRangeOutput, right: &TouchedRangeOutput) -> bool {
    intersects(left.old_start, left.old_end, right.old_start, right.old_end)
        || intersects(left.new_start, left.new_end, right.new_start, right.new_end)
}

fn intersects(
    left_start: Option<usize>,
    left_end: Option<usize>,
    right_start: Option<usize>,
    right_end: Option<usize>,
) -> bool {
    match (left_start, left_end, right_start, right_end) {
        (Some(left_start), Some(left_end), Some(right_start), Some(right_end)) => {
            left_start <= right_end && right_start <= left_end
        }
        _ => false,
    }
}

fn unique_authors(commits: &[RelatedCommitOutput]) -> Vec<crate::json::AuthorOutput> {
    let mut authors = Vec::new();
    for commit in commits {
        let author = crate::json::AuthorOutput {
            name: commit.author_name.clone(),
            email: commit.author_email.clone(),
        };
        if !authors.iter().any(|item: &crate::json::AuthorOutput| {
            item.name == author.name && item.email == author.email
        }) {
            authors.push(author);
        }
    }
    authors
}

fn commit_matches(
    commit: &crate::git::commits::IndexedCommit,
    authors: &[String],
    author_query: Option<&str>,
    message: Option<&str>,
) -> bool {
    let normalized_authors = authors
        .iter()
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();
    let author_matches = normalized_authors.is_empty()
        || normalized_authors.contains(&commit.author_key);
    let author_text = format!(
        "{} {}",
        commit.author_name,
        commit.author_email.clone().unwrap_or_default()
    )
    .to_lowercase();
    let author_query_matches = author_query
        .map(|query| author_text.contains(&query.trim().to_lowercase()))
        .unwrap_or(true);
    let message_matches = message
        .map(|query| commit.subject.to_lowercase().contains(&query.trim().to_lowercase()))
        .unwrap_or(true);
    author_matches && author_query_matches && message_matches
}
```

- [ ] **步骤 4：在 overlay 中接入 patch inference**

修改 `crates/revier-analysis/src/overlay/file_overlay.rs`，在 `let diff = build_overlay_diff(...)` 后替换输出 blocks：

```rust
    let blocks = crate::attribution::patch_inference::attach_patch_inference(
        &context,
        diff.blocks,
        &change.path,
        change.old_path.as_deref(),
        &args.common.authors,
        args.common.author_query.as_deref(),
        args.common.message.as_deref(),
    )?;
```

并将输出中的 `blocks: diff.blocks` 改为：

```rust
            blocks,
```

- [ ] **步骤 5：运行测试确认通过**

运行：

```powershell
cargo test -p revier-analysis --test overlay_attribution file_overlay_attaches_related_commit_from_patch_inference
cargo test -p revier-analysis --test file_overlay_cli
```

预期：通过。

- [ ] **步骤 6：提交**

运行：

```powershell
git add crates/revier-analysis/src/attribution/mod.rs crates/revier-analysis/src/attribution/patch_inference.rs crates/revier-analysis/src/overlay/file_overlay.rs crates/revier-analysis/tests/overlay_attribution.rs
git commit -m "feat: 添加 Rust overlay patch 归因"
```

## 任务 6：实现新增/修改块 blame 归因

**文件：**
- 修改：`crates/revier-analysis/src/attribution/mod.rs`
- 新建：`crates/revier-analysis/src/attribution/blame.rs`
- 修改：`crates/revier-analysis/src/overlay/file_overlay.rs`
- 修改：`crates/revier-analysis/tests/overlay_attribution.rs`

- [ ] **步骤 1：写失败的 blame 归因测试**

修改 `crates/revier-analysis/tests/overlay_attribution.rs`，追加：

```rust
#[test]
fn added_or_modified_block_uses_blame_as_precise_attribution() {
    let fixture = fixtures::linear_with_authors();
    let output = Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
        .args([
            "file-overlay",
            "--repo",
            fixture.repo.path().to_str().expect("repo path"),
            "--base",
            &fixture.base,
            "--head",
            &fixture.head,
            "--branch",
            "main",
            "--file",
            "src/app.txt",
            "--format",
            "json",
        ])
        .output()
        .expect("运行 file-overlay");

    assert!(output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).expect("解析 overlay json");
    let block = &json["overlay"]["blocks"][0];
    assert_eq!(block["attribution"]["confidence"], "precise");
    assert_eq!(block["relatedCommits"][0]["attribution"]["method"], "blame");
    assert!(block["authors"].as_array().expect("authors").len() >= 1);
}
```

- [ ] **步骤 2：运行测试确认失败**

运行：

```powershell
cargo test -p revier-analysis --test overlay_attribution added_or_modified_block_uses_blame_as_precise_attribution
```

预期：失败，块仍是 patch inference。

- [ ] **步骤 3：实现 blame 归因**

修改 `crates/revier-analysis/src/attribution/mod.rs`：

```rust
pub mod blame;
pub mod commit_lookup;
pub mod context;
pub mod patch_inference;
```

新建 `crates/revier-analysis/src/attribution/blame.rs`：

```rust
use crate::attribution::commit_lookup;
use crate::attribution::context::AttributionContext;
use crate::error::AppError;
use crate::json::{
    AuthorOutput, BlockAttributionOutput, AttributionWarningOutput, DiffBlockOutput,
    RelatedCommitAttributionOutput, RelatedCommitOutput,
};

pub fn attach_blame_attribution(
    context: &AttributionContext<'_>,
    head_commit: &str,
    file_path: &str,
    blocks: Vec<DiffBlockOutput>,
) -> Result<Vec<DiffBlockOutput>, AppError> {
    blocks
        .into_iter()
        .map(|block| resolve_block(context, head_commit, file_path, block))
        .collect()
}

fn resolve_block(
    context: &AttributionContext<'_>,
    head_commit: &str,
    file_path: &str,
    mut block: DiffBlockOutput,
) -> Result<DiffBlockOutput, AppError> {
    if block.new_start == 0 || block.new_end == 0 {
        block.attribution = Some(BlockAttributionOutput {
            confidence: "inferred".to_string(),
            warnings: vec![AttributionWarningOutput {
                code: "DELETION_TRACE_INCOMPLETE".to_string(),
                message: "删除块无法通过最终内容 blame，已根据提交差异推断".to_string(),
            }],
        });
        return Ok(block);
    }

    let blame_lines = crate::git::blame::blame_range(
        context.repo,
        head_commit,
        file_path,
        block.new_start,
        block.new_end,
    )
    .unwrap_or_default();
    if blame_lines.is_empty() {
        block.attribution = Some(BlockAttributionOutput {
            confidence: "inferred".to_string(),
            warnings: vec![AttributionWarningOutput {
                code: "BLAME_UNAVAILABLE".to_string(),
                message: "无法使用 blame 结果，已根据提交差异推断".to_string(),
            }],
        });
        return Ok(block);
    }

    let mut related = Vec::new();
    for line in blame_lines {
        let commit = commit_lookup::get_commit(context, &line.commit_hash)?;
        if related.iter().any(|item: &RelatedCommitOutput| item.hash == commit.hash) {
            continue;
        }
        related.push(RelatedCommitOutput {
            hash: commit.hash.clone(),
            short_hash: commit.short_hash,
            author_name: commit.author_name,
            author_email: commit.author_email,
            committed_at: commit.committed_at,
            subject: commit.subject,
            matched_by_filter: false,
            touched_ranges: block.related_commits
                .iter()
                .find(|candidate| candidate.hash == commit.hash)
                .map(|candidate| candidate.touched_ranges.clone())
                .unwrap_or_default(),
            attribution: Some(RelatedCommitAttributionOutput {
                method: if commit.is_merge { "merge-trace" } else { "blame" }.to_string(),
                via_merge_hashes: if commit.is_merge { vec![commit.hash] } else { Vec::new() },
            }),
        });
    }

    block.authors = unique_authors(&related);
    block.related_commits = related;
    block.attribution = Some(BlockAttributionOutput {
        confidence: "precise".to_string(),
        warnings: Vec::new(),
    });
    Ok(block)
}

fn unique_authors(commits: &[RelatedCommitOutput]) -> Vec<AuthorOutput> {
    let mut authors = Vec::new();
    for commit in commits {
        let author = AuthorOutput {
            name: commit.author_name.clone(),
            email: commit.author_email.clone(),
        };
        if !authors.iter().any(|item: &AuthorOutput| item.name == author.name && item.email == author.email) {
            authors.push(author);
        }
    }
    authors
}
```

- [ ] **步骤 4：在 overlay 中接入 blame**

修改 `crates/revier-analysis/src/overlay/file_overlay.rs`，在 patch inference 后追加：

```rust
    let blocks = crate::attribution::blame::attach_blame_attribution(
        &context,
        &args.common.head,
        &change.path,
        blocks,
    )?;
```

- [ ] **步骤 5：运行测试确认通过**

运行：

```powershell
cargo test -p revier-analysis --test overlay_attribution added_or_modified_block_uses_blame_as_precise_attribution
cargo test -p revier-analysis --test overlay_attribution file_overlay_attaches_related_commit_from_patch_inference
```

预期：通过。

- [ ] **步骤 6：提交**

运行：

```powershell
git add crates/revier-analysis/src/attribution/mod.rs crates/revier-analysis/src/attribution/blame.rs crates/revier-analysis/src/overlay/file_overlay.rs crates/revier-analysis/tests/overlay_attribution.rs
git commit -m "feat: 添加 Rust overlay blame 归因"
```

## 任务 7：实现 merge trace 与歧义标记

**文件：**
- 修改：`crates/revier-analysis/src/attribution/mod.rs`
- 新建：`crates/revier-analysis/src/attribution/merge_trace.rs`
- 修改：`crates/revier-analysis/src/attribution/blame.rs`
- 修改：`crates/revier-analysis/tests/fixtures.rs`
- 修改：`crates/revier-analysis/tests/overlay_attribution.rs`

- [ ] **步骤 1：补充多父歧义 fixture**

修改 `crates/revier-analysis/tests/fixtures.rs`，追加：

```rust
pub fn multi_parent_ambiguous() -> FixtureRepo {
    let repo = init_repo("multi-parent-ambiguous");
    write_file(repo.path(), "src/app.txt", "base\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: base"]);
    let base = rev_parse(repo.path(), "HEAD");

    git(repo.path(), ["checkout", "-b", "left"]);
    write_file(repo.path(), "src/app.txt", "base\nshared\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: left shared"]);

    git(repo.path(), ["checkout", "main"]);
    git(repo.path(), ["checkout", "-b", "right"]);
    write_file(repo.path(), "src/app.txt", "base\nshared\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: right shared"]);

    git(repo.path(), ["checkout", "main"]);
    git(repo.path(), ["merge", "--no-ff", "left", "-m", "merge: left"]);
    git(repo.path(), ["merge", "--no-ff", "right", "-m", "merge: right"]);
    let head = rev_parse(repo.path(), "HEAD");

    FixtureRepo {
        name: "multi-parent-ambiguous",
        repo,
        base,
        head,
    }
}
```

- [ ] **步骤 2：写失败的 merge trace 测试**

修改 `crates/revier-analysis/tests/overlay_attribution.rs`，追加：

```rust
#[test]
fn merge_conflict_resolution_attributes_new_content_to_merge_commit() {
    let fixture = fixtures::merge_conflict();
    let output = Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
        .args([
            "file-overlay",
            "--repo",
            fixture.repo.path().to_str().expect("repo path"),
            "--base",
            &fixture.base,
            "--head",
            &fixture.head,
            "--branch",
            "main",
            "--file",
            "src/app.txt",
            "--format",
            "json",
        ])
        .output()
        .expect("运行 merge file-overlay");

    assert!(output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).expect("解析 overlay json");
    let block = &json["overlay"]["blocks"][0];
    assert_eq!(block["attribution"]["confidence"], "precise");
    assert!(
        block["relatedCommits"]
            .as_array()
            .expect("related commits")
            .iter()
            .any(|commit| commit["attribution"]["method"] == "merge-trace"
                && commit["attribution"]["viaMergeHashes"].as_array().expect("via").len() >= 1)
    );
}

#[test]
fn ambiguous_merge_sources_are_marked_partial() {
    let fixture = fixtures::multi_parent_ambiguous();
    let output = Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
        .args([
            "file-overlay",
            "--repo",
            fixture.repo.path().to_str().expect("repo path"),
            "--base",
            &fixture.base,
            "--head",
            &fixture.head,
            "--branch",
            "main",
            "--file",
            "src/app.txt",
            "--format",
            "json",
        ])
        .output()
        .expect("运行 ambiguous merge file-overlay");

    assert!(output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).expect("解析 overlay json");
    let block = &json["overlay"]["blocks"][0];
    assert_eq!(block["attribution"]["confidence"], "partial");
    assert!(
        block["attribution"]["warnings"]
            .as_array()
            .expect("warnings")
            .iter()
            .any(|warning| warning["code"] == "MERGE_TRACE_AMBIGUOUS")
    );
}
```

- [ ] **步骤 3：运行测试确认失败**

运行：

```powershell
cargo test -p revier-analysis --test overlay_attribution merge_conflict_resolution_attributes_new_content_to_merge_commit
cargo test -p revier-analysis --test overlay_attribution ambiguous_merge_sources_are_marked_partial
```

预期：失败，merge commit 仍只标记 blame 或未识别 partial。

- [ ] **步骤 4：实现 merge trace**

修改 `crates/revier-analysis/src/attribution/mod.rs`：

```rust
pub mod blame;
pub mod commit_lookup;
pub mod context;
pub mod merge_trace;
pub mod patch_inference;
```

新建 `crates/revier-analysis/src/attribution/merge_trace.rs`：

```rust
use crate::attribution::commit_lookup;
use crate::attribution::context::AttributionContext;
use crate::error::AppError;
use crate::git::blob;
use crate::json::{
    AttributionWarningOutput, BlockAttributionOutput, DiffBlockOutput,
    RelatedCommitAttributionOutput, RelatedCommitOutput,
};

pub fn resolve_merge_commit(
    context: &AttributionContext<'_>,
    merge_hash: &str,
    file_path: &str,
    block: &DiffBlockOutput,
) -> Result<MergeTraceOutcome, AppError> {
    let merge = commit_lookup::get_commit(context, merge_hash)?;
    if merge.parents.len() <= 1 {
        return Ok(MergeTraceOutcome::NotMerge);
    }

    let block_lines = block
        .rows
        .iter()
        .filter_map(|row| row.new_text.as_deref())
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>();
    let mut explaining_parents = Vec::new();
    for parent in &merge.parents {
        let parent_text = blob::read_text_at_commit(context.repo, parent, file_path).unwrap_or_default();
        if block_lines.iter().all(|line| parent_text.lines().any(|parent_line| parent_line == *line)) {
            explaining_parents.push(parent.clone());
        }
    }

    if explaining_parents.len() == 1 {
        let parent_hash = &explaining_parents[0];
        let parent_commit = commit_lookup::get_commit(context, parent_hash)?;
        Ok(MergeTraceOutcome::Resolved(vec![RelatedCommitOutput {
            hash: parent_commit.hash,
            short_hash: parent_commit.short_hash,
            author_name: parent_commit.author_name,
            author_email: parent_commit.author_email,
            committed_at: parent_commit.committed_at,
            subject: parent_commit.subject,
            matched_by_filter: false,
            touched_ranges: Vec::new(),
            attribution: Some(RelatedCommitAttributionOutput {
                method: "merge-trace".to_string(),
                via_merge_hashes: vec![merge_hash.to_string()],
            }),
        }]))
    } else if explaining_parents.len() > 1 {
        let mut related = Vec::new();
        for parent_hash in explaining_parents {
            let parent_commit = commit_lookup::get_commit(context, &parent_hash)?;
            related.push(RelatedCommitOutput {
                hash: parent_commit.hash,
                short_hash: parent_commit.short_hash,
                author_name: parent_commit.author_name,
                author_email: parent_commit.author_email,
                committed_at: parent_commit.committed_at,
                subject: parent_commit.subject,
                matched_by_filter: false,
                touched_ranges: Vec::new(),
                attribution: Some(RelatedCommitAttributionOutput {
                    method: "merge-trace".to_string(),
                    via_merge_hashes: vec![merge_hash.to_string()],
                }),
            });
        }
        Ok(MergeTraceOutcome::Ambiguous(related))
    } else {
        Ok(MergeTraceOutcome::Resolved(vec![RelatedCommitOutput {
            hash: merge.hash,
            short_hash: merge.short_hash,
            author_name: merge.author_name,
            author_email: merge.author_email,
            committed_at: merge.committed_at,
            subject: merge.subject,
            matched_by_filter: false,
            touched_ranges: Vec::new(),
            attribution: Some(RelatedCommitAttributionOutput {
                method: "merge-trace".to_string(),
                via_merge_hashes: vec![merge_hash.to_string()],
            }),
        }]))
    }
}

pub enum MergeTraceOutcome {
    NotMerge,
    Resolved(Vec<RelatedCommitOutput>),
    Ambiguous(Vec<RelatedCommitOutput>),
}

pub fn partial_attribution() -> BlockAttributionOutput {
    BlockAttributionOutput {
        confidence: "partial".to_string(),
        warnings: vec![AttributionWarningOutput {
            code: "MERGE_TRACE_AMBIGUOUS".to_string(),
            message: "部分行存在多条可能的 merge 来源".to_string(),
        }],
    }
}
```

- [ ] **步骤 5：在 blame 中接入 merge trace**

修改 `crates/revier-analysis/src/attribution/blame.rs`，在创建 related 后、设置 block 前，替换 merge commit：

```rust
    let mut traced = Vec::new();
    let mut partial = false;
    for commit in related {
        if commit
            .attribution
            .as_ref()
            .map(|value| value.method.as_str())
            == Some("merge-trace")
        {
            match crate::attribution::merge_trace::resolve_merge_commit(
                context,
                &commit.hash,
                file_path,
                &block,
            )? {
                crate::attribution::merge_trace::MergeTraceOutcome::NotMerge => traced.push(commit),
                crate::attribution::merge_trace::MergeTraceOutcome::Resolved(commits) => traced.extend(commits),
                crate::attribution::merge_trace::MergeTraceOutcome::Ambiguous(commits) => {
                    partial = true;
                    traced.extend(commits);
                }
            }
        } else {
            traced.push(commit);
        }
    }

    block.authors = unique_authors(&traced);
    block.related_commits = traced;
    block.attribution = Some(if partial {
        crate::attribution::merge_trace::partial_attribution()
    } else {
        BlockAttributionOutput {
            confidence: "precise".to_string(),
            warnings: Vec::new(),
        }
    });
```

删除同一函数里旧的 `block.authors = unique_authors(&related)`、`block.related_commits = related`、`block.attribution = Some(...)` 代码段。

- [ ] **步骤 6：运行测试确认通过**

运行：

```powershell
cargo test -p revier-analysis --test overlay_attribution merge_conflict_resolution_attributes_new_content_to_merge_commit
cargo test -p revier-analysis --test overlay_attribution ambiguous_merge_sources_are_marked_partial
cargo test -p revier-analysis --test no_git_process
```

预期：通过。

- [ ] **步骤 7：提交**

运行：

```powershell
git add crates/revier-analysis/src/attribution/mod.rs crates/revier-analysis/src/attribution/merge_trace.rs crates/revier-analysis/src/attribution/blame.rs crates/revier-analysis/tests/fixtures.rs crates/revier-analysis/tests/overlay_attribution.rs
git commit -m "feat: 添加 Rust overlay merge 追踪"
```

## 任务 8：实现删除块追踪与路径历史 warning

**文件：**
- 修改：`crates/revier-analysis/src/attribution/mod.rs`
- 新建：`crates/revier-analysis/src/attribution/deletion_trace.rs`
- 新建：`crates/revier-analysis/src/attribution/path_history.rs`
- 修改：`crates/revier-analysis/src/overlay/file_overlay.rs`
- 修改：`crates/revier-analysis/tests/fixtures.rs`
- 修改：`crates/revier-analysis/tests/overlay_attribution.rs`

- [ ] **步骤 1：补充删除 merge fixture**

修改 `crates/revier-analysis/tests/fixtures.rs`，追加：

```rust
pub fn deletion_merge() -> FixtureRepo {
    let repo = init_repo("deletion-merge");
    write_file(repo.path(), "src/app.txt", "keep\ndelete me\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "feat: base"]);
    let base = rev_parse(repo.path(), "HEAD");

    git(repo.path(), ["checkout", "-b", "feature"]);
    write_file(repo.path(), "src/app.txt", "keep\n");
    git(repo.path(), ["add", "."]);
    git(repo.path(), ["commit", "-m", "fix: delete line"]);

    git(repo.path(), ["checkout", "main"]);
    git(repo.path(), ["merge", "--no-ff", "feature", "-m", "merge: deletion"]);
    let head = rev_parse(repo.path(), "HEAD");

    FixtureRepo {
        name: "deletion-merge",
        repo,
        base,
        head,
    }
}
```

- [ ] **步骤 2：写失败的删除和 rename 测试**

修改 `crates/revier-analysis/tests/overlay_attribution.rs`，追加：

```rust
#[test]
fn deleted_block_uses_deletion_trace_or_inference() {
    let fixture = fixtures::deletion_merge();
    let output = Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
        .args([
            "file-overlay",
            "--repo",
            fixture.repo.path().to_str().expect("repo path"),
            "--base",
            &fixture.base,
            "--head",
            &fixture.head,
            "--branch",
            "main",
            "--file",
            "src/app.txt",
            "--format",
            "json",
        ])
        .output()
        .expect("运行 deletion file-overlay");

    assert!(output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).expect("解析 overlay json");
    let block = &json["overlay"]["blocks"][0];
    assert_eq!(block["changeType"], "deleted");
    assert!(
        matches!(
            block["attribution"]["confidence"].as_str(),
            Some("precise") | Some("inferred") | Some("partial")
        )
    );
    assert!(
        block["relatedCommits"]
            .as_array()
            .expect("related commits")
            .iter()
            .any(|commit| commit["subject"].as_str().unwrap_or("").contains("delete line"))
    );
}

#[test]
fn rename_overlay_uses_old_path_and_does_not_drop_blocks() {
    let fixture = fixtures::rename_merge();
    let output = Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
        .args([
            "file-overlay",
            "--repo",
            fixture.repo.path().to_str().expect("repo path"),
            "--base",
            &fixture.base,
            "--head",
            &fixture.head,
            "--branch",
            "main",
            "--file",
            "src/new.txt",
            "--format",
            "json",
        ])
        .output()
        .expect("运行 rename file-overlay");

    assert!(output.status.success());
    let json: Value = serde_json::from_slice(&output.stdout).expect("解析 overlay json");
    assert_eq!(json["overlay"]["file"]["oldPath"], "src/old.txt");
    assert!(json["overlay"]["blocks"].as_array().expect("blocks").len() >= 1);
}
```

- [ ] **步骤 3：运行测试确认失败**

运行：

```powershell
cargo test -p revier-analysis --test overlay_attribution deleted_block_uses_deletion_trace_or_inference
cargo test -p revier-analysis --test overlay_attribution rename_overlay_uses_old_path_and_does_not_drop_blocks
```

预期：删除块没有稳定 related commits，rename 可能路径不完整。

- [ ] **步骤 4：实现路径历史候选**

修改 `crates/revier-analysis/src/attribution/mod.rs`：

```rust
pub mod blame;
pub mod commit_lookup;
pub mod context;
pub mod deletion_trace;
pub mod merge_trace;
pub mod patch_inference;
pub mod path_history;
```

新建 `crates/revier-analysis/src/attribution/path_history.rs`：

```rust
use crate::attribution::context::AttributionContext;
use crate::error::AppError;

pub fn path_candidates(
    context: &AttributionContext<'_>,
    base_commit: &str,
    head_commit: &str,
    path: &str,
    old_path: Option<&str>,
) -> Result<(Vec<String>, Vec<String>), AppError> {
    let mut paths = vec![path.to_string()];
    if let Some(old_path) = old_path {
        push_unique(&mut paths, old_path.to_string());
    }
    for rename in crate::git::diff::rename_changes(context.repo, base_commit, head_commit)? {
        if rename.new_path == path || rename.old_path == path {
            push_unique(&mut paths, rename.old_path);
            push_unique(&mut paths, rename.new_path);
        }
    }
    let warnings = if old_path.is_none() && paths.len() == 1 {
        vec!["文件历史路径不完整，结果可能不完整".to_string()]
    } else {
        Vec::new()
    };
    Ok((paths, warnings))
}

fn push_unique(paths: &mut Vec<String>, path: String) {
    if !paths.iter().any(|candidate| candidate == &path) {
        paths.push(path);
    }
}
```

- [ ] **步骤 5：实现删除 trace**

新建 `crates/revier-analysis/src/attribution/deletion_trace.rs`：

```rust
use crate::attribution::{commit_lookup, context::AttributionContext};
use crate::error::AppError;
use crate::git::blob;
use crate::json::{
    AuthorOutput, BlockAttributionOutput, DiffBlockOutput, RelatedCommitAttributionOutput,
    RelatedCommitOutput,
};

pub fn attach_deletion_trace(
    context: &AttributionContext<'_>,
    mut blocks: Vec<DiffBlockOutput>,
    paths: &[String],
) -> Result<Vec<DiffBlockOutput>, AppError> {
    for block in &mut blocks {
        if block.change_type != "deleted" {
            continue;
        }
        let deleted_lines = block
            .rows
            .iter()
            .filter_map(|row| row.old_text.as_deref())
            .collect::<Vec<_>>();
        let mut related = Vec::new();
        for hash in &context.range_hashes {
            let commit = commit_lookup::get_commit(context, hash)?;
            for parent in &commit.parents {
                for path in paths {
                    let old_text = blob::read_text_at_commit(context.repo, parent, path).unwrap_or_default();
                    let new_text = blob::read_text_at_commit(context.repo, &commit.hash, path).unwrap_or_default();
                    if deleted_lines.iter().all(|line| old_text.lines().any(|old| old == *line))
                        && deleted_lines.iter().all(|line| !new_text.lines().any(|new| new == *line))
                    {
                        related.push(RelatedCommitOutput {
                            hash: commit.hash.clone(),
                            short_hash: commit.short_hash.clone(),
                            author_name: commit.author_name.clone(),
                            author_email: commit.author_email.clone(),
                            committed_at: commit.committed_at.clone(),
                            subject: commit.subject.clone(),
                            matched_by_filter: false,
                            touched_ranges: Vec::new(),
                            attribution: Some(RelatedCommitAttributionOutput {
                                method: "patch-inference".to_string(),
                                via_merge_hashes: if commit.is_merge {
                                    vec![commit.hash.clone()]
                                } else {
                                    Vec::new()
                                },
                            }),
                        });
                    }
                }
            }
        }
        dedup_commits(&mut related);
        if !related.is_empty() {
            block.authors = unique_authors(&related);
            block.related_commits = related;
            block.attribution = Some(BlockAttributionOutput {
                confidence: "inferred".to_string(),
                warnings: Vec::new(),
            });
        }
    }
    Ok(blocks)
}

fn dedup_commits(commits: &mut Vec<RelatedCommitOutput>) {
    let mut seen = Vec::new();
    commits.retain(|commit| {
        if seen.iter().any(|hash| hash == &commit.hash) {
            false
        } else {
            seen.push(commit.hash.clone());
            true
        }
    });
}

fn unique_authors(commits: &[RelatedCommitOutput]) -> Vec<AuthorOutput> {
    let mut authors = Vec::new();
    for commit in commits {
        let author = AuthorOutput {
            name: commit.author_name.clone(),
            email: commit.author_email.clone(),
        };
        if !authors.iter().any(|item: &AuthorOutput| item.name == author.name && item.email == author.email) {
            authors.push(author);
        }
    }
    authors
}
```

- [ ] **步骤 6：在 overlay 中接入路径候选和删除 trace**

修改 `crates/revier-analysis/src/overlay/file_overlay.rs`，在 patch inference 前计算路径候选：

```rust
    let (path_candidates, path_warnings) = crate::attribution::path_history::path_candidates(
        &context,
        &args.common.base,
        &args.common.head,
        &change.path,
        change.old_path.as_deref(),
    )?;
```

将 patch inference 调用中的路径参数保持当前路径和 old path，随后在 blame 后追加：

```rust
    let blocks = crate::attribution::deletion_trace::attach_deletion_trace(
        &context,
        blocks,
        &path_candidates,
    )?;
```

输出 warnings 改为：

```rust
    let mut warnings = context.warnings.clone();
    warnings.extend(path_warnings);
```

并将 `warnings: context.warnings.clone()` 和 `warnings: context.warnings` 改为使用 `warnings.clone()` 与 `warnings`。

- [ ] **步骤 7：运行测试确认通过**

运行：

```powershell
cargo test -p revier-analysis --test overlay_attribution deleted_block_uses_deletion_trace_or_inference
cargo test -p revier-analysis --test overlay_attribution rename_overlay_uses_old_path_and_does_not_drop_blocks
cargo test -p revier-analysis --test file_overlay_cli
```

预期：通过。

- [ ] **步骤 8：提交**

运行：

```powershell
git add crates/revier-analysis/src/attribution/mod.rs crates/revier-analysis/src/attribution/deletion_trace.rs crates/revier-analysis/src/attribution/path_history.rs crates/revier-analysis/src/overlay/file_overlay.rs crates/revier-analysis/tests/fixtures.rs crates/revier-analysis/tests/overlay_attribution.rs
git commit -m "feat: 添加 Rust overlay 删除与路径追踪"
```

## 任务 9：实现 `trace-block`

**文件：**
- 修改：`crates/revier-analysis/src/commands/trace_block.rs`
- 新建：`crates/revier-analysis/tests/trace_block_cli.rs`

- [ ] **步骤 1：写失败的 trace-block 测试**

创建 `crates/revier-analysis/tests/trace_block_cli.rs`：

```rust
mod fixtures;

use serde_json::Value;
use std::process::Command;

#[test]
fn trace_block_returns_single_block_attribution_by_block_id() {
    let fixture = fixtures::linear_with_authors();
    let output = Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
        .args([
            "trace-block",
            "--repo",
            fixture.repo.path().to_str().expect("repo path"),
            "--base",
            &fixture.base,
            "--head",
            &fixture.head,
            "--branch",
            "main",
            "--file",
            "src/app.txt",
            "--block-id",
            "block-1",
            "--format",
            "json",
        ])
        .output()
        .expect("运行 trace-block");

    assert!(
        output.status.success(),
        "trace-block 应成功，stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json: Value = serde_json::from_slice(&output.stdout).expect("解析 trace json");
    assert_eq!(json["version"], 1);
    assert_eq!(json["file"], "src/app.txt");
    assert_eq!(json["blockId"], "block-1");
    assert!(json["relatedCommits"].as_array().expect("related").len() >= 1);
}

#[test]
fn trace_block_returns_exit_code_two_when_selector_is_missing() {
    let fixture = fixtures::linear();
    let output = Command::new(env!("CARGO_BIN_EXE_revier-analysis"))
        .args([
            "trace-block",
            "--repo",
            fixture.repo.path().to_str().expect("repo path"),
            "--base",
            &fixture.base,
            "--head",
            &fixture.head,
            "--branch",
            "main",
            "--file",
            "src/app.txt",
            "--format",
            "json",
        ])
        .output()
        .expect("运行 trace-block");

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("参数错误"));
}
```

- [ ] **步骤 2：运行测试确认失败**

运行：

```powershell
cargo test -p revier-analysis --test trace_block_cli
```

预期：失败，命令尚未实现。

- [ ] **步骤 3：实现 trace-block**

替换 `crates/revier-analysis/src/commands/trace_block.rs`：

```rust
use crate::cli::{FileOverlayArgs, TraceBlockArgs};
use crate::error::AppError;
use crate::json::TraceBlockOutput;

pub fn run(args: TraceBlockArgs) -> Result<String, AppError> {
    validate_selector(&args)?;
    let repo = crate::git::repository::open_repository(&args.common.repo)?;
    let overlay_args = FileOverlayArgs {
        common: args.common,
        file: args.file.clone(),
    };
    let output = crate::overlay::file_overlay::build_file_overlay(&repo, &overlay_args)?;
    let block = if let Some(block_id) = &args.block_id {
        output
            .overlay
            .blocks
            .into_iter()
            .find(|block| &block.id == block_id)
    } else {
        output.overlay.blocks.into_iter().find(|block| {
            line_matches(block.old_start, block.old_end, args.old_start, args.old_end)
                && line_matches(block.new_start, block.new_end, args.new_start, args.new_end)
        })
    }
    .ok_or_else(|| AppError::InvalidArgument("未找到匹配的 block".to_string()))?;

    let trace = TraceBlockOutput {
        version: 1,
        file: args.file,
        block_id: block.id,
        attribution: block.attribution,
        authors: block.authors,
        related_commits: block.related_commits,
        warnings: output.warnings,
    };
    crate::serialize_json(&trace, overlay_args.common.pretty)
}

fn validate_selector(args: &TraceBlockArgs) -> Result<(), AppError> {
    if args.block_id.is_some() {
        return Ok(());
    }
    let has_old = args.old_start.is_some() && args.old_end.is_some();
    let has_new = args.new_start.is_some() && args.new_end.is_some();
    if has_old || has_new {
        Ok(())
    } else {
        Err(AppError::InvalidArgument(
            "必须传入 --block-id 或显式行区间".to_string(),
        ))
    }
}

fn line_matches(
    block_start: usize,
    block_end: usize,
    requested_start: Option<usize>,
    requested_end: Option<usize>,
) -> bool {
    match (requested_start, requested_end) {
        (Some(start), Some(end)) if block_start > 0 && block_end > 0 => {
            block_start <= end && start <= block_end
        }
        _ => false,
    }
}
```

- [ ] **步骤 4：运行测试确认通过**

运行：

```powershell
cargo test -p revier-analysis --test trace_block_cli
cargo test -p revier-analysis --test file_overlay_cli
cargo test -p revier-analysis --test overlay_attribution
```

预期：通过。

- [ ] **步骤 5：提交**

运行：

```powershell
git add crates/revier-analysis/src/commands/trace_block.rs crates/revier-analysis/tests/trace_block_cli.rs
git commit -m "feat: 实现 Rust trace-block 命令"
```

## 任务 10：扩展 Electron `RustAnalysisClient`

**文件：**
- 修改：`src/main/analysis/rustAnalysisClient.ts`
- 修改：`tests/unit/rustAnalysisClient.test.ts`

- [ ] **步骤 1：写失败的客户端测试**

修改 `tests/unit/rustAnalysisClient.test.ts`，追加：

```ts
  it('assembles file-overlay arguments and parses overlay output', async () => {
    const executor: RustAnalysisExecutor = vi.fn(async () => ({
      exitCode: 0,
      stdout: JSON.stringify({
        version: 1,
        overlay: {
          mode: 'range',
          file: {
            path: 'src/app.ts',
            oldPath: null,
            status: 'modified',
            additions: 1,
            deletions: 1,
            isBinary: false,
            isPreviewable: true
          },
          range: {
            branch: 'main',
            baseCommit: 'base',
            headCommit: 'head',
            startAt: null,
            endAt: null
          },
          rows: [],
          blocks: [],
          warnings: []
        },
        warnings: []
      }),
      stderr: ''
    }));
    const client = new RustAnalysisClient({ binaryPath: 'revier-analysis', executor });

    const result = await client.getFileOverlay({
      repoPath: 'E:/repo/app',
      baseCommit: 'base',
      headCommit: 'head',
      branch: 'main',
      filePath: 'src/app.ts',
      globRules: ['src/**/*.ts']
    });

    expect(executor).toHaveBeenCalledWith('revier-analysis', [
      'file-overlay',
      '--repo',
      'E:/repo/app',
      '--base',
      'base',
      '--head',
      'head',
      '--branch',
      'main',
      '--file',
      'src/app.ts',
      '--glob',
      'src/**/*.ts',
      '--format',
      'json'
    ]);
    expect(result.mode).toBe('range');
    expect(result.file.oldPath).toBeUndefined();
  });

  it('assembles trace-block arguments and parses trace output', async () => {
    const executor: RustAnalysisExecutor = vi.fn(async () => ({
      exitCode: 0,
      stdout: JSON.stringify({
        version: 1,
        file: 'src/app.ts',
        blockId: 'block-1',
        attribution: { confidence: 'precise', warnings: [] },
        authors: [{ name: 'Alice', email: 'alice@example.com' }],
        relatedCommits: [],
        warnings: []
      }),
      stderr: ''
    }));
    const client = new RustAnalysisClient({ binaryPath: 'revier-analysis', executor });

    const result = await client.traceBlock({
      repoPath: 'E:/repo/app',
      baseCommit: 'base',
      headCommit: 'head',
      branch: 'main',
      filePath: 'src/app.ts',
      blockId: 'block-1',
      globRules: []
    });

    expect(executor).toHaveBeenCalledWith('revier-analysis', [
      'trace-block',
      '--repo',
      'E:/repo/app',
      '--base',
      'base',
      '--head',
      'head',
      '--branch',
      'main',
      '--file',
      'src/app.ts',
      '--block-id',
      'block-1',
      '--format',
      'json'
    ]);
    expect(result.blockId).toBe('block-1');
    expect(result.attribution?.confidence).toBe('precise');
  });
```

- [ ] **步骤 2：运行测试确认失败**

运行：

```powershell
pnpm test -- tests/unit/rustAnalysisClient.test.ts
```

预期：失败，客户端方法尚不存在。

- [ ] **步骤 3：扩展客户端类型和方法**

修改 `src/main/analysis/rustAnalysisClient.ts`，导入类型改为：

```ts
import type {
  ChangedFile,
  FileOverlay,
  ReviewFilters,
  RelatedCommit,
  AuthorSummary,
  BlockAttributionSummary
} from '../../shared/reviewTypes';
```

增加请求类型：

```ts
export interface FileOverlayCliRequest {
  repoPath: string;
  baseCommit: string;
  headCommit: string;
  branch: string;
  filePath: string;
  startAt?: string;
  endAt?: string;
  authorKeys?: string[];
  authorQuery?: string;
  messageQuery?: string;
  globRules: string[];
  requireIndex?: boolean;
}

export interface TraceBlockRequest extends FileOverlayCliRequest {
  blockId?: string;
  oldStart?: number;
  oldEnd?: number;
  newStart?: number;
  newEnd?: number;
}

export interface TraceBlockResult {
  version: number;
  file: string;
  blockId: string;
  attribution?: BlockAttributionSummary;
  authors: AuthorSummary[];
  relatedCommits: RelatedCommit[];
  warnings: string[];
}
```

在 `RustAnalysisClient` 类中增加：

```ts
  async getFileOverlay(request: FileOverlayCliRequest): Promise<FileOverlay> {
    const args = buildFileOverlayArgs(request);
    const result = await this.executor(this.binaryPath, args);
    if (result.exitCode !== 0) {
      throw mapRustError(result.exitCode, result.stderr);
    }
    const parsed = JSON.parse(result.stdout) as { overlay: FileOverlay };
    return normalizeOverlay(parsed.overlay);
  }

  async traceBlock(request: TraceBlockRequest): Promise<TraceBlockResult> {
    const args = buildTraceBlockArgs(request);
    const result = await this.executor(this.binaryPath, args);
    if (result.exitCode !== 0) {
      throw mapRustError(result.exitCode, result.stderr);
    }
    return JSON.parse(result.stdout) as TraceBlockResult;
  }
```

追加参数构建函数：

```ts
export function buildFileOverlayArgs(request: FileOverlayCliRequest): string[] {
  const args = buildOverlayCommonArgs('file-overlay', request);
  args.push('--file', request.filePath);
  args.push('--format', 'json');
  return args;
}

export function buildTraceBlockArgs(request: TraceBlockRequest): string[] {
  const args = buildOverlayCommonArgs('trace-block', request);
  args.push('--file', request.filePath);
  if (request.blockId) {
    args.push('--block-id', request.blockId);
  }
  if (request.oldStart !== undefined) {
    args.push('--old-start', String(request.oldStart));
  }
  if (request.oldEnd !== undefined) {
    args.push('--old-end', String(request.oldEnd));
  }
  if (request.newStart !== undefined) {
    args.push('--new-start', String(request.newStart));
  }
  if (request.newEnd !== undefined) {
    args.push('--new-end', String(request.newEnd));
  }
  args.push('--format', 'json');
  return args;
}

function buildOverlayCommonArgs(command: string, request: FileOverlayCliRequest): string[] {
  const args = [
    command,
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
  for (const glob of request.globRules) {
    args.push('--glob', glob);
  }
  if (request.requireIndex) {
    args.push('--require-index');
  }
  return args;
}
```

追加归一化函数：

```ts
function normalizeOverlay(overlay: FileOverlay): FileOverlay {
  return {
    ...overlay,
    file: {
      ...overlay.file,
      oldPath: overlay.file.oldPath ?? undefined
    },
    range: {
      ...overlay.range,
      startAt: overlay.range.startAt ?? undefined,
      endAt: overlay.range.endAt ?? undefined
    }
  };
}
```

- [ ] **步骤 4：调整错误码映射**

修改 `mapRustError()`：

```ts
function mapRustError(exitCode: number, stderr: string): RustAnalysisError {
  const message = stderr.trim() || `Rust 分析进程退出码 ${exitCode}`;
  if ([4, 5, 6].includes(exitCode)) {
    return new RustAnalysisError('RUST_ANALYSIS_UNAVAILABLE', message, true);
  }
  if (exitCode === 2 || exitCode === 3) {
    return new RustAnalysisError('RUST_QUERY_FAILED', message, true);
  }
  return new RustAnalysisError('RUST_INTERNAL_ERROR', message, false);
}
```

- [ ] **步骤 5：运行测试确认通过**

运行：

```powershell
pnpm test -- tests/unit/rustAnalysisClient.test.ts
```

预期：通过。

- [ ] **步骤 6：提交**

运行：

```powershell
git add src/main/analysis/rustAnalysisClient.ts tests/unit/rustAnalysisClient.test.ts
git commit -m "feat: 扩展 Rust overlay 客户端"
```

## 任务 11：Electron 主进程按开关接入 Rust overlay

**文件：**
- 修改：`src/main/ipc/reviewIpc.ts`
- 修改：`tests/unit/reviewIpc.test.ts`

- [ ] **步骤 1：写失败的 reviewIpc 测试**

修改 `tests/unit/reviewIpc.test.ts`，追加：

```ts
  it('uses Rust file overlay when the overlay flag is enabled', async () => {
    const range: AnalysisRange = {
      branch: 'main',
      baseCommit: 'base',
      headCommit: 'head'
    };
    const rustOverlay = {
      mode: 'range' as const,
      file: modifiedFile,
      range,
      rows: [],
      blocks: [],
      warnings: []
    };

    const overlay = await buildFileOverlayForTask({
      project,
      file: modifiedFile,
      range,
      rangeCommits: [],
      filters: {
        projectId: project.id,
        branch: 'main',
        globRules: ['src/**/*.ts']
      },
      git: {
        listCommits: vi.fn(),
        listChangedFiles: vi.fn(),
        readFileAtCommit: vi.fn(),
        showFilePatch: vi.fn()
      },
      rust: {
        getFileOverlay: vi.fn(async () => rustOverlay)
      },
      useRustOverlay: true
    });

    expect(overlay).toBe(rustOverlay);
  });

  it('falls back to TypeScript file overlay when Rust overlay fails recoverably', async () => {
    const range: AnalysisRange = {
      branch: 'main',
      baseCommit: 'base',
      headCommit: 'head'
    };

    const overlay = await buildFileOverlayForTask({
      project,
      file: modifiedFile,
      range,
      rangeCommits: [commit('alice', '2026-05-10T00:00:00.000Z', 'Alice', 'feature: update app')],
      filters: {
        projectId: project.id,
        branch: 'main',
        globRules: []
      },
      git: {
        listCommits: vi.fn(),
        listChangedFiles: vi.fn(),
        readFileAtCommit: vi.fn(async (_repoPath, commitHash) =>
          commitHash === 'base' ? 'const name = "old";\n' : 'const name = "new";\n'
        ),
        showFilePatch: vi.fn(async () => '@@ -1 +1 @@\n-old\n+new\n')
      },
      rust: {
        getFileOverlay: vi.fn(async () => {
          throw Object.assign(new Error('文件不可分析'), { recoverable: true });
        })
      },
      useRustOverlay: true
    });

    expect(overlay.mode).toBe('range');
    expect(overlay.blocks).toHaveLength(1);
  });
```

- [ ] **步骤 2：运行测试确认失败**

运行：

```powershell
pnpm test -- tests/unit/reviewIpc.test.ts
```

预期：失败，`buildFileOverlayForTask` 尚不接收 `rust` 和 `useRustOverlay`。

- [ ] **步骤 3：扩展接口和调用**

修改 `src/main/ipc/reviewIpc.ts`，扩展导入：

```ts
import {
  RustAnalysisClient,
  type QueryFilesResult,
  type FileOverlayCliRequest
} from '../analysis/rustAnalysisClient';
```

新增接口：

```ts
interface RustOverlayClient {
  getFileOverlay(request: FileOverlayCliRequest): Promise<FileOverlay>;
}
```

修改 `BuildFileOverlayInput`：

```ts
interface BuildFileOverlayInput {
  project: ReviewProject;
  file: ChangedFile;
  range: AnalysisRange;
  rangeCommits: GitCommitSummary[];
  filters: ReviewFilters;
  git: AnalysisGitClient;
  rust?: RustOverlayClient;
  useRustOverlay?: boolean;
}
```

在 `buildFileOverlayForTask()` 的二进制检查后、TypeScript overlay 前加入：

```ts
  if (useRustOverlay && rust) {
    try {
      return await rust.getFileOverlay({
        repoPath: project.repoPath,
        baseCommit: range.baseCommit,
        headCommit: range.headCommit,
        branch: range.branch,
        filePath: file.path,
        startAt: range.startAt,
        endAt: range.endAt,
        authorKeys: filters.authorKeys,
        authorQuery: filters.authorQuery,
        messageQuery: filters.messageQuery,
        globRules: filters.globRules
      });
    } catch (error) {
      if (!isRecoverableRustError(error)) {
        throw error;
      }
    }
  }
```

在 `registerReviewIpc()` 调用 `buildFileOverlayForTask` 时传入：

```ts
      rust,
      useRustOverlay: process.env.REVIER_RUST_OVERLAY === '1'
```

- [ ] **步骤 4：运行测试确认通过**

运行：

```powershell
pnpm test -- tests/unit/reviewIpc.test.ts tests/unit/rustAnalysisClient.test.ts
```

预期：通过。

- [ ] **步骤 5：提交**

运行：

```powershell
git add src/main/ipc/reviewIpc.ts tests/unit/reviewIpc.test.ts
git commit -m "feat: 接入 Rust overlay 降级路径"
```

## 任务 12：添加调试脚本、最终验证与记录

**文件：**
- 修改：`tests/unit/scaffold.test.ts`
- 修改：`package.json`
- 新建：`docs/superpowers/verification/2026-07-02-rust-overlay-attribution.md`

- [ ] **步骤 1：写失败的脚本测试**

修改 `tests/unit/scaffold.test.ts`，追加：

```ts
it('exposes Rust overlay debug scripts', () => {
  const pkg = JSON.parse(readFileSync(join(process.cwd(), 'package.json'), 'utf8')) as {
    scripts: Record<string, string>;
  };

  expect(pkg.scripts['rust:file-overlay']).toBe('cargo run -p revier-analysis -- file-overlay');
  expect(pkg.scripts['rust:trace-block']).toBe('cargo run -p revier-analysis -- trace-block');
});
```

如果文件顶部尚未导入 `readFileSync` 和 `join`，补充：

```ts
import { readFileSync } from 'node:fs';
import { join } from 'node:path';
```

- [ ] **步骤 2：运行测试确认失败**

运行：

```powershell
pnpm test -- tests/unit/scaffold.test.ts
```

预期：失败，脚本尚未添加。

- [ ] **步骤 3：添加 package 脚本**

修改 `package.json` 的 `scripts`，保留现有脚本并追加：

```json
{
  "rust:file-overlay": "cargo run -p revier-analysis -- file-overlay",
  "rust:trace-block": "cargo run -p revier-analysis -- trace-block"
}
```

- [ ] **步骤 4：运行脚本测试确认通过**

运行：

```powershell
pnpm test -- tests/unit/scaffold.test.ts
```

预期：通过。

- [ ] **步骤 5：运行最终验证命令**

运行：

```powershell
cargo test --workspace
pnpm typecheck
pnpm test
```

预期：三个命令全部通过。

- [ ] **步骤 6：执行 CLI 冒烟验证**

运行：

```powershell
$Base = git -C E:/Projects/revier rev-parse HEAD~1
$Head = git -C E:/Projects/revier rev-parse HEAD
cargo run -p revier-analysis -- file-overlay --repo E:/Projects/revier --base $Base --head $Head --branch main --file src/main/ipc/reviewIpc.ts --format json --pretty
cargo run -p revier-analysis -- trace-block --repo E:/Projects/revier --base $Base --head $Head --branch main --file src/main/ipc/reviewIpc.ts --block-id block-1 --format json --pretty
```

预期：

- `file-overlay` 返回 `version: 1`、`overlay.mode: "range"`、`overlay.rows`、`overlay.blocks`。
- `trace-block` 返回 `version: 1`、`file`、`blockId`、`relatedCommits`。
- 若当前文件在 HEAD~1..HEAD 没有变化，改用 `git diff --name-only $Base..$Head` 中的任一文本文件重新运行。

- [ ] **步骤 7：创建验证记录文件**

创建 `docs/superpowers/verification/2026-07-02-rust-overlay-attribution.md`，内容结构：

```markdown
# Rust Overlay 与归因准确性验证记录

## 环境

- 日期：2026-07-02
- Shell：pwsh
- Node：记录 `node --version`
- pnpm：记录 `pnpm --version`
- Rust：记录 `rustc --version`

## 功能验证

- `cargo test --workspace`：记录退出状态和关键输出。
- `pnpm typecheck`：记录退出状态和关键输出。
- `pnpm test`：记录退出状态和关键输出。

## CLI 冒烟验证

- `file-overlay`：记录命令、退出状态、JSON 摘要。
- `trace-block`：记录命令、退出状态、JSON 摘要。

## 归因验证

- 线性新增/修改：记录 confidence、method、author。
- merge conflict：记录 viaMergeHashes。
- 多父歧义：记录 partial warning。
- 删除块：记录 deletion trace 或 inference 结果。
- rename：记录 oldPath 和路径 warning。

## 结论

- 说明 Rust overlay 是否满足本期验收标准。
- 若有失败命令，记录失败原因和后续修复任务，不得写成通过。
```

- [ ] **步骤 8：提交**

运行：

```powershell
git add tests/unit/scaffold.test.ts package.json docs/superpowers/verification/2026-07-02-rust-overlay-attribution.md
git commit -m "docs: 记录 Rust overlay 归因验证结果"
```

## 最终验收清单

- [ ] `cargo run -p revier-analysis -- file-overlay --repo <repo> --base <base> --head <head> --branch <branch> --file <path> --format json` 可运行。
- [ ] `cargo run -p revier-analysis -- trace-block --repo <repo> --base <base> --head <head> --branch <branch> --file <path> --block-id block-1 --format json` 可运行。
- [ ] `file-overlay` stdout 只输出 JSON，stderr 只输出错误信息。
- [ ] 参数错误退出码为 `2`。
- [ ] 仓库读取失败退出码为 `3`。
- [ ] 文件不可分析退出码为 `4`。
- [ ] `file-overlay --require-index` 索引不可用退出码为 `5`。
- [ ] DuckDB 读写失败退出码为 `6`。
- [ ] 分析内部错误退出码为 `10`。
- [ ] `file-overlay` 输出字段与 `src/shared/reviewTypes.ts` 的 `FileOverlay` 兼容。
- [ ] 新增/修改块优先使用 blame，非 merge commit 返回 `confidence: "precise"`。
- [ ] blame 指向 merge commit 时执行 merge trace。
- [ ] 多父歧义场景返回 `confidence: "partial"` 和 `MERGE_TRACE_AMBIGUOUS`。
- [ ] 删除块返回执行删除的相关提交，无法唯一确认时使用 `inferred` 或 `partial`。
- [ ] rename 后输出 `oldPath`，路径历史不完整时输出中文 warning。
- [ ] 生产 Rust 源码通过 `crates/revier-analysis/tests/no_git_process.rs`，未调用 Git 进程。
- [ ] Electron 默认不启用 Rust overlay。
- [ ] 设置 `REVIER_RUST_OVERLAY=1` 后，`reviewGetFileOverlay` 优先调用 Rust overlay。
- [ ] Rust overlay 失败且错误可恢复时，Electron 降级到 TypeScript overlay。
- [ ] `cargo test --workspace` 通过。
- [ ] `pnpm typecheck` 通过。
- [ ] `pnpm test` 通过。

## 自审记录

- 规约目标 1：`file-overlay` 和 `trace-block` 覆盖在任务 1、3、9。
- 规约目标 2：JSON 契约覆盖在任务 1、3、9、10。
- 规约目标 3：gix blob、diff、blame、parent 查询覆盖在任务 2、3、6、7。
- 规约目标 4：索引优先和即时计算 fallback 覆盖在任务 4。
- 规约目标 5：新增、修改、删除、rename、merge conflict、多父 merge 覆盖在任务 5、6、7、8。
- 规约目标 6：`precise`、`inferred`、`partial` 覆盖在任务 6、7、8。
- 规约目标 7：Electron 开关和 TypeScript 降级覆盖在任务 10、11。
- 非目标已保留：本计划没有修改 Renderer、Pinia store、Vue diff 组件，没有移除 TypeScript overlay，没有引入后台服务或 N-API。
- 红旗措辞扫描：本文没有使用英文待办标记或延后实现标记。
- 类型一致性：计划中 Rust JSON 类型统一使用 `*Output` 后缀，TypeScript 客户端返回现有 `FileOverlay`。
