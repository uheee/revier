# Revier 共享 DuckDB 实例与渐进式文件归因实施计划

## 1. 实施依据

本计划对应：

`docs/superpowers/specs/2026-07-23-revier-shared-duckdb-progressive-attribution-design.md`

实施遵循以下规则：

- 设计文档获得确认后才开始业务代码修改。
- 使用 TDD：先新增失败测试，再实现最小代码，再运行定向验证。
- 不擅自改变既有测试语义；如必须调整，先取得书面确认。
- 关键节点完成后提交结果，获得确认再进入下一节点。
- 所有文档、回答和代码注释使用中文。
- 未经明确要求，不执行 Git 提交、推送或发布。
- 如后续获准提交，提交消息使用语义化提交格式。

## 2. 预计改动范围

### 2.1 Rust 分析库

- `crates/revier-analysis/src/index/connection.rs`
- `crates/revier-analysis/src/commands/index_build.rs`
- `crates/revier-analysis/src/attribution/context.rs`
- `crates/revier-analysis/src/attribution/patch_inference.rs`
- `crates/revier-analysis/src/attribution/blame.rs`
- `crates/revier-analysis/src/attribution/deletion_trace.rs`
- 必要时新增 `crates/revier-analysis/src/attribution/pipeline.rs`
- `crates/revier-analysis/src/attribution/mod.rs`
- `crates/revier-analysis/src/contracts.rs`
- `crates/revier-analysis/src/bindings.rs`
- 新增或扩展 `crates/revier-analysis/tests/*`

### 2.2 Tauri

- `src-tauri/src/services/review.rs`
- `src-tauri/src/commands/review.rs`
- 如构造方式需要显式注入，修改 `src-tauri/src/state.rs`、`src-tauri/src/lib.rs`

### 2.3 Vue

- `src/renderer/generated/bindings.ts`
- `src/renderer/stores/reviewStore.ts`
- `src/renderer/pages/ReviewWorkspace.vue`
- `src/renderer/components/review/DiffViewer.vue`
- 必要时调整 `src/renderer/components/review/DiffAuthorRail.vue`
- 对应 `tests/unit/*`

### 2.4 文档

- 本设计文档与实施计划
- 完成后新增验证记录
- 若用户可见行为需要补充，更新 `README.md`

## 3. 阶段一：共享 DuckDB 数据库实例

### 3.1 先写失败测试

新增测试证明：

- 同一 `DatabaseRegistry` 对相同路径的多次连接可同时存在。
- 一个克隆连接读取时，另一个克隆连接可以继续读取。
- 一个克隆连接完成事务写入后，其他克隆连接可观察结果。
- 不同数据库路径拥有不同根实例。
- `try_clone()` 失败不得回退为独立打开。

Windows 定向测试应覆盖原始错误场景；跨平台测试只断言共享连接行为，不绑定平台错误文本。

### 3.2 实现注册表

- 在分析库连接模块新增 `DatabaseRegistry`。
- 数据库路径转换为稳定绝对路径键。
- 首次打开保存根连接并返回克隆连接。
- 后续连接只执行 `try_clone()`。
- 连接注册表互斥锁不跨 SQL 查询持有。

### 3.3 接入分析入口

- 为 `index_build` 增加接受已有连接的内部入口。
- 为 `AttributionContext` 增加接受已有连接的入口。
- 保留 CLI 原入口和参数兼容。
- `ReviewService` 持有单个注册表实例。
- 将所有 Tauri 缓存、索引、归因和下钻路径迁移到注册表。

### 3.4 静态核查

在 Tauri 业务路径中搜索：

```powershell
rg -n "open_database|Connection::open" src-tauri crates/revier-analysis/src
```

允许保留的直接打开仅限：

- 注册表首次打开实现；
- 独立 CLI 包装入口；
- 明确隔离的测试辅助代码。

### 3.5 节点验证

- 数据库注册表定向测试。
- ReviewService 缓存恢复、项目分析、文件归因、下钻测试。
- Windows 上复现“旧连接存活时打开同一数据库”的等价场景，确认共享入口通过。

完成后提交阶段一结果供审核。

## 4. 阶段二：文件归因协作取消

### 4.1 先写失败测试

- 文件 A 归因运行时开始文件 B，A 的令牌被取消。
- A 的后台任务退出后不会移除 B 的令牌。
- 同一文件的新 operation ID 也替代旧操作。
- A 取消后不调用缓存替换。
- 取消报告为 `cancelled`，不报告为 `failed`。
- 前端不为被替代操作弹出错误通知。

### 4.2 后端取消注册

- 在 `ReviewService` 增加按任务保存的活动归因令牌。
- 文件打开开始时取消旧归因。
- 文件归因开始时注册新令牌。
- 使用 operation ID 条件清理。
- 将令牌注入 `AnalysisExecutionContext`。

### 4.3 算法取消检查

在以下边界调用 `check_cancelled()`：

- 路径历史完成后；
- 每个范围提交处理前；
- 每个规范块处理前后；
- 构建持久化模型前；
- `replace_file_analysis` 前。

不通过线程强杀或不安全 FFI 中断实现取消。

### 4.4 节点验证

- 分析库取消测试。
- ReviewService 并发替代测试。
- 取消后的旧缓存保持测试。

完成后提交阶段二结果供审核。

## 5. 阶段三：块级归因流水线与事件

### 5.1 契约测试

先新增测试覆盖：

- `BlockAttributionProgressSnapshot` 使用 camelCase 生成 TypeScript。
- 字段包含 operation、task、路径、签名、编码、完成量、总量和单块归因。
- 事件序列中的完成量从 1 单调递增到 N。
- `totalUnits` 始终等于规范块数量。

### 5.2 重构归因流水线

- patch inference 继续一次扫描全部范围提交。
- 抽取可跨块复用的 blame/merge trace 状态。
- 按原始块顺序完成 blame 和删除追踪。
- 每个块达到最终状态后调用完成回调。
- 保持最终完整结果顺序、作者、相关提交、触达范围和置信度不变。

新增等价测试，使用现有重命名、merge、删除和编码 fixture 比较改造前确认结果。

### 5.3 操作进度

- 候选扫描报告提交 `x/y`。
- 块处理报告 `完成 x/N`。
- 阶段变化立即发送；同阶段普通事件沿用最高 10 Hz。
- 最后一个块及终态必须立即发送，不得被限频丢弃。

### 5.4 块事件

- Tauri 命令接收 `AppHandle` 并创建块事件 sink。
- 后台每完成一个块发送 `review://block-attribution-progress`。
- 事件发送失败不终止计算。
- 缓存命中不发送渐进事件，直接返回完整结果。

### 5.5 原子缓存发布

- 所有块完成后再次检查取消。
- 构建完整 `CachedFileAnalysis`。
- 单次调用 `replace_file_analysis`。
- 任一块失败、取消或缓存发布失败时不产生新半成品。

### 5.6 节点验证

- 契约和绑定生成测试。
- 算法结果等价测试。
- 块回调顺序与计数测试。
- 事件发送与终态测试。
- 缓存命中、失败和取消回归。

完成后提交阶段三结果供审核。

## 6. 阶段四：前端增量作者渲染

### 6.1 先写失败测试

- 当前 operation 的单块事件只更新目标块。
- 作者轨在完整命令结束前显示该块作者。
- 未完成块继续显示“加载中”。
- operation ID、task ID、文件、签名或编码不匹配时忽略事件。
- 当前选中块更新后保持选中。
- 最终命令结果覆盖并确认完整结果。

### 6.2 事件订阅

- 在工作台生命周期中订阅块归因事件。
- 卸载时解除订阅。
- 将事件交给 `ReviewStore` 统一校验和合并。
- 不在组件内重复实现并发判断。

### 6.3 Store 合并

- 根据块 ID 定位目标块。
- 只替换 authors、relatedCommits 和 attribution。
- 使用新 blocks 数组触发 Vue 更新。
- 同步当前 selectedBlock 引用。
- 保持 `diffComputationState='attributing'`，直到最终请求成功。

### 6.4 状态栏

复用现有 `OperationStatusBar`，不新增第二套进度组件。确认：

- 候选阶段显示提交计数。
- 块阶段显示进度条和 `x/N`。
- 消息为“正在归因：完成 x/N”。

### 6.5 节点验证

- ReviewStore 定向测试。
- 工作台事件订阅测试。
- 作者轨增量显示测试。
- 操作状态栏计数与进度条测试。

完成后提交阶段四结果供审核。

## 7. 阶段五：Monaco 位置保持

### 7.1 先写失败测试

- 同一文件仅更新 blocks 时 surface context key 不变。
- 同一文件归因完成时不销毁 Monaco session。
- 光标、滚动位置和选区保持。
- 当前选中块引用更新但块 ID 不变。
- 文件路径、正文或 context key 变化时仍重建。

### 7.2 调整会话身份监听

- `DiffViewer` 不监听完整 `overlay` 引用作为重置条件。
- 仅监听文件路径、old/new 正文和 context key。
- warnings、authors、related commits、attribution 和 blocks 更新不递增 session version。
- 继续通过 `MonacoDiffSurface` 的 `setBlocks()`、`setSelectedBlock()` 更新装饰与选择。

### 7.3 节点验证

- `DiffViewer`、`MonacoDiffSurface` 和 Monaco session 定向测试。
- 工作台完成归因后的 context key 回归测试。
- 手动验证滚动到文件中部后完成归因，位置不跳动。

完成后提交阶段五结果供审核。

## 8. 阶段六：全量回归与文档收尾

### 8.1 自动化验证

按项目既有命令执行：

```powershell
cargo test --workspace
fnm exec --using-file pnpm.CMD generate:bindings:check
fnm exec --using-file pnpm.CMD typecheck
fnm exec --using-file pnpm.CMD test
fnm exec --using-file pnpm.CMD lint
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
git diff --check
```

若实际脚本名称或环境不匹配，先报告，不擅自替换验证流程。

### 8.2 手动验收矩阵

| 场景 | 预期 |
| --- | --- |
| A 未归因完成时点击 B | 无 DuckDB 锁错误，A 被取消，B 正常加载 |
| 大文件候选扫描 | 显示提交扫描计数和持续耗时 |
| 多块文件冷归因 | 显示 `完成 x/N`，作者逐块出现 |
| 文件归因缓存命中 | 快速返回完整作者，显示缓存命中 |
| 归因期间保持滚动 | 完成后位置、光标、选区不变化 |
| 单文件刷新中切换 | 刷新取消，旧缓存保留 |
| 归因失败 | 不发布半成品，终态明确失败 |

### 8.3 文档

- 新增阶段验证记录，包含命令、退出状态和结果摘要。
- 更新 README 中的文件归因进度、切换取消和位置保持行为。
- 明确数据库仍为 Schema v3，无迁移。

### 8.4 最终成果

提交以下内容供最终审核：

- 改动文件清单；
- 共享数据库实例说明；
- 取消与事件时序说明；
- 自动化测试结果；
- 手动复现场景结果；
- 已知限制。

## 9. 建议的语义化提交拆分

仅在后续获得提交授权时使用：

1. `fix(storage): 复用每仓库 DuckDB 数据库实例`
2. `feat(review): 支持取消被替代的文件归因`
3. `feat(progress): 增加块级归因进度与增量结果`
4. `fix(editor): 归因更新时保持 Monaco 浏览位置`
5. `docs(review): 记录渐进式归因验证结果`
