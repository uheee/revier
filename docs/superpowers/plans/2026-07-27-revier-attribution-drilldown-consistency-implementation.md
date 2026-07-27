# Revier 归因与提交下钻一致性修复实施计划

> **状态：** 设计已确认，实施计划待审核；本计划获得确认前不得修改业务代码或测试。
> **设计：** `../specs/2026-07-27-revier-attribution-drilldown-consistency-design.md`

**目标：** 让代码块的每个相关提交都代表真正修改目标文件的来源提交，并保证它能按现有第一父语义成功打开提交下钻。

**技术路线：** 在 Rust 分析层递归追踪 merge 父链和继承内容的普通提交；用统一的第一父来源解析器校验相关提交并记录历史路径；通过可选契约字段把历史路径传递到 Tauri 下钻；将 DuckDB 升级到 schema v4，并独立升级文件归因缓存版本。

**技术栈：** Rust 2021、gix、gix-blame、DuckDB、Tauri 2、Vue 3、Pinia、TypeScript、Vitest、Node 24、fnm、pnpm、PowerShell 7。

## 1. 实施约束

- 全程使用 TDD：先新增失败测试，再实现最小代码，再运行定向与回归测试。
- 第一个实施节点只允许执行基线检查，以及新增 Git fixture 和失败测试；完成后暂停并提交红灯证据。
- 不修改既有测试逻辑。若既有断言必须变化，先列出文件、原断言、拟修改断言和原因，获得书面确认后执行。
- 不新增第三方依赖，不修改 Cargo 或 pnpm 依赖版本。
- 不改变提交下钻的第一父语义。
- 不修改 UI 布局、颜色、动效、列表样式和已有交互入口。
- 所有回答、文档、错误信息和新增代码注释使用中文。
- Shell 使用 PowerShell；Node 使用 fnm 和 pnpm；本计划不需要 Python。
- 工作树中的既有改动均视为用户改动，不覆盖、不重置、不混入本功能范围。
- 未经用户明确要求，不执行 Git 提交或推送。需要提交时采用本计划末尾的语义化提交消息。

## 2. 预计文件范围

### 2.1 新增文件

- `crates/revier-analysis/src/attribution/source_validation.rs`
- `docs/superpowers/verification/2026-07-27-revier-attribution-drilldown-consistency.md`

### 2.2 Rust 分析库

- `crates/revier-analysis/src/attribution/mod.rs`
- `crates/revier-analysis/src/attribution/blame.rs`
- `crates/revier-analysis/src/attribution/merge_trace.rs`
- `crates/revier-analysis/src/attribution/patch_inference.rs`
- `crates/revier-analysis/src/attribution/deletion_trace.rs`
- `crates/revier-analysis/src/attribution/path_history.rs`
- `crates/revier-analysis/src/git/diff.rs`
- `crates/revier-analysis/src/json.rs`
- `crates/revier-analysis/src/contracts.rs`
- `crates/revier-analysis/src/cache/mod.rs`
- `crates/revier-analysis/src/cache/models.rs`
- `crates/revier-analysis/src/cache/repository.rs`
- `crates/revier-analysis/src/index/schema.rs`
- `crates/revier-analysis/src/index/migrations.rs`
- `crates/revier-analysis/tests/fixtures.rs`
- `crates/revier-analysis/tests/overlay_attribution.rs`
- `crates/revier-analysis/tests/cache_schema.rs`
- `crates/revier-analysis/tests/cache_repository.rs`
- `crates/revier-analysis/tests/api_contract.rs`
- `crates/revier-analysis/tests/bindings.rs`

### 2.3 Tauri

- `src-tauri/src/services/review.rs`
- `src-tauri/src/commands/review.rs`，仅在请求适配需要时修改

### 2.4 Vue 与 TypeScript

- `src/renderer/generated/bindings.ts`
- `src/renderer/stores/reviewStore.ts`
- `tests/unit/rendererReviewStore.test.ts`
- `tests/unit/revierClient.test.ts`，仅在客户端请求断言覆盖不到新增字段时修改

### 2.5 文档

- `docs/cache-schema.md`
- 本计划对应的验证记录

若实施中发现必须修改上述范围之外的业务文件，先暂停并说明原因、文件和预期影响。

## 3. 节点与审核门

### 节点 A：TDD 红灯

允许：

- 基线只读检查。
- 新增临时 Git fixture。
- 新增失败测试。

禁止：

- 修改归因、契约、缓存、Tauri 或前端业务实现。
- 修改既有测试断言。

交付：

- 新增测试清单。
- 每项失败测试的命令和失败原因。
- 基线中发现的既有失败。

获得确认后进入节点 B。

### 节点 B：归因核心

实现：

- 递归 merge 来源。
- 继承内容提交继续 blame。
- 第一父候选规则。
- 发布前可下钻性校验。

交付定向测试结果，确认后进入节点 C。

### 节点 C：契约与缓存

实现：

- `historicalPath`。
- schema v4。
- 文件归因版本拆分。
- warning 缓存。

该节点包含公共契约和数据库迁移，是关键变更。交付迁移与冷热缓存验证后，确认再进入节点 D。

### 节点 D：下钻与前端接入

实现：

- Tauri 验证和使用历史路径。
- Store 传递历史路径。
- 全相关提交下钻不变量测试。

完成后进入全量验证并提交最终成果审核。

## 4. Task 0：工作区与基线

### Step 1：确认工作树范围

```powershell
Set-Location E:\Projects\revier
git status --short
git diff --check
```

要求：

- 记录当前已有改动。
- 本功能文件与用户改动重叠时暂停确认。
- 不使用 `git reset`、`git checkout --` 或其他覆盖命令。

### Step 2：激活 Node 工具链

```powershell
fnm env --use-on-cd --shell powershell | Out-String | Invoke-Expression
fnm use
node --version
pnpm --version
rustc --version
cargo --version
```

预期：

- Node 主版本为 24。
- pnpm 使用项目声明的 10.x。
- Rust 使用仓库现有 stable 工具链。

### Step 3：运行基线

```powershell
cargo fmt --all -- --check
cargo test -p revier-analysis
cargo test -p revier-tauri
pnpm test
pnpm typecheck
pnpm lint
git diff --check
```

要求：

- 基线失败不得归入本功能导致的失败。
- 记录测试数量、失败用例和耗时。
- 若基线存在失败，先提交用户决定是否继续在已知失败上开展定向 TDD。

## 5. Task 1：新增归因语义红灯测试

本任务属于节点 A，只修改测试和 fixture。

**文件：**

- 修改：`crates/revier-analysis/tests/fixtures.rs`
- 修改：`crates/revier-analysis/tests/overlay_attribution.rs`

### Step 1：新增“不相关分支末端提交”fixture

新增：

```text
merge_source_followed_by_unrelated_commit
```

提交图：

```text
base
  ├─ main preparation
  └─ A: 修改 src/app.txt
       B: 只修改 docs/notes.txt
main + merge(feature) -> head
```

测试断言：

- 目标块包含 A。
- 目标块不包含 B。
- A 的归因方式为 `merge-trace` 或最终确认的精确来源方式。
- `viaMergeHashes` 包含 merge 提交。
- 不把只携带内容的 B 计入作者。

当前实现会把 B 作为匹配父提交，测试应失败。

### Step 2：新增多层 merge fixture

新增：

```text
nested_merge_source
```

提交图：

```text
A: 修改目标文件
M1: feature -> integration
M2: integration -> main
```

测试断言：

- 最终来源为 A。
- 不包含仅作为链路节点的 M1 父端末梢提交。
- `viaMergeHashes` 按来源到 head 的稳定顺序包含 M1 和 M2。
- 相同 merge 哈希不重复。

当前实现只展开一层，测试应失败。

### Step 3：新增仅后续父差异 fixture

新增：

```text
merge_only_differs_from_second_parent
```

测试断言：

- Merge 提交相对第一父未修改目标文件时，不作为可点击来源提交。
- 能追到第二父链真实修改提交时，相关提交为真实来源。
- 不能追到唯一来源时，块为 `partial`，且不输出违反第一父契约的 merge 提交。

当前 Patch 推断扫描所有 `parentIndex`，测试应失败。

### Step 4：新增多层歧义 fixture

新增：

```text
ambiguous_nested_merge_sources
```

测试断言：

- 返回左右父链中真正修改文件的提交。
- 不返回只携带内容的父链末端提交。
- 块置信度为 `partial`。
- warning 包含 `MERGE_TRACE_AMBIGUOUS`。
- 每个来源保留自己的 merge 链路。

### Step 5：新增 merge 删除 fixture

新增：

```text
merge_deletion_source_followed_by_unrelated_commit
```

测试断言：

- 删除块归因给真正执行删除的提交。
- 不归因给后续不相关提交或仅引入分支的 merge 提交。
- 删除来源相对第一父确实存在删除变化。

### Step 6：运行红灯测试

```powershell
cargo test -p revier-analysis --test overlay_attribution merge_source_followed_by_unrelated_commit
cargo test -p revier-analysis --test overlay_attribution nested_merge_source
cargo test -p revier-analysis --test overlay_attribution merge_only_differs_from_second_parent
cargo test -p revier-analysis --test overlay_attribution ambiguous_nested_merge_sources
cargo test -p revier-analysis --test overlay_attribution merge_deletion_source_followed_by_unrelated_commit
```

再运行整个文件：

```powershell
cargo test -p revier-analysis --test overlay_attribution
```

预期：

- 新用例因错误来源提交、缺失递归链路或第一父不一致而失败。
- 既有用例保持原结果。
- 若新测试意外通过，先检查 fixture 是否真实覆盖目标提交图，不写无意义实现。

### Step 7：提交节点 A 结果

整理：

- 每个 fixture 的提交图与关键哈希。
- 实际相关提交和期望相关提交。
- 测试失败摘要。
- 基线与新增失败的边界。

暂停，等待用户确认进入归因核心实现。

## 6. Task 2：递归解析真实 merge 来源

本任务在节点 A 获得确认后执行。

**文件：**

- 修改：`crates/revier-analysis/src/attribution/merge_trace.rs`
- 修改：`crates/revier-analysis/src/attribution/blame.rs`
- 修改：`crates/revier-analysis/src/attribution/path_history.rs`
- 修改：`crates/revier-analysis/tests/overlay_attribution.rs`

### Step 1：先补充模块级失败测试

在 `merge_trace.rs` 的测试模块补充：

- 非 merge 提交没有触达文件时继续 blame，而不是返回该提交。
- 一个父提交唯一包含完整内容指纹时进入该父链。
- 相同行窗口不匹配，但完整内容指纹在文件中唯一出现时更新行窗口。
- 完整内容指纹多处出现时返回歧义，不按最近距离选择。
- 多层递归累计 `viaMergeHashes`。
- 多条链路到达同一真实来源时按哈希去重。
- Merge 结果首次产生内容时归因给 merge 提交自身。

先运行：

```powershell
cargo test -p revier-analysis attribution::merge_trace
```

确认新增测试失败。

### Step 2：扩展递归上下文

在 `merge_trace.rs` 中引入内部递归上下文，持有：

```text
文本编码
commitHash + filePath 文本缓存
commitHash + historicalPath + contentFingerprint 来源缓存
当前展开访问集合
```

要求：

- 访问集合用于避免重复展开，不设置任意深度上限。
- 缓存只存在于单次文件归因任务。
- 不引入全局可变状态。

### Step 3：实现父路径和内容窗口解析

为每个父提交：

1. 从当前路径和已知 rename 对端得到父边界路径候选。
2. 优先检查相同行窗口。
3. 相同行窗口不匹配时查找完整内容指纹。
4. 唯一匹配才继续；多处匹配进入歧义分支。

路径解析继续集中在 `path_history.rs`，不在 `merge_trace.rs` 复制 rename 规则。

### Step 4：对匹配父提交继续 blame

- 在父提交、父路径和匹配行窗口执行 `git::blame::blame_range`。
- Blame 返回普通修改提交时继续验证该提交是否真正触达文件。
- Blame 返回 merge 提交时递归展开。
- Blame 返回仅继承内容的普通提交时继续向更早来源追踪。
- Blame 不可用时返回未解析状态，不把当前父提交作为来源。

### Step 5：合并来源

- 真实来源按完整哈希去重。
- `viaMergeHashes` 按首次出现顺序合并。
- 多个不同来源把块标记为 `partial`。
- Conflict resolution 内容不存在于任何父提交时保留 merge 自身。
- `matchedByFilter` 对真实来源提交计算。

### Step 6：定向验证

```powershell
cargo test -p revier-analysis attribution::merge_trace
cargo test -p revier-analysis --test overlay_attribution merge_source_followed_by_unrelated_commit
cargo test -p revier-analysis --test overlay_attribution nested_merge_source
cargo test -p revier-analysis --test overlay_attribution ambiguous_nested_merge_sources
cargo test -p revier-analysis --test overlay_attribution merge_conflict_resolution
cargo test -p revier-analysis --test overlay_attribution
```

通过标准：

- Task 1 中 merge 来源相关红灯转绿。
- 既有 conflict resolution 和歧义测试继续通过。
- 仅后续父差异的安全规则可留到 Task 3，不得用错误断言临时转绿。

## 7. Task 3：统一第一父来源和发布前校验

**文件：**

- 新增：`crates/revier-analysis/src/attribution/source_validation.rs`
- 修改：`crates/revier-analysis/src/attribution/mod.rs`
- 修改：`crates/revier-analysis/src/attribution/patch_inference.rs`
- 修改：`crates/revier-analysis/src/attribution/deletion_trace.rs`
- 修改：`crates/revier-analysis/src/attribution/blame.rs`
- 修改：`crates/revier-analysis/src/overlay/file_overlay.rs`
- 修改：`crates/revier-analysis/src/json.rs`
- 修改：`crates/revier-analysis/tests/overlay_attribution.rs`

### Step 1：新增来源校验失败测试

为新模块定义测试输入和预期：

- 普通修改提交相对第一父触达文件时通过。
- 根提交新增文件时通过。
- Merge 只相对第二父修改文件时不通过。
- 当前路径经过 rename 后可解析提交边界路径。
- 候选提交未修改目标文件时不通过。
- 同一 `commitHash + historicalPath` 多次校验复用结果。

新增文件前先在测试中引用预期接口，运行：

```powershell
cargo test -p revier-analysis attribution::source_validation
```

预期因模块或行为尚未实现而失败。

### Step 2：定义内部来源结构

内部结构至少包含：

```text
commitHash
historicalPath
firstParentHash
oldBlobId
newBlobId
changeStatus
```

职责：

- 只表达可按第一父生成文件 Diff 的来源。
- 不承载 UI 文案。
- 不把后续父差异伪装成第一父来源。

### Step 3：实现统一来源解析

解析步骤：

1. 获取提交和第一父；根提交使用空树。
2. 获取该提交的全部文件变化。
3. 只检查 `parentIndex == 0`。
4. 使用当前路径、旧路径和归因路径候选匹配目标文件。
5. 返回规范化历史路径和 Blob ID。

### Step 4：收紧 Patch 推断

- 第一父变化继续生成直接候选。
- 后续父变化只触发来源追踪，不直接生成 merge 候选。
- 无法追到真实来源时不发布 merge 提交。
- 对应块置信度降为 `partial`。

### Step 5：校验删除来源

- 删除候选必须通过统一来源解析。
- 分支删除经 merge 引入时保留真正的非 merge 删除提交。
- Merge conflict resolution 的删除只有相对第一父存在删除时才可作为来源。

### Step 6：增加最终安全网

文件归因聚合完成后：

- 对每个去重相关提交执行来源解析。
- 无效候选从 `relatedCommits` 排除。
- 同步重新聚合作者，避免保留被排除提交的作者。
- 块置信度至少为 `partial`。
- 增加内部 warning：

```text
RELATED_COMMIT_DRILLDOWN_UNAVAILABLE
相关提交无法按第一父语义生成文件差异，已从结果中排除
```

公共 warning 枚举和缓存位在 Task 4、Task 5 接入。

### Step 7：定向验证

```powershell
cargo test -p revier-analysis attribution::source_validation
cargo test -p revier-analysis --test overlay_attribution merge_only_differs_from_second_parent
cargo test -p revier-analysis --test overlay_attribution merge_deletion_source_followed_by_unrelated_commit
cargo test -p revier-analysis --test overlay_attribution
cargo test -p revier-analysis --test file_overlay_cli
```

节点 B 验收：

- Task 1 新增归因语义测试全部通过。
- 每个相关提交都能解析为第一父来源。
- 尚未接入公共 `historicalPath` 和缓存；不得提前修改前端。

完成后提交节点 B 结果供审核。

## 8. Task 4：扩展公共契约与绑定

本任务在节点 B 获得确认后执行。

**文件：**

- 修改：`crates/revier-analysis/src/json.rs`
- 修改：`crates/revier-analysis/src/contracts.rs`
- 修改：`crates/revier-analysis/tests/api_contract.rs`
- 修改：`crates/revier-analysis/tests/bindings.rs`
- 生成：`src/renderer/generated/bindings.ts`

### Step 1：新增契约失败测试

断言：

- `RelatedCommitOutput` 序列化可选 `historicalPath`。
- 公共 `RelatedCommit` 生成 `historicalPath?: string`。
- `CommitOverlayRequest` 生成 `historicalPath?: string`。
- `AttributionWarningCode` 包含 `related-commit-drilldown-unavailable`。
- 缺少新增字段的旧 JSON 仍可反序列化。

运行：

```powershell
cargo test -p revier-analysis --test api_contract
cargo test -p revier-analysis --test bindings
```

预期因字段和枚举尚未定义而失败。

### Step 2：最小扩展契约

新增：

```text
RelatedCommitOutput.historical_path: Option<String>
RelatedCommit.historical_path: Option<String>
CommitOverlayRequest.historical_path: Option<String>
AttributionWarningCode::RelatedCommitDrilldownUnavailable
```

要求：

- Serde 和 Specta 使用 camelCase。
- 新字段可选，保持旧请求兼容。
- 所有 `RelatedCommitOutput` 构造点显式设置历史路径或 `None`。
- 归因链路通过 Task 3 的来源结构填写 `Some(path)`。
- 提交下钻 Overlay 自身附带的唯一提交可使用实际变化路径。

### Step 3：生成绑定

```powershell
pnpm generate:bindings
pnpm typecheck
```

要求：

- 只接受由绑定生成器产生的变更。
- 不手工编辑生成文件。
- 未经提交授权，不运行依赖“工作树无差异”的 `pnpm generate:bindings:check`。

### Step 4：验证

```powershell
cargo test -p revier-analysis --test api_contract --test bindings
pnpm typecheck
git diff --check
```

## 9. Task 5：升级 schema v4 和归因缓存版本

**文件：**

- 修改：`crates/revier-analysis/src/index/schema.rs`
- 修改：`crates/revier-analysis/src/index/migrations.rs`
- 修改：`crates/revier-analysis/src/cache/mod.rs`
- 修改：`crates/revier-analysis/src/cache/models.rs`
- 修改：`crates/revier-analysis/src/cache/repository.rs`
- 修改：`crates/revier-analysis/tests/cache_schema.rs`
- 修改：`crates/revier-analysis/tests/cache_repository.rs`
- 修改：`src-tauri/src/services/review.rs`
- 修改：`docs/cache-schema.md`

### Step 1：新增 schema 迁移失败测试

断言：

- 新建数据库版本为 4。
- `file_block_commits` 包含可空 `historical_path text`。
- v3 → v4 后既有提交、快照、文件分析和下钻数据仍存在。
- 旧行的 `historical_path` 为 `NULL`。
- 重复执行兼容检查幂等。
- 迁移事务失败时 schema 版本和原数据不变。

运行：

```powershell
cargo test -p revier-analysis --test cache_schema
```

预期因当前 schema 仍为 3 而失败。

### Step 2：新增缓存仓储失败测试

断言：

- `CachedBlockCommit` 写入和恢复 `historical_path`。
- `None` 能兼容旧记录。
- 冷写入后热恢复字段完整等价。
- 文件分析原子替换失败时旧路径数据仍在。

运行：

```powershell
cargo test -p revier-analysis --test cache_repository
```

预期因模型和 SQL 尚未支持字段而失败。

### Step 3：实现 schema v4

- `CURRENT_SCHEMA_VERSION` 改为 4。
- 新 schema 的 `file_block_commits` 增加 `historical_path text`。
- 新增独立的 `migrate_version_three_to_four`。
- v1、v2 迁移链最终继续到 v4。
- 每次迁移只负责相邻版本。
- 事务成功后才更新 `metadata.schema_version`。

### Step 4：扩展缓存模型与仓储

- `CachedBlockCommit` 新增 `historical_path: Option<String>`。
- 写入 SQL 增加列。
- 读取 SQL 恢复列。
- 测试 helper 和新生产构造点补齐字段。
- 不改变现有主键。

### Step 5：拆分版本常量

将当前单一常量拆为：

```text
SNAPSHOT_ANALYSIS_VERSION = 1
FILE_ATTRIBUTION_VERSION = 2
```

替换规则：

- 分支快照创建、恢复和 stale 判断使用 `SNAPSHOT_ANALYSIS_VERSION`。
- 文件归因读写使用 `FILE_ATTRIBUTION_VERSION`。
- 提交下钻缓存读写使用 `FILE_ATTRIBUTION_VERSION`。
- 旧文件归因未命中时冷计算。
- 新文件归因发布成功后沿用现有逻辑删除旧下钻。

不得通过把快照版本整体升级为 2 来绕过版本拆分测试。

### Step 6：接入 warning flag

为 `RELATED_COMMIT_DRILLDOWN_UNAVAILABLE` 分配：

```text
1 << 5
```

要求：

- 冷结果转换、缓存写入和缓存恢复一致。
- 未知位不导致崩溃，沿用现有未知位忽略策略。
- 已有五个 warning 位值保持不变。

### Step 7：更新缓存文档

`docs/cache-schema.md` 更新：

- 当前 schema 版本为 4。
- `file_block_commits.historical_path` 含义。
- 项目快照版本与文件归因版本的区别。
- v3 → v4 迁移行为。
- 旧归因缓存失效、项目快照继续有效的规则。

### Step 8：定向验证

```powershell
cargo test -p revier-analysis --test cache_schema
cargo test -p revier-analysis --test cache_repository
cargo test -p revier-analysis --test index_schema
cargo test -p revier-tauri services::review::tests::file_attribution_is_persisted_then_restored_by_signature
cargo test -p revier-tauri services::review::tests::commit_overlay_is_cached_by_file_analysis_commit_and_current_encoding
git diff --check
```

节点 C 验收：

- v3 可事务升级到 v4。
- 项目快照保持命中。
- v1 文件归因不命中。
- v2 文件归因冷写入与热恢复一致。
- 历史路径和新 warning 不因缓存丢失。

完成后提交节点 C 结果供审核。

## 10. Task 6：使用历史路径生成提交下钻

本任务在节点 C 获得确认后执行。

**文件：**

- 修改：`src-tauri/src/services/review.rs`
- 修改：`src-tauri/src/commands/review.rs`，仅在适配请求字段确有需要时

### Step 1：新增 Tauri 服务失败测试

在 `review.rs` 测试模块新增：

1. 合法 `historical_path`
   - 当前文件经历重命名。
   - 请求携带来源提交边界路径。
   - 成功返回第一父 Overlay。

2. 不连通路径
   - 请求携带同仓库其他文件路径。
   - 后端不得直接信任。
   - 回退解析仍找不到时返回结构化错误。

3. 路径缺失兼容
   - 请求不携带历史路径。
   - 现有多次 rename 测试继续成功。

4. 第一父约束
   - 目标提交只相对第二父修改文件。
   - 即使传入路径也不能生成违反契约的 Overlay。

5. 根提交
   - 携带历史路径。
   - 旧侧继续使用空树。

先运行：

```powershell
cargo test -p revier-tauri services::review::tests::get_commit_overlay
```

确认新增测试失败。

### Step 2：验证请求历史路径

`get_commit_overlay_with_cache` 的路径解析顺序：

1. 获取任务当前文件。
2. 若请求包含 `historical_path`，确认它是仓库相对路径。
3. 使用目标提交第一父文件变化验证该路径或 rename 对端。
4. 验证它与任务当前文件的路径历史连通。
5. 成功时使用该变化。
6. 字段缺失或验证失败时运行现有 Git 历史路径解析。
7. 两条路径都失败时保留现有 `FILE_NOT_CHANGED_IN_COMMIT`。

要求：

- 不从 UI 接受父提交索引。
- 始终只选择 `parentIndex == 0`。
- 缓存命中仍核对父提交哈希和历史路径。
- 现有可达性、根提交、编码和二进制规则不变。

### Step 3：保存提交下钻缓存

- 冷路径把最终验证后的 `change.path` 保存到 `commit_overlays.historical_path`。
- 热路径校验缓存历史路径与当前提交对象仍一致。
- 请求字段只作为解析提示，不作为缓存真值。

### Step 4：定向验证

```powershell
cargo test -p revier-tauri services::review::tests::get_commit_overlay
cargo test -p revier-tauri services::review::tests::commit_overlay
cargo test -p revier-tauri
```

## 11. Task 7：前端传递历史路径

**文件：**

- 修改：`src/renderer/stores/reviewStore.ts`
- 修改：`tests/unit/rendererReviewStore.test.ts`
- 必要时修改：`tests/unit/revierClient.test.ts`

### Step 1：新增 Store 失败测试

断言：

- `loadCommitOverlay(filePath, hash, encoding, selectedCommit)` 请求包含 `selectedCommit.historicalPath`。
- `selectedCommit` 没有该字段时，请求不包含或传递 `undefined`。
- 编码重载使用当前 `selectedCommit.historicalPath`。
- 切换到另一个提交后不复用旧提交路径。
- 过期请求不能覆盖新提交状态。

运行：

```powershell
pnpm test -- tests/unit/rendererReviewStore.test.ts
```

预期因请求尚未传递字段而失败。

### Step 2：最小实现

- 首次下钻从方法参数 `selectedCommit` 读取 `historicalPath`。
- 编码重载从 Store 当前 `selectedCommit` 读取。
- 请求字段保持可选。
- 不增加新 Store 状态。
- 不修改组件布局；现有 `ReviewWorkspace` 已传递完整 `RelatedCommit` 时不改页面。

若现有页面没有传递完整提交对象，先暂停并报告需要修改的页面文件，不在本任务中擅自扩大范围。

### Step 3：验证

```powershell
pnpm test -- tests/unit/rendererReviewStore.test.ts tests/unit/revierClient.test.ts
pnpm typecheck
pnpm lint
```

## 12. Task 8：验证“所有相关提交均可下钻”不变量

**文件：**

- 修改：`src-tauri/src/services/review.rs`
- 必要时扩展：`crates/revier-analysis/tests/fixtures.rs`

### Step 1：新增端到端服务测试

为以下场景分别执行完整流程：

```text
完成项目分析
-> 打开文件并计算 Monaco 规范块归因
-> 枚举全部 relatedCommits
-> 使用 commitHash + historicalPath 请求提交下钻
-> 断言每个请求成功且 blocks 非空
```

覆盖：

- 线性新增和修改。
- 根提交。
- 多层 merge。
- Merge conflict resolution。
- 多父歧义来源。
- 删除。
- 多次重命名。
- 来源提交位于分析时间范围之前但可从 head 追溯。

### Step 2：冷缓存与热缓存各运行一次

- 第一次强制冷计算。
- 第二次恢复文件归因缓存。
- 两次枚举到的提交哈希、历史路径、归因方式和 merge 链路相同。
- 两次所有提交均可下钻。

### Step 3：运行定向测试

```powershell
cargo test -p revier-tauri services::review::tests::all_related_commits_are_drilldownable
cargo test -p revier-analysis --test overlay_attribution
cargo test -p revier-analysis --test cache_repository
```

节点 D 验收：

- 正常产生的相关提交不再触发 `FILE_NOT_CHANGED_IN_COMMIT`。
- 冷热缓存结果一致。
- 旧请求兼容。
- UI 无视觉变化。

## 13. Task 9：全量回归与验证记录

### Step 1：格式化

```powershell
cargo fmt --all
```

格式化只处理本功能涉及的 Rust 文件。若格式化波及无关用户改动，暂停并拆分处理。

### Step 2：全量 Rust 验证

```powershell
cargo fmt --all -- --check
cargo test --workspace --all-features
```

### Step 3：全量前端验证

```powershell
pnpm test
pnpm typecheck
pnpm lint
```

### Step 4：绑定幂等与差异检查

```powershell
pnpm generate:bindings
git diff --check
git status --short
```

未经 Git 提交授权时，不使用要求生成文件相对 HEAD 无差异的 `pnpm generate:bindings:check`。获得提交授权并完成对应提交后，再运行：

```powershell
pnpm generate:bindings:check
```

### Step 5：验证记录

新增：

```text
docs/superpowers/verification/2026-07-27-revier-attribution-drilldown-consistency.md
```

记录：

- 根因与最终修复语义。
- 新增 fixture 提交图。
- 递归 merge、第一父校验和历史路径结果。
- schema v4 迁移结果。
- 项目快照命中与文件归因失效结果。
- 冷热缓存一致性。
- 全相关提交下钻枚举结果。
- 定向与全量测试命令、数量和耗时。
- 未解决限制。

### Step 6：最终审核

提交：

- 文件变更清单。
- 关键实现说明。
- 测试结果。
- 迁移和兼容说明。
- 未提交的工作树状态。

未经最终审核，不扩大功能范围。

## 14. 计划中的 Git 提交边界

仅在用户明确要求执行提交后使用：

```text
docs(review): 规划归因与提交下钻一致性修复
test(attribution): 覆盖不可下钻的 merge 来源
fix(attribution): 递归解析可下钻的真实来源
feat(cache): 保存相关提交历史路径
fix(review): 使用归因路径加载提交下钻
docs(review): 记录归因下钻一致性验证结果
```

每次提交前：

```powershell
git status --short
git diff --check
git diff --name-only
```

只暂存对应任务文件，不使用 `git add .`。

## 15. 完成定义

只有同时满足以下条件才算完成：

- Task 1 的全部红灯用例已由实现转绿。
- Merge 来源递归到真正修改文件的提交。
- 仅继承内容的提交不再出现在相关提交列表。
- Patch 推断不会发布只相对后续父修改文件的 merge 提交。
- 所有相关提交通过第一父来源校验。
- `historicalPath` 在分析输出、公共契约、缓存、请求和下钻间完整传递。
- schema v3 可事务升级到 v4。
- 项目快照缓存保持有效，旧文件归因缓存自动失效。
- 冷计算和热缓存的相关提交完全一致。
- 枚举全部相关提交时，每个提交均能成功打开下钻。
- 既有测试业务断言未被擅自修改。
- Rust 全量测试、前端全量测试、类型检查、lint 和格式检查通过。
- UI 布局、样式和交互没有未确认变化。
- 验证文档已完成并提交用户审核。
