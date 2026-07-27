# Revier 归因与提交下钻一致性修复设计

> 状态：待审核。本文记录代码块相关提交与提交下钻语义不一致问题的完整修复设计。本文获得确认前，不进入测试或实现阶段。

## 1. 背景

评审工作台会先为文件变更块计算作者和相关提交，再允许用户点击相关提交，查看该提交相对第一父提交造成的文件差异。

当前两条链路存在不同语义：

- 文件归因会扫描提交相对所有父提交的差异。
- Merge 追踪在父提交包含目标内容时，直接把父提交本身作为来源，但不会继续追到真正引入内容的提交。
- 相关提交只保存提交哈希、触达范围和 merge 链路，不保存归因时确认的历史文件路径。
- 提交下钻只接受目标提交相对第一父提交确实修改了目标文件的情况。

因此，归因列表可能出现无法按第一父语义生成提交级 Diff 的提交。用户点击后会收到 `FILE_NOT_CHANGED_IN_COMMIT`，表现为“该提交未修改当前文件”。

典型场景如下：

```text
A：修改目标文件，引入目标内容
B：在同一分支只修改其他文件，继承 A 的目标内容
M：把 B 所在分支合入目标分支

当前结果：M 的父链内容匹配后把 B 列为来源
真实来源：A
下钻结果：B 相对 B^1 没有修改目标文件，加载失败
```

另一个场景是 merge 提交只相对第二父或后续父提交存在目标文件差异。Patch 推断能够从该父差异选中 merge 提交，但第一父下钻没有对应文件变化。

## 2. 目标与非目标

### 2.1 目标

- 相关提交表示真正引入、修改或删除目标块的提交，不表示仅携带目标内容的分支末端提交。
- 所有返回给界面的相关提交都满足现有第一父下钻契约。
- Merge 来源追踪支持多层父链，并保留完整 `viaMergeHashes`。
- 多个父链都能解释来源时保留实际来源并标记 `partial`，不伪造唯一来源。
- 归因结果保存提交边界上的历史文件路径，下钻不再仅依赖当前路径重新猜测。
- 旧归因缓存自动失效；项目提交索引和项目快照不因本次算法升级而整体失效。
- 冷计算和缓存恢复返回相同的相关提交、历史路径、归因方法、merge 链路与置信度。
- 正常界面流程不再向用户暴露“相关提交却未修改当前文件”的矛盾结果。

### 2.2 非目标

- 不改变提交下钻的第一父语义。
- 不增加多父切换器或 combined diff。
- 不修改评审工作台布局、颜色、动效和列表样式。
- 不改变作者、提交信息和时间筛选规则。
- 不修改 Monaco 变更块几何计算规则。
- 不引入新的第三方依赖。
- 不修改既有测试的业务断言；若既有断言与本设计冲突，暂停实现并重新确认。

## 3. 设计原则

### 3.1 单一可点击提交契约

相关提交进入最终结果前必须满足：

1. 目标提交可从任务 `headCommit` 追溯。
2. 能解析该提交边界上的历史文件路径。
3. 目标提交相对第一父提交在该历史路径上存在文件变化；根提交使用空树作为第一父等价物。
4. 该变化能够生成文本提交级 Diff；二进制或无法按所选编码读取时沿用现有不可分析错误。

前三项是“可下钻性”不变量。归因算法不能把不满足该不变量的提交作为普通可点击相关提交发布。

### 3.2 来源提交与引入链路分离

- `RelatedCommit.hash` 保存真正修改文件的来源提交。
- `RelatedCommit.attribution.viaMergeHashes` 保存该来源进入目标历史时经过的 merge 提交。
- 仅携带相同内容、但没有修改目标文件的中间提交不进入 `relatedCommits`。
- Merge 冲突解决首次产生目标内容时，merge 提交自身是来源提交。

### 3.3 第一父口径保持一致

- 普通来源提交必须能由 `commit^1..commit` 解释。
- 根提交使用空树。
- Patch 推断只发布第一父差异能够解释的提交。
- 只有第二父或后续父能够解释的 merge 差异不直接发布 merge 提交；优先递归追踪对应父链的真实来源。
- 递归仍无法得到真实来源时，保留块级 `partial` 警告，不生成不可下钻提交。

## 4. 架构设计

本次修改沿用现有 Vue 3、Pinia、Tauri、Rust、gix 和 DuckDB 架构。

### 4.1 Git 分析层

涉及模块：

- `crates/revier-analysis/src/attribution/merge_trace.rs`
- `crates/revier-analysis/src/attribution/blame.rs`
- `crates/revier-analysis/src/attribution/patch_inference.rs`
- `crates/revier-analysis/src/attribution/deletion_trace.rs`
- `crates/revier-analysis/src/attribution/path_history.rs`
- `crates/revier-analysis/src/git/diff.rs`

职责：

- 递归解析 merge 来源。
- 解析来源提交边界上的历史路径。
- 统一第一父变化校验。
- 为每个相关提交输出已验证的历史路径。

### 4.2 服务与契约层

涉及模块：

- `crates/revier-analysis/src/json.rs`
- `crates/revier-analysis/src/contracts.rs`
- `src-tauri/src/services/review.rs`
- 生成的 TypeScript bindings

职责：

- 在 Rust 分析输出、Tauri 契约和前端请求中传递 `historicalPath`。
- 提交下钻优先使用归因阶段确认的历史路径，并再次执行可信校验。
- 兼容没有 `historicalPath` 的旧请求，使用现有 Git 路径解析作为降级。

### 4.3 缓存层

涉及模块：

- `crates/revier-analysis/src/index/schema.rs`
- `crates/revier-analysis/src/index/migrations.rs`
- `crates/revier-analysis/src/cache/models.rs`
- `crates/revier-analysis/src/cache/repository.rs`
- `docs/cache-schema.md`

职责：

- schema v3 升级到 v4。
- 为块相关提交保存历史路径。
- 区分项目快照算法版本与文件归因算法版本。
- 原子保存和恢复新增字段。

### 4.4 前端状态层

涉及模块：

- `src/renderer/stores/reviewStore.ts`
- 相关前端单元测试

职责：

- 点击相关提交时，把 `historicalPath` 作为可选上下文传给提交下钻请求。
- 不改变详情列表布局和交互。
- 旧数据没有历史路径时仍可发起请求。

## 5. Merge 来源递归算法

### 5.1 输入

递归解析器接收：

```text
commitHash
filePath
目标块内容指纹
目标块行窗口
已经过的 merge 哈希
当前文本编码
```

内容指纹由目标块新侧的完整行序列组成，不只使用单行文本。

### 5.2 非 merge 提交

对非 merge 提交：

1. 解析目标文件在该提交边界上的历史路径。
2. 比较该提交与第一父在该路径上的文件变化。
3. 若变化触达目标内容窗口，将该提交作为真实来源。
4. 若该提交没有修改文件但只是继承目标内容，在该提交执行目标区间 blame，继续追踪 blame 返回的更早提交。
5. 若 blame 不可用，返回未解析状态，不把当前提交伪装成来源。

步骤 4 专门处理“来源提交后又有不相关提交”的场景。

### 5.3 Merge 提交

对 merge 提交：

1. 读取 merge 结果中目标路径的文本，确认目标块内容窗口存在。
2. 对每个父提交解析对应历史路径，并检查相同行窗口。
3. 相同行窗口不匹配时，仅在该文件中搜索完整内容指纹：
   - 唯一匹配时使用匹配到的行窗口。
   - 多处匹配时视为歧义，不凭距离擅自选择。
4. 对每个匹配父提交执行 blame，并递归解析 blame 来源。
5. 把当前 merge 哈希追加到每个真实来源的 `viaMergeHashes`。
6. 多个父链最终得到不同真实来源时，合并去重并把块标记为 `partial`。
7. 没有任何父提交包含目标内容，但 merge 结果包含目标内容时，认为内容由冲突解决产生，merge 提交自身为真实来源。

Git 提交图是有向无环图。实现使用访问集合避免同一 `commitHash + historicalPath + contentFingerprint` 被重复展开，不设置任意递归深度上限。

### 5.4 结果合并

相关提交按完整哈希去重：

- `touchedRanges` 取并集。
- `viaMergeHashes` 按首次出现顺序去重。
- `matchedByFilter` 对真实来源提交本身计算，不对中间 merge 提交计算。
- 多条路径到达同一真实来源时只显示一次。
- 最终排序沿用现有 `committedAt` 降序、完整哈希升序规则。

## 6. Patch 与删除归因

### 6.1 Patch 推断

当前 `commit_file_changes` 返回所有父提交差异。本次保留该底层能力，但 Patch 推断输出调整为：

- 普通提交和根提交使用 `parentIndex = 0`。
- Merge 提交只把第一父变化作为可直接发布的候选。
- 后续父变化只用于发现需要追踪的父链，不直接把 merge 提交发布为相关提交。
- 后续父来源无法递归解析时，块置信度降为 `partial`。

### 6.2 删除归因

删除追踪继续优先寻找真正执行删除的非 merge 提交：

- 非 merge 删除提交必须通过第一父变化校验。
- Merge 引入分支删除时，沿对应父链寻找真正执行删除的提交。
- Merge 冲突解决中首次执行删除时，merge 提交自身只有在相对第一父也存在该删除时才可下钻。
- 无法唯一解析时保留 `DELETION_TRACE_INCOMPLETE`，不新增不可下钻提交。

## 7. 历史路径设计

### 7.1 输出字段

`RelatedCommitOutput` 与公共 `RelatedCommit` 新增可选字段：

```rust
pub historical_path: Option<String>
```

生成的 TypeScript 字段为：

```ts
historicalPath?: string;
```

字段含义是“目标提交相对第一父发生目标文件变化时使用的新侧路径；删除提交使用删除前的旧侧路径”。

具体规则：

- 新增、修改：`change.path`
- 重命名：`change.path`
- 删除：`change.old_path.unwrap_or(change.path)`
- 根提交新增：`change.path`

### 7.2 下钻请求

`CommitOverlayRequest` 新增可选字段：

```rust
pub historical_path: Option<String>
```

前端从选中的 `RelatedCommit.historicalPath` 传入。后端不直接信任该路径，而是校验：

1. 路径必须是仓库相对路径。
2. 目标提交第一父变化必须包含该路径或其 rename 对端。
3. 该路径必须与任务当前文件的路径历史连通。

校验成功时直接使用该路径；校验失败或字段缺失时，调用现有路径解析逻辑重新计算。

### 7.3 发布前校验

归因完成后，对去重后的相关提交执行一次批量可下钻性校验。校验结果按：

```text
commitHash + historicalPath
```

缓存，避免同一文件多个块重复读取 Git 差异。

无法通过校验的候选不会进入最终 `relatedCommits`，块增加：

```text
RELATED_COMMIT_DRILLDOWN_UNAVAILABLE
相关提交无法按第一父语义生成文件差异，已从结果中排除
```

块置信度至少降为 `partial`。该警告是最终安全网，正常递归路径不应依赖它修正结果。

## 8. 接口规范

### 8.1 `RelatedCommit`

新增向后兼容的可选字段：

```ts
historicalPath?: string;
```

其余字段不变。

### 8.2 `CommitOverlayRequest`

新增向后兼容的可选字段：

```ts
historicalPath?: string;
```

现有 `taskId`、`filePath`、`commitHash`、`operationId`、`cacheMode` 和 `encoding` 不变。

### 8.3 错误契约

- 正常归因结果必须在发布前满足可下钻性，不应触发 `FILE_NOT_CHANGED_IN_COMMIT`。
- 手工构造、旧前端或过期上下文仍可能触发该错误，现有错误码保留。
- 新增块级 warning code：

```text
RELATED_COMMIT_DRILLDOWN_UNAVAILABLE
```

- 不新增新的前端通知类型；现有归因可信度区域展示 warning 文案。

## 9. 数据流程

```text
Monaco 规范变更块
  -> Patch 候选与 head blame
  -> 遇到 merge 或继承内容的提交
  -> 按父链路径和内容指纹递归 blame
  -> 得到真正修改文件的来源提交
  -> 解析并记录 historicalPath
  -> 批量执行第一父可下钻性校验
  -> 聚合作者、相关提交、merge 链路和置信度
  -> 原子写入文件归因缓存
  -> 前端显示相关提交
  -> 点击时传递 commitHash + historicalPath
  -> 后端验证历史路径和第一父变化
  -> 读取或生成提交下钻 Overlay
```

## 10. 缓存与迁移

### 10.1 Schema v4

`file_block_commits` 新增可空列：

```sql
historical_path text
```

采用可空列是为了让 v3 数据无损迁移。新版本写入的归因记录必须包含已验证路径；旧记录因归因算法版本不匹配，不会作为新归因结果恢复。

迁移事务：

1. 为 `file_block_commits` 增加 `historical_path`。
2. 将 `metadata.schema_version` 更新为 `4`。
3. 任一步失败时回滚，保留 v3 数据库。

不修改主键和已有提交下钻表结构。

### 10.2 版本拆分

当前单一 `ANALYSIS_VERSION = 1` 同时用于项目快照、文件归因和提交下钻。本次拆分为：

```rust
pub const SNAPSHOT_ANALYSIS_VERSION: u32 = 1;
pub const FILE_ATTRIBUTION_VERSION: u32 = 2;
```

使用规则：

- `analysis_snapshots.analysis_version` 使用 `SNAPSHOT_ANALYSIS_VERSION`，现有项目快照继续有效。
- `file_analyses.analysis_version` 使用 `FILE_ATTRIBUTION_VERSION`，旧归因缓存不再命中。
- `commit_overlays.analysis_version` 使用 `FILE_ATTRIBUTION_VERSION`。
- 新文件归因原子替换成功时，沿用现有行为删除该文件旧提交下钻。

这样只重算用户打开的文件归因，不强制重建提交索引或全部项目文件快照。

### 10.3 缓存模型

`CachedBlockCommit` 新增：

```rust
pub historical_path: Option<String>
```

保存和恢复必须覆盖：

- 提交哈希
- 历史路径
- 筛选命中状态
- 归因方式
- 触达范围
- merge 链路

冷结果与热结果按以上字段做完整等价断言。

## 11. 状态、性能与取消

- 递归解析复用任务级提交元数据缓存和文本缓存。
- 新增 `commitHash + historicalPath + contentFingerprint` 的 merge 来源缓存。
- 新增 `commitHash + historicalPath` 的可下钻性校验缓存。
- 多个块引用同一来源时不重复执行 Git tree diff。
- 现有文件归因取消检查和原子发布边界保持不变。
- 取消或失败时不写入半成品归因，不删除旧可用缓存。
- 不预计算所有相关提交的完整 Overlay；仍在用户点击时按需生成。

## 12. TDD 测试设计

严格按照“失败测试 → 最小实现 → 回归”执行。只新增测试和 fixture，不修改既有测试逻辑。

### 12.1 Git fixture

新增以下临时仓库场景：

1. `merge_source_followed_by_unrelated_commit`
   - A 修改目标文件。
   - B 只修改其他文件。
   - Merge 分支。
   - 期望相关提交包含 A，不包含 B。

2. `nested_merge_source`
   - A 修改目标文件。
   - A 经两层 merge 进入 head。
   - 期望来源为 A，`viaMergeHashes` 保留两层 merge。

3. `merge_only_differs_from_second_parent`
   - Merge 结果相对第一父目标文件不变，但相对第二父不同。
   - 期望不把 merge 提交作为可点击来源。

4. `ambiguous_nested_merge_sources`
   - 两条父链各自包含独立的真实修改提交。
   - 期望保留两个真实来源并标记 `partial`。

5. `renamed_source_followed_by_merge`
   - 目标文件经历多次重命名、修改和 merge。
   - 期望来源提交携带其提交边界上的历史路径。

6. `merge_deletion_source_followed_by_unrelated_commit`
   - 分支中先删除目标内容，再提交不相关变化后 merge。
   - 期望归因给真正执行删除的提交。

### 12.2 Rust 单元测试

- Merge 递归遇到非修改父提交时继续 blame。
- 多层 `viaMergeHashes` 顺序与去重。
- 内容指纹唯一匹配和多处匹配。
- 第一父候选发布规则。
- 可下钻性校验排除无效候选并生成 warning。
- 历史路径字段的新增、修改、重命名、删除规则。

### 12.3 Rust 集成测试

对每个 fixture：

1. 执行文件归因。
2. 枚举每个块的全部 `relatedCommits`。
3. 使用对应 `historicalPath` 请求提交下钻。
4. 断言每个请求成功且提交级 Diff 包含目标文件变化。

该枚举测试直接验证“所有相关提交都可下钻”的核心不变量。

### 12.4 缓存测试

- v3 → v4 迁移保留已有数据并增加新列。
- 新归因写入并恢复 `historicalPath`。
- `SNAPSHOT_ANALYSIS_VERSION` 仍命中现有项目快照。
- 旧 `FILE_ATTRIBUTION_VERSION` 不命中文件归因。
- 新文件归因替换后清除旧提交下钻。
- 冷缓存和热缓存的相关提交完整等价。

### 12.5 Tauri 与前端测试

- Store 点击提交时传递 `historicalPath`。
- 缺少历史路径时保持旧请求兼容。
- 后端拒绝与当前文件历史不连通的路径。
- 合法历史路径直接生成第一父下钻。
- UI 不新增布局和样式变化。

### 12.6 回归命令

开发环境使用 PowerShell：

```powershell
fnm use
pnpm test
cargo test -p revier-analysis
cargo test --manifest-path src-tauri/Cargo.toml
pnpm lint
pnpm typecheck
```

若仓库实际脚本名称与上述命令不同，实施前先从 `package.json` 和 Cargo workspace 核对，不自行新增替代脚本。

## 13. 开发环境配置

- Shell：PowerShell 7。
- Node：使用仓库 `.node-version`，通过 `fnm use` 激活。
- Node 包管理器：`pnpm`。
- Rust：沿用仓库现有 toolchain 和 Cargo workspace。
- 数据库：沿用当前 DuckDB 依赖，不升级依赖版本。
- Python：本次设计不需要 Python；如后续验证确需使用，只通过 `uv` 执行。

首次进入仓库：

```powershell
Set-Location E:\Projects\revier
fnm use
pnpm install
cargo test -p revier-analysis --no-run
```

依赖已完整安装时不重复安装。

## 14. 实施节点

### 节点一：文档确认

确认本文的算法、接口、schema v4、版本拆分和测试范围。

### 节点二：TDD 红灯

只新增 fixture 与失败测试，提交测试结果供确认。若需要修改现有测试断言，暂停并单独申请许可。

### 节点三：归因核心实现

实现递归 merge 来源、第一父候选规则、历史路径和发布前校验。核心语义发生偏差时重新提交确认。

### 节点四：契约与缓存实现

实现可选字段、schema v4、缓存版本拆分和冷热恢复。

### 节点五：前端接入

只传递历史路径，不调整视觉和交互。

### 节点六：全量验证

执行定向测试、全量测试、lint 和类型检查，整理验证结果供最终审核。

## 15. Git 提交规划

提交消息采用语义化格式，建议拆分为：

```text
docs(review): 设计归因与提交下钻一致性修复
test(attribution): 覆盖不可下钻的 merge 来源
fix(attribution): 递归解析可下钻的真实来源
feat(cache): 保存相关提交历史路径
fix(review): 使用归因路径加载提交下钻
docs(review): 记录归因下钻一致性验证结果
```

实际提交前逐次核对变更范围，不混入用户已有改动。

## 16. 验收标准

- 新增、修改、删除、重命名和 merge 场景中的每个相关提交均可成功打开提交下钻。
- 不再把仅继承目标内容、但未修改目标文件的提交显示为相关提交。
- Merge 冲突解决产生的新内容仍归因给 merge 提交。
- 多层 merge 显示真实来源并保留完整 merge 链路。
- 仅后续父差异不能产生违反第一父下钻契约的相关提交。
- 历史路径经过缓存保存和恢复后保持不变。
- schema v3 可事务升级到 v4。
- 旧文件归因缓存失效，项目快照缓存继续有效。
- 冷计算与热缓存结果一致。
- 所有新增和既有测试通过。
- 前端布局、样式和交互范围不发生未确认变化。
