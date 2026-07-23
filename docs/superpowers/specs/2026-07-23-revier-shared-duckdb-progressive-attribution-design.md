# Revier 共享 DuckDB 实例与渐进式文件归因设计

## 1. 文档状态

- 日期：2026-07-23
- 状态：待审核
- 适用范围：Tauri 桌面应用中的 DuckDB 生命周期、文件归因取消、块级进度、作者轨增量渲染与 Monaco 状态保持
- 不在范围内：数据库 Schema 迁移、归因算法语义调整、SQLite 迁移、项目分析交互重设计、提交下钻交互重设计

## 2. 背景与问题

当前每次访问索引或缓存都会调用 `duckdb::Connection::open(path)`。`duckdb-rs` 的该调用会创建新的 DuckDB 数据库实例，而不是连接到应用中已经存在的实例。在 Windows 上，旧文件归因仍持有数据库实例时，用户点击另一个文件会再次打开同一数据库文件，从而出现同进程文件占用错误。

现有前端通过请求序号忽略迟到结果，但不会取消后台 `spawn_blocking` 归因任务。旧任务会继续扫描 Git 历史、持有 DuckDB 连接，并可能在完成后写入旧文件的归因缓存。

文件归因目前一次性返回全部块。状态栏虽然已经支持 `completedUnits`、`totalUnits` 和进度条，但归因路径没有上报可靠计数；作者轨也只能在全部归因完成后一次性显示作者。

归因结果合并会替换整个 `overlay` 对象。`DiffViewer` 监听对象变化并重建 Monaco 会话，因此作者信息更新会把滚动位置、光标和选区重置。

## 3. 已确认目标

1. 每个仓库数据库文件只保留一个 DuckDB 数据库实例。
2. 各业务操作从同一数据库实例创建独立逻辑连接，不用单一互斥连接串行全部操作。
3. 选择新文件时取消上一个仍在运行的文件归因。
4. 候选提交扫描显示可确定的提交进度。
5. 块归因显示“完成 4/60”一类进度和进度条。
6. 每个块得到最终归因后立即渲染作者信息。
7. 只有全部块成功完成后才原子发布文件归因缓存。
8. 归因结果更新不得重建 Monaco，不改变用户当前滚动位置、光标和选区。
9. 保留 DuckDB，不迁移到 SQLite。

## 4. 技术选型

### 4.1 DuckDB 实例模型

采用“每数据库路径一个根连接，业务操作使用 `try_clone()`”的模型。

`duckdb-rs 1.10504.0` 的 `Connection::try_clone()` 会共享底层 `duckdb_database`，同时创建独立 `duckdb_connection`。这与重复调用 `Connection::open()` 不同：前者属于同一数据库实例，后者会创建相互竞争文件锁的独立实例。

不采用以下方案：

- 全应用只有一个连接：不同仓库对应不同数据库文件，无法由单个连接直接覆盖；一个全局互斥连接也会让耗时查询阻塞无关读取。
- 归因改用只读实例：归因完成后需要原子写入文件缓存，而且独立只读实例仍可能与已有读写实例发生配置或文件锁冲突。
- 依赖操作系统共享文件锁：DuckDB 的数据库实例、事务和 WAL 管理由数据库引擎负责，业务层不应绕过该模型管理文件锁。

### 4.2 DuckDB 与 SQLite

继续使用 DuckDB。当前数据库同时承担提交索引、范围筛选、文件变化关系和规范化缓存，具有批量写入、范围扫描和分析查询特征。SQLite 更适合大量短事务、点查询和多进程 WAL 场景，但迁移不能解决 Git Diff、blame、删除追踪耗时，也不能替代正确的连接生命周期管理。

本次不改变 Schema v3，不新增迁移。

### 4.3 渐进式传输

保留现有 `review_attribute_blocks` 请求作为整项操作和最终结果来源，同时新增块级事件。状态进度继续复用 `review://operation-progress`；块作者数据使用新的 `review://block-attribution-progress` 事件传输。

不把每个块拆成独立前端命令，因为 patch 候选扫描应在同一文件内共享，逐块请求会重复扫描提交历史并显著放大成本。

## 5. 总体架构

```text
ReviewStore
  │ 选择文件 / 规范块
  ▼
Tauri review 命令
  │ 注册 operation_id 与取消令牌
  ├──────────────► review://operation-progress
  │
  ▼
ReviewService
  ├─ DatabaseRegistry ─► 每路径一个根 Connection
  │                         └─ try_clone() 给当前操作
  │
  └─ AttributionPipeline
       ├─ 公共候选提交扫描
       ├─ 逐块 blame / merge trace / 删除追踪
       ├──────────────► review://block-attribution-progress
       └─ 全部成功后单事务替换文件缓存

ReviewStore 收到块事件
  └─ 校验 operation_id、路径、签名、编码
       └─ 原位合并 overlay.blocks
            └─ MonacoDiffSession.setBlocks()
                 └─ 不重建编辑器
```

## 6. 模块划分

### 6.1 分析库数据库注册表

在 `crates/revier-analysis/src/index/connection.rs` 增加 `DatabaseRegistry`：

```rust
pub struct DatabaseRegistry {
    roots: Mutex<HashMap<PathBuf, duckdb::Connection>>,
}

impl DatabaseRegistry {
    pub fn connect(&self, path: &Path) -> Result<duckdb::Connection, AppError>;
}
```

职责：

- 以规范化绝对数据库路径为键。
- 首次访问时创建根连接并保存在注册表中。
- 后续访问调用根连接的 `try_clone()`。
- 只在查找或克隆根连接时持有注册表互斥锁，不在查询期间持锁。
- 根连接随 `ReviewService` 生命周期保留，应用退出时自然释放。

独立 CLI 保留原有临时 `Connection::open()` 行为，因为单次 CLI 进程内没有 Tauri 多操作生命周期。Tauri 业务路径必须统一使用注册表。

### 6.2 ReviewService 数据库依赖

`ReviewService` 持有共享 `DatabaseRegistry`。以下路径改为从注册表获取连接：

- 分支缓存状态读取；
- 分支缓存恢复；
- 最后选中文件写入；
- 项目增量索引与快照发布；
- 文件内容及文件缓存读取；
- 文件归因索引读取与缓存替换；
- 提交下钻缓存读取与写入。

`index_build` 增加接受已有连接的内部入口，现有 CLI 入口继续负责自行打开数据库。`AttributionContext` 增加接受已有连接的入口，Tauri 不再让它自行打开同一路径。

### 6.3 文件操作取消管理

`ReviewService` 增加按任务保存的活动文件归因注册：

```text
task_id -> operation_id + CancellationToken
```

行为：

1. `review_attribute_blocks` 开始时注册当前归因令牌。
2. 同一任务开始新的文件打开操作时，取消旧归因令牌。
3. 新归因注册时也取消同任务中尚未退出的旧归因。
4. 归因终止后仅在 operation ID 仍匹配时移除注册，避免旧任务删除新令牌。
5. 取消属于正常终态，后端报告 `cancelled`，不报告 `failed`。

`AnalysisExecutionContext` 继续承载取消检查。候选提交循环、块循环、缓存发布前必须检查取消；耗时的内部循环在合理边界补充检查。取消后不得调用 `replace_file_analysis`。

本次不承诺强制终止正在执行的单次 Git 或 DuckDB FFI 调用；取消在下一协作检查点生效。

### 6.4 归因流水线

新增分析库级文件归因流水线，保持现有算法顺序与结果语义：

1. 从范围提交建立路径历史和候选路径。
2. 扫描范围提交，统一计算 patch inference 候选及触达范围。
3. 对每个规范块依次执行 blame 和 merge trace。
4. 对具有旧侧删除的块执行删除追踪。
5. 当前块得到最终作者、相关提交和置信度后调用完成回调。
6. 所有块成功后返回完整有序结果。

公共候选扫描只执行一次。块级处理保持原始块顺序，因此完成计数稳定递增。共享的 merge trace 文本缓存继续跨块复用。

若某块处理失败，整项文件归因失败；已经发给前端的临时块结果不写入数据库。当前文件仍可显示已收到的作者，但操作终态明确标记失败。再次打开或刷新时重新计算完整结果。

## 7. 数据流程

### 7.1 正常冷归因

```text
Monaco 生成 N 个规范块
  -> review_attribute_blocks
  -> 缓存签名未命中
  -> 候选扫描：operation-progress 显示 提交 x/y
  -> 块 1 完成：
       operation-progress 显示 完成 1/N
       block-attribution-progress 携带块 1 结果
  -> ReviewStore 合并块 1，作者轨立即更新
  -> ...
  -> 块 N 完成
  -> 再次检查取消
  -> 单事务 replace_file_analysis
  -> 命令返回完整结果
  -> 操作 completed
```

### 7.2 缓存命中

签名、编码和算法版本均匹配时，直接返回全部缓存结果。由于缓存读取是短路径，不逐块发送事件；状态栏直接完成并显示缓存命中。

### 7.3 切换文件

```text
文件 A 正在归因
  -> 用户点击文件 B
  -> B 的文件打开操作取消 A 的令牌
  -> A 在下一个检查点退出，不发布缓存
  -> A 的迟到进度和块事件被 operation_id 校验丢弃
  -> B 使用同一 DuckDB 数据库实例的克隆连接继续读取
```

### 7.4 文件刷新

显式刷新沿用 `cache_mode=refresh`。若刷新期间被新文件替代，旧归因取消并保留刷新前缓存。只有完整成功后才替换缓存并使该文件提交下钻失效。

## 8. 接口规范

### 8.1 操作进度

继续使用 `OperationProgressSnapshot`：

- `attribute-candidates`：`completedUnits/totalUnits` 表示已扫描提交数／范围提交总数。
- `blame-ranges` 或 `trace-deletions`：表示已完成最终归因块数／块总数。
- 块阶段消息统一为 `正在归因：完成 {completed}/{total}`。
- 完成、失败、取消沿用现有终态规则。

### 8.2 块归因事件

新增契约：

```rust
pub struct BlockAttributionProgressSnapshot {
    pub operation_id: String,
    pub task_id: TaskId,
    pub file_path: String,
    pub block_signature: String,
    pub resolved_encoding: ResolvedTextEncoding,
    pub completed_units: u64,
    pub total_units: u64,
    pub attribution: DiffBlockAttribution,
}
```

事件名：

```text
review://block-attribution-progress
```

前端只有同时满足以下条件才合并：

- `operation_id` 等于当前文件操作；
- `task_id` 等于当前任务；
- `file_path` 等于当前 overlay 文件；
- `block_signature` 等于当前 Monaco 规范块签名；
- `resolved_encoding` 等于当前解析编码；
- 事件中的块 ID 存在于当前块集合。

### 8.3 最终命令结果

`AttributeBlocksResult` 保持现有字段，不改变前端最终一致性逻辑。即使块事件全部到达，命令最终结果仍作为完整确认，并负责合并 warnings 与 cache state。

## 9. 前端状态设计

### 9.1 ReviewStore

增加块事件处理方法：

- 校验事件上下文；
- 只替换匹配 ID 的单个块归因字段；
- 保留块坐标和 Monaco 生成的变化类型；
- 若当前选中该块，同步更新 `selectedBlock` 引用；
- 不把文件标记为 `ready`，直到最终命令成功；
- 迟到事件无副作用。

### 9.2 作者轨

`DiffAuthorRail` 继续根据 `overlay.blocks` 渲染。块事件合并后，已完成块立即显示作者；未完成块继续显示“加载中”。无需新增作者轨内部状态。

### 9.3 Monaco 状态保持

`DiffViewer` 不再监听整个 `overlay` 对象决定是否重置。会话身份只由以下值决定：

- 文件路径；
- old/new 正文；
- 工作台传入的 context key。

块、作者、相关提交、置信度和 warnings 变化只通过现有 `session.setBlocks()` 更新。只要会话身份未变化，就保留：

- 原始侧与修改侧滚动位置；
- 光标位置；
- 选区；
- 当前折叠和可见区域；
- 当前选中块。

切换文件、正文实际改变或编码重载导致正文改变时仍创建新会话。

## 10. 并发、事务与一致性

- 根连接注册表只保护实例创建和连接克隆，不序列化业务查询。
- 所有克隆连接共享同一 DuckDB 数据库实例，由 DuckDB 管理同实例事务并发。
- 文件归因计算阶段不持有缓存写事务。
- 完整结果产生后才开启并提交缓存替换事务。
- 取消、失败或进程退出不会发布半成品。
- 部分块事件只是当前界面内存状态，不构成持久化事实。
- 新操作 ID 具有前台优先级，旧事件不得覆盖新文件状态。

## 11. 错误处理

- 数据库根实例首次打开失败：沿用稳定 `DuckDb` 错误并终止当前操作。
- `try_clone()` 失败：终止当前操作，不回退为新的独立 `Connection::open()`，避免重新引入锁冲突。
- 归因取消：返回取消错误，操作状态为 `cancelled`，不弹出“归因失败”通知。
- 块事件发送失败：不影响后台完整计算和最终命令结果；最终结果仍能恢复一致状态。
- 最终缓存发布失败：操作失败，界面可保留本次内存归因，但数据库继续保留上一个成功缓存。

## 12. 开发环境

- Shell：PowerShell。
- Rust：workspace Cargo 工具链。
- Node：通过 `fnm` 使用 `.node-version`，包管理器为 `pnpm`。
- Python：本次不需要；如后续需要辅助脚本，统一使用 `uv`。
- 不新增第三方依赖。

调试时继续使用：

```text
REVIER_TRACE_OPERATIONS=1
```

新增诊断字段应包含 operation ID、数据库路径摘要、阶段、完成量、总量和取消状态，不记录源码正文。

## 13. 测试策略

### 13.1 Rust 分析库

- 同一路径多次 `connect()` 返回共享同一底层实例的可用连接。
- 根连接保留期间，克隆连接可并发读取并顺序事务写入。
- Tauri 路径不得回退为第二个独立实例。
- 候选提交只扫描一次。
- 冷归因按块顺序发送 N 次最终块回调。
- 回调结果合并后与一次性归因结果完全一致。
- 取消发生在候选阶段或块阶段时停止后续处理。
- 取消后不调用缓存发布。

### 13.2 Tauri

- 文件 B 开始时取消文件 A 的归因。
- 旧任务退出时不会删除文件 B 的取消令牌。
- 取消终态不报告为失败。
- 候选和块进度携带可靠计数。
- 块事件包含完整上下文字段。
- 缓存命中仍走短路径并返回完整结果。

### 13.3 前端

- `4/60` 显示为状态栏文本并渲染进度条。
- 当前操作的块事件立即合并作者。
- 路径、签名、编码或操作 ID 不匹配的事件被忽略。
- 已选中块在归因更新后保持选中并获得最新作者。
- 仅更新块归因不改变 Monaco surface key。
- 归因完成不重建编辑器，不重置光标和滚动位置。
- 切换文件或正文变化仍重建编辑器。

不修改既有测试的业务语义；如果实施中发现必须调整既有断言，先单独提交原断言、拟调整内容和原因供确认。

## 14. 验收标准

- 文件 A 未归因完成时点击文件 B，不再出现同进程 DuckDB 文件占用错误。
- 文件 A 的后台归因在协作取消点停止，且不发布缓存。
- 同一仓库的 Tauri 数据库访问均来自同一根实例。
- 候选扫描期间状态栏有提交计数；块阶段显示准确的 `完成 x/N`。
- 已完成块的作者轨在整个文件完成前可见。
- 最终归因结果与改造前算法结果一致。
- 归因块更新及最终完成不改变用户的编辑器位置。
- 缓存命中、刷新、失败和取消行为保持一致。
- 所有新增测试和既有回归通过。

## 15. 风险与控制

| 风险 | 影响 | 控制 |
| --- | --- | --- |
| 某条 Tauri 路径仍直接 open | Windows 锁错误仍可复现 | 集中连接入口、静态搜索和覆盖各命令测试 |
| 取消检查粒度过粗 | 切换后旧任务仍运行较久 | 在提交循环、块循环和发布前检查 |
| 旧任务清理新令牌 | 新文件无法取消或状态错误 | 移除时同时比较 operation ID |
| 块事件乱序或迟到 | 作者显示到错误文件 | 五项上下文联合校验 |
| 渐进结果与最终结果不一致 | UI 闪烁或错误 | 最终结果作为权威确认，增加等价测试 |
| overlay 更新仍触发重建 | 位置保持失效 | 会话身份与数据更新分离，断言 surface key 不变 |
| 单次 FFI 调用不可立即取消 | 取消存在延迟 | 明确协作取消边界，不伪造即时终止 |
