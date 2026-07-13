# Revier 范围最终变更文件行数统计设计

> 状态：已确认并实施。本文记录“变更文件”列表中 `+/-` 统计按分析范围最终变化计算的设计。

## 背景

当前 Review 工作台左侧“变更文件”列表会展示每个文件的状态与行数统计，例如 `+10 -2`。实际运行中会出现文件确实有 diff，但列表显示 `+0 -0` 的情况。

排查后确认，前端列表只是展示 `ChangedFile.additions` 和 `ChangedFile.deletions`。这两个字段来自 Rust 索引查询链路，但 `crates/revier-analysis/src/git/diff.rs` 在构造 `CommitFileChange` 时目前将 `additions` 和 `deletions` 固定写为 0。因此问题不是 UI 渲染错误，而是分析数据源没有计算行级统计。

本设计确认新的统计口径为：**变更文件列表显示分析范围 `baseCommit -> headCommit` 的最终 tree diff 行数统计**。

## 目标

1. “变更文件”列表中的 `+/-` 按 `baseCommit -> headCommit` 最终差异计算。
2. 文件列表仍只展示筛选范围内存在最终净变化的文件。
3. 作者、作者搜索、提交信息、时间和 glob 筛选语义保持现有行为。
4. 不把范围内每个提交的行数累加到列表统计。
5. 不修改 overlay、提交级下钻、归因展示或 UI 布局。

## 非目标

1. 不新增 UI 控件或统计口径切换入口。
2. 不把二进制文件解析为文本 diff。
3. 不改变 `ChangedFile` 的前后端字段结构。
4. 不改变 DuckDB schema 中 `commit_files.additions`、`commit_files.deletions` 的字段定义。
5. 不重新设计索引构建策略或索引存储位置。

## 统计口径

### 范围最终变化

对当前分析范围只计算一次最终 diff：

```text
baseCommit tree -> headCommit tree
```

列表中的每个文件统计值来自这次最终 diff，而不是来自范围内提交的逐提交 diff 累加。

示例：

```text
base: A
commit 1: A -> B    +10 -0
commit 2: B -> A    +0 -10
head: A
```

最终变化为空，该文件不应出现在“变更文件”列表中。

### 筛选条件与最终变化取交集

作者、作者搜索、提交信息和时间筛选仍用于找出范围内命中的提交；glob 仍用于筛选路径。最终列表取两类结果的交集：

```text
命中提交触达过的路径
  ∩
baseCommit -> headCommit 的最终变更路径
```

这样可以保留“只看某些提交触达过的最终变更文件”的语义，同时过滤掉最终已经还原的文件。

### 行数统计来源

对文本文件：

1. 根据最终变更记录确定旧路径和新路径。
2. 从 `baseCommit` 读取旧内容。
3. 从 `headCommit` 读取新内容。
4. 使用现有行级 diff 能力统计新增行和删除行。
5. 写入 `ChangedFile.additions` 和 `ChangedFile.deletions`。

对无法作为文本预览的文件：

```text
additions = 0
deletions = 0
status = binary
isBinary = true
isPreviewable = false
```

## 边界规则

| 场景 | 列表状态 | additions | deletions |
| --- | --- | ---: | ---: |
| 新增文本文件 | `added` | 新文件行数 | 0 |
| 删除文本文件 | `deleted` | 0 | 旧文件行数 |
| 修改文本文件 | `modified` | 最终 diff 新增行数 | 最终 diff 删除行数 |
| 纯重命名且内容不变 | `renamed` | 0 | 0 |
| 重命名且内容变化 | `renamed` | 最终内容 diff 新增行数 | 最终内容 diff 删除行数 |
| 二进制新增、删除、修改、重命名 | `binary` | 0 | 0 |
| 范围内修改后完全还原 | 不展示 | 不适用 | 不适用 |

## 架构设计

### 当前链路

```text
ReviewService.run_analysis
  -> revier_analysis::api::query_files_with_context
    -> index::queries::query_files_with_context
      -> commit_files.additions/deletions
        -> git::diff::file_change 固定 0
```

### 调整后链路

```text
ReviewService.run_analysis
  -> revier_analysis::api::query_files_with_context
    -> commands::query_files::query_request_with_context
      -> index::queries::query_files_for_commits_with_context
        -> 读取筛选命中提交触达路径
      -> git::diff::range_file_changes(base, head)
        -> 计算最终 tree diff 和行数统计
      -> 命令层执行路径交集并应用 glob
      -> 返回带最终行数统计的 ChangedFileOutput
```

### 模块职责

#### `git::diff`

新增范围级最终 diff API，职责：

- 接收 `baseCommit`、`headCommit`。
- 使用 gix 对两个 tree 执行 diff 和 rename 检测。
- 为每个最终变更文件计算状态、旧路径、新路径、二进制标记、预览标记、相似度。
- 对文本文件读取 base/head blob 并计算行数统计。

建议接口形态：

```rust
pub fn range_file_changes(
    repo: &gix::Repository,
    base_commit: &str,
    head_commit: &str,
) -> Result<Vec<CommitFileChange>, AppError>
```

返回类型继续复用 `CommitFileChange`，避免新增一套几乎相同的文件变更结构。

#### `commands::query_files`

命令层职责：

- 打开仓库和索引。
- 获取 `baseCommit -> headCommit` 范围内的提交哈希。
- 调用索引查询得到筛选命中提交触达过的路径。
- 调用 `git::diff::range_file_changes()` 得到最终差异。
- 执行触达路径交集、glob 过滤、路径排序和结果转换。

#### `index::queries`

查询层职责：

- 保持 `matching_commits()` 的筛选逻辑。
- 从 `commit_files` 读取命中提交触达过的路径集合。
- 将查询限制在 `baseCommit -> headCommit` 的提交哈希集合内。
- 提供同时匹配新旧路径的 glob 判断。

`query_files_with_context()` 仍返回 `ChangedFileOutput`，不改变 JSON 契约。

#### `ReviewService`

Tauri 服务层只适配结果，不新增统计逻辑。`adapt_changed_file()` 保持字段映射。

#### 前端

`ChangedFileList.vue` 不需要改动。它继续展示 `file.additions` 和 `file.deletions`。

## 数据流程

1. 用户在 Review 工作台启动分析。
2. Tauri 解析分析范围，得到 `baseCommit` 和 `headCommit`。
3. 索引准备完成后进入 `query_request_with_context()`。
4. 命令层计算范围提交哈希，查询层按作者、提交信息、时间筛选得到命中提交。
5. 查询层从 `commit_files` 得到命中提交触达路径集合。
6. 命令层调用 `range_file_changes()` 得到最终变更文件及行数统计。
7. 命令层对最终变更结果执行路径交集和 glob 过滤。
8. Tauri 将结果缓存到 `filesByTask`。
9. 前端读取列表并显示正确 `+/-`。

## 接口规范

### Rust 内部接口

新增：

```rust
pub fn range_file_changes(
    repo: &gix::Repository,
    base_commit: &str,
    head_commit: &str,
) -> Result<Vec<CommitFileChange>, AppError>
```

复用：

```rust
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
```

范围级结果中：

- `commit_hash` 填 `headCommit`。
- `parent_hash` 填 `baseCommit`。
- `parent_index` 填 `0`。

### JSON 契约

不变：

```json
{
  "path": "src/app.rs",
  "oldPath": null,
  "status": "modified",
  "additions": 10,
  "deletions": 2,
  "isBinary": false,
  "isPreviewable": true
}
```

### 前端类型

不变，继续使用 `src/renderer/generated/bindings.ts` 中的 `ChangedFile`。

## 开发环境配置

- Shell 使用 `pwsh`。
- Rust 测试使用 Cargo workspace 现有配置。
- 前端类型生成或组件验证如需运行，使用 `fnm` 和 `pnpm`。
- 本改动不新增 Rust、Node 或 Python 依赖。
- Python 工具不参与本任务；如后续需要必须使用 `uv`。

## 测试策略

### Rust diff 单元或集成测试

新增覆盖：

1. 文本文件新增时，范围最终统计为新增行数。
2. 文本文件删除时，范围最终统计为删除行数。
3. 文本文件修改时，范围最终统计为新增行和删除行。
4. 纯重命名内容不变时，状态为 `renamed` 且 `+0 -0`。
5. 重命名并修改时，状态为 `renamed` 且统计内容变化。
6. 二进制文件保持 `binary` 和 `+0 -0`。

### Rust 查询测试

新增覆盖：

1. 范围内修改后还原的文件不出现在 `query-files` 结果中。
2. 命中提交触达路径与最终 diff 取交集。
3. glob 同时匹配新路径和 rename 旧路径。
4. `query-files` 输出 JSON 中 `additions/deletions` 不再固定为 0。

### Tauri 服务测试

现有 `start_analysis_builds_index_and_completes_when_index_missing` 可扩展断言：

- 返回文件包含正确 `additions/deletions`。

如测试夹具不足，新增一个最小线性仓库测试，避免改动复杂流程。

### 前端测试

前端列表只是展示字段，不需要为统计逻辑新增组件测试。若实施中修改了列表渲染，必须补充对应 Vitest 测试。

## 验收标准

1. 启动分析后，“变更文件”列表中有文本 diff 的文件不再因为占位字段显示 `+0 -0`。
2. 行数统计符合 `baseCommit -> headCommit` 最终变化。
3. 范围内修改后还原的文件不显示在列表中。
4. 纯重命名且内容不变的文件允许显示 `+0 -0`。
5. 二进制文件继续显示 `+0 -0`，并保持不可预览语义。
6. `ChangedFile` 前后端接口不变。
7. `index_git_diff`、`index_query_files` 和 Tauri Review 服务中覆盖本口径的 Rust 测试通过。

## 风险与处理

| 风险 | 影响 | 处理 |
| --- | --- | --- |
| gix 行级统计与 Git CLI `--numstat` 存在细微差异 | 用户可能看到与 Git 命令不同的数字 | 以 Revier 自身 overlay diff 口径为准，并用 fixture 固化结果。 |
| 查询层同时依赖索引和实时范围 diff | 查询成本增加 | 只对 `base -> head` 计算一次最终 diff，不按提交重复计算。 |
| rename 路径交集规则遗漏旧路径 | 作者筛选下重命名文件可能丢失 | 交集匹配时同时检查 `path` 和 `old_path`。 |
| 二进制文件无法给出真实行数 | 列表仍可能出现 `+0 -0` | 保持状态为 `binary`，用状态解释统计不可用。 |
