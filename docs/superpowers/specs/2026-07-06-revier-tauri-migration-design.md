# Revier Tauri 迁移设计

> 状态：设计草案已按用户确认内容整理；2026-07-06 起按实施计划迁移到 Tauri/Rust 生产链路。本文保留历史背景说明，并记录当前迁移目标与验收口径。

## 实施状态记录

截至 2026-07-06，迁移实施已完成以下结构性调整：

- 前端生产 IPC 已切换到 `src/renderer/api/revierClient.ts`，不再依赖 `window.revier`。
- Electron 主进程、preload、Electron 构建配置和生产 TypeScript/Node 分析 fallback 已从生产路径移除。
- 前端核心业务类型改为使用 `src/renderer/generated/bindings.ts`，该文件由 Rust contracts 生成。
- `src-tauri` 作为 Tauri 桌面壳承接项目和 Review commands，`crates/revier-analysis` 继续作为 Rust 分析库。
- 构建发布入口切换到 Tauri CLI；Release CI 上传 Tauri bundle 产物。
- DuckDB 依赖改为启用 `bundled` feature，由 Rust native build 在编译期产出可链接库，不再把平台动态库作为 Tauri resources 单独拷贝。
- `projects_select_directory` 已接入 `tauri-plugin-dialog`，取消选择返回空结果，选择成功后返回规范化为 `/` 分隔符的本地目录路径和目录名。

仍需跟进的能力差异：

- TODO：旧 Electron `userData/projects.json` 到 Tauri app data 的首次迁移尚未实现；需要先确认各平台旧 `userData` 解析规则和产品名。
- `review_start_analysis` 仍是同步执行路径；真正的在途取消需要后台任务 runner 和取消令牌。

## 背景

Revier 当前是 Electron + Vue 3 + TypeScript 桌面应用。现有生产链路由 Vue 渲染层、Electron preload、Electron 主进程、Node/TypeScript Git 与分析逻辑、Rust 分析 CLI 共同组成。近期实现已经把部分文件列表查询、文件 overlay 和 DuckDB 索引能力迁入 `crates/revier-analysis`，但 Electron 主进程仍负责 IPC 编排、项目存储、Git 服务、任务管理、Rust CLI 调用和部分 TypeScript fallback。

本次迁移目标是切换到 Tauri，同时保留 Vue 作为 UI 层。Vue 只负责显示、输入、加载状态和错误展示，不再承担业务 fallback 或后端路径选择。生产业务能力统一放到 Tauri/Rust 侧。

## 目标

1. 使用 Tauri 替换 Electron 作为桌面运行时。
2. 保留现有 Vue 3、Pinia、Naive UI 和 Vite 前端体验。
3. 移除 Electron 主进程、preload、Electron IPC 和 electron-builder 发布链路。
4. 移除生产路径中的 TypeScript/Node Git 与分析 fallback。
5. Rust/Tauri 负责项目管理、目录选择、Git 访问、Review 任务、文件列表、文件 overlay、提交 overlay、作者列表和错误返回。
6. 移除 `recoverable` 错误字段。错误只表达错误码、消息和可选详情。
7. 使用 Rust schema 生成 TypeScript 类型，Rust 数据结构作为核心业务契约的唯一来源。
8. Windows、macOS、Linux 发布链路改为 Tauri bundler。

## 非目标

1. 不把 Vue 改成 Rust 原生 UI、Qt、SwiftUI、WinUI、egui 或 iced。
2. 不保留 Electron 与 Tauri 双运行时并行发布。
3. 不使用 Node sidecar 作为生产业务后端。
4. 不引入 TypeScript/Node 分析逻辑作为生产对照或自动 fallback。
5. 不在本设计阶段执行文件删除、依赖安装或代码迁移。

## 技术选型

| 层级 | 技术 | 说明 |
| --- | --- | --- |
| 桌面运行时 | Tauri 2 | 替代 Electron，提供窗口、命令、事件、权限和打包能力。具体 patch 版本在实施计划阶段确认并锁定。 |
| UI | Vue 3、Pinia、Naive UI、Vite | 保留现有 UI 技术栈，只收窄职责到显示层。 |
| 前端 API | `@tauri-apps/api` | Vue 通过 `invoke` 和事件监听访问 Tauri/Rust。 |
| Rust 类型导出 | `specta`、`specta-typescript` | 由 Rust struct 生成 TypeScript bindings。具体版本在实施计划阶段确认并锁定。 |
| Git 与分析 | Rust、gix、DuckDB | 生产 Git 与分析能力统一放到 Rust。 |
| 构建发布 | Tauri CLI、pnpm、Cargo | 前端由 Vite 构建，桌面包由 Tauri bundler 产出。 |

## 总体架构

```text
Vue UI
  ↓
Tauri 前端 API 适配层
  ↓ invoke/listen
Tauri Commands / Events
  ↓
Rust 应用服务层
  ↓
Rust Git / 分析 / 项目存储 / DuckDB
```

### 职责边界

- `src/renderer` 保留，职责收窄为 UI、表单状态、加载状态、错误展示和结果渲染。
- 新增 `src/renderer/api/revierClient.ts`，封装 Tauri `invoke` 和事件监听，替代当前 `window.revier`。
- `revierClient` 只做参数传递、类型约束、错误转换和事件订阅，不做 fallback、不做业务分支。
- 新增 `src-tauri` 作为桌面入口，负责窗口、权限、命令注册、事件发送、应用配置和打包。
- 根 `Cargo.toml` 使用同一个 Cargo workspace，成员为 `crates/revier-analysis` 与 `src-tauri`。
- `src-tauri` 是 Tauri 桌面壳的独立 workspace member，依赖并调用分析库。
- `crates/revier-analysis` 是可复用 Rust 分析库的独立 workspace member，从 CLI 优先调整为库 API 优先，CLI 可保留用于测试和调试。
- 迁移不执行 `src/analysis` 或 `src/view` 目录重命名，也不重命名 `crates/revier-analysis` 或 `src-tauri`。
- Rust 侧新增应用服务模块，承接当前 `src/main/ipc`、`src/main/projects`、`src/main/git`、`src/main/analysis` 的生产职责。
- Rust/Tauri 失败时返回结构化错误，Vue 只展示错误和可操作提示，不切换旧实现。

## 功能模块划分

### 桌面壳模块

新增 `src-tauri`，负责：

- Tauri 应用入口。
- 窗口配置。
- 图标配置。
- 权限与 capabilities。
- Tauri command 注册。
- Tauri event 发送。
- 打包配置。

`src-tauri` 保持为根 Cargo workspace 成员，不放入 `src/analysis`、`src/view` 或其他重命名目录。它通过 Cargo path dependency 引用 `../crates/revier-analysis`。

替代并删除：

- `src/main/index.ts`
- `src/main/app/createWindow.ts`
- `electron.vite.config.ts`
- `electron-builder.yml`

### 前端 API 适配模块

新增 `src/renderer/api/revierClient.ts`：

```ts
revierClient.projects.list()
revierClient.projects.add(repoPath, options)
revierClient.projects.update(project)
revierClient.projects.remove(projectId)
revierClient.projects.selectDirectory()
revierClient.projects.validateRepository(repoPath)
revierClient.projects.listBranches(projectId)

revierClient.review.startAnalysis(filters)
revierClient.review.cancelAnalysis(taskId)
revierClient.review.getTask(taskId)
revierClient.review.onTaskUpdate(callback)
revierClient.review.listChangedFiles(taskId)
revierClient.review.listAuthors(request)
revierClient.review.getFileOverlay(request)
revierClient.review.getCommitOverlay(request)
```

替代：

- `window.revier`
- `src/preload/index.ts`
- Electron `ipcRenderer.invoke`

### 项目管理模块

Rust 侧负责：

- 项目列表读取。
- 项目添加、更新、删除。
- Git 仓库校验。
- 分支列表读取。
- 系统目录选择。
- 项目偏好持久化。

持久化策略：

- 当前实现读取和写入 Tauri app data 目录下的 `projects.json`。
- JSON 文件结构保持兼容当前 `ReviewProject` 语义。
- TODO：若旧 Electron 数据存在，首次启动时迁移到 Tauri app data 目录；实现前需确认旧 Electron `userData` 在 Windows、macOS、Linux 的精确路径。
- 迁移完成后生产逻辑仍只读取 Tauri app data 目录。

### Review 任务模块

Rust 侧维护：

- `AnalysisTaskManager` 等价能力。
- 任务状态。
- 取消状态。
- 任务内存缓存。
- 文件列表缓存。
- 分析范围缓存。
- 项目与筛选条件缓存。

Tauri commands：

```text
review_start_analysis
review_cancel_analysis
review_get_task
review_list_changed_files
review_list_authors
review_get_file_overlay
review_get_commit_overlay
```

任务更新事件：

```text
review://task-updated
```

Vue 只订阅事件并显示任务状态，不参与任务调度或降级。

### Git 与分析模块

生产 Git 访问改为 Rust：

- 仓库校验。
- 分支读取。
- 提交列表读取。
- 作者列表读取。
- 提交范围解析。
- 变更文件列表查询。
- 文件 overlay。
- 提交 overlay。
- 块级归因。
- DuckDB 索引读写。

移除生产路径：

- `simple-git`
- Git CLI 生产封装
- TypeScript `analysis-core` fallback
- Electron 主进程 Rust CLI `spawn` 适配层
- `REVIER_USE_RUST_OVERLAY=0` 类型的生产回退开关

若 Rust 分析失败，返回错误；不回退到 TypeScript 实现。

### 构建发布模块

构建职责：

- `pnpm dev` 启动 Tauri dev，内部拉起 Vite。
- `pnpm build` 执行类型生成校验、前端类型检查、Vite build、Tauri build。
- `cargo test --workspace` 覆盖 Rust workspace。
- `pnpm test` 覆盖前端 store/component 测试。
- `pnpm lint` 覆盖前端 lint 和类型检查。

发布职责：

- Windows、macOS、Linux 使用 Tauri bundler。
- Release CI 校验产物存在。
- Release CI 校验包内不包含 Electron runtime、`dist/main`、`dist/preload` 和 Electron resources 布局。
- Release CI 校验生成的 TS bindings 已提交且与 Rust schema 同步。

## 接口规范

### Tauri Commands

项目命令：

```text
projects_list
projects_add
projects_update
projects_remove
projects_select_directory
projects_validate_repository
projects_list_branches
```

Review 命令：

```text
review_start_analysis
review_cancel_analysis
review_get_task
review_list_changed_files
review_list_authors
review_get_file_overlay
review_get_commit_overlay
```

### 事件

```text
review://task-updated
```

事件 payload 使用 `AnalysisTaskSnapshot` 语义，字段名称保持 camelCase。

### 错误结构

移除 `recoverable`。错误结构固定为：

```ts
export interface AppError {
  code: string
  message: string
  detail?: string
}
```

Rust 侧对应：

```rust
#[derive(Debug, serde::Serialize, serde::Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    pub code: String,
    pub message: String,
    pub detail: Option<String>,
}
```

规则：

- 不存在 `recoverable`。
- 不存在 TypeScript fallback。
- 不存在前端业务降级。
- Rust/Tauri 是唯一业务错误来源。
- Vue 不根据错误字段选择备用业务实现。
- 二进制或不可预览文件可以返回空 overlay 加 warning；这是业务结果，不是 fallback。

## 类型生成策略

Rust 数据结构作为唯一 schema 来源。核心 struct 使用 `serde` 与 `specta::Type`，通过 `specta-typescript` 生成 TypeScript bindings。

生成文件：

```text
src/renderer/generated/bindings.ts
```

拟生成类型：

```text
ReviewProject
ProjectPreferences
ReviewFilters
AnalysisTaskSnapshot
ChangedFile
FileOverlay
DiffBlock
RelatedCommit
AuthorFilterOption
AppError
```

约束：

- 前端不再手写核心业务类型作为权威来源。
- `src/renderer/generated/bindings.ts` 必须提交。
- CI 中重新生成 bindings，若存在 diff 则失败。
- Rust command 返回值、事件 payload、错误 payload 都必须受 schema 约束。

## 数据流设计

### 项目列表

```text
ProjectHome.vue
  ↓
projectStore.loadProjects()
  ↓
revierClient.projects.list()
  ↓
projects_list
  ↓
ProjectService
  ↓
Tauri app data/projects.json
```

### 添加项目

```text
ProjectEditor.vue
  ↓
revierClient.projects.selectDirectory()
  ↓
projects_select_directory
  ↓
Tauri dialog
  ↓
revierClient.projects.add(repoPath, options)
  ↓
projects_add
  ↓
ProjectService.validate + ProjectService.add
  ↓
projects.json
```

### 启动 Review 分析

```text
ReviewWorkspace.vue
  ↓
reviewStore.start(filters)
  ↓
revierClient.review.startAnalysis(filters)
  ↓
review_start_analysis
  ↓
ReviewService
  ↓
AnalysisService.resolve_scope
  ↓
gix / DuckDB
  ↓
任务缓存 + review://task-updated
```

### 打开文件 overlay

```text
DiffViewer 选择文件
  ↓
reviewStore.loadOverlay(filePath)
  ↓
revierClient.review.getFileOverlay({ taskId, filePath })
  ↓
review_get_file_overlay
  ↓
ReviewService
  ↓
AnalysisService.file_overlay
  ↓
FileOverlay
```

### 提交级下钻

```text
BlockDetailPanel 选择提交
  ↓
reviewStore.loadCommitOverlay(filePath, commitHash)
  ↓
revierClient.review.getCommitOverlay({ taskId, filePath, commitHash })
  ↓
review_get_commit_overlay
  ↓
AnalysisService.commit_overlay
  ↓
FileOverlay(mode = commit)
```

## 删除范围

生产删除：

- `src/main/**`
- `src/preload/**`
- `electron.vite.config.ts`
- `electron-builder.yml`
- Electron E2E 启动路径
- 生产 `src/analysis-core/**`
- 生产 `src/cli/**`
- `electron`
- `electron-vite`
- `simple-git`

保留或迁移：

- `src/renderer/**` 保留并改造 API 调用。
- `src/shared/**` 的核心业务类型迁移到 Rust schema 生成；迁移完成后不再作为权威类型来源。
- `tests/fixtures/**` 可保留为测试 fixture。
- `crates/revier-analysis/**` 保留并改造为库 API 优先，作为根 Cargo workspace 的分析库成员。
- `src-tauri/**` 保留为根 Cargo workspace 的 Tauri 桌面壳成员。
- `scripts/build.ps1` 与 `scripts/build.sh` 可重写为 Tauri 构建辅助脚本。

## 开发环境配置

实施后开发环境要求：

- Node.js 24.x。
- pnpm 10.28.1。
- Rust stable。
- Tauri 平台前置依赖。
- Git CLI 只作为开发工具和测试 fixture 辅助，不作为生产分析 fallback。

常用命令目标：

```powershell
fnm use 24
pnpm install
pnpm dev
pnpm build
pnpm test
pnpm lint
cargo test --workspace
```

Windows 构建脚本继续使用 `pwsh`。Node 相关工具继续使用 `fnm` 和 `pnpm`。Rust 相关依赖通过 Cargo 管理。Python 相关工具若后续需要，使用 `uv`。

根 Cargo workspace 配置保持：

```toml
[workspace]
members = ["crates/revier-analysis", "src-tauri"]
resolver = "2"
```

## 测试策略

### Rust 单元测试

覆盖：

- 项目存储。
- Git 仓库校验。
- 分支读取。
- 提交范围解析。
- 文件列表查询。
- 文件 overlay。
- 提交 overlay。
- 错误结构。
- `AppError` 不包含 `recoverable`。
- Rust schema 可以生成 TS 类型。

### Rust 集成测试

使用临时 Git 仓库 fixture 验证完整 Review 流程：

- `query-files`
- `file-overlay`
- `commit-overlay`
- `list-authors`
- `list-branches`
- `validate-repository`

生产代码不得调用 TypeScript/Node 业务逻辑作为对照 fallback。对照测试可读取固定 fixture 期望值。

### 前端单元测试

Vue store 测试改为 mock `revierClient`：

- 验证 loading 状态。
- 验证 error 展示。
- 验证数据写入。
- 验证事件订阅。
- 验证 Tauri command 返回错误时只写入错误，不触发备用请求。

### 类型生成测试

CI 执行 Rust schema 导出并校验：

- `src/renderer/generated/bindings.ts` 已生成。
- 生成结果与已提交文件一致。
- 若生成文件有 diff，CI 失败。

### 端到端测试

删除 Electron Playwright E2E。第一阶段采用组合测试策略：

- Rust command 集成测试覆盖业务链路。
- 前端 store/component 测试覆盖 UI 链路。

若后续引入 Tauri WebDriver，需要单独设计启动、驱动安装、平台差异和 CI 运行策略。

## 构建与发布

### 本地构建

```text
pnpm dev
  -> tauri dev
  -> Vite dev server
  -> Tauri app

pnpm build
  -> generate bindings check
  -> vue-tsc
  -> vite build
  -> tauri build
```

### CI

Checks 阶段：

- 安装 pnpm。
- 安装 Node.js 24。
- 安装 Rust stable。
- 安装 Tauri 平台前置依赖。
- `pnpm install --frozen-lockfile`。
- bindings 同步校验。
- `pnpm lint`。
- `pnpm test`。
- `cargo test --workspace`。
- `pnpm build`。

Release 阶段：

- Windows、Linux、macOS 分平台运行 Tauri build。
- 上传 Tauri bundler 产物。
- 校验包内不包含 Electron runtime。
- 校验包内不包含 `dist/main` 和 `dist/preload`。
- 校验包内不存在 Electron resources 下的 `revier-analysis` 布局。

## 风险与控制

| 风险 | 影响 | 控制 |
| --- | --- | --- |
| WebView 差异 | Diff 布局、字体、滚动和选择样式跨平台不一致 | 增加跨平台 smoke、截图或人工验证清单。 |
| Rust 能力缺口 | 删除 TypeScript fallback 后功能断层 | 实施计划先补齐 Rust 等价能力，再删除旧路径。 |
| 类型生成复杂度 | 构建步骤增加，CI 失败点增加 | bindings 生成脚本固定，CI 做 diff 校验。 |
| 数据迁移 | 用户旧项目列表丢失 | 当前保持 JSON 结构兼容；旧 Electron userData 首次迁移标记为后续 TODO，实施前需确认各平台旧路径。 |
| 发布链路重建 | 现有 Electron release CI 不能复用 | 分平台重写 Tauri release，并校验产物内容。 |
| DuckDB native build | `duckdb/bundled` 会增加本地编译耗时，并依赖平台 C/C++ 编译工具链 | CI 安装平台 native build 依赖；Windows 使用带 MSVC 工具链的 runner；本地以 `cargo test --workspace` 和 Tauri build 验证。 |
| 破坏性删除 | 删除范围过大导致测试和工具断裂 | 按实施计划分阶段删除，每阶段有验证命令。 |

## 完成标准

1. 应用以 Tauri 启动。
2. Vue UI 能完成添加仓库、运行分析、查看文件列表、查看文件 overlay、查看提交 overlay。
3. 生产依赖不再包含 Electron、electron-vite、simple-git。
4. 生产代码不存在 TypeScript/Node 分析 fallback。
5. `AppError` 不包含 `recoverable`。
6. 核心 TypeScript 类型由 Rust schema 生成。
7. `src/renderer/generated/bindings.ts` 已提交且 CI 校验同步。
8. Release CI 使用 Tauri bundler。
9. Windows、macOS、Linux 产物校验通过。
10. README 和开发文档更新为 Tauri 架构。

## 后续实施顺序建议

1. 建立 `src-tauri` 最小 Tauri 壳和 Vite 构建配置。
2. 引入 Rust schema 与 TS bindings 生成。
3. 把项目管理迁移到 Rust command。
4. 把 Review task manager 迁移到 Rust command/event。
5. 把 `query-files`、`file-overlay`、`commit-overlay`、`list-authors`、`list-branches` 补齐为 Rust 生产能力。
6. 改造 Vue store 使用 `revierClient`。
7. 删除 Electron、preload、Electron IPC 和生产 TypeScript/Node fallback。
8. 重写构建脚本和 release CI。
9. 更新 README、测试和验证文档。
