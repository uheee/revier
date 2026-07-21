# Revier 性能、持久化缓存与进度状态设计

## 1. 背景

当前 Review 工作台存在三类相互关联的性能问题：

1. 项目分析会遍历完整提交历史；索引缺失最新提交时会全量重建；文件统计会对范围内全部变更文件读取内容并计算行级差异。
2. 点击单个文件时，后端会重新计算整个分析范围的文件差异，再从结果中查找目标文件；作者归因开始时又重复加载同一范围文件。
3. 作者归因按范围提交逐个扫描，按变更块分别执行 blame，删除块还会重复扫描历史；结果只保存在当前页面内存中，重新打开项目、文件或提交下钻时无法复用。

此外，当前任务状态只有粗粒度阶段，没有稳定的当前项目、完成数量、百分比和真实耗时。用户无法判断程序正在处理什么，也无法区分缓存读取、Git 计算和界面 Diff 计算。

## 2. 已确认需求

### 2.1 持久化与恢复

- 打开项目时优先加载 DuckDB 中最近一次成功分析，不自动重新分析。
- 每个项目的每个分支只保留一次最近成功的项目分析。
- 不同分支分别保存，切换回已有缓存的分支时直接恢复。
- 仓库分支 HEAD 已变化时，仍允许展示旧缓存，但必须明确标记“数据可能已过期”。
- 旧缓存只有在新的项目分析完整成功后才被替换；失败或取消不得覆盖旧缓存。
- 文件 Diff、作者轨、相关提交和已经打开过的提交下钻需要持久化，后续优先读取缓存。

### 2.2 手动刷新

- 提供“分析项目／重新分析项目”按钮。
- 提供“重新分析当前文件”按钮。
- 项目刷新更新分析范围、基础索引、筛选结果和文件列表。
- 当前文件刷新立即重算文件 Diff、作者轨和相关提交，并清除该文件已有的提交下钻缓存。
- 提交下钻不在项目或文件刷新时全部预计算；用户再次点击具体提交时按需计算并缓存。
- 项目缓存已过期时，当前文件刷新不得把新 HEAD 的单文件结果写入旧项目范围；用户必须先重新分析项目。

### 2.3 状态栏与耗时

- 任意用户可见加载过程都持续显示当前操作、操作对象、进度和已用时间。
- 能准确确定总量的阶段显示“已完成／总量”和百分比。
- 无法准确确定总量的阶段只显示当前操作和已用时间，不显示虚假百分比。
- 完成、失败或取消后显示本次总耗时。
- 最后一条完成信息保留到下一项操作开始。
- 缓存命中同样显示来源和读取耗时。

## 3. 目标与非目标

### 3.1 目标

- 项目二次打开不触发隐式全量分析。
- 相同分支、相同分析快照下再次打开文件时，不重新执行范围级文件扫描和历史归因。
- 已缓存的提交下钻可直接恢复派生数据，仅从 Git 对象库读取必要的不可变文件内容。
- 手动刷新具有原子替换、取消保护和明确的缓存失效规则。
- 将全量索引改为增量索引，并通过事务批量写入。
- 为分析、缓存读取、文件加载、Monaco Diff、归因、编码重载和提交下钻提供统一进度模型。
- 记录包含数据库写入在内的真实端到端耗时。

### 3.2 非目标

- 不增加自动刷新、定时刷新或后台静默分析。
- 不保存工作区未提交修改；分析范围仍以 Git 提交为准。
- 不预计算所有文件的作者归因或所有相关提交的下钻。
- 不跨项目共享文件归因缓存。
- 不在数据库中重复保存完整源码正文。
- 不增加缓存管理或手动清理界面。

## 4. 总体架构

系统继续采用 Vue 3 渲染层、Tauri 命令层、Rust 分析库和每仓库 DuckDB 数据库。新增职责如下：

```text
Vue ReviewWorkspace
  ├─ FilterPanel：分析项目／重新分析项目
  ├─ ReviewStore：缓存恢复、请求代次、当前操作状态
  ├─ DiffViewer：文件缓存与 Monaco Diff
  └─ OperationStatusBar：操作、对象、进度、耗时、缓存状态
             │
             │ Tauri command + review://operation-progress
             ▼
Tauri ReviewService
  ├─ AnalysisCoordinator：项目分析与原子发布
  ├─ FileAnalysisCoordinator：文件缓存、归因缓存、下钻缓存
  ├─ OperationReporter：限频进度事件
  └─ CacheRepository：DuckDB 快照读写
             │
             ▼
revier-analysis
  ├─ 增量提交索引
  ├─ 范围文件差异一次计算
  ├─ 路径候选提交查询
  ├─ 批量 blame／删除追踪
  └─ Git blob ID 直接读取
```

项目分析快照负责确定不可变的 `base_commit`、`head_commit`、筛选条件和文件列表。文件缓存与下钻缓存必须从属于该快照或不可变提交对，禁止把不同范围的数据拼接在一起。

## 5. 技术选型

### 5.1 DuckDB 持久化

继续使用现有 DuckDB，原因如下：

- 已经按仓库维护数据库和提交索引，不需要引入第二套持久化引擎。
- 提交、文件、块、作者与相关提交之间是结构化关系，适合规范化表和关联查询。
- DuckDB 自带列压缩；通过整数序号、提交哈希外键和关联表可避免 JSON 中重复的字段名、作者信息和提交信息。
- 项目刷新可以在单个事务中写入候选快照并切换当前快照。

### 5.2 不保存完整源码

数据库只保存 Git blob ID、路径和已解析编码，不保存 `old_content`、`new_content` 的重复副本。文件打开时通过 blob ID 直接从本地 Git 对象库读取不可变内容。这样既能避免范围级扫描，也不会让数据库复制仓库源码。

### 5.3 规范化数据而非整包 JSON

- 提交元数据复用现有 `commits` 表。
- Diff 块使用文件分析 ID 与小整数序号定位。
- 块与提交、块与触达范围使用关联表。
- 作者摘要由块关联提交聚合得到，不重复保存每个块的作者姓名和邮箱。
- 警告代码和归因方法使用受控短值，不保存重复的长描述；界面根据代码映射中文文案。

本期不新增 zstd、MessagePack 或其他压缩依赖。

## 6. 缓存模型

### 6.1 缓存层级

| 层级 | 唯一键 | 保存内容 | 替换规则 |
| --- | --- | --- | --- |
| 项目分支快照 | `repo_id + branch` | 范围、筛选、文件列表、状态、耗时 | 每分支仅保留一次成功结果 |
| 文件分析 | `analysis_id + path` | blob ID、编码、块、归因、相关提交 | 当前文件刷新或编码重载成功后替换 |
| 提交下钻 | `file_analysis_id + commit_hash` | 父子 blob ID、历史路径、块 | 点击时按需写入；文件刷新时删除 |
| 提交基础索引 | `commit_hash` | 提交、父关系、提交文件 | 增量追加；项目刷新时清理不可达项 |

### 6.2 项目分支快照

同一仓库的每个本地分支最多存在一个已发布快照。快照包含：

- 分支名、base/head 哈希；
- 开始与结束时间；
- 作者、作者搜索、提交信息和 glob 筛选；
- 筛选指纹；
- 分析算法版本；
- 文件列表及最终 `+/-` 统计；
- 创建时间、完成时间和端到端耗时；
- 缓存 HEAD 与当前分支 HEAD 是否一致。

同一分支使用新的筛选条件重新分析成功后，替换该分支原快照。旧快照在事务提交前始终可读。

### 6.3 文件分析缓存

每个项目快照中的文件只保留一个当前编码版本。编码切换成功后替换该文件原缓存，以控制空间。

文件分析缓存包含：

- 当前路径、旧路径、状态；
- base/head 两端 blob ID；
- 具体解析编码；
- Monaco 块签名和块坐标；
- 每个块的置信度、警告代码、相关提交与触达范围；
- 文件计算与归因耗时；
- 分析算法版本。

首次打开文件时直接使用项目快照中的文件记录定位 blob，不再调用全范围 `range_file_changes()`。Monaco 产生规范块签名后，若数据库签名一致，直接合并归因；不一致时重新归因并替换文件缓存。

### 6.4 提交下钻缓存

下钻缓存只保存已经点击过的提交。缓存包含：

- 提交哈希、第一父提交哈希；
- 当前文件在该历史提交中的解析路径；
- 两端 blob ID和编码；
- 下钻块坐标和固定提交元数据；
- 计算耗时与算法版本。

下钻源码仍从 Git blob 读取。因为提交和 blob 不可变，同一个文件分析未刷新时可稳定复用。

### 6.5 缓存有效性

缓存有效必须同时满足：

- 数据库 schema 版本兼容；
- 分析算法版本一致；
- 项目仓库身份一致；
- 文件缓存属于当前已加载的 `analysis_id`；
- 文件 blob ID 与项目快照记录一致；
- 具体编码一致；
- Monaco 块签名一致；
- 下钻提交与父提交哈希一致。

分支 HEAD 变化只把项目快照标记为过期，不自动删除或计算。算法版本不一致时，项目文件列表仍可在契约兼容时展示，但文件归因和下钻必须按未命中处理；具体兼容范围由 migration 明确，不能静默猜测。

## 7. 数据库设计

### 7.1 现有表调整

保留 `metadata`、`commits`、`commit_parents`、`commit_files` 和 `index_runs`。索引写入从“清空后逐行重写”改为：

1. 查询可达但未索引的提交哈希；
2. 只计算缺失提交及其父差异；
3. 在事务中批量写入；
4. 显式刷新索引运行记录；
5. 在手动项目刷新成功后，删除不再被任何本地分支或当前分支快照引用的提交数据；
6. 完成大批量替换后执行数据库检查点。

### 7.2 新增表

以下为逻辑结构，字段类型在实施时沿用 DuckDB 的 `TEXT`、`BIGINT`、`INTEGER`、`BOOLEAN`、`TIMESTAMP` 和数组类型。

#### `analysis_snapshots`

```text
analysis_id              text primary key
repo_id                  text not null
branch                   text not null
base_commit              text not null
head_commit              text not null
start_at                 timestamp
end_at                   timestamp
author_query             text
message_query            text
filter_fingerprint       text not null
analysis_version         integer not null
started_at               timestamp not null
completed_at             timestamp not null
elapsed_ms               bigint not null
last_selected_path       text
unique (repo_id, branch)
```

#### `analysis_filter_authors`

```text
analysis_id              text not null
ordinal                  integer not null
author_key               text not null
primary key (analysis_id, ordinal)
```

#### `analysis_filter_globs`

```text
analysis_id              text not null
ordinal                  integer not null
glob                     text not null
primary key (analysis_id, ordinal)
```

#### `analysis_files`

```text
analysis_id              text not null
path                     text not null
old_path                 text
status                   smallint not null
additions                bigint not null
deletions                bigint not null
is_binary                boolean not null
is_previewable           boolean not null
old_blob_id              text
new_blob_id              text
primary key (analysis_id, path)
```

#### `file_analyses`

```text
file_analysis_id         text primary key
analysis_id              text not null
path                     text not null
resolved_encoding        smallint not null
block_signature          text not null
analysis_version         integer not null
content_elapsed_ms       bigint not null
attribution_elapsed_ms   bigint not null
completed_at             timestamp not null
unique (analysis_id, path)
```

#### `file_blocks`

```text
file_analysis_id         text not null
block_ordinal            integer not null
old_start                integer not null
old_end                  integer not null
new_start                integer not null
new_end                  integer not null
change_type              smallint not null
confidence               smallint
warning_flags            integer not null
primary key (file_analysis_id, block_ordinal)
```

#### `file_block_commits`

```text
file_analysis_id         text not null
block_ordinal            integer not null
commit_hash              text not null
matched_by_filter        boolean not null
attribution_method       smallint
primary key (file_analysis_id, block_ordinal, commit_hash)
```

#### `file_block_commit_ranges`

```text
file_analysis_id         text not null
block_ordinal            integer not null
commit_hash              text not null
range_ordinal            integer not null
old_start                integer
old_end                  integer
new_start                integer
new_end                  integer
primary key (file_analysis_id, block_ordinal, commit_hash, range_ordinal)
```

#### `file_block_merge_sources`

```text
file_analysis_id         text not null
block_ordinal            integer not null
commit_hash              text not null
merge_ordinal            integer not null
merge_hash               text not null
primary key (file_analysis_id, block_ordinal, commit_hash, merge_ordinal)
```

#### `commit_overlays`

```text
commit_overlay_id        text primary key
file_analysis_id         text not null
commit_hash              text not null
parent_hash              text not null
historical_path          text not null
old_blob_id              text
new_blob_id              text
resolved_encoding        smallint not null
analysis_version         integer not null
elapsed_ms               bigint not null
completed_at             timestamp not null
unique (file_analysis_id, commit_hash)
```

#### `commit_overlay_blocks`

```text
commit_overlay_id        text not null
block_ordinal            integer not null
old_start                integer not null
old_end                  integer not null
new_start                integer not null
new_end                  integer not null
change_type              smallint not null
primary key (commit_overlay_id, block_ordinal)
```

### 7.3 原子发布

项目刷新使用候选 `analysis_id`：

1. 在内存和临时事务数据中完成范围解析、索引更新和文件列表计算。
2. 开启事务，写入候选快照和文件。
3. 删除同一 `repo_id + branch` 的旧快照及其文件、文件分析和下钻关联数据。
4. 提交事务后，候选快照成为唯一当前快照。

任何错误或取消都回滚事务。文件刷新采用相同模式：先完成计算，再在事务中替换该文件的 `file_analyses`、块和下钻数据。

## 8. 性能改造

### 8.1 分析范围

- 保留 Git 图正确性，不按提交时间盲目提前终止历史遍历。
- 将范围提交哈希在单次项目分析上下文中计算一次并复用。
- DuckDB 范围查询使用批量关系或临时表，不对每个提交分别执行 `commit_exists`。

### 8.2 增量索引

- 按提交哈希判断缺失项。
- 只对缺失提交执行 `commit_file_changes()`。
- 使用 DuckDB 事务和批量 prepared statement；不在每次更新时清空全部表。
- `elapsed_ms` 在数据库提交与检查点完成后采集，表示真实端到端耗时。

### 8.3 文件列表

将范围树差异和行数统计分离：

1. 先计算 base/head 的文件级树差异，不读取所有文件正文。
2. 用索引筛选命中路径并执行 glob 过滤。
3. 只对最终保留的文件读取两端 blob 并计算 `+/-`。
4. 把文件状态、统计和 blob ID写入 `analysis_files`。

### 8.4 文件打开

- 从 `analysis_files` 直接取得目标记录。
- 按 blob ID 读取两端内容。
- 不再调用 `changed_file_between()` 或 `range_file_changes()`。
- 缓存命中时直接读取文件块、归因和相关提交。
- 文件缓存未命中时只计算目标文件。

### 8.5 作者归因

- 通过 `commit_files`、范围提交集合和路径历史筛选真正触达目标文件的候选提交。
- 批量读取候选提交元数据，避免每提交一次 SQL。
- 每个候选提交的文件差异只计算一次，并在当前文件操作中复用。
- 合并 Monaco 块的新侧范围，以一次或最少次数调用 blame；所有调用共享 Git diff 资源缓存。
- 删除追踪按文件执行一次历史扫描，同时处理全部删除子范围，不再按父块重复扫描。
- merge trace 在同一文件操作中按提交与 blob 指纹记忆化。

## 9. 进度与状态模型

### 9.1 统一契约

新增绑定契约：

```rust
pub struct OperationProgressSnapshot {
    pub operation_id: String,
    pub kind: OperationKind,
    pub status: OperationStatus,
    pub project_id: String,
    pub branch: Option<String>,
    pub file_path: Option<String>,
    pub commit_hash: Option<String>,
    pub stage: OperationStage,
    pub message: String,
    pub completed_units: Option<u64>,
    pub total_units: Option<u64>,
    pub progress: Option<f64>,
    pub started_at: String,
    pub elapsed_ms: u64,
    pub cache_state: CacheState,
}
```

`OperationKind` 至少包含：

- `project-metadata`
- `project-analysis`
- `file-overlay`
- `monaco-diff`
- `file-attribution`
- `encoding-reload`
- `commit-overlay`

`OperationStatus` 包含 `running`、`completed`、`failed`、`cancelled`。

`CacheState` 包含 `none`、`hit`、`miss`、`stale`、`refresh`。

### 9.2 事件

- 统一事件名：`review://operation-progress`。
- 后端操作在开始、阶段变化、限频进度、终态时发送事件。
- 同一操作的普通进度事件最高每秒 10 次；阶段变化和终态不受限频影响。
- 前端按 `operation_id` 和请求代次拒绝过期事件。
- 前端本地计时器在运行期间刷新 `elapsed_ms` 展示，不要求后端每 100 毫秒发送事件。

### 9.3 可确定进度

| 阶段 | 单位 | 是否显示百分比 |
| --- | --- | --- |
| 缺失提交索引 | 提交 | 是 |
| 索引文件写入 | 文件记录 | 是 |
| 文件统计 | 文件 | 是 |
| 候选提交归因 | 提交 | 是 |
| blame 范围 | 合并范围 | 是 |
| 删除追踪 | 候选提交 | 是 |
| 缓存恢复 | 表／记录批次 | 是 |
| 仓库发现、数据库打开、Monaco 初始化 | 无稳定总量 | 否 |

所有百分比限制在 `0.0..=1.0`。开始时未知总量不得伪造 `0%`；终态完成固定为 `100%`。

## 10. 前端交互

### 10.1 打开项目

1. 加载项目配置和分支列表。
2. 根据保存的当前分支读取该分支最新成功快照。
3. 有缓存时恢复快照保存的筛选条件、文件列表和上次选中文件；没有缓存时保持空工作台并显示“分析项目”。
4. 快速解析当前分支 HEAD，与缓存 head 比较。
5. 不一致时显示“缓存可能已过期”，但不清空旧内容，也不自动分析。

### 10.2 项目刷新按钮

- 无缓存时文案为“分析项目”。
- 有缓存时文案为“重新分析项目”。
- 使用当前筛选表单和分支创建新项目分析。
- 分析期间禁止再次提交同一项目刷新；保留取消入口。
- 成功后切换新快照并关闭旧文件与下钻上下文。

### 10.3 当前文件刷新按钮

- 放在文件编辑器状态栏的操作区域。
- 没有选中文件、文件不可预览或项目缓存已过期时禁用。
- 点击后强制重新读取当前快照的两个 blob、重新计算 Monaco 块、作者归因和相关提交。
- 成功后替换文件缓存并清除该文件全部下钻缓存。

### 10.4 状态栏布局

现有 `EditorStatusBar` 保留行列、编码和语言控件，新增可伸缩操作区域：

```text
[当前操作 · 对象] [进度条 128/420 30%] [已用 3.2 秒] [重新分析当前文件] [Ln/Col 编码 语言]
```

没有打开文件时仍渲染状态栏，但隐藏编辑器专属控件。完成信息保留到下一项操作开始。

示例：

- `正在更新索引 · develop · 128/420 个提交 · 30% · 已用 3.2 秒`
- `正在归因 · src/main.rs · 8/24 个候选提交 · 33% · 已用 1.6 秒`
- `已从缓存加载 · src/main.rs · 本次耗时 38 毫秒`
- `项目分析已取消 · develop · 已用 12.4 秒`
- `缓存可能已过期 · develop · 缓存 HEAD abc123，当前 HEAD def456`

屏幕阅读器使用 `role="status"` 和 `aria-live="polite"`。频繁计时变化不重复朗读，只有阶段变化和终态更新无障碍文案。

### 10.5 并发与显示优先级

- 项目刷新期间不允许启动文件刷新或提交下钻。
- 文件切换会使旧文件前端请求代次失效，但已开始的后端计算只有支持取消的阶段才终止。
- 提交下钻属于当前文件上下文；文件切换或刷新立即关闭下钻。
- 状态栏显示最近启动的前台操作；终态只能覆盖相同 `operation_id` 的运行状态。
- 项目元数据等后台操作不得覆盖正在进行的文件或项目分析状态。

## 11. 接口调整

### 11.1 新增 Tauri 命令

```text
review_restore_branch_analysis(project_id, branch)
  -> BranchAnalysisRestoreResult

review_get_branch_cache_status(project_id, branch)
  -> BranchCacheStatus

review_set_branch_selected_file(project_id, branch, file_path)
  -> void
```

### 11.2 调整现有请求

```text
review_start_analysis(filters, operation_id)
  语义：始终执行用户明确触发的项目刷新

review_get_file_overlay(request)
  新增 operation_id、cache_mode

review_attribute_blocks(request)
  新增 operation_id、cache_mode、block_signature

review_get_commit_overlay(request)
  新增 operation_id、cache_mode
```

`cache_mode` 仅包含：

- `prefer-cache`：正常打开；有效缓存命中直接读取。
- `refresh`：用户明确刷新；跳过当前范围对应缓存，成功后替换。

不提供 `no-store`，避免产生界面已更新但数据库仍是旧结果的不一致状态。

### 11.3 恢复结果

`BranchAnalysisRestoreResult` 返回：

- 是否命中缓存；
- 缓存是否过期；
- 快照、筛选和文件列表；
- 上次选中文件路径（仍存在时）；
- 当前 HEAD 与缓存 HEAD；
- 缓存读取耗时。

## 12. 错误、取消与恢复

- 数据库 schema 不兼容：保留 Git 仓库可用性，提示缓存需要重新分析，不读取不兼容派生数据。
- Git blob 已不存在：将对应文件缓存判为失效，提示重新分析项目。
- 缓存记录不完整：整项视为未命中，不返回部分作者或部分下钻。
- 项目刷新失败：继续展示旧快照，并在状态栏显示失败耗时。
- 文件刷新失败：继续展示旧文件缓存，不删除旧下钻。
- 用户取消：停止可协作取消的阶段，候选数据不发布。
- 应用异常退出：未提交事务自动回滚；下次仍读取上一个成功结果。

## 13. 数据迁移

缓存设计落地后的 schema 版本为 3，并提供 `1 -> 2 -> 3` 与 `2 -> 3` 显式迁移：

1. 保留已有提交索引表。
2. 创建新增缓存表和索引。
3. 不把旧的内存任务伪装成持久化快照。
4. 首次打开升级后的项目时无分支快照，界面显示“分析项目”。
5. migration 失败时返回稳定错误，不删除原数据库。
6. schema 3 在分支快照中增加可空 `last_selected_path`，只保存仍属于该快照的文件路径。

开发阶段不得通过直接删除用户数据库绕过 migration 测试。

## 14. 开发环境与诊断配置

继续使用项目既有环境：

- PowerShell 作为 Shell。
- Rust 使用 workspace Cargo 工具链。
- Node 通过 `fnm` 选择 `.node-version`，包管理使用 `pnpm`。
- Python 辅助工具如有需要使用 `uv`，本方案本身不新增 Python 依赖。

仅在开发构建启用详细性能日志。建议环境开关：

```text
REVIER_TRACE_OPERATIONS=1
```

日志字段包括 `operation_id`、阶段、项目、分支、文件、缓存状态、完成量、总量和耗时；不得记录源码正文、邮箱搜索文本之外的敏感文件内容。

## 15. 测试策略

### 15.1 Rust 单元与集成测试

- 每项目每分支只有一个成功快照。
- 不同分支缓存互不覆盖。
- 新分析失败或取消时旧快照仍可读。
- 增量索引只计算缺失提交。
- 批量写入失败时事务回滚。
- 文件打开不调用全范围文件差异。
- 文件归因只扫描候选提交。
- 多块 blame 共享资源和结果。
- 文件刷新清除下钻缓存；失败时不清除。
- schema migration 保留现有提交索引。
- 缓存算法版本、编码和块签名失配时返回未命中。

### 15.2 前端测试

- 打开项目自动恢复分支缓存，但不启动分析。
- HEAD 变化显示过期标记。
- 项目刷新与当前文件刷新按钮的启用条件正确。
- 缓存命中、未命中、刷新、失败和取消状态正确显示。
- 状态栏只接受当前操作事件。
- 已用时间持续增长，完成后显示最终耗时并保留到下一操作。
- 未知总量不显示百分比。
- 文件刷新后下钻关闭，下一次点击重新请求。

### 15.3 性能基准

至少记录以下场景的冷、热耗时：

1. 首次项目分析。
2. HEAD 增加一个提交后的增量项目刷新。
3. 二次打开相同分支。
4. 首次打开文件。
5. 再次打开已归因文件。
6. 首次与再次打开同一提交下钻。
7. 当前文件刷新。

性能记录必须分解为范围解析、索引读取／写入、文件列表、blob 读取、Monaco Diff、候选查询、patch、blame、删除追踪、数据库发布和前端合并，不能只记录命令总耗时。

## 16. 验收标准

- 再次打开已有分支缓存的项目时，不启动项目分析任务。
- 分支 HEAD 变化时可读取旧缓存并明确显示过期状态。
- 同一项目的不同分支各自最多保存一个成功快照。
- 同一分支重新分析成功后旧快照及其文件缓存被替换；失败或取消不替换。
- 已缓存文件再次打开时不执行全范围差异和历史归因。
- 已缓存提交下钻再次打开时不重新解析历史路径和 Diff 块。
- 当前文件刷新只刷新该文件，并使该文件下钻缓存失效。
- 项目刷新使用增量索引和事务批量写入。
- 所有加载阶段状态栏持续显示当前操作与耗时；有可靠总量时显示准确进度。
- 完成、失败、取消与缓存命中均显示真实耗时。
- 数据库不保存重复源码正文，提交和作者信息不在每个块中重复存储。
- 所有新增测试通过，且没有通过放宽既有断言掩盖行为回归。

## 17. 风险与控制

| 风险 | 影响 | 控制 |
| --- | --- | --- |
| 旧缓存被误认为最新 | Review 结论错误 | HEAD 对比、醒目的过期标记、禁止旧快照单文件追新 HEAD |
| 算法升级读取旧归因 | 作者或块错配 | `analysis_version + block_signature + blob_id` 联合校验 |
| 数据库替换中断 | 丢失可用缓存 | 候选结果完成后单事务原子发布 |
| 规范化表查询过多 | 热缓存仍慢 | 一次联表批量读取文件块、提交和范围 |
| 进度事件过密 | UI 卡顿 | 后端每秒最多 10 次，前端本地计时 |
| 增量索引保留不可达提交 | 数据库膨胀 | 仅在用户明确项目刷新时按全部本地分支与当前快照清理 |
| 不保存源码导致 Git blob 缺失 | 缓存无法展示 | 判为失效并要求项目刷新，不返回伪造内容 |
