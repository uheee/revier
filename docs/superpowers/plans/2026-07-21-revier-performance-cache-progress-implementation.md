# Revier 性能、持久化缓存与进度状态实施计划

## 1. 实施原则

本计划对应 `docs/superpowers/specs/2026-07-21-revier-performance-cache-progress-design.md`。实施必须遵循：

- 先完成契约与数据库迁移，再接入缓存恢复和性能路径。
- 每个任务使用 TDD：先新增失败测试，再实现最小代码，再运行相关测试。
- 不擅自修改既有测试逻辑。若现有测试预期因接口契约变化必须调整，先列出文件、原断言、拟修改断言和原因，获得用户书面确认后执行。
- 每个关键阶段完成后提交验证结果，获得确认后进入下一阶段。
- 所有文档、错误信息、代码注释使用中文。
- Git 提交使用语义化提交格式；未经用户明确要求，本计划不执行提交或推送。

## 2. 预计改动范围

### 2.1 Rust 分析库

- `crates/revier-analysis/src/contracts.rs`
- `crates/revier-analysis/src/execution.rs`
- `crates/revier-analysis/src/index/schema.rs`
- `crates/revier-analysis/src/index/migrations.rs`
- `crates/revier-analysis/src/index/writer.rs`
- `crates/revier-analysis/src/index/queries.rs`
- `crates/revier-analysis/src/git/blob.rs`
- `crates/revier-analysis/src/git/diff.rs`
- `crates/revier-analysis/src/git/blame.rs`
- `crates/revier-analysis/src/attribution/*`
- 新增 `crates/revier-analysis/src/cache/*`
- 新增或扩展 `crates/revier-analysis/tests/*`

### 2.2 Tauri

- `src-tauri/src/services/review.rs`
- `src-tauri/src/commands/review.rs`
- `src-tauri/src/state.rs`
- `src-tauri/src/lib.rs`
- 必要时新增 `src-tauri/src/services/operation_progress.rs`

### 2.3 Vue

- `src/renderer/generated/bindings.ts`
- `src/renderer/api/revierClient.ts`
- `src/renderer/stores/reviewStore.ts`
- `src/renderer/pages/ReviewWorkspace.vue`
- `src/renderer/components/review/FilterPanel.vue`
- `src/renderer/components/review/EditorStatusBar.vue`
- `src/renderer/components/review/DiffViewer.vue`
- `src/renderer/components/review/DiffDrilldownOverlay.vue`
- 新增或扩展 `tests/unit/*`

## 3. 阶段一：性能基线与可观测性契约

### 3.1 新增操作进度契约测试

新增测试覆盖：

- `OperationKind`、`OperationStatus`、`OperationStage`、`CacheState` 序列化为稳定 kebab-case。
- `OperationProgressSnapshot` 绑定生成到 TypeScript。
- `progress` 仅在存在可靠总量时提供。
- `elapsed_ms` 为非负整数，终态不再变化。

### 3.2 扩展执行上下文

将现有取消上下文扩展为可选进度报告器：

```rust
pub trait OperationProgressReporter: Send + Sync {
    fn report(&self, update: OperationProgressUpdate);
}
```

要求：

- 分析库只报告语义阶段和计数，不依赖 Tauri。
- `AnalysisExecutionContext` 同时承载取消检查和报告器。
- 无报告器时零副作用。

### 3.3 修正耗时采集

- 索引耗时必须在 DuckDB 事务提交及检查点完成后采集。
- 文件内容、Monaco、归因和下钻分别记录耗时。
- 增加开发日志开关，默认关闭详细日志。

### 3.4 节点验证

- Rust 契约测试。
- 绑定生成检查。
- 证明旧 `elapsed_ms` 不再漏掉数据库写入。

完成后提交阶段一结果供审核。

## 4. 阶段二：数据库迁移与缓存仓储

### 4.1 migration 测试

先新增测试：

- 空数据库初始化新 schema。
- schema 1 升级后保留 `commits`、`commit_parents`、`commit_files` 数据。
- migration 重复执行幂等。
- migration 中途失败不破坏原数据。

### 4.2 新增缓存表

按设计文档创建：

- `analysis_snapshots`
- `analysis_filter_authors`
- `analysis_filter_globs`
- `analysis_files`
- `file_analyses`
- `file_blocks`
- `file_block_commits`
- `file_block_commit_ranges`
- `file_block_merge_sources`
- `commit_overlays`
- `commit_overlay_blocks`

为以下查询建立索引：

- `analysis_snapshots(repo_id, branch)`
- `analysis_files(analysis_id, path)`
- `file_analyses(analysis_id, path)`
- `commit_overlays(file_analysis_id, commit_hash)`
- `commit_files(path, commit_hash)` 与旧路径等价查询所需组合索引。

### 4.3 CacheRepository

新增独立仓储模块，职责只包括：

- 读取分支快照与文件列表；
- 原子发布项目快照；
- 读取／替换文件分析；
- 读取／写入提交下钻；
- 计算缓存状态；
- 删除当前文件下钻；
- 清理不再引用的数据。

仓储不得调用 Git 或执行归因算法。

### 4.4 空间结构验证

- 断言数据库不保存 `old_content`、`new_content` 正文列。
- 相同提交关联多个块时，提交元数据只在 `commits` 表保存一次。
- 同一分支第二次成功发布后只存在一个快照。
- 不同分支分别存在一个快照。

### 4.5 节点验证

- migration 与 cache repository 测试。
- 使用测试仓库记录两分支缓存后的数据库表行数和文件大小。
- 提交阶段二结果供审核。

## 5. 阶段三：增量索引与项目分析性能

### 5.1 增量索引测试

构造包含分支、新提交、merge 和 force-move 引用的 fixture，验证：

- 已索引提交不重新执行 diff。
- 新增一个提交只计算该提交。
- 多分支共享提交不重复写入。
- 项目刷新后只清理不被任何本地分支或当前快照引用的提交。
- 取消或写入失败不产生半成品索引。

### 5.2 重构索引构建

- 提交可达集合只计算一次。
- 查询缺失哈希集合。
- 只为缺失提交生成提交和文件记录。
- 在单事务中批量写入。
- 更新 `index_runs` 的真实完成时间与耗时。
- 项目刷新成功后执行受控清理与检查点。

### 5.3 分离文件树差异与统计

- 新增不读取正文的范围文件树差异接口。
- 先执行索引筛选和 glob 过滤。
- 只对最终文件计算行数。
- 同时解析并保存 old/new blob ID。
- 一次项目分析内缓存范围哈希与树差异。

### 5.4 项目快照原子发布

- `ReviewService` 使用候选分析上下文。
- 完整成功后调用 `CacheRepository` 发布。
- 更新内存任务缓存，使当前会话与数据库快照一致。
- 失败或取消继续使用旧分支快照。

### 5.5 进度接入

至少报告：

- 仓库读取；
- 范围解析；
- 缺失提交索引；
- 索引记录写入；
- 文件筛选；
- 文件统计；
- 数据库发布。

### 5.6 节点验证

- 全量与增量索引测试。
- 比较 HEAD 新增一个提交前后的实际 diff 调用次数。
- 记录首次项目分析与增量刷新的分阶段耗时。
- 提交阶段三结果供审核。

## 6. 阶段四：分支缓存恢复与项目刷新交互

### 6.1 Tauri 命令

实现并测试：

- `review_restore_branch_analysis`
- `review_get_branch_cache_status`
- `review_start_analysis` 显式刷新语义和 `operation_id`
- `review://operation-progress` 事件限频、阶段变化直发和终态直发。

### 6.2 前端恢复流程

- 打开项目后加载保存分支的缓存。
- 缓存命中时恢复筛选、文件列表和仍有效的上次选中文件。
- 快速读取当前 HEAD 并标记 stale。
- 没有缓存时不自动分析。
- 分支切换时读取目标分支缓存；目标分支无缓存时显示空状态。

### 6.3 项目按钮

- 无缓存显示“分析项目”。
- 有缓存显示“重新分析项目”。
- 点击时使用当前筛选条件。
- stale 状态下仍可查看旧缓存，只有按钮触发刷新。
- 分析成功后关闭旧文件和下钻，再选择新文件列表首项；该自动选择行为须保持与当前交互一致。

### 6.4 节点验证

- 前端 Store 和页面测试。
- 手动验证应用重启后同一分支直接恢复。
- 手动验证两个分支互不覆盖。
- 手动验证 HEAD 变化只显示过期标记，不自动分析。
- 提交阶段四结果供审核。

## 7. 阶段五：目标文件快速路径与持久化归因

### 7.1 Git blob ID 快速读取

新增接口：

- 从树差异返回 old/new blob ID。
- 通过 blob ID 读取字节。
- blob 缺失返回稳定缓存失效错误。

测试证明打开单文件不会调用 `range_file_changes()`。

### 7.2 文件缓存读取

正常点击文件：

1. 从 `analysis_files` 读取目标记录。
2. 读取两个 Git blob。
3. 返回不可变内容和已有文件缓存摘要。
4. Monaco 生成块签名。
5. 签名命中时批量读取块归因并合并。
6. 未命中时进入归因计算。

### 7.3 候选提交归因

- 从索引按范围哈希与新旧路径筛选候选提交。
- 批量加载提交元数据。
- 同一候选提交文件差异只计算一次。
- 归因完成后原子替换文件缓存。

### 7.4 blame 与删除追踪批量化

- 合并相邻或重叠的新侧块范围。
- 所有 blame 调用共享资源缓存。
- 将 blame 行映射回 Monaco 块。
- 删除追踪按文件一次扫描全部候选提交和删除子范围。
- merge trace 使用当前操作内缓存。

### 7.5 当前文件刷新

- 新增 `refresh` cache mode。
- 项目 stale 时禁用并解释原因。
- 刷新成功后替换文件缓存并删除该文件下钻。
- 刷新失败或取消时保留旧文件缓存和旧下钻。

### 7.6 节点验证

- 首次与再次打开文件的调用计数和耗时对比。
- 多块、删除块、重命名和 merge fixture 归因等价测试。
- 当前文件刷新成功／失败／取消测试。
- 提交阶段五结果供审核。

## 8. 阶段六：提交下钻缓存

### 8.1 下钻持久化测试

- 首次点击计算并保存 blob ID、路径和块。
- 第二次点击不重新解析历史路径和提交差异。
- 不同提交独立保存。
- 文件刷新后下钻全部失效。
- 编码变化后同一下钻只保留当前编码版本。

### 8.2 下钻快速路径

- 校验下钻缓存属于当前 `file_analysis_id`。
- 读取 Git blob 内容并恢复块。
- 缓存未命中时执行现有可达性、历史路径和第一父提交逻辑。
- 成功后原子写入。

### 8.3 节点验证

- Rust 下钻缓存测试。
- 前端打开、关闭、再次打开测试。
- 记录首次与热缓存耗时。
- 提交阶段六结果供审核。

## 9. 阶段七：统一状态栏

### 9.1 ReviewStore 操作状态

新增状态：

- 当前前台操作快照；
- 最后终态快照；
- 本地耗时基准；
- 当前 operation ID；
- cache hit／miss／stale 状态。

旧请求不得覆盖新请求状态。

### 9.2 状态栏组件

扩展 `EditorStatusBar`：

- 左侧显示操作和对象。
- 中间显示进度条、数量与百分比。
- 显示持续更新的已用时间。
- 增加“重新分析当前文件”按钮。
- 右侧保留行列、编码和语言。
- 无文件时隐藏编辑器控件但保留操作状态。

### 9.3 Monaco 进度

Monaco Diff 在前端创建本地 `monaco-diff` 操作：

- Model 设置后进入 running。
- `getLineChanges()` 返回有效结果后 completed。
- 计算期间无可靠总量，只显示操作和耗时。
- 文件后端归因开始后切换为 `file-attribution`。

### 9.4 限频与无障碍

- 后端普通事件最高 10 Hz。
- 前端计时显示最高 10 Hz。
- `aria-live` 只在阶段变化和终态更新。
- 未知总量不渲染百分比进度条语义。

### 9.5 节点验证

- 状态格式、耗时、终态保留、并发优先级测试。
- 缓存命中耗时展示测试。
- 键盘、屏幕阅读器属性和窄宽度布局测试。
- 提交阶段七结果供审核。

## 10. 阶段八：回归、性能验收与文档

### 10.1 自动化验证

按项目规范使用：

```powershell
cargo test --workspace
fnm exec --using-file pnpm.CMD typecheck
fnm exec --using-file pnpm.CMD test
fnm exec --using-file pnpm.CMD lint
```

若命令名称或现有脚本与环境不一致，先报告，不擅自替换构建流程。

### 10.2 性能矩阵

对小型 fixture 与当前仓库分别记录：

| 场景 | 冷缓存 | 热缓存 | 必须记录的阶段 |
| --- | --- | --- | --- |
| 首次项目分析 | 是 | 不适用 | 范围、索引、文件、发布 |
| 新增一个提交后刷新 | 是 | 不适用 | 缺失提交、写入、文件 |
| 再次打开分支 | 否 | 是 | DB 读取、HEAD 校验、前端恢复 |
| 首次／再次打开文件 | 是 | 是 | blob、Monaco、归因、DB |
| 首次／再次下钻 | 是 | 是 | 路径、blob、Diff、DB |
| 当前文件刷新 | 是 | 不适用 | blob、Monaco、归因、替换 |

### 10.3 正确性回归

- 作者、相关提交和置信度与改造前 fixture 结果一致。
- 重命名、merge、删除块、根提交、二进制和多编码行为一致。
- stale 缓存不会混入新 HEAD。
- 取消和错误不会清除可用缓存。

### 10.4 文档

更新：

- README 中的缓存与手动刷新行为。
- 开发环境中的性能日志开关。
- schema 与 migration 说明。
- 新增验证记录，包含命令、退出状态、耗时和结果摘要。

### 10.5 最终审核

提交以下成果供用户审核：

- 改动文件清单。
- 数据库迁移说明。
- 自动化测试结果。
- 冷热缓存性能对比。
- 已知限制与后续建议。

未经用户确认，不执行 Git 提交、推送或发布。

## 11. 建议的语义化提交拆分

仅作为后续获得提交授权后的建议：

1. `feat(cache): 新增分支分析与文件归因缓存结构`
2. `perf(index): 将仓库索引改为增量事务写入`
3. `perf(review): 增加目标文件与批量归因快速路径`
4. `feat(review): 恢复分支缓存并支持手动刷新`
5. `feat(review): 持久化提交下钻缓存`
6. `feat(ui): 增加统一加载进度与耗时状态栏`
7. `docs(review): 记录缓存与性能验证结果`

