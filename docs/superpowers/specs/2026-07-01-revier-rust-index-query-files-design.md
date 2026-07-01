# Revier Rust 索引与文件列表查询一期设计

> 状态：待审阅。本文记录 Rust + DuckDB 索引一期的设计，目标是优先解决筛选文件列表卡顿，不迁移 overlay 和归因逻辑。

## 背景

Revier 当前分析入口在 Electron 主进程中完成范围解析、文件列表加载和筛选。文件列表初始加载依赖 `GitService.listChangedFiles()`，当作者、提交信息等显示筛选开启时，会进入 `filterFilesByMatchingCommits()`：

1. 先用筛选条件找出匹配提交。
2. 再对每个候选文件遍历匹配提交。
3. 对每个 `file + commit` 组合调用 `showFilePatch()`。
4. 解析 patch touched ranges 后判断该文件是否应显示。

这个模型在小仓库可用，但在大仓库或提交范围较大时会形成 `候选文件数 * 匹配提交数` 的重复 Git 子进程调用和 patch 解析，直接影响筛选文件列表的响应速度。

此前 `2026-06-15-revier-rust-gix-duckdb-index-design.md` 已确认长期方向：使用 Rust CLI、gix 和 DuckDB 建立本地索引。最新 Rust gix capability spike 已验证基础 gix 能力。本规约把长期设计收敛为第一条正式业务纵切：只建设索引与 `query-files` 查询，并接入 Electron 文件列表查询路径。

## 目标

1. 在现有 `crates/revier-analysis` 上扩展 Rust CLI 索引命令。
2. 使用 gix 读取本地 Git 仓库，生产路径不调用 `git` 进程。
3. 使用 DuckDB 保存提交、父提交、提交触碰文件和索引运行状态。
4. 提供 `index status`、`index build`、`index query-files` 三个命令。
5. Electron 主进程在显示提交筛选阶段调用 Rust `query-files`。
6. 已建索引后，作者、作者搜索、提交信息搜索和 glob 组合筛选不再逐文件逐提交读取 patch。
7. 索引不可用时返回明确错误，由 Electron 使用现有 TypeScript 路径降级或显示可恢复提示。

## 非目标

- 本阶段不迁移 `file-overlay`、`trace-block`、blame、merge trace 或 UI 渲染。
- 本阶段不保存完整文件内容到 DuckDB。
- 本阶段不做语义级函数、类或 AST 索引。
- 本阶段不引入远程 Git 平台 API。
- 本阶段不实现后台常驻服务；Rust CLI 仍由 Electron 主进程按需调用。
- 本阶段不承诺冷索引瞬时完成，但必须可观测、可失败恢复。

## 技术选型

### Rust CLI

沿用当前 workspace 中的 `crates/revier-analysis`。理由：

- 已有 `clap`、`serde`、gix 能力 spike 和 JSON 输出基础。
- 本地 Git 对象遍历与索引写入属于 I/O 密集任务，Rust 更适合控制资源与错误边界。
- 独立 CLI 便于 Electron 主进程调用，也便于后续性能 fixture 验证。

### gix

gix 继续作为生产 Git 读取层：

- 读取 commit、parent、tree 和 blob 元数据。
- 对每个提交与每个 parent 执行 pairwise tree diff。
- 读取 rename/rewrite 信息时沿用 spike 中已经验证的 gix diff 能力。

本阶段不使用 Git CLI 作为生产 fallback。测试 fixture 可以使用 Git CLI 构造仓库，但被测 Rust 查询路径不得调用 `git` 进程。

### DuckDB

DuckDB 用于保存结构化索引：

- 提交元数据适合按作者、时间、提交信息做 SQL 筛选。
- 文件触碰关系适合从匹配提交反查文件列表。
- 本地单文件数据库便于按仓库隔离和重建。

第三方 Rust DuckDB binding 的具体 crate 不在本规约内直接选定。实施计划必须列出候选、版本、维护状态和 Windows 构建约束，获得确认后再添加依赖。

## 架构设计

### 总体结构

```text
Renderer
  -> review store
    -> preload IPC
      -> Electron 主进程 reviewIpc
        -> RustAnalysisClient
          -> revier-analysis index query-files
            -> DuckDB index
            -> gix repository
```

索引构建路径：

```text
Electron 主进程或调试命令
  -> revier-analysis index build
    -> git 模块读取提交和 tree diff
    -> index 模块写入 DuckDB
```

### Rust 模块划分

```text
crates/revier-analysis/src/
  cli.rs
  error.rs
  json.rs
  git/
    repository.rs
    commits.rs
    diff.rs
  index/
    connection.rs
    schema.rs
    writer.rs
    queries.rs
    migrations.rs
  commands/
    index_status.rs
    index_build.rs
    query_files.rs
```

依赖方向：

```text
commands -> index -> DuckDB
commands -> git -> gix
index 不依赖 git
git 不依赖 index
json 只负责稳定输出契约
```

### Electron 模块划分

新增一个主进程适配层：

```text
src/main/analysis/rustAnalysisClient.ts
```

职责：

- 解析 Rust 二进制路径。
- 组装 CLI 参数。
- 读取 stdout JSON。
- 将 Rust 错误码转换为 `AppError` 或可降级错误。
- 不解析 Git 语义，不持有索引业务逻辑。

`src/main/ipc/reviewIpc.ts` 的职责保持为任务编排。第一阶段只在 `resolveAnalysisScope()` 且存在作者、作者搜索或提交信息筛选时尝试调用 Rust `query-files`。没有显示提交筛选时继续使用现有 `listChangedFiles()` 加 glob 过滤路径，避免为简单场景引入额外 CLI 启动开销。

## 数据流程

### 索引构建

1. CLI 接收 `index build --repo <path> --branch <branch>`。
2. 使用 gix 打开仓库并解析仓库根目录和 common dir。
3. 根据 repo root 与 common dir 生成稳定 `repo_id`。
4. 打开或初始化 DuckDB。
5. 读取 schema version，不兼容时返回明确错误或按实施计划执行迁移。
6. 遍历目标分支可达提交。
7. 对缺失提交写入 `commits` 和 `commit_parents`。
8. 对每个提交与每个 parent 执行 pairwise tree diff。
9. 写入 `commit_files`。
10. 写入 `index_runs`，记录耗时、状态、提交数和文件数。

### 文件列表查询

1. Electron 根据当前项目、分支和筛选条件解析 `baseCommit`、`headCommit`。
2. 主进程调用 `revier-analysis index query-files`。
3. Rust 查询 DuckDB：
   - 根据 `base..head` 得到范围提交。
   - 根据作者、作者搜索、提交信息搜索和时间条件筛选提交。
   - 根据 glob 规则筛选文件路径。
   - 从 `commit_files` 聚合文件状态、additions、deletions、binary 状态。
4. Rust 输出与现有 `ChangedFile[]` 兼容的 JSON。
5. Electron 将结果写入现有 `filesByTask`，后续 overlay 仍走 TypeScript 链路。

### 降级流程

如果 Rust 返回以下状态：

- 索引不存在。
- schema 不兼容。
- DuckDB 打开失败。
- Rust 二进制不可用。

Electron 第一阶段允许降级到现有 TypeScript 文件列表路径，并在任务 warning 中记录中文提示。是否在 UI 中提供“构建索引”入口由后续实施计划确认，本规约不直接新增 UI。

## DuckDB 表设计

### `metadata`

```sql
create table metadata (
  key text primary key,
  value text not null
);
```

必须包含：

- `schema_version`
- `repo_id`
- `repo_root`
- `git_common_dir`
- `created_at`
- `updated_at`

### `commits`

```sql
create table commits (
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
```

### `commit_parents`

```sql
create table commit_parents (
  commit_hash text not null,
  parent_hash text not null,
  parent_index integer not null,
  primary key (commit_hash, parent_index)
);
```

### `commit_files`

```sql
create table commit_files (
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
```

`old_path` 没有值时写入空字符串，Rust 输出 JSON 时再转换为 `null` 或省略字段。

`status` 取值与现有 `ChangedFile.status` 对齐：

- `added`
- `modified`
- `deleted`
- `renamed`
- `binary`

### `index_runs`

```sql
create table index_runs (
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
```

`status` 取值：

- `running`
- `completed`
- `failed`
- `cancelled`

## CLI 设计

### 命令

```bash
revier-analysis index status --repo E:/repo/app --format json
revier-analysis index build --repo E:/repo/app --branch main --format json
revier-analysis index query-files --repo E:/repo/app --base <hash> --head <hash> --branch main --author <key> --author-query "alice" --message "fix" --glob "src/**/*.ts" --format json
```

### 通用参数

- `--repo`：本地 Git 仓库路径。
- `--db`：可选 DuckDB 文件路径；不传时使用默认 app data 索引目录。
- `--format`：本阶段只支持 `json`。
- `--pretty`：输出格式化 JSON。

### `query-files` 参数

- `--base`：分析基准提交。
- `--head`：分析目标提交。
- `--branch`：当前筛选分支。
- `--author`：作者 key，可重复传入。
- `--author-query`：作者名或邮箱文本搜索。
- `--message`：提交信息文本搜索。
- `--since`：提交时间下限，ISO 字符串。
- `--until`：提交时间上限，ISO 字符串。
- `--glob`：文件路径规则，可重复传入。

### 退出码

- `0`：成功。
- `2`：参数错误。
- `3`：仓库读取失败。
- `4`：索引不存在或不可用。
- `5`：索引 schema 不兼容。
- `6`：DuckDB 读写失败。
- `10`：内部错误。

错误信息写入 stderr，stdout 只写 JSON。

## JSON 契约

### `index status`

```json
{
  "version": 1,
  "repoId": "repo-id",
  "schemaVersion": 1,
  "status": "ready",
  "indexedCommitCount": 1200,
  "indexedFileCount": 8500,
  "updatedAt": "2026-07-01T00:00:00.000Z",
  "warnings": []
}
```

`status` 取值：

- `missing`
- `building`
- `ready`
- `stale`
- `incompatible`

### `index build`

```json
{
  "version": 1,
  "repoId": "repo-id",
  "status": "completed",
  "indexedCommitCount": 1200,
  "indexedFileCount": 8500,
  "elapsedMs": 3200,
  "warnings": []
}
```

### `index query-files`

```json
{
  "version": 1,
  "range": {
    "baseCommit": "base",
    "headCommit": "head"
  },
  "files": [
    {
      "path": "src/app.ts",
      "oldPath": null,
      "status": "modified",
      "additions": 10,
      "deletions": 2,
      "isBinary": false,
      "isPreviewable": true
    }
  ],
  "warnings": []
}
```

字段命名使用 camelCase，保持与 TypeScript `ChangedFile` 契约一致。

## 筛选语义

第一阶段只替换文件列表筛选，不改变用户可见语义：

- `authorKeys` 对应 `--author`，匹配 `author_key`。
- `authorQuery` 对应 `--author-query`，匹配作者名和邮箱。
- `messageQuery` 对应 `--message`，匹配提交 subject。
- `globRules` 对应 `--glob`，匹配文件新路径；rename 文件同时尝试 `old_path`。
- `startAt`、`endAt` 对应 `--since`、`--until`。

当没有作者、作者搜索或提交信息筛选时，Electron 第一阶段保留现有 `listChangedFiles()` 路径。Rust CLI 仍提供基础 `query-files` 能力，供测试和后续扩大接入范围使用。

## 索引位置与失效

默认索引文件放在 Revier app data 目录：

```text
<app-data>/revier/indexes/<repo-id>.duckdb
```

`repo_id` 生成规则：

1. 规范化 repo root。
2. 规范化 Git common dir。
3. 组合后计算稳定哈希。

失效规则：

- schema version 不一致时标记 `incompatible`。
- 查询范围存在未索引提交时标记 `stale`。
- 仓库对象缺失时标记 `stale` 并返回 warning。
- `query-files` 不隐式执行长时间 `index build`。

## 开发环境配置

- Shell 使用 pwsh。
- Node 工具使用 `fnm` 和 `pnpm`。
- Rust 使用现有 Cargo workspace。
- 新增 Rust 依赖前必须在实施计划中列出并确认。
- DuckDB 数据库文件不得写入被分析仓库，除非用户显式传入 `--db` 调试路径。

建议新增脚本由实施计划确认：

```json
{
  "scripts": {
    "rust:index:status": "cargo run -p revier-analysis -- index status",
    "rust:index:build": "cargo run -p revier-analysis -- index build",
    "rust:index:query-files": "cargo run -p revier-analysis -- index query-files"
  }
}
```

## 测试策略

### Rust 单元测试

- CLI 参数解析。
- JSON 输出字段命名。
- 错误码映射。
- schema 初始化。
- SQL 查询条件组合。

### Rust 集成测试

- 使用临时 Git 仓库构造线性提交、rename、merge 提交。
- `index build` 后 `index status` 返回 `ready`。
- `query-files` 能按作者、作者搜索、提交信息、时间和 glob 返回正确文件。
- 索引不存在时返回退出码 `4`。
- schema 不兼容时返回退出码 `5`。

### Electron 回归测试

- `resolveAnalysisScope()` 在 Rust 查询成功时使用 Rust 返回文件列表。
- Rust 索引不可用时保留现有 TypeScript 行为。
- `filesByTask`、`rangeCommitsByTask` 和 overlay 请求链路不变。

### 性能验证

至少记录以下指标：

- 当前 TypeScript 筛选文件列表耗时。
- 冷索引构建耗时。
- 已建索引后 `query-files` 耗时。
- Rust CLI 启动和 JSON 解析开销。

目标：已建索引后，常规作者、提交信息和 glob 组合筛选在 500ms 内返回文件列表。该目标作为性能验收参考，真实阈值可在实施计划中结合 fixture 规模细化。

## 验收标准

- Rust CLI 提供 `index status`、`index build`、`index query-files`。
- 生产 Rust 查询路径不调用 `git` 进程。
- DuckDB 索引保存提交、父提交、文件触碰关系和索引运行状态。
- `query-files` 输出与现有 `ChangedFile[]` 兼容。
- Electron 文件列表筛选可使用 Rust 查询结果。
- Rust 查询不可用时有明确错误和可恢复降级，不影响 overlay 打开。
- Rust 单元测试、Rust 集成测试和相关 Electron 回归测试通过。

## 风险与处理

- 冷索引较慢：不在 `query-files` 中隐式构建，避免阻塞用户筛选。
- DuckDB binding 在 Windows 构建存在风险：实施计划中先确认 crate 和构建方式。
- gix diff 与 Git CLI 结果存在差异：使用 fixture 记录差异，并以 Revier 稳定 JSON 契约为准。
- 索引 stale 导致结果不完整：返回 `stale` warning，提示重建索引或降级到现有路径。
- CLI 启动开销抵消小仓库收益：性能验证需单独记录 CLI 启动时间，必要时只在筛选复杂时启用 Rust 查询。
