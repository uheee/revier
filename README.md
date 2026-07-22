# Revier

Revier 是一款基于 Tauri、Vue 3 和 Rust 的本地 Git 代码 Review 辅助工具。Vue 只负责界面显示和交互状态，Tauri/Rust 负责本地仓库访问、项目持久化、分析任务、DuckDB 索引和 Diff/归因计算。

## 核心能力

- 本地仓库管理：添加、校验、保存和移除本地 Git 仓库项目。
- 多维度筛选提交范围：按分支、时间、作者、提交信息和文件 Glob 规则筛选 Review 范围。
- 变更文件列表：展示筛选范围内存在最终净变化的文件，并标记新增、修改、删除、重命名、二进制等状态。
- Side-by-side Diff 浏览：以左右对照方式查看文件在筛选区间内的最终净变化，并支持修改行词级高亮。
- 变更块归因：在每个连续变更块上聚合历史参与作者，辅助快速识别代码改动来源。
- 持久化分析缓存：每个分支保留最近一次成功项目快照，文件作者轨按规范块签名写入 DuckDB，后续打开优先从缓存恢复。
- 显式刷新：项目和当前文件分别提供重新分析入口；刷新失败时继续保留原有文件结果与提交下钻。
- 提交详情面板：选中变更块后查看相关提交、作者、时间、标题和命中筛选条件的信息。
- 提交级下钻：从变更块详情中点击具体提交，打开覆盖式提交 Diff 视图，查看单个提交对当前文件的影响。
- 项目偏好：持久化项目名称、路径、默认分支、默认时间范围、Glob 规则及最近筛选条件。

## 技术栈

| 层级 | 技术/库 | 用途 |
| --- | --- | --- |
| 桌面运行时 | Tauri 2 | 本地窗口、命令调用、事件订阅和应用打包 |
| 前端 | Vue 3、TypeScript、Vite | Review 工作区和项目管理界面 |
| 状态管理 | Pinia | 项目列表、分析任务、Diff 结果与 UI 状态 |
| UI | Naive UI、lucide-vue-next、CSS | 表单、按钮、布局和图标 |
| Rust 分析 | Rust、gix、DuckDB | 仓库访问、索引、文件列表查询、Overlay Diff 与块级归因 |
| 测试 | Vitest、Vue Test Utils、Cargo test | 前端单元测试与 Rust 分析/服务测试 |

## 运行环境

请确保本机已安装：

- Node.js 24.x：项目声明的引擎范围为 `>=24 <25`。
- pnpm 10.28.1：仓库通过 `packageManager` 固定 pnpm 版本。
- Rust stable：Tauri 应用和 `revier-analysis` crate 都依赖 Rust 工具链。
- Git CLI：测试夹具和部分构建验证会使用本机 Git。

建议使用 Corepack 或 `fnm` 管理 Node/pnpm。当前仓库脚本假定可直接执行 `pnpm`。

## 快速开始

```bash
pnpm install
pnpm dev
```

启动后，在项目首页打开一个本地 Git 仓库目录，Revier 会校验该目录是否为 Git 仓库，并将其加入项目列表。

## 常用脚本

| 命令 | 说明 |
| --- | --- |
| `pnpm dev` | 启动 Tauri 开发环境 |
| `pnpm build` | 执行类型检查、生成类型同步检查，并构建 Tauri 应用 |
| `pnpm preview` | 预览 Vite 构建产物 |
| `pnpm typecheck` | 检查渲染层 TypeScript/Vue 类型 |
| `pnpm test` | 运行 Vitest 前端单元测试 |
| `pnpm test:watch` | 以监听模式运行 Vitest |
| `pnpm lint` | 执行类型检查并运行 ESLint |
| `pnpm generate:bindings` | 从 Rust contracts 生成前端 TypeScript bindings |
| `pnpm generate:bindings:check` | 生成 bindings 并确认没有未提交差异 |

Rust 验证：

```powershell
cargo test --workspace
```

平台构建脚本仍可用于准备图标和调用 Tauri 打包：

```powershell
pwsh -NoLogo -ExecutionPolicy Bypass -File scripts/build.ps1 -Platform win -Arch x64 -Package
```

```bash
bash scripts/build.sh --platform linux --arch x64 --package
bash scripts/build.sh --platform mac --universal --package
```

## 架构概览

```text
Vue 组件
  ↓ Pinia Store
revierClient
  ↓ Tauri invoke/listen
Tauri Commands / Events
  ↓
Rust ProjectService / ReviewService / Analysis API
  ↓
gix / DuckDB / 文件系统
```

前端通过 `src/renderer/api/revierClient.ts` 调用 Tauri 命令，不直接访问 Node.js、文件系统或 Git。Rust 侧通过 `src-tauri` 管理应用状态和服务，通过 `crates/revier-analysis` 提供仓库访问、索引查询、范围解析、文件 overlay 和归因能力。

## 项目结构

```text
.
├── crates/
│   └── revier-analysis/       # Rust 分析库与 CLI
├── docs/superpowers/          # 设计文档与实现计划
├── src/
│   └── renderer/              # Vue 前端
│       ├── api/               # Tauri 客户端和错误适配
│       ├── components/        # 项目管理、筛选、文件列表、Diff、详情和下钻组件
│       ├── composables/       # 可复用交互逻辑
│       ├── generated/         # Rust contracts 生成的 TypeScript 类型
│       ├── pages/             # 项目首页与 Review 工作区页面
│       └── stores/            # Pinia 状态管理
├── src-tauri/                 # Tauri 应用、命令、状态和服务
├── tests/unit/                # 前端单元测试
├── vitest.config.ts           # Vitest 测试配置
└── package.json               # 依赖、脚本和运行环境声明
```

## Review 分析流程

1. 前端收集项目 ID、分支、时间范围、作者、提交信息和 Glob 规则。
2. `revierClient` 通过 Tauri `invoke` 调用 Rust review command。
3. `ReviewService` 校验项目仓库并解析分析范围。
4. `revier-analysis` 查询 DuckDB 索引中的变更文件和提交元数据。
5. 用户选择文件后，Rust 通过项目快照中的 Blob ID 读取不可变内容；规范块签名命中时直接恢复文件归因，否则仅计算并缓存目标文件。
6. 用户点击相关提交时，Rust 优先按当前文件分析和提交哈希恢复下钻缓存；未命中时生成提交级 Overlay Diff 并原子写入缓存。

## 数据持久化

项目列表和偏好保存在 Tauri 应用数据目录下的 `projects.json` 中。持久化内容包括：

- 项目 ID、名称、仓库路径、固定状态和最近打开时间。
- 默认分支、默认时间范围、默认 Glob 规则。
- 每个项目最近一次 Review 筛选条件。

仓库索引数据库按“项目 + 分支”保存一份最近成功的分析快照，并以规范化结构保存文件块、作者归因、相关提交引用和已经打开过的提交下钻，不复制源码正文。再次打开项目、文件或同一提交时优先读取缓存；分支 HEAD 变化会将快照标记为过期，不会自动混入新提交。此时需先点击“重新分析项目”，项目缓存有效时也可单独点击“重新分析当前文件”。所有载入操作都会在底部状态栏持续显示当前对象、阶段、缓存状态和耗时。

缓存数据库当前为 schema v3。每个仓库的不同分支分别保留一份最近成功快照；同一分支的新快照以事务替换旧快照。文件归因按当前项目快照与文件保存一份，提交下钻按当前文件分析与提交分别保存一份，源码正文不进入 DuckDB，只记录 Git Blob ID、块坐标和归因关系。详细表结构、迁移和失效规则见 [`docs/cache-schema.md`](docs/cache-schema.md)。

## 性能诊断

开发构建可通过环境变量输出后端分析阶段、完成量、总量和累计耗时：

```powershell
$env:REVIER_TRACE_OPERATIONS = '1'
pnpm tauri dev
Remove-Item Env:REVIER_TRACE_OPERATIONS
```

`REVIER_TRACE_OPERATIONS` 只在 Rust debug 构建中生效，日志写入启动终端，不改变缓存或分析结果。前端底部状态栏会显示当前操作、目标、缓存状态和耗时；未知总量的阶段不会显示虚假的百分比。最终冷热路径验证数据见 [`阶段八验收记录`](docs/superpowers/verification/2026-07-23-revier-performance-cache-progress-phase-8.md)。

## 测试说明

常规开发建议至少运行：

```bash
pnpm typecheck
pnpm test
cargo test --workspace
```

涉及 Rust/前端类型契约时还应运行：

```bash
pnpm generate:bindings:check
```

## 设计文档

更多产品范围、交互设计和迭代计划可参考：

- `docs/superpowers/specs/2026-06-03-revier-mvp-design.md`
- `docs/superpowers/specs/2026-06-04-revier-review-workspace-interaction-design.md`
- `docs/superpowers/specs/2026-06-04-revier-visual-design.md`
- `docs/superpowers/specs/2026-06-12-revier-drilldown-overlay-stability-design.md`
- `docs/superpowers/specs/2026-07-06-revier-tauri-migration-design.md`
- `docs/superpowers/plans/`

## 开发提示

- 新增前端调用时，优先在 Rust contracts 中定义类型，重新生成 `src/renderer/generated/bindings.ts`。
- 新增 Tauri 能力时，在 `src-tauri/src/commands` 暴露命令，并在 `src-tauri/src/lib.rs` 注册。
- 新增分析能力时，优先落在 `crates/revier-analysis`，并补 Rust contract/API 测试。
- 新增可见交互时，同步补充 Vue 组件或 store 测试。
