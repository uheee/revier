# Revier Tauri 迁移实施计划

> **给 agentic workers：** REQUIRED SUB-SKILL: 使用 `superpowers:subagent-driven-development`（推荐）或 `superpowers:executing-plans` 按任务执行本计划。步骤使用 checkbox（`- [ ]`）语法跟踪。

**目标：** 将 Revier 从 Electron 迁移到 Tauri，保留 Vue UI，移除生产 TypeScript/Node fallback，并由 Rust schema 生成 TypeScript 类型。

**架构：** Vue 只负责 UI 显示和交互状态，通过 `revierClient` 调用 Tauri commands/events。Tauri/Rust 负责项目管理、Git 访问、Review 任务、分析结果和错误返回。`crates/revier-analysis` 调整为库 API 优先，CLI 仅用于测试和调试。

**技术栈：** Tauri 2、Vue 3、Pinia、Naive UI、Vite、TypeScript、Rust stable、gix、DuckDB、specta、tauri-specta、Vitest、Cargo test、pnpm、fnm、pwsh。

**已确认布局：** 根 `Cargo.toml` 的 workspace 成员为 `crates/revier-analysis` 和 `src-tauri`。`crates/revier-analysis` 是复用 Rust 分析库，`src-tauri` 是 Tauri 桌面壳；两者是同一 Cargo workspace 的独立成员。本迁移不做 `src/analysis` 或 `src/view` 重命名，也不重命名 `crates/revier-analysis` 或 `src-tauri`。

---

## 范围检查

本计划覆盖桌面运行时、Rust 服务、前端 API、类型生成、测试和发布链路。虽然范围较大，但这些改动都服务于同一条破坏性迁移链路，不能在保留旧 Electron 生产路径的前提下独立完成。执行时必须按任务顺序推进，每个任务都要有测试或构建验证，并在关键阶段提交。

## 文件结构

新增或重写的主要文件：

- `Cargo.toml`：根 Cargo workspace，包含 `crates/revier-analysis` 与 `src-tauri` 两个独立成员。
- `src-tauri/Cargo.toml`：Tauri 应用 crate 依赖和 build 配置。
- `src-tauri/build.rs`：Tauri 构建脚本。
- `src-tauri/tauri.conf.json`：Tauri 窗口、构建、权限和打包配置。
- `src-tauri/src/main.rs`：Tauri 应用入口。
- `src-tauri/src/lib.rs`：Tauri builder、state、command 注册和插件注册。
- `src-tauri/src/state.rs`：全局 AppState。
- `src-tauri/src/error.rs`：Tauri command 使用的 `AppError`。
- `src-tauri/src/commands/projects.rs`：项目相关 commands。
- `src-tauri/src/commands/review.rs`：Review 相关 commands 和事件发送。
- `src-tauri/src/services/projects.rs`：项目存储、迁移、仓库校验和目录选择以外的项目业务。
- `src-tauri/src/services/review.rs`：任务管理、分析缓存和 Review 编排。
- `crates/revier-analysis/src/contracts.rs`：Rust schema 权威类型。
- `crates/revier-analysis/src/api.rs`：生产库 API。
- `crates/revier-analysis/src/bindings.rs`：TS bindings 导出函数。
- `src/renderer/generated/bindings.ts`：由 Rust schema 生成的 TS 类型。
- `src/renderer/api/revierClient.ts`：前端唯一 Tauri API 适配层。
- `vite.config.ts`：替代 Electron Vite 配置。

迁移后删除的生产文件：

- `src/main/**`
- `src/preload/**`
- `electron.vite.config.ts`
- `electron-builder.yml`
- `src/analysis-core/**`
- `src/cli/**`

## Task 1: 建立最小 Tauri 壳和 Vite 构建入口

**Files:**
- Create: `src-tauri/Cargo.toml`
- Create: `src-tauri/build.rs`
- Create: `src-tauri/tauri.conf.json`
- Create: `src-tauri/src/main.rs`
- Create: `src-tauri/src/lib.rs`
- Create: `vite.config.ts`
- Modify: `package.json`
- Modify: `tsconfig.web.json`
- Modify: `Cargo.toml`
- Modify: `docs/superpowers/specs/2026-07-06-revier-tauri-migration-design.md`
- Modify: `docs/superpowers/plans/2026-07-06-revier-tauri-migration-implementation.md`

Task 1 只建立最小运行时入口和构建入口，不执行目录重命名，不进入 schema、command 迁移或 Electron 删除任务。

- [ ] **Step 0: 更新 Cargo workspace 和已确认文档布局**

Modify root `Cargo.toml`:

```toml
[workspace]
members = ["crates/revier-analysis", "src-tauri"]
resolver = "2"
```

Update the design and implementation docs to state that `crates/revier-analysis` is the reusable Rust analysis library and `src-tauri` is the Tauri desktop shell. Both are independent members of the same Cargo workspace, and there is no `src/analysis` or `src/view` directory rename.

- [ ] **Step 1: 写最小 Tauri command smoke 测试**

Create `src-tauri/src/lib.rs` with this initial shape:

```rust
#[tauri::command]
fn health_check() -> &'static str {
    "ok"
}

pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![health_check])
        .run(tauri::generate_context!())
        .expect("Tauri 应用启动失败");
}
```

Create `src-tauri/src/main.rs`:

```rust
fn main() {
    revier_tauri::run();
}
```

- [ ] **Step 2: 添加 Tauri crate 配置**

Create `src-tauri/Cargo.toml`:

```toml
[package]
name = "revier-tauri"
version = "0.1.0"
edition = "2021"
publish = false

[lib]
name = "revier_tauri"
path = "src/lib.rs"

[[bin]]
name = "revier"
path = "src/main.rs"

[build-dependencies]
tauri-build = { version = "2", features = [] }

[dependencies]
tauri = { version = "2", features = [] }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
revier-analysis = { path = "../crates/revier-analysis" }
```

- [ ] **Step 3: 添加 Tauri build 脚本**

Create `src-tauri/build.rs`:

```rust
fn main() {
    tauri_build::build();
}
```

- [ ] **Step 4: 添加 Tauri 配置**

Create `src-tauri/tauri.conf.json`:

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "Revier",
  "version": "0.1.0",
  "identifier": "com.revier.app",
  "build": {
    "beforeDevCommand": "pnpm vite:dev",
    "beforeBuildCommand": "pnpm vite:build",
    "devUrl": "http://localhost:5173",
    "frontendDist": "../dist"
  },
  "app": {
    "windows": [
      {
        "title": "Revier",
        "width": 1280,
        "height": 820,
        "minWidth": 1024,
        "minHeight": 700
      }
    ],
    "security": {
      "csp": null
    }
  },
  "bundle": {
    "active": true,
    "targets": "all",
    "icon": [
      "../icon.png"
    ]
  }
}
```

- [ ] **Step 5: 添加 Vite 配置**

Create `vite.config.ts`:

```ts
import vue from '@vitejs/plugin-vue';
import { defineConfig } from 'vite';
import { resolve } from 'node:path';

const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [vue()],
  clearScreen: false,
  server: {
    port: 5173,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: 'ws',
          host,
          port: 1421
        }
      : undefined,
    watch: {
      ignored: ['**/src-tauri/**']
    }
  },
  envPrefix: ['VITE_', 'TAURI_ENV_*'],
  resolve: {
    alias: {
      '@renderer': resolve(__dirname, 'src/renderer'),
      '@shared': resolve(__dirname, 'src/shared')
    }
  },
  build: {
    target: process.env.TAURI_ENV_PLATFORM === 'windows' ? 'chrome105' : 'safari13',
    sourcemap: Boolean(process.env.TAURI_ENV_DEBUG),
    minify: process.env.TAURI_ENV_DEBUG ? false : 'esbuild'
  }
});
```

- [ ] **Step 6: 修改 package scripts**

Modify `package.json` scripts to this shape:

```json
{
  "dev": "pnpm tauri dev",
  "build": "pnpm typecheck && pnpm tauri build",
  "preview": "vite preview",
  "vite:dev": "vite",
  "vite:build": "vite build",
  "tauri": "tauri",
  "typecheck": "vue-tsc --noEmit -p tsconfig.web.json",
  "test": "vitest run",
  "test:watch": "vitest",
  "lint": "pnpm typecheck && eslint .",
  "generate:bindings": "cargo run -p revier-analysis -- export-bindings --out src/renderer/generated/bindings.ts",
  "generate:bindings:check": "pnpm generate:bindings && git diff --exit-code -- src/renderer/generated/bindings.ts"
}
```

Task 1 只要求 `build` 覆盖当前已实现的前端类型检查和 Tauri 构建。`generate:bindings` 与
`generate:bindings:check` 可在 Task 2 实现 `export-bindings` 后引入；Task 2 完成 bindings
生成链路后，再将 `generate:bindings:check` 接回 `build` 或全量验证流程。

- [ ] **Step 7: 添加 Node 依赖命令**

Run:

```powershell
fnm use 24
pnpm add @tauri-apps/api@2
pnpm add -D @tauri-apps/cli@2 vite@^5.4.2 @vitejs/plugin-vue@^5.1.2
```

Expected: `package.json` and `pnpm-lock.yaml` update. If network fails in sandbox, rerun with approval according to sandbox policy.

- [ ] **Step 8: 运行最小构建检查**

Run:

```powershell
pnpm exec tauri --version
cargo check -p revier-tauri
pnpm vite:build
```

Expected: Tauri CLI prints version, Cargo checks `revier-tauri`, Vite builds `dist`.

- [ ] **Step 9: 提交**

```powershell
git add Cargo.toml package.json pnpm-lock.yaml vite.config.ts src-tauri docs/superpowers/specs/2026-07-06-revier-tauri-migration-design.md docs/superpowers/plans/2026-07-06-revier-tauri-migration-implementation.md
git commit -m "chore: 初始化 Tauri 运行时"
```

## Task 2: 建立 Rust schema 和 TS bindings 生成

**Files:**
- Create: `crates/revier-analysis/src/contracts.rs`
- Create: `crates/revier-analysis/src/bindings.rs`
- Modify: `crates/revier-analysis/src/lib.rs`
- Modify: `crates/revier-analysis/src/cli.rs`
- Modify: `crates/revier-analysis/Cargo.toml`
- Create: `src/renderer/generated/bindings.ts`
- Test: `crates/revier-analysis/tests/bindings.rs`

- [ ] **Step 1: 添加 specta 依赖**

Modify `crates/revier-analysis/Cargo.toml` dependencies:

```toml
specta = { version = "2", features = ["typescript"] }
```

- [ ] **Step 2: 写 bindings 测试**

Create `crates/revier-analysis/tests/bindings.rs`:

```rust
use std::fs;
use tempfile::tempdir;

#[test]
fn exports_typescript_bindings_without_recoverable() {
    let dir = tempdir().expect("创建临时目录失败");
    let output = dir.path().join("bindings.ts");

    revier_analysis::bindings::export_typescript_bindings(&output)
        .expect("导出 TypeScript bindings 失败");

    let content = fs::read_to_string(output).expect("读取 bindings 失败");
    assert!(content.contains("export type AppError"));
    assert!(content.contains("code: string"));
    assert!(content.contains("message: string"));
    assert!(content.contains("detail?: string"));
    assert!(!content.contains("recoverable"));
}
```

Run:

```powershell
cargo test -p revier-analysis --test bindings exports_typescript_bindings_without_recoverable
```

Expected: FAIL because `revier_analysis::bindings` does not exist.

- [ ] **Step 3: 新增 contracts**

Create `crates/revier-analysis/src/contracts.rs` with the first contract set:

```rust
use serde::{Deserialize, Serialize};
use specta::Type;

pub type ProjectId = String;
pub type TaskId = String;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: String,
    pub message: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProjectReviewFilters {
    pub branch: Option<String>,
    pub start_at: Option<String>,
    pub end_at: Option<String>,
    pub author_keys: Option<Vec<String>>,
    pub author_query: Option<String>,
    pub message_query: Option<String>,
    pub glob_rules: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProjectPreferences {
    pub default_branch: Option<String>,
    pub default_days: Option<u32>,
    pub default_glob_rules: Vec<String>,
    pub review_filters: Option<ProjectReviewFilters>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReviewProject {
    pub id: ProjectId,
    pub name: String,
    pub repo_path: String,
    pub pinned: bool,
    pub last_opened_at: Option<String>,
    pub preferences: ProjectPreferences,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryValidation {
    pub valid: bool,
    pub repo_path: String,
    pub current_branch: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct GitBranch {
    pub name: String,
    pub current: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DirectorySelection {
    pub path: String,
    pub name: String,
}
```

Append the remaining review contracts in the same file:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReviewFilters {
    pub project_id: ProjectId,
    pub branch: String,
    pub start_at: Option<String>,
    pub end_at: Option<String>,
    pub author_keys: Option<Vec<String>>,
    pub author_query: Option<String>,
    pub message_query: Option<String>,
    pub glob_rules: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisRange {
    pub branch: String,
    pub base_commit: String,
    pub head_commit: String,
    pub start_at: Option<String>,
    pub end_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum AnalysisTaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum AnalysisStage {
    ReadRepository,
    ResolveRange,
    LoadCommits,
    LoadChangedFiles,
    Ready,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisTaskSnapshot {
    pub task_id: TaskId,
    pub project_id: ProjectId,
    pub status: AnalysisTaskStatus,
    pub stage: AnalysisStage,
    pub progress: Option<f64>,
    pub message: Option<String>,
    pub error: Option<AppError>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum ChangedFileStatus {
    Added,
    Modified,
    Deleted,
    Renamed,
    Binary,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ChangedFile {
    pub path: String,
    pub old_path: Option<String>,
    pub status: ChangedFileStatus,
    pub additions: u64,
    pub deletions: u64,
    pub is_binary: bool,
    pub is_previewable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FileOverlayRequest {
    pub task_id: TaskId,
    pub file_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ReviewAuthorOptionsRequest {
    pub project_id: ProjectId,
    pub branch: String,
    pub start_at: Option<String>,
    pub end_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CommitOverlayRequest {
    pub task_id: TaskId,
    pub file_path: String,
    pub commit_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AuthorSummary {
    pub name: String,
    pub email: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AuthorFilterOption {
    pub key: String,
    pub name: String,
    pub email: Option<String>,
    pub commit_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum AttributionConfidence {
    Precise,
    Inferred,
    Partial,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AttributionWarningCode {
    BlameUnavailable,
    MergeTraceAmbiguous,
    PathHistoryIncomplete,
    DeletionTraceIncomplete,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AttributionWarning {
    pub code: AttributionWarningCode,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BlockAttributionSummary {
    pub confidence: AttributionConfidence,
    pub warnings: Vec<AttributionWarning>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum AttributionMethod {
    Blame,
    MergeTrace,
    PatchInference,
    DeletionTrace,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RelatedCommitAttribution {
    pub method: AttributionMethod,
    pub via_merge_hashes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TouchedRange {
    pub old_start: Option<u64>,
    pub old_end: Option<u64>,
    pub new_start: Option<u64>,
    pub new_end: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RelatedCommit {
    pub hash: String,
    pub short_hash: String,
    pub author_name: String,
    pub author_email: Option<String>,
    pub committed_at: String,
    pub subject: String,
    pub matched_by_filter: bool,
    pub touched_ranges: Vec<TouchedRange>,
    pub attribution: Option<RelatedCommitAttribution>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct WordChange {
    pub value: String,
    pub added: Option<bool>,
    pub removed: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum SideBySideDiffRowType {
    Context,
    Added,
    Deleted,
    Modified,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SideBySideDiffRow {
    pub old_line_number: Option<u64>,
    pub new_line_number: Option<u64>,
    pub old_text: Option<String>,
    pub new_text: Option<String>,
    pub r#type: SideBySideDiffRowType,
    pub word_changes: Option<Vec<WordChange>>,
    pub block_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum DiffBlockChangeType {
    Added,
    Deleted,
    Modified,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DiffBlock {
    pub id: String,
    pub old_start: u64,
    pub old_end: u64,
    pub new_start: u64,
    pub new_end: u64,
    pub row_start_index: Option<u64>,
    pub row_end_index: Option<u64>,
    pub change_type: DiffBlockChangeType,
    pub authors: Vec<AuthorSummary>,
    pub rows: Vec<SideBySideDiffRow>,
    pub related_commits: Vec<RelatedCommit>,
    pub attribution: Option<BlockAttributionSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "kebab-case")]
pub enum FileOverlayMode {
    Range,
    Commit,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FileOverlay {
    pub mode: Option<FileOverlayMode>,
    pub file: ChangedFile,
    pub range: AnalysisRange,
    pub rows: Option<Vec<SideBySideDiffRow>>,
    pub blocks: Vec<DiffBlock>,
    pub warnings: Vec<AppError>,
    pub commit: Option<RelatedCommit>,
    pub parent_hash: Option<String>,
}
```

- [ ] **Step 4: 新增 bindings 导出模块**

Create `crates/revier-analysis/src/bindings.rs`:

```rust
use std::path::Path;

use crate::contracts::*;

pub fn export_typescript_bindings(output: &Path) -> Result<(), std::io::Error> {
    let bindings = specta::ts::export::<(
        AppError,
        ProjectReviewFilters,
        ProjectPreferences,
        ReviewProject,
        RepositoryValidation,
        GitBranch,
        DirectorySelection,
        ReviewFilters,
        AnalysisRange,
        AnalysisTaskStatus,
        AnalysisStage,
        AnalysisTaskSnapshot,
        ChangedFileStatus,
        ChangedFile,
        FileOverlayRequest,
        ReviewAuthorOptionsRequest,
        CommitOverlayRequest,
        AuthorSummary,
        AuthorFilterOption,
        AttributionConfidence,
        AttributionWarningCode,
        AttributionWarning,
        BlockAttributionSummary,
        AttributionMethod,
        RelatedCommitAttribution,
        TouchedRange,
        RelatedCommit,
        WordChange,
        SideBySideDiffRowType,
        SideBySideDiffRow,
        DiffBlockChangeType,
        DiffBlock,
        FileOverlayMode,
        FileOverlay,
    )>(&Default::default())
    .expect("生成 TypeScript bindings 失败");

    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(output, bindings)?;
    Ok(())
}
```

Modify `crates/revier-analysis/src/lib.rs`:

```rust
pub mod bindings;
pub mod contracts;
```

Keep the existing module exports in the same file.

- [ ] **Step 5: 添加 CLI 子命令**

Modify `crates/revier-analysis/src/cli.rs`:

```rust
ExportBindings(ExportBindingsArgs),
```

Add args:

```rust
#[derive(Debug, Parser)]
pub struct ExportBindingsArgs {
    #[arg(long)]
    pub out: std::path::PathBuf,
}
```

Modify `crates/revier-analysis/src/lib.rs` command match:

```rust
Command::ExportBindings(args) => {
    bindings::export_typescript_bindings(&args.out)?;
    Ok(String::new())
}
```

If `?` cannot convert `std::io::Error`, add an `Io` variant to `error::AppError`.

- [ ] **Step 6: 运行测试并生成 bindings**

Run:

```powershell
cargo test -p revier-analysis --test bindings
pnpm generate:bindings
rg -n "recoverable" src/renderer/generated/bindings.ts
```

Expected: tests pass; `rg` exits with code 1 because `recoverable` is absent.

- [ ] **Step 7: 提交**

```powershell
git add crates/revier-analysis src/renderer/generated/bindings.ts package.json
git commit -m "feat: 从 Rust schema 生成前端类型"
```

## Task 3: 新增 Tauri AppState、错误模型和项目服务

**Files:**
- Create: `src-tauri/src/error.rs`
- Create: `src-tauri/src/state.rs`
- Create: `src-tauri/src/services/mod.rs`
- Create: `src-tauri/src/services/projects.rs`
- Create: `src-tauri/src/commands/mod.rs`
- Create: `src-tauri/src/commands/projects.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: Rust unit tests inside `src-tauri/src/services/projects.rs`

- [ ] **Step 1: 写项目存储测试**

Add to `src-tauri/src/services/projects.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn adds_and_lists_projects() {
        let dir = tempdir().expect("创建临时目录失败");
        let store_path = dir.path().join("projects.json");
        let service = ProjectService::new(store_path);

        let project = service
            .add_project("E:/repo/example", Some("Example".to_string()))
            .expect("添加项目失败");

        let projects = service.list_projects().expect("读取项目失败");
        assert_eq!(projects.len(), 1);
        assert_eq!(projects[0].id, project.id);
        assert_eq!(projects[0].name, "Example");
        assert_eq!(projects[0].repo_path, "E:/repo/example");
    }
}
```

Run:

```powershell
cargo test -p revier-tauri adds_and_lists_projects
```

Expected: FAIL because `ProjectService` does not exist.

- [ ] **Step 2: 新增错误模型**

Create `src-tauri/src/error.rs`:

```rust
use revier_analysis::contracts::AppError;

pub type CommandResult<T> = Result<T, AppError>;

pub fn command_error(code: impl Into<String>, message: impl Into<String>) -> AppError {
    AppError {
        code: code.into(),
        message: message.into(),
        detail: None,
    }
}

pub fn command_error_with_detail(
    code: impl Into<String>,
    message: impl Into<String>,
    detail: impl Into<String>,
) -> AppError {
    AppError {
        code: code.into(),
        message: message.into(),
        detail: Some(detail.into()),
    }
}
```

- [ ] **Step 3: 新增 ProjectService**

Create `src-tauri/src/services/projects.rs`:

```rust
use std::fs;
use std::path::{Path, PathBuf};

use revier_analysis::contracts::{ProjectPreferences, ReviewProject};
use serde::{Deserialize, Serialize};

use crate::error::{command_error, CommandResult};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProjectStoreFile {
    projects: Vec<ReviewProject>,
}

pub struct ProjectService {
    file_path: PathBuf,
}

impl ProjectService {
    pub fn new(file_path: PathBuf) -> Self {
        Self { file_path }
    }

    pub fn list_projects(&self) -> CommandResult<Vec<ReviewProject>> {
        Ok(self.read_store()?.projects)
    }

    pub fn add_project(
        &self,
        repo_path: impl Into<String>,
        name: Option<String>,
    ) -> CommandResult<ReviewProject> {
        let mut store = self.read_store()?;
        let repo_path = normalize_path(repo_path.into());
        if store.projects.iter().any(|project| project.repo_path == repo_path) {
            return Err(command_error("PROJECT_ALREADY_EXISTS", "该仓库已在项目列表中"));
        }

        let project = ReviewProject {
            id: uuid::Uuid::new_v4().to_string(),
            name: name
                .and_then(|value| {
                    let trimmed = value.trim().to_string();
                    (!trimmed.is_empty()).then_some(trimmed)
                })
                .unwrap_or_else(|| default_project_name(&repo_path)),
            repo_path,
            pinned: false,
            last_opened_at: Some(chrono::Utc::now().to_rfc3339()),
            preferences: ProjectPreferences {
                default_branch: None,
                default_days: Some(30),
                default_glob_rules: Vec::new(),
                review_filters: None,
            },
        };
        store.projects.push(project.clone());
        self.write_store(&store)?;
        Ok(project)
    }

    fn read_store(&self) -> CommandResult<ProjectStoreFile> {
        if !self.file_path.exists() {
            return Ok(ProjectStoreFile { projects: Vec::new() });
        }
        let content = fs::read_to_string(&self.file_path)
            .map_err(|error| command_error("PROJECT_STORE_READ_FAILED", error.to_string()))?;
        serde_json::from_str(&content)
            .map_err(|error| command_error("PROJECT_STORE_PARSE_FAILED", error.to_string()))
    }

    fn write_store(&self, store: &ProjectStoreFile) -> CommandResult<()> {
        if let Some(parent) = self.file_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| command_error("PROJECT_STORE_WRITE_FAILED", error.to_string()))?;
        }
        let content = serde_json::to_string_pretty(store)
            .map_err(|error| command_error("PROJECT_STORE_SERIALIZE_FAILED", error.to_string()))?;
        fs::write(&self.file_path, format!("{content}\n"))
            .map_err(|error| command_error("PROJECT_STORE_WRITE_FAILED", error.to_string()))
    }
}

fn normalize_path(path: String) -> String {
    path.replace('\\', "/").trim_end_matches('/').to_string()
}

fn default_project_name(repo_path: &str) -> String {
    Path::new(repo_path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("未命名仓库")
        .to_string()
}
```

Add dependencies to `src-tauri/Cargo.toml`:

```toml
chrono = { version = "0.4", default-features = false, features = ["clock", "serde", "std"] }
uuid = { version = "1", features = ["v4"] }
tempfile = "3.10"
```

- [ ] **Step 4: 新增 AppState**

Create `src-tauri/src/state.rs`:

```rust
use std::sync::Arc;

use crate::services::projects::ProjectService;

pub struct AppState {
    pub projects: Arc<ProjectService>,
}

impl AppState {
    pub fn new(projects: ProjectService) -> Self {
        Self {
            projects: Arc::new(projects),
        }
    }
}
```

- [ ] **Step 5: 新增项目 commands**

Create `src-tauri/src/commands/projects.rs`:

```rust
use revier_analysis::contracts::ReviewProject;
use tauri::State;

use crate::error::CommandResult;
use crate::state::AppState;

#[tauri::command]
pub fn projects_list(state: State<'_, AppState>) -> CommandResult<Vec<ReviewProject>> {
    state.projects.list_projects()
}
```

Create `src-tauri/src/commands/mod.rs`:

```rust
pub mod projects;
pub mod review;
```

Create `src-tauri/src/services/mod.rs`:

```rust
pub mod projects;
pub mod review;
```

- [ ] **Step 6: 注册 state 和 command**

Modify `src-tauri/src/lib.rs`:

```rust
mod commands;
mod error;
mod services;
mod state;

use services::projects::ProjectService;
use state::AppState;

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let project_service = ProjectService::new(data_dir.join("projects.json"));
            app.manage(AppState::new(project_service));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![commands::projects::projects_list])
        .run(tauri::generate_context!())
        .expect("Tauri 应用启动失败");
}
```

- [ ] **Step 7: 运行测试**

Run:

```powershell
cargo test -p revier-tauri adds_and_lists_projects
cargo check -p revier-tauri
```

Expected: tests pass and Cargo check succeeds.

- [ ] **Step 8: 提交**

```powershell
git add src-tauri
git commit -m "feat: 添加 Tauri 项目服务"
```

## Task 4: 在 Rust 分析 crate 中补齐生产 API

**Files:**
- Create: `crates/revier-analysis/src/api.rs`
- Modify: `crates/revier-analysis/src/lib.rs`
- Modify: `crates/revier-analysis/src/json.rs`
- Test: `crates/revier-analysis/tests/api_contract.rs`

- [ ] **Step 1: 写 API 契约测试**

Create `crates/revier-analysis/tests/api_contract.rs`:

```rust
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
    gix::create::into(repo.path(), gix::create::Kind::WithWorktree, gix::create::Options::default())
        .expect("初始化测试仓库失败");

    let branches = list_branches(repo.path()).expect("读取分支失败");
    assert!(branches.iter().any(|branch| branch.current));
}
```

Run:

```powershell
cargo test -p revier-analysis --test api_contract
```

Expected: FAIL because `revier_analysis::api` does not exist.

- [ ] **Step 2: 新增 API 模块骨架**

Create `crates/revier-analysis/src/api.rs`:

```rust
use std::path::Path;

use crate::contracts::{GitBranch, RepositoryValidation};
use crate::error::AppError;

pub fn validate_repository(repo_path: &Path) -> Result<RepositoryValidation, AppError> {
    let repo_path_string = normalize_path(repo_path);
    match gix::discover(repo_path) {
        Ok(repo) => {
            let workdir = repo
                .workdir()
                .map(normalize_path)
                .unwrap_or_else(|| repo_path_string.clone());
            let current_branch = repo
                .head_name()
                .ok()
                .flatten()
                .and_then(|name| name.shorten().to_string().into());
            Ok(RepositoryValidation {
                valid: true,
                repo_path: workdir,
                current_branch,
                error: None,
            })
        }
        Err(_) => Ok(RepositoryValidation {
            valid: false,
            repo_path: repo_path_string,
            current_branch: None,
            error: Some("请选择一个 Git 仓库目录".to_string()),
        }),
    }
}

pub fn list_branches(repo_path: &Path) -> Result<Vec<GitBranch>, AppError> {
    let repo = gix::discover(repo_path).map_err(|error| AppError::Repository(error.to_string()))?;
    let current = repo
        .head_name()
        .ok()
        .flatten()
        .map(|name| name.shorten().to_string());
    let mut branches = Vec::new();
    for reference in repo.references()?.local_branches()? {
        let reference = reference.map_err(|error| AppError::Repository(error.to_string()))?;
        let name = reference.name().shorten().to_string();
        branches.push(GitBranch {
            current: current.as_deref() == Some(name.as_str()),
            name,
        });
    }
    Ok(branches)
}

fn normalize_path(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/").trim_end_matches('/').to_string()
}
```

Modify `crates/revier-analysis/src/lib.rs`:

```rust
pub mod api;
```

- [ ] **Step 3: 修正 gix API 编译问题**

Run:

```powershell
cargo test -p revier-analysis --test api_contract
```

Expected: compile errors identify exact gix method adjustments. Replace only the failing gix calls with the crate-supported equivalent and rerun until pass.

- [ ] **Step 4: 为 query/files/overlay 暴露库 API**

Add to `crates/revier-analysis/src/api.rs`:

```rust
use crate::cli::{FileOverlayArgs, IndexCommonArgs, OverlayCommonArgs, OutputFormat, QueryFilesArgs};
use crate::json::{FileOverlayCommandOutput, QueryFilesOutput};

pub fn query_files(args: QueryFilesArgs) -> Result<QueryFilesOutput, AppError> {
    crate::commands::query_files::query(args)
}

pub fn file_overlay(args: FileOverlayArgs) -> Result<FileOverlayCommandOutput, AppError> {
    crate::commands::file_overlay::analyze(args)
}
```

Then refactor `commands::query_files::run` and `commands::file_overlay::run` so each has:

```rust
pub fn query(args: QueryFilesArgs) -> Result<QueryFilesOutput, AppError> {
    query_files_output(args)
}

pub fn run(args: QueryFilesArgs) -> Result<String, AppError> {
    let pretty = args.common.pretty;
    let output = query(args)?;
    crate::serialize_json(&output, pretty)
}
```

Create `query_files_output(args)` by moving the current `commands::query_files::run` statements that build `QueryFilesOutput` into a private helper. The helper must return `Result<QueryFilesOutput, AppError>` and must not call `serialize_json`.

and:

```rust
pub fn analyze(args: FileOverlayArgs) -> Result<FileOverlayCommandOutput, AppError> {
    file_overlay_output(args)
}

pub fn run(args: FileOverlayArgs) -> Result<String, AppError> {
    let pretty = args.common.pretty;
    let output = analyze(args)?;
    crate::serialize_json(&output, pretty)
}
```

Create `file_overlay_output(args)` by moving the current `commands::file_overlay::run` statements that build `FileOverlayCommandOutput` into a private helper. The helper must return `Result<FileOverlayCommandOutput, AppError>` and must not call `serialize_json`.

- [ ] **Step 5: 运行 Rust 测试**

Run:

```powershell
cargo test -p revier-analysis
```

Expected: all `revier-analysis` tests pass.

- [ ] **Step 6: 提交**

```powershell
git add crates/revier-analysis
git commit -m "feat: 暴露 Rust 分析库 API"
```

## Task 5: 实现 Tauri Review 服务和 commands/events

**Files:**
- Create: `src-tauri/src/services/review.rs`
- Create: `src-tauri/src/commands/review.rs`
- Modify: `src-tauri/src/lib.rs`
- Test: unit tests inside `src-tauri/src/services/review.rs`

- [ ] **Step 1: 写任务状态测试**

Add to `src-tauri/src/services/review.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use revier_analysis::contracts::{AnalysisStage, AnalysisTaskStatus};

    #[test]
    fn creates_and_completes_task() {
        let service = ReviewService::default();
        let task = service.create_task("project-1".to_string());
        assert_eq!(task.status, AnalysisTaskStatus::Pending);

        service.mark_running(&task.task_id, AnalysisStage::LoadChangedFiles, "读取变更文件");
        let running = service.get_task(&task.task_id).expect("读取任务失败");
        assert_eq!(running.status, AnalysisTaskStatus::Running);

        service.mark_completed(&task.task_id);
        let completed = service.get_task(&task.task_id).expect("读取任务失败");
        assert_eq!(completed.status, AnalysisTaskStatus::Completed);
    }
}
```

Run:

```powershell
cargo test -p revier-tauri creates_and_completes_task
```

Expected: FAIL because `ReviewService` does not exist.

- [ ] **Step 2: 实现 ReviewService 状态**

Create `src-tauri/src/services/review.rs`:

```rust
use std::collections::HashMap;
use std::sync::Mutex;

use revier_analysis::contracts::{
    AnalysisStage, AnalysisTaskSnapshot, AnalysisTaskStatus, ChangedFile, FileOverlay,
    ProjectId, ReviewFilters, TaskId,
};

use crate::error::{command_error, CommandResult};

#[derive(Default)]
pub struct ReviewService {
    tasks: Mutex<HashMap<TaskId, AnalysisTaskSnapshot>>,
    filters_by_task: Mutex<HashMap<TaskId, ReviewFilters>>,
    files_by_task: Mutex<HashMap<TaskId, Vec<ChangedFile>>>,
}

impl ReviewService {
    pub fn create_task(&self, project_id: ProjectId) -> AnalysisTaskSnapshot {
        let task = AnalysisTaskSnapshot {
            task_id: uuid::Uuid::new_v4().to_string(),
            project_id,
            status: AnalysisTaskStatus::Pending,
            stage: AnalysisStage::ReadRepository,
            progress: None,
            message: None,
            error: None,
        };
        self.tasks.lock().expect("任务锁被污染").insert(task.task_id.clone(), task.clone());
        task
    }

    pub fn get_task(&self, task_id: &str) -> CommandResult<AnalysisTaskSnapshot> {
        self.tasks
            .lock()
            .expect("任务锁被污染")
            .get(task_id)
            .cloned()
            .ok_or_else(|| command_error("TASK_NOT_FOUND", format!("未找到任务：{task_id}")))
    }

    pub fn mark_running(&self, task_id: &str, stage: AnalysisStage, message: &str) {
        if let Some(task) = self.tasks.lock().expect("任务锁被污染").get_mut(task_id) {
            task.status = AnalysisTaskStatus::Running;
            task.stage = stage;
            task.message = Some(message.to_string());
        }
    }

    pub fn mark_completed(&self, task_id: &str) {
        if let Some(task) = self.tasks.lock().expect("任务锁被污染").get_mut(task_id) {
            task.status = AnalysisTaskStatus::Completed;
            task.stage = AnalysisStage::Ready;
            task.message = None;
        }
    }
}
```

- [ ] **Step 3: 把 ReviewService 加入 AppState**

Modify `src-tauri/src/state.rs`:

```rust
use crate::services::review::ReviewService;

pub struct AppState {
    pub projects: Arc<ProjectService>,
    pub review: Arc<ReviewService>,
}

impl AppState {
    pub fn new(projects: ProjectService, review: ReviewService) -> Self {
        Self {
            projects: Arc::new(projects),
            review: Arc::new(review),
        }
    }
}
```

Modify `src-tauri/src/lib.rs` setup:

```rust
let review_service = services::review::ReviewService::default();
app.manage(AppState::new(project_service, review_service));
```

- [ ] **Step 4: 新增 Review commands 骨架**

Create `src-tauri/src/commands/review.rs`:

```rust
use revier_analysis::contracts::{AnalysisTaskSnapshot, ChangedFile, ReviewFilters};
use tauri::{AppHandle, Emitter, State};

use crate::error::CommandResult;
use crate::state::AppState;

const TASK_UPDATED_EVENT: &str = "review://task-updated";

#[tauri::command]
pub fn review_get_task(
    state: State<'_, AppState>,
    task_id: String,
) -> CommandResult<AnalysisTaskSnapshot> {
    state.review.get_task(&task_id)
}

#[tauri::command]
pub fn review_start_analysis(
    app: AppHandle,
    state: State<'_, AppState>,
    filters: ReviewFilters,
) -> CommandResult<AnalysisTaskSnapshot> {
    let task = state.review.create_task(filters.project_id.clone());
    state
        .review
        .mark_running(&task.task_id, revier_analysis::contracts::AnalysisStage::LoadChangedFiles, "读取变更文件");
    let snapshot = state.review.get_task(&task.task_id)?;
    app.emit(TASK_UPDATED_EVENT, &snapshot)
        .map_err(|error| crate::error::command_error("TASK_EVENT_FAILED", error.to_string()))?;
    Ok(snapshot)
}

#[tauri::command]
pub fn review_list_changed_files(
    _state: State<'_, AppState>,
    _task_id: String,
) -> CommandResult<Vec<ChangedFile>> {
    Ok(Vec::new())
}
```

- [ ] **Step 5: 注册 Review commands**

Modify `src-tauri/src/lib.rs` invoke handler:

```rust
.invoke_handler(tauri::generate_handler![
    commands::projects::projects_list,
    commands::review::review_start_analysis,
    commands::review::review_get_task,
    commands::review::review_list_changed_files
])
```

- [ ] **Step 6: 逐步接入真实分析**

Replace `review_start_analysis` internals with a `ReviewService::start_analysis` method that performs these exact operations in order:

1. Load the project from `ProjectService` by `filters.project_id`.
2. Call `revier_analysis::api::validate_repository(project.repo_path)`.
3. Return `REPOSITORY_INVALID` if validation is not valid.
4. Resolve the selected range through the Rust analysis API.
5. Call `revier_analysis::api::query_files` with the resolved range and filters.
6. Store returned files in `files_by_task`.
7. Mark the task completed.
8. Emit `review://task-updated` with the completed snapshot.

The concrete implementation must call Rust library API only. It must not call `src/main`, `src/analysis-core`, `simple-git`, Node, or Electron code.

- [ ] **Step 7: 运行测试**

Run:

```powershell
cargo test -p revier-tauri
cargo test -p revier-analysis
```

Expected: both crates pass.

- [ ] **Step 8: 提交**

```powershell
git add src-tauri crates/revier-analysis
git commit -m "feat: 添加 Tauri Review 服务"
```

## Task 6: 添加前端 revierClient 并切换 Vue store

**Files:**
- Create: `src/renderer/api/revierClient.ts`
- Modify: `src/renderer/stores/projectStore.ts`
- Modify: `src/renderer/stores/reviewStore.ts`
- Modify: `src/renderer/pages/ReviewWorkspace.vue`
- Modify: `src/renderer/components/projects/ProjectEditor.vue`
- Test: `tests/unit/rendererProjectStore.test.ts`
- Test: `tests/unit/rendererReviewStore.test.ts`

- [ ] **Step 1: 写 project store 失败测试**

Modify `tests/unit/rendererProjectStore.test.ts` to mock `revierClient` instead of `window.revier`:

```ts
vi.mock('../../src/renderer/api/revierClient', () => ({
  revierClient: {
    projects: {
      list: vi.fn(),
      add: vi.fn(),
      update: vi.fn(),
      remove: vi.fn(),
      validateRepository: vi.fn(),
      listBranches: vi.fn(),
      selectDirectory: vi.fn()
    },
    review: {
      startAnalysis: vi.fn(),
      cancelAnalysis: vi.fn(),
      getTask: vi.fn(),
      onTaskUpdate: vi.fn(),
      listChangedFiles: vi.fn(),
      getFileOverlay: vi.fn(),
      getCommitOverlay: vi.fn(),
      listAuthors: vi.fn()
    }
  }
}));
```

Run:

```powershell
pnpm test -- tests/unit/rendererProjectStore.test.ts
```

Expected: FAIL because stores still use `window.revier`.

- [ ] **Step 2: 新增 revierClient**

Create `src/renderer/api/revierClient.ts`:

```ts
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type {
  AnalysisTaskSnapshot,
  AuthorFilterOption,
  ChangedFile,
  CommitOverlayRequest,
  DirectorySelection,
  FileOverlay,
  FileOverlayRequest,
  GitBranch,
  ReviewAuthorOptionsRequest,
  ReviewFilters,
  ReviewProject
} from '../generated/bindings';

type Unsubscribe = () => void;

export const revierClient = {
  projects: {
    list: () => invoke<ReviewProject[]>('projects_list'),
    add: (repoPath: string, options?: Partial<ReviewProject>) =>
      invoke<ReviewProject>('projects_add', { repoPath, options }),
    update: (project: ReviewProject) => invoke<ReviewProject>('projects_update', { project }),
    remove: (projectId: string) => invoke<void>('projects_remove', { projectId }),
    selectDirectory: () => invoke<DirectorySelection | undefined>('projects_select_directory'),
    validateRepository: (repoPath: string) =>
      invoke('projects_validate_repository', { repoPath }),
    listBranches: (projectId: string) =>
      invoke<GitBranch[]>('projects_list_branches', { projectId })
  },
  review: {
    startAnalysis: (filters: ReviewFilters) =>
      invoke<AnalysisTaskSnapshot>('review_start_analysis', { filters }),
    cancelAnalysis: (taskId: string) => invoke<void>('review_cancel_analysis', { taskId }),
    getTask: (taskId: string) => invoke<AnalysisTaskSnapshot>('review_get_task', { taskId }),
    async onTaskUpdate(callback: (task: AnalysisTaskSnapshot) => void): Promise<Unsubscribe> {
      const unlisten: UnlistenFn = await listen<AnalysisTaskSnapshot>(
        'review://task-updated',
        (event) => callback(event.payload)
      );
      return unlisten;
    },
    listChangedFiles: (taskId: string) =>
      invoke<ChangedFile[]>('review_list_changed_files', { taskId }),
    listAuthors: (request: ReviewAuthorOptionsRequest) =>
      invoke<AuthorFilterOption[]>('review_list_authors', { request }),
    getFileOverlay: (request: FileOverlayRequest) =>
      invoke<FileOverlay>('review_get_file_overlay', { request }),
    getCommitOverlay: (request: CommitOverlayRequest) =>
      invoke<FileOverlay>('review_get_commit_overlay', { request })
  }
};
```

- [ ] **Step 3: 切换 projectStore**

Modify `src/renderer/stores/projectStore.ts`:

```ts
import { revierClient } from '../api/revierClient';
```

Replace all `window.revier.projects` calls with `revierClient.projects`.

- [ ] **Step 4: 切换 reviewStore 和事件订阅**

Modify `src/renderer/stores/reviewStore.ts`:

```ts
import { revierClient } from '../api/revierClient';
```

Replace all `window.revier.review` calls with `revierClient.review`.

Modify `src/renderer/pages/ReviewWorkspace.vue` so `onTaskUpdate` handles the Promise:

```ts
let unsubscribe: (() => void) | undefined;

onMounted(async () => {
  unsubscribe = await revierClient.review.onTaskUpdate((snapshot) => {
    reviewStore.task = snapshot;
  });
});
```

- [ ] **Step 5: 切换 ProjectEditor**

Modify `src/renderer/components/projects/ProjectEditor.vue`:

```ts
import { revierClient } from '../../api/revierClient';

const selection = await revierClient.projects.selectDirectory();
```

- [ ] **Step 6: 运行前端测试和类型检查**

Run:

```powershell
pnpm test -- tests/unit/rendererProjectStore.test.ts tests/unit/rendererReviewStore.test.ts
pnpm typecheck
```

Expected: tests pass and typecheck succeeds.

- [ ] **Step 7: 提交**

```powershell
git add src/renderer tests/unit/rendererProjectStore.test.ts tests/unit/rendererReviewStore.test.ts
git commit -m "feat: 使用 Tauri 前端客户端"
```

## Task 7: 移除 recoverable 与生产 TypeScript/Node fallback

**Files:**
- Delete: `src/shared/errors.ts`
- Delete: `src/analysis-core/**`
- Delete: `src/cli/**`
- Modify: tests referencing `recoverable`
- Modify: `src/renderer/generated/bindings.ts`
- Modify: `package.json`

- [ ] **Step 1: 写禁止 recoverable 的仓库扫描测试**

Create `tests/unit/noRecoverableContract.test.ts`:

```ts
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

it('生成的核心类型不包含 recoverable 字段', () => {
  const bindings = readFileSync(resolve(process.cwd(), 'src/renderer/generated/bindings.ts'), 'utf8');
  expect(bindings).not.toContain('recoverable');
});
```

Run:

```powershell
pnpm test -- tests/unit/noRecoverableContract.test.ts
```

Expected: PASS after Task 2. If it fails, fix Rust contracts and regenerate bindings before continuing.

- [ ] **Step 2: 删除旧错误类型**

Run:

```powershell
git rm src/shared/errors.ts
```

Replace imports of `../../shared/errors` with generated `AppError` or local UI error strings. No production module may import `createAppError` or `isRecoverableError`.

- [ ] **Step 3: 删除生产 TypeScript 分析路径**

Run:

```powershell
git rm -r src/analysis-core src/cli
```

If tests still need fixtures, move only fixture helpers to `tests/fixtures` before deletion.

- [ ] **Step 4: 删除 Electron/Node fallback tests**

Remove or rewrite tests that assert fallback behavior:

```text
tests/unit/reviewIpc.test.ts
tests/unit/rustAnalysisClient.test.ts
tests/unit/analysisCli.test.ts
tests/unit/analysisCoreTypes.test.ts
tests/unit/attributionEngine.test.ts
tests/unit/overlayEngine.test.ts
```

Replacement tests must target Rust crates or `revierClient` store behavior. They must not assert TypeScript fallback.

- [ ] **Step 5: 删除 simple-git 生产依赖**

Run:

```powershell
pnpm remove simple-git
```

If `tests/fixtures/gitRepo.ts` still uses `simple-git`, either keep `simple-git` as `devDependency` or rewrite the fixture with direct `git` command calls. Production dependencies must not include `simple-git`.

- [ ] **Step 6: 扫描禁止项**

Run:

```powershell
rg -n "recoverable|createAppError|isRecoverableError|simple-git|analysis-core|REVIER_USE_RUST_OVERLAY" src tests package.json
```

Expected: no production matches. Test matches are allowed only when the test name proves absence, such as `noRecoverableContract.test.ts`.

- [ ] **Step 7: 运行测试**

Run:

```powershell
pnpm test
pnpm typecheck
cargo test --workspace
```

Expected: all pass.

- [ ] **Step 8: 提交**

```powershell
git add src tests package.json pnpm-lock.yaml
git commit -m "refactor: 移除 TypeScript 分析回退"
```

## Task 8: 删除 Electron 运行时和构建配置

**Files:**
- Delete: `src/main/**`
- Delete: `src/preload/**`
- Delete: `electron.vite.config.ts`
- Delete: `electron-builder.yml`
- Modify: `package.json`
- Modify: `tsconfig.json`
- Modify: `tsconfig.node.json`
- Modify: `playwright.config.ts`
- Modify: `.vscode/launch.json`

- [ ] **Step 1: 删除 Electron 文件**

Run:

```powershell
git rm -r src/main src/preload
git rm electron.vite.config.ts electron-builder.yml
```

- [ ] **Step 2: 删除 Electron 依赖**

Run:

```powershell
pnpm remove electron electron-vite
```

- [ ] **Step 3: 更新 TypeScript 配置**

Modify `tsconfig.json`:

```json
{
  "files": [],
  "references": [
    { "path": "./tsconfig.web.json" }
  ]
}
```

Modify `tsconfig.web.json` include:

```json
{
  "include": [
    "src/renderer/**/*.ts",
    "src/renderer/**/*.vue",
    "src/renderer/generated/**/*.ts",
    "tests/unit/**/*.ts"
  ]
}
```

Remove `tsconfig.node.json` only after no script references it.

- [ ] **Step 4: 更新 VS Code 启动配置**

Modify `.vscode/launch.json` to replace Electron launch entries with Tauri dev terminal:

```json
{
  "name": "Revier: 启动 Tauri 开发环境",
  "type": "node-terminal",
  "request": "launch",
  "command": "pwsh -NoLogo -ExecutionPolicy Bypass -Command \"fnm use 24; pnpm dev\"",
  "cwd": "${workspaceFolder}"
}
```

- [ ] **Step 5: 扫描 Electron 残留**

Run:

```powershell
rg -n "electron|BrowserWindow|ipcMain|ipcRenderer|electron-builder|electron-vite" src tests package.json README.md .github .vscode
```

Expected: production code has no matches. README and CI may still match before Task 10 updates.

- [ ] **Step 6: 运行构建检查**

Run:

```powershell
pnpm typecheck
pnpm test
cargo check --workspace
```

Expected: all pass.

- [ ] **Step 7: 提交**

```powershell
git add src tests package.json pnpm-lock.yaml tsconfig.json tsconfig.web.json .vscode/launch.json
git commit -m "chore: 移除 Electron 运行时"
```

## Task 9: 重写测试矩阵

**Files:**
- Modify: `vitest.config.ts`
- Modify: `playwright.config.ts`
- Create: `tests/unit/revierClient.test.ts`
- Create: `crates/revier-analysis/tests/review_flow.rs`
- Modify: existing renderer tests

- [ ] **Step 1: 添加 revierClient 错误测试**

Create `tests/unit/revierClient.test.ts`:

```ts
import { describe, expect, it, vi } from 'vitest';

vi.mock('@tauri-apps/api/core', () => ({
  invoke: vi.fn(async () => {
    throw { code: 'INDEX_MISSING', message: '索引不存在', detail: 'repo-1' };
  })
}));

describe('revierClient', () => {
  it('Tauri 错误直接向 store 暴露且不触发备用请求', async () => {
    const { invoke } = await import('@tauri-apps/api/core');
    const { revierClient } = await import('../../src/renderer/api/revierClient');

    await expect(revierClient.review.listChangedFiles('task-1')).rejects.toMatchObject({
      code: 'INDEX_MISSING',
      message: '索引不存在'
    });
    expect(invoke).toHaveBeenCalledTimes(1);
  });
});
```

- [ ] **Step 2: 添加 Rust review flow 测试**

Create `crates/revier-analysis/tests/review_flow.rs` with fixture setup using existing `crates/revier-analysis/tests/fixtures.rs` helpers:

```rust
mod fixtures;

#[test]
fn analyzes_changed_files_and_overlay_from_rust_only() {
    let fixture = fixtures::linear_history();
    let files = revier_analysis::api::query_files(fixture.query_files_args())
        .expect("Rust 文件列表查询失败");
    assert!(!files.files.is_empty());

    let overlay = revier_analysis::api::file_overlay(fixture.file_overlay_args("src/main.rs"))
        .expect("Rust overlay 查询失败");
    assert!(!overlay.overlay.blocks.is_empty());
}
```

If fixture helper names differ, adapt the helper exports in `fixtures.rs` so the test remains explicit and uses Rust paths only.

- [ ] **Step 3: 更新 Vitest 配置**

Modify `vitest.config.ts`:

```ts
import vue from '@vitejs/plugin-vue';
import { defineConfig } from 'vitest/config';

export default defineConfig({
  plugins: [vue()],
  test: {
    globals: true,
    environment: 'jsdom',
    include: ['tests/unit/**/*.test.ts']
  }
});
```

- [ ] **Step 4: 删除 Electron E2E**

Run:

```powershell
git rm tests/e2e/review-workflow.spec.ts
```

Keep `playwright.config.ts` only if a browser-level frontend test remains. If no Playwright tests remain, remove `playwright.config.ts` and `@playwright/test`.

- [ ] **Step 5: 运行测试矩阵**

Run:

```powershell
pnpm test
cargo test --workspace
```

Expected: all tests pass.

- [ ] **Step 6: 提交**

```powershell
git add tests crates vitest.config.ts playwright.config.ts package.json pnpm-lock.yaml
git commit -m "test: 更新 Tauri 迁移测试矩阵"
```

## Task 10: 重写构建脚本、CI 和文档

**Files:**
- Modify: `scripts/build.ps1`
- Modify: `scripts/build.sh`
- Modify: `.github/workflows/release.yml`
- Modify: `README.md`
- Modify: `docs/superpowers/specs/2026-07-06-revier-tauri-migration-design.md`

- [ ] **Step 1: 修改 Windows 构建脚本入口**

Modify `scripts/build.ps1` package branch so packaging calls:

```powershell
function Invoke-TauriBuild {
  $repositoryRoot = Get-RepositoryRoot
  Push-Location $repositoryRoot
  try {
    Write-Host "正在执行 Tauri 构建：pnpm tauri build"
    & pnpm tauri build
    if ($LASTEXITCODE -ne 0) {
      throw "Tauri 构建失败，退出码：$LASTEXITCODE"
    }
  }
  finally {
    Pop-Location
  }
}
```

Replace electron-builder invocation with `Invoke-TauriBuild`.

- [ ] **Step 2: 修改 Bash 构建脚本入口**

Modify `scripts/build.sh` packaging branch:

```bash
run_tauri_build() {
  echo "正在执行 Tauri 构建：pnpm tauri build"
  (
    cd "$repo_root"
    pnpm tauri build
  )
}
```

Replace electron-builder calls with `run_tauri_build`.

- [ ] **Step 3: 更新 Release CI 校验**

Modify `.github/workflows/release.yml`:

```yaml
- name: Verify no Electron runtime remains
  shell: bash
  run: |
    set -euo pipefail
    if grep -R "electron-builder\|electron-vite" -n package.json .github scripts README.md; then
      echo "发现 Electron 构建残留。" >&2
      exit 1
    fi
```

For Windows jobs use `shell: pwsh` equivalent:

```powershell
$matches = Select-String -Path package.json,.github/workflows/release.yml,scripts/build.ps1,README.md -Pattern "electron-builder|electron-vite"
if ($matches) {
  throw "发现 Electron 构建残留。"
}
```

- [ ] **Step 4: 更新 README**

Replace Electron architecture sections with:

```markdown
Revier 是一款基于 Tauri、Vue 3 和 Rust 的本地 Git 代码 Review 辅助工具。Vue 只负责界面显示和交互状态，Tauri/Rust 负责本地仓库访问、项目持久化、分析任务、DuckDB 索引和 Diff/归因计算。
```

Replace quick start:

```markdown
pnpm install
pnpm dev
```

Replace architecture diagram:

```text
Vue 组件
  ↓ Pinia Store
revierClient
  ↓ Tauri invoke/listen
Tauri Commands / Events
  ↓
Rust ProjectService / ReviewService / AnalysisService
  ↓
gix / DuckDB / 文件系统
```

- [ ] **Step 5: 运行全量验证**

Run:

```powershell
pnpm generate:bindings:check
pnpm typecheck
pnpm test
cargo test --workspace
pnpm build
```

Expected: all commands exit 0.

- [ ] **Step 6: 提交**

```powershell
git add scripts .github README.md docs package.json pnpm-lock.yaml
git commit -m "ci: 切换 Tauri 构建发布"
```

## Task 11: 最终清理和验收

**Files:**
- No planned file edits. If verification reveals a concrete residual file, create a focused cleanup edit for that exact file and include it in the final cleanup commit.

- [ ] **Step 1: 扫描禁止项**

Run:

```powershell
rg -n "electron|electron-vite|electron-builder|BrowserWindow|ipcMain|ipcRenderer|contextBridge|simple-git|recoverable|REVIER_USE_RUST_OVERLAY|analysis-core" src src-tauri crates tests package.json README.md .github scripts
```

Expected: no production matches. Documentation matches are allowed only when describing removed historical behavior in archived design docs.

- [ ] **Step 2: 校验依赖**

Run:

```powershell
pnpm why electron
pnpm why electron-vite
pnpm why simple-git
```

Expected: each command reports no dependency path or exits non-zero because package is absent.

- [ ] **Step 3: 校验生成类型同步**

Run:

```powershell
pnpm generate:bindings:check
```

Expected: exit 0 and no git diff.

- [ ] **Step 4: 全量测试**

Run:

```powershell
pnpm typecheck
pnpm test
cargo test --workspace
pnpm build
```

Expected: all pass.

- [ ] **Step 5: 检查 git 状态**

Run:

```powershell
git status --short
```

Expected: no output.

- [ ] **Step 6: 提交最终清理**

If Step 1-4 required cleanup changes:

```powershell
git add .
git commit -m "chore: 完成 Tauri 迁移清理"
```

If no cleanup changes were required, do not create an empty commit.
