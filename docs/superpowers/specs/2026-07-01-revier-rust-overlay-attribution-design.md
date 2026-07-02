# Revier Rust Overlay 与归因准确性迁移设计

> 状态：待审阅。本文记录 Rust overlay 与归因准确性迁移的设计，目标是在索引一期之后迁移 `file-overlay` 和 `trace-block`，不做全量 Rust 重写。

## 背景

Revier 当前 overlay 与归因链路主要由 TypeScript 实现：

- `src/analysis-core/overlay/fileOverlayService.ts` 构建文件 overlay。
- `src/main/analysis/overlayEngine.ts` 构建 side-by-side diff 行和变更块。
- `src/analysis-core/attribution/attributionService.ts` 使用 blame、merge trace 和 patch inference 给块归因。
- `src/main/ipc/reviewIpc.ts` 负责把任务文件、筛选条件和 Git 服务接到 overlay 构建流程。

这条链路已经能支持基础 overlay 和复杂 merge 归因提示，但仍有几个长期问题：

1. blame、patch、parent、commit 元数据读取依赖 Git 子进程，重复调用成本高。
2. merge、rename、删除块和行号漂移场景的准确性需要更多 fixture 固化。
3. TypeScript overlay 与未来 DuckDB 索引之间缺少统一的数据读取边界。
4. 文件列表索引一期完成后，overlay 如果继续完全绕开索引，会重复读取已经结构化保存的提交和文件触碰信息。

此前 gix capability spike 已验证 Rust 侧可以读取 blame、rename、pairwise merge diff 和多父父链。本规约定义第二条业务纵切：迁移 overlay 与块级归因命令，让 Electron 在打开文件和下钻提交时可以消费 Rust 输出的 `FileOverlay` 兼容 JSON。

## 目标

1. 在 Rust CLI 中提供 `file-overlay` 和 `trace-block` 命令。
2. 输出与现有 `FileOverlay`、`DiffBlock`、`RelatedCommit` 兼容的 JSON。
3. 使用 gix 构建文件 diff、blame、parent 查询和 pairwise merge diff。
4. 优先读取 DuckDB 索引中的提交、文件触碰和路径事件数据，缺失时按明确规则即时计算。
5. 对新增、修改、删除、rename、merge conflict 和多父 merge 场景给出稳定归因。
6. 保留 `precise`、`inferred`、`partial` 三种归因可信度语义。
7. Electron 主进程可按开关逐步从 TypeScript overlay 切换到 Rust overlay。

## 非目标

- 本阶段不迁移 Renderer、Pinia store 或 Vue diff 组件。
- 本阶段不改变用户可见 overlay 布局。
- 本阶段不改变 `ChangedFile`、`FileOverlay`、`DiffBlock` 的前端消费形态。
- 本阶段不实现完整后台服务或 N-API 绑定。
- 本阶段不做语义级函数、类或 AST 归因。
- 本阶段不要求移除 TypeScript overlay 实现；TypeScript 路径保留为降级和对照。

## 技术选型

### Rust CLI

沿用 `crates/revier-analysis`。Rust 侧负责：

- 读取 Git 对象和索引数据。
- 构建 side-by-side diff。
- 计算块级归因。
- 输出稳定 JSON。

Electron 主进程只做参数转换、子进程调用、错误处理和结果透传。

### gix

gix 负责生产 Git 读取：

- `readFileAtCommit` 等价能力由 blob 读取实现。
- `showFilePatch` 等价能力由 tree diff 与 hunk 构建实现。
- `blameFileRange` 等价能力由 gix blame 实现。
- merge commit 统一按每个 parent 执行 pairwise diff，不解析 combined diff。
- rename 和 rewrite tracking 使用 spike 中已验证的 gix diff 配置。

### DuckDB 索引

Rust overlay 可以读取索引一期产生的数据：

- `commits`
- `commit_parents`
- `commit_files`

本阶段可扩展索引表，但不能让 overlay 强依赖尚未构建的高级归因缓存。索引缺失时，`file-overlay` 默认即时计算并返回 warning；调用方传入 `--require-index` 时，索引不可用必须失败并返回退出码 `5`。

## 架构设计

### 总体结构

```text
Renderer
  -> review store
    -> preload IPC
      -> Electron 主进程 reviewIpc
        -> RustAnalysisClient
          -> revier-analysis file-overlay / trace-block
            -> overlay 模块
              -> attribution 模块
                -> git 模块
                -> index 查询模块
```

### Rust 模块划分

```text
crates/revier-analysis/src/
  commands/
    file_overlay.rs
    trace_block.rs
  overlay/
    mod.rs
    diff_builder.rs
    rows.rs
    blocks.rs
  attribution/
    mod.rs
    blame.rs
    merge_trace.rs
    patch_inference.rs
    deletion_trace.rs
  git/
    repository.rs
    diff.rs
    blame.rs
    commits.rs
  index/
    queries.rs
```

依赖方向：

```text
commands -> overlay -> attribution -> git
commands -> overlay -> index 查询
attribution 可以读取 index 查询结果
index 不依赖 overlay 或 attribution
```

### Electron 接入边界

Electron 新增或扩展 `RustAnalysisClient`：

- `getFileOverlay(request)` 调用 Rust `file-overlay`。
- `traceBlock(request)` 调用 Rust `trace-block`。
- 失败时按配置降级到现有 TypeScript `buildFileOverlayForTask()`。

第一阶段接入必须使用显式开关，例如主进程内部配置或环境变量。默认不开启 Rust overlay，避免未完成对照验证时改变用户可见行为。

## 数据流程

### 文件 overlay

1. Electron 收到 `reviewGetFileOverlay` IPC 请求。
2. 主进程找到任务对应的 project、range、filters 和 file。
3. 调用 Rust：

```bash
revier-analysis file-overlay --repo <repo> --base <base> --head <head> --branch <branch> --file <path> --format json
```

4. Rust 读取 base 与 head 的文件内容。
5. Rust 构建 side-by-side diff rows 和 blocks。
6. Rust 查询索引中的范围提交和文件触碰信息。
7. Rust 对每个 block 执行归因：
   - 新增和修改块优先 blame。
   - blame 指向 merge commit 时进入 merge trace。
   - 删除块使用 deletion trace 与 patch inference。
8. Rust 输出 `FileOverlay` 兼容 JSON。
9. Electron 将结果返回 Renderer，不改变组件消费方式。

### 单块追踪

`trace-block` 用于调试和未来详情面板：

1. 输入 task range、file path、block id 或显式行区间。
2. Rust 定位对应 block。
3. 返回该 block 的作者、相关提交、归因方法、merge 链路和 warning。

`trace-block` 可以复用 `file-overlay` 的 diff 和归因实现，但输出只聚焦单个块，便于 fixture 和人工排查。

### 提交下钻 overlay

提交下钻 overlay 可以复用 `file-overlay` 的 diff 构建能力：

1. 以目标 commit 的 first parent 作为 base。
2. 以目标 commit 作为 head。
3. file path 使用当前任务文件路径与 old path 候选。
4. block 作者默认是目标 commit 作者。
5. 后续如需对 merge commit 下钻做更精细归因，再单独扩展。

## JSON 契约

### `file-overlay`

```json
{
  "version": 1,
  "overlay": {
    "mode": "range",
    "file": {
      "path": "src/app.ts",
      "oldPath": null,
      "status": "modified",
      "additions": 10,
      "deletions": 2,
      "isBinary": false,
      "isPreviewable": true
    },
    "range": {
      "branch": "main",
      "baseCommit": "base",
      "headCommit": "head",
      "startAt": null,
      "endAt": null
    },
    "rows": [],
    "blocks": [],
    "warnings": []
  },
  "warnings": []
}
```

`overlay` 内部字段必须与 `src/shared/reviewTypes.ts` 的 `FileOverlay` 兼容。

### `trace-block`

```json
{
  "version": 1,
  "file": "src/app.ts",
  "blockId": "block-1",
  "attribution": {
    "confidence": "precise",
    "warnings": []
  },
  "authors": [
    {
      "name": "Alice",
      "email": "alice@example.com"
    }
  ],
  "relatedCommits": [
    {
      "hash": "commit",
      "shortHash": "commit",
      "authorName": "Alice",
      "authorEmail": "alice@example.com",
      "committedAt": "2026-07-01T00:00:00.000Z",
      "subject": "feat: example",
      "matchedByFilter": true,
      "touchedRanges": [],
      "attribution": {
        "method": "blame",
        "viaMergeHashes": []
      }
    }
  ],
  "warnings": []
}
```

### 退出码

- `0`：成功。
- `2`：参数错误。
- `3`：仓库读取失败。
- `4`：文件不可分析。
- `5`：索引不可用且命令要求必须使用索引。
- `6`：DuckDB 读写失败。
- `10`：分析内部错误。

错误信息写入 stderr，stdout 只写 JSON。

## 归因算法

### 新增和修改块

1. 对 block 新侧行区间执行 gix blame。
2. blame 返回非 merge commit 时，归因给该 commit。
3. blame 返回 merge commit 时，进入 merge trace。
4. merge trace 对 merge commit 与每个 parent 执行 pairwise diff。
5. 如果内容只来自某一条父链，继续沿该父链追踪。
6. 如果内容由 merge conflict resolution 首次写入，归因给 merge commit。
7. 如果多个父链都能解释同一内容，返回多个候选并标记 `partial`。

### 删除块

删除块没有 head 侧最终行，不能直接 blame：

1. 使用 base 侧行内容、旧侧行区间和路径生成删除指纹。
2. 在范围提交中查找删除或替换 hunk。
3. 非 merge 提交命中删除指纹时，归因给该提交。
4. merge 提交命中时，沿 parent 追踪删除行为来源。
5. 无法唯一确认时使用 patch inference，并标记 `inferred` 或 `partial`。

### Rename 与路径历史

路径候选集合包括：

- 当前 `file.path`。
- 当前 `file.oldPath`。
- 索引中的 `commit_files.old_path`。
- gix rewrite tracking 发现的历史路径。

路径历史不完整时必须添加 warning：

```text
文件历史路径不完整，结果可能不完整
```

### 可信度语义

- `precise`：blame、唯一 merge trace 或明确 deletion trace 得到结果。
- `inferred`：无法直接证明来源，结果来自 patch inference。
- `partial`：块内存在多条可能来源，或部分行精确、部分行推断。

UI 继续使用既有中文提示，不引入新的用户可见术语。

## 与索引一期的关系

本规约依赖索引一期提供的基础表，但不要求高级归因缓存：

- `commits` 用于提交元数据和作者信息。
- `commit_parents` 用于 parent 查询。
- `commit_files` 用于快速定位候选提交和路径。

如需新增以下表，应在实施计划中单独拆分步骤：

- `commit_file_ranges`
- `file_path_events`
- `blame_ranges`
- `block_attributions`

第一版 Rust overlay 可以在这些表缺失时即时计算，避免把 overlay 迁移阻塞在完整缓存体系上。

## CLI 设计

### `file-overlay`

```bash
revier-analysis file-overlay --repo E:/repo/app --base <hash> --head <hash> --branch main --file src/app.ts --format json
```

参数：

- `--repo`：本地 Git 仓库路径。
- `--db`：可选 DuckDB 路径。
- `--base`：分析基准提交。
- `--head`：分析目标提交。
- `--branch`：当前分支。
- `--file`：目标文件路径。
- `--glob`：可选文件规则，用于保持筛选上下文。
- `--author`：可选作者 key，可重复传入。
- `--author-query`：可选作者搜索。
- `--message`：可选提交信息搜索。
- `--require-index`：要求必须读取 DuckDB 索引；索引不可用时失败。
- `--format`：本阶段只支持 `json`。
- `--pretty`：格式化 JSON。

### `trace-block`

```bash
revier-analysis trace-block --repo E:/repo/app --base <hash> --head <hash> --file src/app.ts --block-id block-1 --format json
```

参数：

- `--block-id`：overlay block id。
- `--old-start`、`--old-end`、`--new-start`、`--new-end`：可选显式行区间。没有 block id 时用于调试。
- 其他参数与 `file-overlay` 保持一致。

## 开发环境配置

- Shell 使用 pwsh。
- Node 工具使用 `fnm` 和 `pnpm`。
- Rust 使用现有 Cargo workspace。
- 新增 Rust 依赖前必须在实施计划中列出并确认。
- fixture 测试可以使用 Git CLI 构造仓库；生产 Rust overlay 路径不得调用 `git` 进程。
- TypeScript overlay 保留，作为回归对照和降级路径。

建议新增脚本由实施计划确认：

```json
{
  "scripts": {
    "rust:file-overlay": "cargo run -p revier-analysis -- file-overlay",
    "rust:trace-block": "cargo run -p revier-analysis -- trace-block"
  }
}
```

## 测试策略

### Rust 单元测试

- side-by-side diff rows 构建。
- block 切分和行号映射。
- blame 结果归并。
- merge trace 唯一来源、多来源和 conflict resolution。
- deletion trace 和 patch inference。
- JSON 契约序列化。

### Git fixture 测试

沿用并扩展 gix spike fixture：

- 线性历史中的新增、修改、删除。
- feature 分支原始提交早于筛选时间，通过范围内 merge 进入 head。
- merge conflict 中由 merge commit 首次写入内容。
- 多父 merge 中不同 parent 包含相同内容，产生局部歧义。
- rename 后通过 merge 引入修改。
- 删除行为在 feature 分支发生，再通过 merge 进入目标分支。

每个 fixture 断言：

- block 数量和行区间。
- authors。
- related commits。
- attribution confidence。
- warning code。
- via merge hashes。

### TypeScript 对照测试

- 使用相同 fixture 跑 TypeScript overlay 和 Rust overlay。
- 对简单线性历史要求结构一致。
- 对复杂 merge 以 Rust 结果为目标口径，记录 TypeScript 差异。
- Electron 接入前必须验证 Rust 输出能被现有 `DiffViewer` 消费。

### Electron 回归测试

- Rust overlay 成功时 `reviewGetFileOverlay` 返回 Rust JSON。
- Rust overlay 失败时降级到 TypeScript overlay。
- 二进制或不可预览文件仍返回现有 warning 语义。
- 提交下钻 overlay 行为不回退。

## 验收标准

- Rust CLI 提供 `file-overlay` 和 `trace-block`。
- 生产 Rust overlay 路径不调用 `git` 进程。
- `file-overlay` 输出与现有 `FileOverlay` 兼容。
- `trace-block` 能单独输出块级归因链路。
- 复杂 merge fixture 中，最终新增或修改块优先显示原始非 merge 提交作者。
- 删除块能归因给执行删除的提交；无法确认时返回明确 `inferred` 或 `partial`。
- rename 后的路径追踪能覆盖当前路径、old path 和 gix rewrite path。
- Electron 可以按开关使用 Rust overlay，并保留 TypeScript 降级。
- Rust 测试、TypeScript 对照测试和 Electron 回归测试通过。

## 风险与处理

- gix blame 与 Git CLI 结果不完全一致：以 fixture 固化 Revier 口径，差异写入测试说明。
- 删除块天然不可直接 blame：使用 deletion trace，不伪装成精确结果。
- 多父 merge 可能存在多个合理来源：返回 `partial`，避免虚构唯一作者。
- Rust overlay 与现有 UI 契约偏离：先用 JSON 契约测试和 TypeScript 类型对照测试约束。
- 索引缺失导致 overlay 慢：允许即时计算，但必须输出 warning 和耗时指标。
- 全量迁移范围过大：本规约只迁移 `file-overlay` 和 `trace-block`，不重写 Renderer 或项目管理逻辑。
