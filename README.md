# Revier

Revier 是一款基于 **Electron + Vue 3 + TypeScript** 的本地 Git 代码 Review 辅助工具。它面向需要回顾一段提交历史、定位文件最终净变化、追踪变更块历史作者与相关提交的场景，帮助 Reviewer 从“文件发生了什么变化”进一步下钻到“这些变化由哪些提交与作者参与”。

## 核心能力

- **本地仓库管理**：添加、校验、保存和移除本地 Git 仓库项目。
- **多维度筛选提交范围**：按分支、时间、作者、提交信息和文件 Glob 规则筛选 Review 范围。
- **变更文件列表**：展示筛选范围内存在最终净变化的文件，并标记新增、修改、删除、重命名、二进制等状态。
- **Side-by-side Diff 浏览**：以左右对照方式查看文件在筛选区间内的最终净变化，并支持修改行词级高亮。
- **变更块归因**：在每个连续变更块上聚合历史参与作者，辅助快速识别代码改动来源。
- **提交详情面板**：选中变更块后查看相关提交、作者、时间、标题和命中筛选条件的信息。
- **提交级下钻**：从变更块详情中点击具体提交，打开覆盖式提交 Diff 视图，查看单个提交对当前文件的影响。
- **会话缓存与项目偏好**：缓存分析任务数据，并持久化项目名称、路径、默认分支、默认时间范围、Glob 规则及最近筛选条件。

## 技术栈

| 层级 | 技术/库 | 用途 |
| --- | --- | --- |
| 桌面运行时 | Electron 32、electron-vite | 主进程、预加载脚本与渲染进程打包运行 |
| 前端 | Vue 3、TypeScript、Vite | Review 工作区和项目管理界面 |
| 状态管理 | Pinia | 项目列表、分析任务、Diff 结果与 UI 状态 |
| UI | Naive UI、lucide-vue-next、CSS | 表单、按钮、布局和图标 |
| Git 访问 | simple-git + Git CLI | 仓库校验、分支、提交、文件内容和 Patch 读取 |
| Diff/筛选 | diff、minimatch | 行级/词级 Diff、Glob 规则匹配 |
| Rust 分析 | Rust、gix、DuckDB | 本地索引、文件列表查询、Rust overlay 与块级归因实验接入 |
| 测试 | Vitest、Vue Test Utils、Playwright | 单元、集成、性能和端到端测试 |

## 运行环境

请确保本机已安装：

- **Node.js 24.x**：项目声明的引擎范围为 `>=24 <25`。
- **pnpm 10.28.1**：仓库通过 `packageManager` 固定 pnpm 版本。
- **Git CLI**：Revier 通过本机 Git 读取仓库历史。
- **Rust stable**：`crates/revier-analysis` 提供本地索引、Rust overlay 和归因 CLI。

建议使用 Corepack 管理 pnpm：

```bash
corepack enable
corepack prepare pnpm@10.28.1 --activate
```

### Windows DuckDB 处理

Rust 侧使用 `duckdb` crate，但不启用 `bundled` 特性。Windows 开发和验证采用 DuckDB 上游支持的替代路径：设置 `DUCKDB_DOWNLOAD_LIB=1`，由构建脚本下载官方预编译动态库并完成链接，避免依赖本机系统级 DuckDB 安装。

仓库中的 Rust 相关 pnpm 脚本统一通过 `scripts/cargo-duckdb-download.mjs` 调用 Cargo。该包装脚本会自动注入 `DUCKDB_DOWNLOAD_LIB=1`：

```bash
pnpm rust:test
pnpm rust:index:build -- --repo E:/repo/app --branch main --format json
pnpm rust:file-overlay -- --repo E:/repo/app --base <base> --head <head> --branch main --file src/app.ts --format json
```

如果绕过 pnpm 脚本直接运行 Cargo，需要手动设置环境变量：

```powershell
$env:DUCKDB_DOWNLOAD_LIB = '1'
cargo test --workspace
cargo run -p revier-analysis -- index status --repo E:/repo/app --format json
```

直接运行已构建的 `target/debug/revier-analysis.exe` 时，如遇到 DuckDB 动态库加载失败，请将 `target/debug/deps` 加入当前终端的 `PATH`，或改用上面的 `cargo run` / `pnpm rust:*` 脚本。

## 快速开始

```bash
# 1. 安装依赖
pnpm install

# 2. 启动 Electron 开发环境
pnpm dev
```

启动后，在项目首页选择一个本地 Git 仓库目录，Revier 会校验该目录是否为 Git 仓库，并将其加入项目列表。

## 常用脚本

| 命令 | 说明 |
| --- | --- |
| `pnpm dev` | 启动 electron-vite 开发环境 |
| `pnpm build` | 先执行类型检查，再构建 Electron 应用产物到 `dist` |
| `pnpm preview` | 预览已构建的 Electron 应用 |
| `pnpm typecheck` | 分别检查渲染进程和 Node/Electron 侧 TypeScript 类型 |
| `pnpm test` | 运行 Vitest 单元与集成测试 |
| `pnpm test:watch` | 以监听模式运行 Vitest |
| `pnpm test:e2e` | 构建后运行 Playwright 端到端测试 |
| `pnpm test:perf` | 运行性能测试配置 |
| `pnpm lint` | 使用 ESLint 检查代码 |
| `pnpm rust:test` | 设置 `DUCKDB_DOWNLOAD_LIB=1` 后运行 Rust workspace 测试 |
| `pnpm rust:index:status` | 调用 Rust CLI 查看 DuckDB 索引状态 |
| `pnpm rust:index:build` | 调用 Rust CLI 构建 DuckDB 索引 |
| `pnpm rust:index:query-files` | 调用 Rust CLI 查询索引中的变更文件 |
| `pnpm rust:file-overlay` | 调用 Rust CLI 输出文件级 overlay JSON |
| `pnpm rust:trace-block` | 调用 Rust CLI 输出单个变更块归因 JSON |

## 使用流程

1. **添加项目**
   - 在首页选择本地 Git 仓库目录。
   - 应用会读取仓库根目录和当前分支，并保存项目记录。

2. **进入 Review 工作区**
   - 选择项目后进入三栏布局：左侧筛选与文件列表，中间 Diff 主视图，右侧变更块详情。

3. **设置筛选条件**
   - 选择分支和时间范围。
   - 可按作者、提交信息关键字和文件 Glob 规则进一步过滤。
   - Glob 规则支持包含和排除，例如：

   ```text
   src/**/*.ts
   !**/*.test.ts
   ```

4. **运行分析**
   - 主进程创建分析任务，解析提交区间并计算最终净变化文件列表。
   - 首个变更文件会自动加载 Diff；也可以从左侧文件列表手动切换文件。

5. **浏览与下钻**
   - 在中间区域查看文件级 Side-by-side Diff。
   - 点击变更块可在右侧查看参与作者和相关提交。
   - 点击某条相关提交可打开提交级下钻视图；点击返回关闭下钻，回到文件级 Diff。

## 项目结构

```text
.
├── docs/superpowers/          # 设计文档与实现计划
├── src/
│   ├── main/                  # Electron 主进程：窗口、IPC、Git、分析与项目持久化
│   │   ├── analysis/          # 变更文件解析、范围解析、Overlay Diff、归因、Glob 等分析逻辑
│   │   ├── app/               # BrowserWindow 创建
│   │   ├── cache/             # 会话缓存
│   │   ├── git/               # simple-git 封装
│   │   ├── ipc/               # projects/review IPC 路由
│   │   └── projects/          # JSON 项目存储
│   ├── preload/               # contextBridge 暴露的安全 API
│   ├── renderer/              # Vue 渲染进程
│   │   ├── components/        # 项目管理、筛选、文件列表、Diff、详情和下钻组件
│   │   ├── composables/       # 可复用交互逻辑
│   │   ├── pages/             # 项目首页与 Review 工作区页面
│   │   └── stores/            # Pinia 状态管理
│   └── shared/                # 主进程、预加载和渲染进程共享类型
├── tests/
│   ├── unit/                  # 单元测试
│   ├── integration/           # 集成测试
│   ├── e2e/                   # Playwright 端到端测试
│   ├── perf/                  # 性能测试
│   └── fixtures/              # Git 仓库测试夹具
├── electron.vite.config.ts    # Electron/Vite 构建配置
├── playwright.config.ts       # E2E 测试配置
├── vitest.config.ts           # Vitest 测试配置
└── package.json               # 依赖、脚本和运行环境声明
```

## 架构概览

Revier 采用 Electron 三段式架构：

1. **Renderer（渲染进程）**
   - 负责 Vue 页面、筛选表单、项目列表、Diff 展示和交互。
   - 不直接访问 Node.js 文件系统或 Git，而是通过预加载脚本暴露的受控 API 通信。

2. **Preload（预加载脚本）**
   - 使用 `contextBridge` 在 `window.revier` 下暴露项目和 Review 相关方法。
   - 将渲染进程请求映射到类型化 IPC Channel。

3. **Main（主进程）**
   - 负责窗口生命周期、项目 JSON 持久化、IPC 处理、Git 调用和分析任务。
   - Review 分析按任务组织，避免把耗时 Git 计算直接放在 UI 渲染流程中。

核心数据流：

```text
Vue 组件
  ↓ Pinia Store
window.revier API（Preload）
  ↓ IPC
Main IPC Handler
  ↓
GitService / Analysis Engine / ProjectStore
  ↓
FileOverlay、ChangedFile、RelatedCommit 等共享类型
```

## Review 分析逻辑

一次 Review 分析大致包含以下阶段：

1. 根据项目和筛选条件读取分支提交列表。
2. 按开始/结束时间解析真实提交区间。
3. 使用 `git diff --name-status --find-renames` 和 `git diff --numstat` 生成变更文件摘要。
4. 按 Glob 规则过滤文件。
5. 用户选择文件后，读取区间起点和终点的文件内容，生成文件级 Overlay Diff。
6. 收集筛选范围内触达当前文件的提交，解析 Patch 影响行范围。
7. 将相关提交归因到 Diff 变更块，得到作者列表、相关提交和筛选命中信息。
8. 用户点击具体提交时，再按需生成提交级 Overlay Diff。

## 数据持久化

项目列表和偏好保存在 Electron `userData` 目录下的 `projects.json` 中。测试或自动化场景可以通过 `REVIER_USER_DATA_DIR` 环境变量覆写该目录，避免污染真实用户数据。

持久化内容包括：

- 项目 ID、名称、仓库路径、固定状态和最近打开时间。
- 默认分支、默认时间范围、默认 Glob 规则。
- 每个项目最近一次 Review 筛选条件。

## 测试说明

项目测试覆盖了以下重点：

- Git 服务、变更文件解析和提交范围解析。
- 分析任务管理、Overlay Diff、变更块归因和 Patch 行范围解析。
- 项目存储、IPC 行为和共享类型约束。
- Vue 组件交互、布局尺寸、筛选面板、Diff 下钻稳定性。
- 中型仓库性能场景与端到端 Review 流程。

常规开发建议至少运行：

```bash
pnpm typecheck
pnpm test
pnpm rust:test
```

涉及完整 Electron 流程时再运行：

```bash
pnpm test:e2e
```

## 设计文档

更多产品范围、交互设计和迭代计划可参考：

- `docs/superpowers/specs/2026-06-03-revier-mvp-design.md`
- `docs/superpowers/specs/2026-06-04-revier-review-workspace-interaction-design.md`
- `docs/superpowers/specs/2026-06-04-revier-visual-design.md`
- `docs/superpowers/specs/2026-06-12-revier-drilldown-overlay-stability-design.md`
- `docs/superpowers/plans/`

## 开发提示

- 渲染进程如需新增主进程能力，应优先在 `src/shared/ipcTypes.ts` 定义类型和 Channel，再通过 `src/preload/index.ts` 暴露受控 API。
- Git 相关能力集中在 `src/main/git/gitService.ts`，分析算法集中在 `src/main/analysis/`，便于单元测试。
- 共享数据结构应放在 `src/shared/`，避免主进程和渲染进程类型漂移。
- 新增可见交互时，建议同步补充 Vue 组件测试或 E2E 流程。

## 当前状态

Revier 目前处于 MVP/早期迭代阶段，重点能力已围绕本地 Git Review、筛选、文件级 Diff、变更块归因和提交级下钻展开。后续可继续增强大型仓库索引、更多 Diff 模式、持久化缓存和发布打包能力。
