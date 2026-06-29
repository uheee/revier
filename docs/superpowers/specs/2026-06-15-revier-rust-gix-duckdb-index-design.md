# Revier Rust CLI、gix 与 DuckDB 本地索引设计

## 背景

现有 Revier 分析链路已经开始把 Electron 无关逻辑抽到 `analysis-core`，并提供 TypeScript CLI 作为独立调试入口。随着多父 merge 归因、作者筛选、时间区间筛选、提交信息筛选和文件规则筛选叠加，当前基于 Git 子进程和逐文件逐提交扫描的实现会出现明显卡顿。

当前高成本路径主要来自两类操作：

1. 筛选文件列表时，需要判断“匹配提交是否触碰某个文件”，容易形成 `候选文件数 * 匹配提交数` 的重复 patch 读取和解析。
2. 打开文件 overlay 时，需要重复读取提交 patch、blame、父提交和提交元数据，任务级内存缓存无法跨分析任务复用。

本设计在原有“多父 Merge 链路归因与 CLI 分离设计”基础上，确认新的长期技术方向：使用 Rust CLI 承担本地分析和索引能力，使用 gix 作为首选 Git 读取层，使用 DuckDB 保存结构化索引。目标是在不调用 `git` 进程的前提下，为筛选、文件列表、行区间归因和后续复杂 merge 追踪提供稳定、可查询、可增量更新的本地数据基础。

## 目标

1. 使用 Rust CLI 作为新的本地分析与索引入口。
2. 首选 gix 读取本地 Git 仓库，不调用 `git` 进程。
3. 使用 DuckDB 保存提交、父链、文件变更、行区间、路径变更、blame 和归因缓存。
4. 将作者、时间、提交信息和文件规则筛选转换为 DuckDB 查询，避免重复 Git 遍历。
5. 在正式实现前完成 gix 能力 spike，验证 blame、rename、pairwise merge diff 和多父父链追踪是否满足 fixture 验收。
6. 保持 Electron 与 Rust CLI 之间的 JSON 契约稳定，便于后续逐步迁移 TypeScript 分析能力。

## 非目标

- 本阶段不直接重写现有 Electron UI。
- gix capability spike 不接入 UI，也不替换现有 TypeScript 分析链路。
- gix capability spike 不实现 libgit2 fallback，只记录是否需要 fallback 的风险结论。
- 本阶段不引入远程 Git 平台 API、PR 信息、认证或托管平台语义。
- 本阶段不保存完整仓库源码内容到 DuckDB，除非后续单独确认。
- 本阶段不实现语义级函数、类或 AST 归因。

## 术语

- Rust CLI：未来由 Electron 主进程调用的本地二进制，负责索引构建、查询和后续分析。
- gix：gitoxide 项目的 Rust Git 实现，本设计中作为首选 Git 读取库。
- DuckDB 索引：按仓库维护的本地结构化数据库，用于保存提交、文件、行区间和归因缓存。
- 冷索引：首次扫描仓库或索引不存在时构建的数据。
- 增量索引：已有索引后，只扫描新增提交或缺失数据。
- pairwise merge diff：对 merge commit 与每个 parent 分别做普通 tree diff，而不是解析 combined diff。
- capability spike：正式实现前的最小验证程序，用 fixture 验证 gix 是否覆盖关键 Git 语义。

## 技术选型

### Rust CLI

Rust 作为 CLI 主语言。理由：

- 本地 Git 对象遍历、diff、blame 和索引写入属于 CPU、I/O 与内存密集型工作，Rust 更容易控制资源开销。
- 强类型和枚举适合表达归因可信度、错误类别、索引状态和 JSON 契约。
- 单独二进制便于 Electron 主进程调用，也便于后续脱离 Electron 运行性能 fixture。
- 与未来 `clap` 风格命令、DuckDB native 绑定和 gix 生态匹配。

### gix

gix 作为首选 Git 读取层。使用边界：

- 读取提交元数据、父提交、tree、blob 和 commit graph。
- 使用 `gix-diff` 进行 tree diff、blob diff 和 rewrite tracking。
- 使用 `gix-blame` 进行行级 blame，并启用范围 blame 与 rewrite/rename tracking。
- 对 merge commit 不解析 combined diff，统一对每个 parent 执行 pairwise diff。

gix 风险：

- rename、copy 和相似度阈值结果可能与 Git CLI 不完全一致。
- blame 在复杂 rename、copy、merge 场景中需要 fixture 验证。
- 部分 porcelain 语义不是现成 API，需要在 Revier 归因层组合 plumbing 能力。

### DuckDB

DuckDB 作为本地索引存储。理由：

- 筛选查询天然是结构化多条件查询，适合使用 SQL 表达。
- 提交、文件、行区间和归因数据有明确表结构。
- 后续可以对大仓库做聚合分析和性能诊断。
- Electron 主进程无需持有大量内存数组即可查询索引结果。

DuckDB 使用边界：

- 保存元数据、路径、行区间、内容指纹和归因结果。
- 默认不保存完整文件内容。
- 同一仓库对应一个索引库，库内通过 schema version 管理升级。
- 查询缺失索引时返回明确状态，不在查询命令里隐式执行长时间构建。

## 架构设计

### 总体结构

```text
Electron 主进程
  -> 调用 Rust CLI JSON 命令
      -> index 模块
          -> DuckDB
      -> git 模块
          -> gix
      -> attribution 模块
          -> blame / merge trace / patch inference

Renderer
  -> 仍通过现有 IPC 消费文件列表、overlay、block 和 related commits
```

### Rust CLI 模块划分

```text
crates/revier-analysis/
  src/
    main.rs
    cli.rs
    error.rs
    json.rs
    git/
      mod.rs
      repository.rs
      commits.rs
      diff.rs
      blame.rs
      merge.rs
    index/
      mod.rs
      connection.rs
      schema.rs
      writer.rs
      queries.rs
      migrations.rs
    attribution/
      mod.rs
      block.rs
      merge_trace.rs
      patch_inference.rs
    commands/
      mod.rs
      index_build.rs
      index_status.rs
      query_files.rs
      query_commits.rs
      file_overlay.rs
      trace_block.rs
```

### 依赖方向

```text
commands -> attribution -> git
commands -> index
attribution -> index
index -> DuckDB
git -> gix
json -> shared contract
```

`git` 模块不依赖 DuckDB。`index` 模块不直接执行 Git 读取。`commands` 负责把 Git 读取结果写入索引，并把索引查询结果序列化为 JSON。

## 数据流程

### 索引构建流程

1. CLI 接收 `index build --repo <path>`。
2. 使用 gix 打开仓库并计算仓库唯一标识。
3. 读取当前索引 schema version 和已索引提交集合。
4. 遍历目标分支或目标 commit range 的提交。
5. 写入 `commits`、`commit_parents`。
6. 对每个提交读取 parent tree 和 commit tree，执行 pairwise tree diff。
7. 写入 `commit_files` 和 `commit_file_ranges`。
8. 对 rename/copy 结果写入 `file_path_events`。
9. 按需计算 blame 或归因缓存，写入 `blame_ranges`、`block_attributions`。
10. 写入 `index_runs` 和性能统计。

### 筛选查询流程

1. Electron 主进程调用 `index query-files`。
2. Rust CLI 使用 DuckDB 根据 branch、base、head、作者、时间、提交信息和 glob 条件查询候选提交。
3. 通过 `commit_files` 查询被候选提交触碰的文件。
4. 返回稳定 JSON 文件列表。
5. 如果索引不存在或 schema 不匹配，返回明确错误码和中文错误信息。

### 文件 overlay 流程

第一阶段保留现有 TypeScript overlay 能力。Rust CLI 只提供索引查询，减少筛选阶段卡顿。

后续阶段由 Rust CLI 提供 `file-overlay`：

1. 根据 `base` 和 `head` 读取旧版和新版文件 blob。
2. 构建 side-by-side diff。
3. 从索引读取相关提交和 touched ranges。
4. 对新增、修改块使用 gix blame 和 merge trace。
5. 对删除块使用 `commit_file_ranges` 与内容指纹推断。
6. 输出与现有 `FileOverlay` 兼容的 JSON。

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
- `repo_root`
- `repo_id`
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
  similarity real,
  primary key (commit_hash, parent_index, path, old_path)
);
```

### `commit_file_ranges`

```sql
create table commit_file_ranges (
  range_key text primary key,
  commit_hash text not null,
  parent_hash text not null,
  parent_index integer not null,
  path text not null,
  old_start integer,
  old_end integer,
  new_start integer,
  new_end integer,
  change_kind text not null,
  content_fingerprint text
);
```

### `file_path_events`

```sql
create table file_path_events (
  commit_hash text not null,
  parent_hash text not null,
  parent_index integer not null,
  old_path text not null,
  new_path text not null,
  event_kind text not null,
  similarity real,
  primary key (commit_hash, parent_index, new_path, old_path)
);
```

### `blame_ranges`

```sql
create table blame_ranges (
  head_commit text not null,
  path text not null,
  start_line integer not null,
  end_line integer not null,
  source_commit text not null,
  source_path text,
  source_start_line integer,
  source_end_line integer,
  method text not null,
  primary key (head_commit, path, start_line, end_line, source_commit)
);
```

### `block_attributions`

```sql
create table block_attributions (
  range_key text not null,
  path text not null,
  block_id text not null,
  confidence text not null,
  method text not null,
  related_commit_hash text not null,
  via_merge_hashes_json text not null,
  warnings_json text not null,
  primary key (range_key, path, block_id, related_commit_hash)
);
```

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

## CLI 设计

### 命令

```bash
revier-analysis index build --repo E:/repo/app --branch main --jobs 4 --format json
revier-analysis index status --repo E:/repo/app --format json
revier-analysis index query-files --repo E:/repo/app --base <hash> --head <hash> --author <key> --since <iso> --until <iso> --glob "src/**/*.ts" --format json
revier-analysis index query-commits --repo E:/repo/app --base <hash> --head <hash> --author <key> --since <iso> --until <iso> --message "fix" --format json
revier-analysis file-overlay --repo E:/repo/app --base <hash> --head <hash> --file src/app.ts --format json
revier-analysis trace-block --repo E:/repo/app --base <hash> --head <hash> --file src/app.ts --block-id block-1 --format json
```

### 通用参数

- `--repo`：本地 Git 仓库路径。
- `--db`：可选 DuckDB 文件路径。不传时使用默认 app data 目录。
- `--format`：输出格式，本阶段只支持 `json`。
- `--pretty`：格式化 JSON。
- `--jobs`：并发任务数，默认 4。

### 查询参数

- `--base`：分析基准提交。
- `--head`：分析目标提交。
- `--branch`：目标分支。
- `--author`：作者 key，可重复传入。
- `--since`：提交时间下限。
- `--until`：提交时间上限。
- `--message`：提交信息包含文本。
- `--glob`：文件规则，可重复传入。

### 退出码

- `0`：成功。
- `2`：参数错误。
- `3`：仓库读取失败。
- `4`：索引不存在或不可用。
- `5`：索引 schema 不兼容。
- `6`：DuckDB 读写失败。
- `10`：分析内部错误。

错误信息写入 stderr，JSON 结果只写 stdout。

## JSON 契约

### `index status`

```json
{
  "version": 1,
  "repoId": "...",
  "schemaVersion": 1,
  "status": "ready",
  "indexedCommitCount": 1200,
  "indexedFileCount": 8500,
  "updatedAt": "2026-06-15T00:00:00.000Z",
  "warnings": []
}
```

### `index query-files`

```json
{
  "version": 1,
  "range": {
    "baseCommit": "...",
    "headCommit": "..."
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

### `index query-commits`

```json
{
  "version": 1,
  "commits": [
    {
      "hash": "...",
      "shortHash": "...",
      "authorName": "Name",
      "authorEmail": "name@example.com",
      "committedAt": "2026-06-15T00:00:00.000Z",
      "subject": "feat: example"
    }
  ],
  "warnings": []
}
```

## gix Capability Spike

### 目标

在正式迁移前，用最小 Rust 程序验证 gix 是否满足 Revier 的关键 Git 语义。不接 UI，不写 DuckDB，不实现业务迁移。

### 验证项

1. `blame`：对比 gix blame 与 `git blame -M -C` 的行级来源提交。
2. `rename`：对比 gix rewrite tracking 与 `git diff --find-renames` 的路径变化。
3. `combined diff` 替代方案：确认 merge commit 可以通过 `merge parent -> merge result` 的 pairwise tree diff 覆盖需要的 hunk 信息。
4. 多父 merge：确认可以枚举全部 parent，并输出每个 parent 的 diff 结果。
5. rename + merge：确认 rename 后通过 merge 引入修改时，路径追踪和归因候选可恢复。

### fixture 场景

1. 线性历史中的新增、修改、删除。
2. feature 分支原始提交早于筛选时间，通过范围内 merge 进入 head。
3. merge conflict 中由 merge commit 首次写入内容。
4. 多父 merge 中不同 parent 包含相同内容，产生局部歧义。
5. rename 后通过 merge 引入修改。
6. 删除行为在 feature 分支发生，再通过 merge 进入目标分支。

### 输出

spike 程序输出 JSON：

```json
{
  "version": 1,
  "fixture": "rename-merge",
  "checks": [
    {
      "name": "gix-blame",
      "status": "pass",
      "differences": []
    }
  ],
  "decision": "gix-only"
}
```

`decision` 可取值：

- `gix-only`：gix 满足该 fixture。
- `algorithm-adjustment-required`：gix 能力足够，但 Revier 算法需调整。
- `fallback-risk`：gix 结果不稳定或缺失，需要后续确认是否引入 libgit2 fallback。

### 通过标准

- spike 不调用 `git` 进程。
- 所有 fixture 都能输出稳定 JSON。
- 输出能支撑现有 `RelatedCommit`、`DiffBlock` 和 `BlockAttributionSummary` 模型。
- 对 gix 与 Git CLI 不一致的行为有明确记录。
- 对每个不一致项给出接受、调整算法或 fallback 风险结论。

## 缓存与失效

### 索引位置

默认索引放在 Revier app data 目录下，按 `repo_id` 建库，不写入被分析仓库。

```text
<app-data>/revier/indexes/<repo-id>.duckdb
```

后续可以通过 `--db` 指定调试用数据库路径。

### repo_id

`repo_id` 由规范化 repo root、Git common dir 和必要仓库标识生成。具体算法在实现前确认，要求：

- 同一仓库路径稳定。
- 不同仓库不冲突。
- 不依赖当前分支名。

### schema version

DuckDB 中保存 `schema_version`。CLI 当前 schema 与数据库不一致时：

- 查询命令返回退出码 `5`。
- `index build` 可以执行迁移或要求重建，具体迁移策略在实现计划中确认。

### 增量更新

初始阶段按提交 hash 判断是否已索引。缺失提交才追加写入。遇到历史改写或 force push 时：

- 如果目标 range 中存在缺失提交，追加索引。
- 如果已索引提交对象无法在仓库中读取，标记索引为 `stale`。
- `stale` 状态下查询返回 warning，必要时提示用户重建索引。

## 并发策略

Rust CLI 默认 `--jobs 4`。并发边界：

- Git 对象读取和 diff 可以按提交或文件分片并发。
- DuckDB 写入通过批量事务合并，避免多个 writer 争用。
- 单个仓库同一时间只允许一个 `index build` 写入。
- 查询命令可以并发读取。

## 性能验收

正式实现前需要用真实或合成大仓库记录基线：

1. 未建索引时，当前 TypeScript 筛选耗时。
2. 冷索引构建耗时。
3. 增量索引构建耗时。
4. 已建索引后 `query-files` 耗时。
5. 打开单文件 overlay 的 Git 读取和归因耗时。

建议目标：

- 已建索引后，常规作者、时间、提交信息和 glob 组合筛选在 500ms 内返回文件列表。
- 大仓库冷索引允许较慢，但必须可显示进度并可取消。
- 增量索引应明显快于冷索引，只处理新增提交。

## 开发环境配置

### Node 与现有项目

继续遵守现有项目规范：

- Node 工具使用 `fnm` 和 `pnpm`。
- Electron 与 TypeScript 代码保持现有构建链路。

### Rust

新增 Rust 工具链前需要确认具体版本。建议：

- 使用 stable Rust。
- 使用 `cargo` 管理 Rust crate。
- 使用 `clap` 解析 CLI 参数。
- 使用 `serde` 和 `serde_json` 输出 JSON。
- 使用 gix 作为 Git 读取库。
- 使用 DuckDB Rust binding 访问索引库，具体 crate 在实现计划前单独确认。

## 测试策略

### Rust 单元测试

- CLI 参数解析。
- JSON 输出结构。
- DuckDB schema 初始化。
- 查询 SQL 构造。
- 错误码映射。

### Git fixture 测试

- 使用临时 Git 仓库构造 fixture。
- spike 阶段允许使用 Git CLI 构造 fixture 和生成对照数据，但被测 gix 读取路径不得调用 `git` 进程。
- 每个 fixture 输出 JSON 快照。

### 集成测试

- `index build` 后执行 `index status`。
- `query-files` 验证作者、时间、提交信息和 glob 组合筛选。
- schema version 不匹配时返回稳定错误。
- 索引不存在时返回稳定错误。

### Electron 回归测试

第一阶段只在 Electron 主进程接入 `query-files` 后验证：

- 筛选文件列表仍能正确显示。
- 索引缺失时 UI 能提示需要构建索引。
- 已建索引后打开文件 overlay 仍走现有逻辑，不改变现有展示行为。

## 迁移步骤

1. 编写并审核本设计文档。
2. 编写 gix capability spike 实施计划。
3. 创建 Rust spike 工程或 crate。
4. 构造 Git fixture。
5. 实现 gix blame 验证。
6. 实现 gix rename 验证。
7. 实现 pairwise merge diff 验证。
8. 实现多父 merge 链路验证。
9. 输出 spike 结论。
10. 根据结论确认是否继续 gix-only。
11. 设计 DuckDB schema 与 Rust CLI 正式实现计划。
12. 分阶段接入 Electron 主进程。

## 风险与处理

- gix blame 与 Git CLI 结果不完全一致：用 fixture 明确差异，判断是否调整 Revier 归因口径。
- rename 相似度结果不一致：记录相似度阈值、limit 和 copy tracking 配置，必要时把结果标记为推断。
- DuckDB schema 过早固化：通过 schema version 和迁移机制控制。
- 冷索引耗时长：提供进度、取消和增量索引。
- 索引体积过大：默认不保存完整源码内容，仅保存元数据、行区间和指纹。
- Electron 调用 Rust CLI 失败：保留明确错误码和中文错误信息，UI 给出可恢复提示。

## 待确认事项

1. DuckDB Rust binding 具体 crate。
2. Rust toolchain 版本。
3. 默认索引目录的跨平台实现细节。
4. `repo_id` 生成规则。
5. 冷索引是否在用户点击分析时自动触发，还是必须由用户显式构建。
6. 第一阶段 Electron 是否只接入 `query-files`，不接入 `file-overlay`。
7. gix spike 失败时，是否允许引入 `git2-rs/libgit2` fallback。
