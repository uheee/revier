# Revier Monaco 单一 Diff 块边界实施计划

> **状态：** 已完成独立审核，待用户批准；批准前不得修改业务代码或测试。  
> **设计：** `../specs/2026-07-20-revier-monaco-canonical-diff-design.md`  
> **视觉：** `../specs/2026-07-20-revier-monaco-canonical-diff-visual.html`

**目标：** 让 Monaco 成为 Tauri 页面唯一的 Diff 显示块来源，使绿色/红色背景、选中边界、点击命中、
AuthorRail 和 Rust 归因严格使用同一范围，同时修复 Minimap 滑块透明度与 AuthorRail 交互外观。

**技术路线：** Monaco 公开 API 发布规范化范围；Tauri 首次只返回文本；Store 按会话代次批量请求
Rust 归因；Rust 校验范围并复用现有 patch inference、blame、merge trace、deletion trace；块更新使用
增量 API，不重建 Editor/Model。

**约束：** 使用 pwsh、fnm、Node 24、pnpm、Rust stable；全程 TDD；允许按此前书面确认修改现有测试；
不新增依赖、不修改 TOML 结构、不使用 Monaco 私有 DOM/API；所有文档、回答和代码注释使用中文；
每个任务使用语义化 Git 提交。

## 实施范围

### 新增文件

- `crates/revier-analysis/src/overlay/block_ranges.rs`
- `crates/revier-analysis/tests/overlay_block_ranges.rs`
- `src/renderer/editor/monacoDiffBlocks.ts`
- `tests/unit/monacoDiffBlocks.test.ts`
- `docs/superpowers/verification/2026-07-20-revier-monaco-canonical-diff.md`

### 主要修改文件

- `crates/revier-analysis/src/contracts.rs`
- `crates/revier-analysis/src/bindings.rs`
- `crates/revier-analysis/src/overlay/mod.rs`
- `crates/revier-analysis/src/overlay/file_overlay.rs`
- `src-tauri/src/commands/review.rs`
- `src-tauri/src/services/review.rs`
- `src-tauri/src/lib.rs`
- `src/renderer/generated/bindings.ts`
- `src/renderer/api/revierClient.ts`
- `src/renderer/editor/editorTheme.ts`
- `src/renderer/editor/monacoDiffSession.ts`
- `src/renderer/components/review/MonacoDiffSurface.vue`
- `src/renderer/components/review/DiffViewer.vue`
- `src/renderer/components/review/DiffAuthorRail.vue`
- `src/renderer/components/review/BlockDetailPanel.vue`
- `src/renderer/components/review/DiffDrilldownOverlay.vue`
- `src/renderer/stores/reviewStore.ts`
- `src/renderer/pages/ReviewWorkspace.vue`
- `src/renderer/styles.css`
- 对应的既有 Rust、Store、组件和 API 单元测试

实施中如发现必须新增依赖、变更 TOML、读取 Monaco 私有 API 或扩大到 Git 筛选逻辑，立即暂停并重新确认。

## Task 0：基线、工作区与工具链

### Step 1：确认审核门

- [ ] 用户批准设计与本计划。
- [ ] 记录用户已允许修改现有测试逻辑。
- [ ] 确认工作树中是否存在用户未提交改动；相关文件有重叠时先停止确认。

### Step 2：激活 Node 24

```powershell
& 'C:\Users\Snowind\AppData\Local\Microsoft\WinGet\Links\fnm.exe' env --use-on-cd --shell powershell | Out-String | Invoke-Expression
fnm use 24
node --version
pnpm --version
```

### Step 3：记录自动化基线

```powershell
cargo fmt --all -- --check
cargo test --workspace --all-features
pnpm test
pnpm typecheck
pnpm lint
pnpm generate:bindings:check
git diff --check
```

预期全部通过。如基线失败，不把既有失败混入本功能提交；先报告并确认处理方式。

## Task 1：显式修复 Minimap 与滚动条滑块主题

**文件：**

- 修改：`src/renderer/editor/editorTheme.ts`
- 修改：`src/renderer/editor/monacoDiffSession.ts`
- 修改：`tests/unit/editorTheme.test.ts`
- 修改：`tests/unit/monacoDiffSession.test.ts`

### Step 1：先写失败测试

- [ ] 主题颜色包含六个显式键：
  `minimapSlider.background/hoverBackground/activeBackground` 和
  `scrollbarSlider.background/hoverBackground/activeBackground`。
- [ ] 从 `theme.muted` 派生 `#RRGGBBAA`，普通、hover、拖动 alpha 严格递增。
- [ ] Minimap 分别为 16%、30%、45%；滚动条分别为 22%、36%、52%。
- [ ] Monaco options 显式包含 `minimap.showSlider = 'always'`。

### Step 2：运行失败测试

```powershell
pnpm test -- tests/unit/editorTheme.test.ts tests/unit/monacoDiffSession.test.ts
```

预期因颜色键和 `showSlider` 尚未实现而失败。

### Step 3：最小实现

- [ ] 提取仅接受合法 `#RRGGBB` 的 alpha 派生函数，复用已有主题校验前提。
- [ ] 同时覆盖 Minimap 和普通滚动条三态，禁止回落到 Monaco 不同来源的默认色。
- [ ] 显式设置 `showSlider: 'always'`，保留真实 Minimap。
- [ ] 不新增 TOML 字段。

### Step 4：验证并提交

```powershell
pnpm test -- tests/unit/editorTheme.test.ts tests/unit/monacoDiffSession.test.ts
pnpm typecheck
git diff --check
git add src/renderer/editor/editorTheme.ts src/renderer/editor/monacoDiffSession.ts tests/unit/editorTheme.test.ts tests/unit/monacoDiffSession.test.ts
git commit -m "fix(editor): 调整 Minimap 滑块透明状态"
```

## Task 2：定义范围归因共享契约与 Tauri Command

**文件：**

- 修改：`crates/revier-analysis/src/contracts.rs`
- 修改：`crates/revier-analysis/src/bindings.rs`
- 修改：`crates/revier-analysis/tests/editor_contract.rs`
- 修改：`src/renderer/api/revierClient.ts`
- 修改：`tests/unit/revierClient.test.ts`
- 生成：`src/renderer/generated/bindings.ts`

### Step 1：编写失败的契约测试

- [ ] 新增 `DiffBlockRange`，字段只有 ID、四个边界和 `DiffBlockChangeType`。
- [ ] 新增 `AttributeBlocksRequest`：任务、文件、必填具体 `resolvedEncoding`、完整范围数组。
- [ ] 新增 `DiffBlockAttribution`：只含 ID、作者、相关提交和可信度，不含边界、change type 或 rows。
- [ ] 新增 `AttributeBlocksResult`：回显具体编码、归因元数据和警告。
- [ ] `AttributionWarningCode` 新增 `EOF_NEWLINE_ATTRIBUTION_UNAVAILABLE`，并锁定 kebab/camel 映射。
- [ ] 验证 camelCase 序列化及 `u64 → TypeScript number` 绑定。
- [ ] 验证 EOF warning 的 Rust 序列化、Specta union 与 TypeScript 绑定。
- [ ] API 客户端必须调用 `review_attribute_blocks`，参数名保持 `{ request }`。

### Step 2：运行失败测试

```powershell
cargo test -p revier-analysis --test editor_contract
pnpm test -- tests/unit/revierClient.test.ts
```

### Step 3：实现最小契约

- [ ] 在 Specta 导出列表注册四种类型。
- [ ] `revierClient.review.attributeBlocks` 返回 `AttributeBlocksResult`。
- [ ] 此任务只建立可编译的共享类型和客户端调用；Tauri command 与 service 在 Task 4 同步落地，
  不提交临时 `unimplemented` 入口。

### Step 4：生成并校验 bindings

```powershell
pnpm generate:bindings
cargo test -p revier-analysis --test editor_contract
pnpm test -- tests/unit/revierClient.test.ts
pnpm typecheck
cargo check -p revier-tauri --all-targets
git diff --check
```

### Step 5：提交契约

```powershell
git add crates/revier-analysis/src/contracts.rs crates/revier-analysis/src/bindings.rs crates/revier-analysis/tests/editor_contract.rs src/renderer/api/revierClient.ts tests/unit/revierClient.test.ts src/renderer/generated/bindings.ts
git commit -m "feat(review): 定义 Monaco 范围归因契约"
pnpm generate:bindings:check
```

## Task 3：用 Monaco 范围构造并校验 Rust 归因块

**文件：**

- 新增：`crates/revier-analysis/src/overlay/block_ranges.rs`
- 新增：`crates/revier-analysis/tests/overlay_block_ranges.rs`
- 修改：`crates/revier-analysis/src/overlay/mod.rs`
- 修改：`crates/revier-analysis/src/attribution/merge_trace.rs`
- 修改：`crates/revier-analysis/src/attribution/deletion_trace.rs`

### Step 1：编写失败的纯函数测试

覆盖：

- [ ] 单个新增块：旧侧 `0..0`，新侧切片正确。
- [ ] 单个删除块：新侧 `0..0`，旧侧切片正确。
- [ ] 不等长修改块：按最大行数生成行数据和空侧占位。
- [ ] 保留传入 ID、顺序和 change type。
- [ ] `rowStartIndex/rowEndIndex` 为 `None`。
- [ ] 空 ID、重复 ID、两侧同时为空、0 与非 0 混合、逆序、越界被拒绝。
- [ ] change type 与空侧关系不一致被拒绝。
- [ ] 同侧重叠或非单调范围被拒绝。
- [ ] CRLF、末尾换行和空文件行数与 Monaco Model 语义一致。
- [ ] `""` 为一行，`"a\n"` 为两行；保留末尾空项和原始 `\r` 文本。

### Step 2：运行测试确认失败

```powershell
cargo test -p revier-analysis --test overlay_block_ranges
```

### Step 3：实现范围构造器

- [ ] 输入为已解码 old/new 文本和 `DiffBlockRange[]`。
- [ ] 先完整校验再构造，禁止部分成功。
- [ ] 按行范围切片并构造 `SideBySideDiffRowOutput`。
- [ ] 提取 Monaco 行计数 helper，并让 merge trace、deletion trace 的范围窗口复用该语义。
- [ ] `rows` 只服务归因算法，不恢复 Tauri 自绘行定位语义。
- [ ] 错误使用稳定错误码和中文信息。

### Step 4：运行 Rust 验证并提交

```powershell
cargo fmt --all -- --check
cargo test -p revier-analysis
cargo clippy -p revier-analysis --all-targets --all-features -- -D warnings
git diff --check
git add crates/revier-analysis/src/overlay/block_ranges.rs crates/revier-analysis/tests/overlay_block_ranges.rs crates/revier-analysis/src/overlay/mod.rs crates/revier-analysis/src/attribution/merge_trace.rs crates/revier-analysis/src/attribution/deletion_trace.rs
git commit -m "feat(analysis): 按编辑器范围构造归因块"
```

## Task 4：拆分文本加载与批量归因后端流程

**文件：**

- 修改：`crates/revier-analysis/src/overlay/file_overlay.rs`
- 修改：`crates/revier-analysis/src/api.rs`（仅在需要公开可复用文本加载入口时）
- 修改：`crates/revier-analysis/tests/file_overlay_cli.rs`
- 修改：`crates/revier-analysis/tests/overlay_attribution.rs`
- 修改：`crates/revier-analysis/tests/trace_block_cli.rs`
- 修改：`crates/revier-analysis/src/attribution/deletion_trace.rs`
- 修改：`src-tauri/src/services/review.rs`
- 修改：`src-tauri/src/commands/review.rs`
- 修改：`src-tauri/src/lib.rs`
- 修改：`src-tauri` 现有 service 测试

### Step 1：先写失败测试

- [ ] Tauri `get_file_overlay` 返回完整文本、编码和元数据，但 `rows=None`、`blocks=[]`。
- [ ] Tauri `get_commit_overlay` 同样返回 `rows=None`、`blocks=[]`，但保留 commit、parentHash、文本和编码。
- [ ] 第一阶段不调用 patch inference、blame 或 deletion trace。
- [ ] 两个 Tauri 第一阶段入口均不打开 `AttributionContext`，也不执行 range hash、path history、索引或
  deletion candidates 准备。
- [ ] 提交下钻第一阶段不调用 Rust LCS，也不预先附加旧块。
- [ ] CLI `file-overlay` 仍返回现有 Rust LCS rows/blocks，兼容测试不变。
- [ ] CLI `trace-block` 仍使用现有 Rust LCS 块，兼容测试不变。
- [ ] `attribute_blocks` 校验任务已完成、文件属于任务，并使用同一任务 base/head。
- [ ] 第二阶段只接受第一阶段返回的具体 `resolvedEncoding`，重读结果和回显编码不一致时拒绝归因。
- [ ] 使用解析后的 `change.path/change.old_path`，旧路径别名不得直接进入 blame/path history。
- [ ] 合法批量范围对父块执行 patch inference/blame，对内部删除子片段执行 deletion trace。
- [ ] 一个 Monaco 父块覆盖多个内部删除/修改片段时，多个提交按 hash 聚合并重新统计作者。
- [ ] 父块 patch/blame 与删除片段结果取并集；重复提交按既定 method 优先级、range/merge hash
  并集、matchedByFilter OR 和提交排序规则合并。
- [ ] Monaco 与旧 Rust 边界偏移一行、一个内部块跨两个父块时，返回 ID 仍严格对应父块。
- [ ] 返回只含 ID 和归因元数据，不含可覆盖 Monaco 的边界、change type 或 rows。
- [ ] Tauri warning adapter 识别 `EOF_NEWLINE_ATTRIBUTION_UNAVAILABLE`，EOF-only 适配不会报未知 code。
- [ ] 返回顺序和 ID 与请求完全一致，重复/缺失/未知 ID 被前端拒绝合并。
- [ ] 越界范围返回稳定错误，不能导致 panic 或部分块回写。
- [ ] 解码错误、二进制文件和 Git 读取错误保持原错误语义。

### Step 2：运行失败测试

```powershell
cargo test -p revier-analysis --test file_overlay_cli
cargo test -p revier-analysis --test overlay_attribution
cargo test -p revier-analysis --test trace_block_cli
cargo test -p revier-tauri
```

### Step 3：拆分文档加载与归因上下文准备

- [ ] `load_overlay_document` 只执行变更文件查找、真实路径规范化、Git Blob 读取、编码解析和二进制判定。
- [ ] `prepare_attribution_context` 才执行 range hash、path history、索引和 deletion candidates 准备。
- [ ] CLI 路径：加载文档 → 准备归因上下文 → Rust LCS → 现有归因链。
- [ ] Tauri 初始路径：只加载文档 → 直接返回文本，不执行 LCS、归因上下文或历史扫描。
- [ ] Tauri 范围路径：加载同一文档 → 准备归因上下文 → `block_ranges` 构造父块 → patch/blame →
  父块内部删除子分段。
- [ ] 不引入跨页面内存缓存；Git 提交不可变性与请求代次负责一致性。

### Step 4：实现 ReviewService 与 Command

- [ ] `ReviewService::attribute_blocks` 复用任务上下文和筛选条件。
- [ ] `review_attribute_blocks` 声明为异步 Tauri command，克隆 `Arc<ReviewService>` 后通过
  `tauri::async_runtime::spawn_blocking` 调用同步 service 并返回 `AttributeBlocksResult`。
- [ ] `spawn_blocking` join 失败映射为稳定错误；前端代次门禁迟到结果，本期不实现后台 Git 任务强制取消。
- [ ] 测试用阻塞 service 证明 command future 不阻塞 Tauri async runtime 上的并发探针。
- [ ] deletion trace 将内部片段结果按父 ID 聚合，置信度和 warning 使用设计文档的保守规则。
- [ ] 全局归因 warning 与初始文件 warning 在前端按 code/message/detail 去重合并。

### Step 5：验证并提交

```powershell
cargo fmt --all -- --check
cargo test --workspace --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
pnpm generate:bindings:check
git diff --check
git add crates/revier-analysis/src/overlay/file_overlay.rs crates/revier-analysis/src/api.rs crates/revier-analysis/src/attribution/deletion_trace.rs crates/revier-analysis/tests/file_overlay_cli.rs crates/revier-analysis/tests/overlay_attribution.rs crates/revier-analysis/tests/trace_block_cli.rs src-tauri/src/services/review.rs src-tauri/src/commands/review.rs src-tauri/src/lib.rs
git commit -m "feat(review): 按 Monaco 块批量执行归因"
```

提交前按实际修改文件收窄 `git add`，不得纳入无关测试夹具或用户文件。

## Task 5：从 Monaco 公开 API 生成唯一显示块

**文件：**

- 新增：`src/renderer/editor/monacoDiffBlocks.ts`
- 新增：`tests/unit/monacoDiffBlocks.test.ts`
- 新增：`tests/integration/monacoDiffContract.html`
- 新增：`tests/integration/monacoDiffContract.ts`
- 新增：`scripts/verify-monaco-diff.ps1`
- 修改：`src/renderer/editor/monacoDiffSession.ts`
- 修改：`tests/unit/monacoDiffSession.test.ts`
- 修改：`src/renderer/components/review/MonacoDiffSurface.vue`
- 修改：`tests/unit/monacoDiffSurface.test.ts`

### Step 1：编写纯函数失败测试

- [ ] `null` 表示尚未完成，空数组表示明确无变更。
- [ ] 纯新增、纯删除、修改、不等长修改正确转为 `DiffBlockRange`。
- [ ] 前端显示块保留纯新增/删除空侧的 `oldAnchor/newAnchor`，Rust 范围单独规范化为 `0..0`。
- [ ] 空字符串 Model 的 `1..1` 哨兵不会被误认为真实旧行或新行。
- [ ] 末尾空模型行保留在显示范围，有效归因范围排除该哨兵。
- [ ] ID 为四边界稳定签名，顺序确定。
- [ ] 重复、越界和非法空侧拒绝。
- [ ] 重复大括号、空行、纯缩进 fixture 直接采用 Monaco mock 范围，不再二次分块。

### Step 2：锁定真实 Monaco 0.55.1 行语义

- [ ] 建立只用于测试的 Vite 页面，直接创建真实 Monaco Diff Editor，不导入任何内部模块。
- [ ] 页面执行 `""↔"x"`、`""↔"x\n"`、`"x"↔"x\n"`、LF↔CRLF、整文件新增/删除，
  以及同一文件内“普通代码修改 + EOF 换行变化”的混合场景。
- [ ] 混合场景断言末尾空哨兵保留在 Monaco 显示父块范围，但不会进入有效归因范围或 deletion 子分段。
- [ ] PowerShell 脚本启动临时 Vite、调用本机 Edge headless 读取结果、在 finally 中停止子进程。
- [ ] 只调用公开 `onDidUpdateDiff/getLineChanges`，结果断言写入 integration 页面，命令失败返回非 0。

```powershell
pwsh -File scripts/verify-monaco-diff.ps1
```

### Step 3：编写 Session 失败测试

- [ ] options 显式包含 `diffAlgorithm: 'advanced'` 与 `ignoreTrimWhitespace: false`。
- [ ] 只订阅一次 `onDidUpdateDiff`。
- [ ] 每次会话生成不可复用的 `sessionGeneration`，块事件携带 generation、contextKey 和签名。
- [ ] 初始化后主动读取一次结果，避免计算先于监听完成。
- [ ] `getLineChanges() === null` 只保持 `computing`，不启动超时；仅 Editor、Model 或 Worker 初始化异常
  进入现有错误页并发送 Monaco error 通知。
- [ ] 相同块签名不重复发出。
- [ ] `setBlocks` 只更新点击命中和 Decoration，不重建 Model/Editor。
- [ ] draft 后清除块并停止发布真实块。
- [ ] dispose 恰好释放 Diff 监听一次。

### Step 4：运行测试确认失败

```powershell
pnpm test -- tests/unit/monacoDiffBlocks.test.ts tests/unit/monacoDiffSession.test.ts tests/unit/monacoDiffSurface.test.ts
```

### Step 5：实现适配层

- [ ] `MonacoDiffSessionOptions` 新增带 generation/contextKey/signature 的 `onDiffBlocksChange`。
- [ ] `MonacoDiffSession` 新增 `setBlocks`。
- [ ] `MonacoDiffSurface` 发出 `diff-blocks-change`，块 prop 更新调用 `setBlocks`。
- [ ] 从会话重建 watch 中移除 `props.blocks`，防止归因结果重建编辑器。
- [ ] 全部使用 `IDiffEditor` 公共类型，不访问 `getDiffComputationResult` 或私有字段。

### Step 6：验证并提交

```powershell
pnpm test -- tests/unit/monacoDiffBlocks.test.ts tests/unit/monacoDiffSession.test.ts tests/unit/monacoDiffSurface.test.ts
pwsh -File scripts/verify-monaco-diff.ps1
pnpm typecheck
pnpm lint
git diff --check
git add src/renderer/editor/monacoDiffBlocks.ts tests/unit/monacoDiffBlocks.test.ts tests/integration/monacoDiffContract.html tests/integration/monacoDiffContract.ts scripts/verify-monaco-diff.ps1 src/renderer/editor/monacoDiffSession.ts tests/unit/monacoDiffSession.test.ts src/renderer/components/review/MonacoDiffSurface.vue tests/unit/monacoDiffSurface.test.ts
git commit -m "feat(editor): 以 Monaco 结果生成唯一变更块"
```

## Task 6：接入两阶段 Store 状态与过期门禁

**文件：**

- 修改：`src/renderer/stores/reviewStore.ts`
- 修改：`tests/unit/rendererReviewStore.test.ts`
- 修改：`src/renderer/pages/ReviewWorkspace.vue`
- 修改：`tests/unit/reviewWorkspace.test.ts`
- 修改：`src/renderer/components/review/DiffViewer.vue`
- 修改：`tests/unit/diffViewer.test.ts`
- 修改：`src/renderer/components/review/DiffDrilldownOverlay.vue`
- 修改：`tests/unit/diffDrilldownOverlay.test.ts`
- 修改：`src/renderer/components/review/BlockDetailPanel.vue`
- 修改：`tests/unit/blockDetailPanel.test.ts`

### Step 1：编写 Store 失败测试

- [ ] `loadOverlay` 接受初始空块并立即展示文本。
- [ ] 当前 generation/contextKey 的 Monaco 范围到达后同步建立 canonical block 并进入 `attributing`。
- [ ] 一份文件只发送一次批量归因请求，相同签名去重。
- [ ] 成功结果只按 ID 合并 authors/relatedCommits/attribution；四边界、change type、锚点和顺序不变。
- [ ] Rust 返回未知、重复或缺失 ID 时拒绝整批合并，不能部分污染 canonical blocks。
- [ ] 失败保留范围和代码，写入 `attributionError` 并通知一次。
- [ ] 文件、编码、草稿、恢复、取消、页面离开使旧结果失效。
- [ ] 旧 Monaco session 的迟到块事件不能启动当前文件的归因请求。
- [ ] 结果必须匹配请求 ID、任务、文件、具体 resolvedEncoding、session generation 和块签名。
- [ ] attribution warnings 与初始 warnings 去重合并。

### Step 2：编写组件状态失败测试

- [ ] 初始 `overlay.blocks=[]` 仍渲染 Monaco，不提前显示“无变更”。
- [ ] Monaco 空结果才显示“无可显示变更”。
- [ ] Monaco 空结果但原始文本仅 LF/CRLF 表示不同时显示“仅换行符变化，无行级变更块”，且不归因。
- [ ] 状态栏块数量来自 Monaco 范围。
- [ ] `attributing` 期间范围可点击，详情显示“正在加载归因”。
- [ ] `failed` 保留块选择，详情显示“归因不可用”。
- [ ] 更新 blocks 不改变 Monaco Surface key。
- [ ] 归因合并前后 Decoration、点击命中、AuthorRail 几何只读取 Monaco canonical 边界。
- [ ] 草稿清除选择、隐藏 AuthorRail 并使归因代次失效。

### Step 3：提交下钻一致性测试

- [ ] 下钻 Monaco 范围为每块附加唯一 commit/author。
- [ ] 下钻不调用 `review_attribute_blocks`。
- [ ] 下钻维护独立 blocks、selectedBlockId、状态和 generation，AuthorRail 选中状态可见。
- [ ] 切提交、切编码、关闭或卸载后，旧下钻 Monaco 事件不能回写。
- [ ] 编码重载后用新 Model 范围重新生成块。

### Step 4：运行失败测试

```powershell
pnpm test -- tests/unit/rendererReviewStore.test.ts tests/unit/reviewWorkspace.test.ts tests/unit/diffViewer.test.ts tests/unit/diffDrilldownOverlay.test.ts tests/unit/blockDetailPanel.test.ts
```

### Step 5：实现状态机与门禁

- [ ] 新增 `computing/attributing/ready/empty/failed` 会话态和独立 `attributionRequestId`。
- [ ] Store 提供 `acceptDiffBlocks`、`loadBlockAttribution` 和 `invalidateAttribution` 等最小职责方法。
- [ ] 使用 Monaco ID 维护 `selectedBlock`；归因元数据按 ID 合并后保持 canonical block 的边界、change type、
  anchor 与排序语义，选择仍引用同一 ID。
- [ ] 归因失败调用现有通知中心，source 为 `Review`，不复用全页 overlay error。
- [ ] 工作台和下钻透传新的块事件与状态。

### Step 6：验证并提交

```powershell
pnpm test -- tests/unit/rendererReviewStore.test.ts tests/unit/reviewWorkspace.test.ts tests/unit/diffViewer.test.ts tests/unit/diffDrilldownOverlay.test.ts tests/unit/blockDetailPanel.test.ts
pnpm typecheck
pnpm lint
git diff --check
git add src/renderer/stores/reviewStore.ts tests/unit/rendererReviewStore.test.ts src/renderer/pages/ReviewWorkspace.vue tests/unit/reviewWorkspace.test.ts src/renderer/components/review/DiffViewer.vue tests/unit/diffViewer.test.ts src/renderer/components/review/DiffDrilldownOverlay.vue tests/unit/diffDrilldownOverlay.test.ts src/renderer/components/review/BlockDetailPanel.vue tests/unit/blockDetailPanel.test.ts
git commit -m "feat(review): 接入 Monaco 块两阶段归因"
```

## Task 7：实现 AuthorRail 块框与完整交互状态

**文件：**

- 修改：`src/renderer/components/review/DiffAuthorRail.vue`
- 修改：`src/renderer/editor/diffBlockGeometry.ts`
- 修改：`src/renderer/styles.css`
- 修改：`tests/unit/diffAuthorRail.test.ts`
- 修改：`tests/unit/diffBlockGeometry.test.ts`
- 修改：`tests/unit/diffViewerSelectionStyles.test.ts`

### Step 1：先写视觉和可访问性失败测试

- [ ] 组件接收 selected block ID、attribution state 和 attribution error。
- [ ] 卡片外壳为非交互容器，具有卡片 class 和变更类型 class。
- [ ] 内部块选择 button 占满除 `…` 外的高度，具有行区间 `aria-label` 和 `aria-pressed`。
- [ ] `…` 是与选择 button 同级的独立 button，DOM 中不存在 button/`role=button` 嵌套。
- [ ] 选中块外壳有 `is-selected`，选择 button 有 `aria-pressed=true`。
- [ ] 加载和失败状态有稳定 class 与 `aria-busy`/状态文本。
- [ ] `…` 是有边界按钮，点击不选择块。
- [ ] Enter/空格、普通点击和 Popover 行为保持。
- [ ] 卡片使用 inset 边界，不增加上下 margin/padding，不改变几何高度。
- [ ] modified 几何取左右可见区域纵向并集；覆盖 old≫new、new≫old、不同折行、单侧折叠和裁剪。

### Step 2：运行失败测试

```powershell
pnpm test -- tests/unit/diffAuthorRail.test.ts tests/unit/diffBlockGeometry.test.ts tests/unit/diffViewerSelectionStyles.test.ts
```

### Step 3：实现视觉状态

- [ ] 横向 inset `4px`、圆角 `4px`、1px 默认内描边。
- [ ] added/deleted/modified 分别使用绿/红/teal 左侧状态条。
- [ ] hover、focus-visible、selected 使用统一主题变量。
- [ ] `…` 使用紧凑框体并保留向左 Popover。
- [ ] 加载态显示不溢出的骨架；失败态显示短警告标识。
- [ ] 新增只用修改侧、删除只用原始侧、修改使用两侧最小 top 到最大 bottom。
- [ ] 全部颜色使用现有浅深主题变量，不写死只适合某一主题的背景。

### Step 4：验证并提交

```powershell
pnpm test -- tests/unit/diffAuthorRail.test.ts tests/unit/diffBlockGeometry.test.ts tests/unit/diffViewerSelectionStyles.test.ts
pnpm typecheck
pnpm lint
git diff --check
git add src/renderer/components/review/DiffAuthorRail.vue src/renderer/editor/diffBlockGeometry.ts src/renderer/styles.css tests/unit/diffAuthorRail.test.ts tests/unit/diffBlockGeometry.test.ts tests/unit/diffViewerSelectionStyles.test.ts
git commit -m "feat(editor): 完善 AuthorRail 块卡交互"
```

## Task 8：全链路回归、真实视觉验收与文档同步

**文件：**

- 新增：`docs/superpowers/verification/2026-07-20-revier-monaco-canonical-diff.md`
- 修改：`docs/superpowers/specs/2026-07-16-revier-monaco-editor-design.md`
- 修改：`docs/superpowers/specs/2026-07-16-revier-monaco-editor-visual.html`
- 必要时更新：`docs/superpowers/specs/2026-07-16-revier-monaco-editor-visual.png`

### Step 1：运行完整自动化

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
pnpm test
pwsh -File scripts/verify-monaco-diff.ps1
pnpm typecheck
pnpm lint
pnpm generate:bindings:check
pnpm vite:build
git diff --check
```

### Step 2：静态审计

- [ ] `rg` 确认没有 Monaco 私有字段、私有 DOM 选择器或 `getDiffComputationResult`。
- [ ] Tauri 页面没有继续使用 Rust LCS blocks 决定点击、选中或 AuthorRail。
- [ ] blocks 更新不会进入创建 Session 的 watch。
- [ ] 草稿没有 IPC、Pinia 文本或持久化路径。
- [ ] 新监听、Decoration、Model、rAF 都有对应释放。
- [ ] 归因错误全部进入通知中心，不写 TOML。

### Step 3：真实 Tauri 验收

使用 Node 24 启动：

```powershell
pnpm tauri dev
```

逐项验证：

- [ ] 在造成 L203–215/L204–216 偏移的真实文件上重现旧场景。
- [ ] 绿色块、青色边界、作者卡片、状态栏和详情行号完全一致。
- [ ] 一个连续绿色块不再对应多个业务块。
- [ ] 纯缩进变化可见。
- [ ] Minimap 普通/hover/拖动状态都不遮挡缩略代码。
- [ ] 浅色和深色作者卡片完整一致。
- [ ] 单行、多行、视口裁剪、折叠、换行和作者溢出不超高。
- [ ] 鼠标点击、键盘聚焦、`…` Popover 正确。
- [ ] 快速切文件、切编码、进入草稿时旧归因不会回写。
- [ ] 归因失败时 Diff 仍可浏览，通知中心出现错误。

### Step 4：同步设计与验收记录

- [ ] 在 2026-07-16 原设计顶部标记本设计替代“双事实来源”决策。
- [ ] 更新主视觉稿中的 Minimap 与 AuthorRail 说明。
- [ ] 保存真实浅色/深色验收截图或更新组合 PNG。
- [ ] 验收文档记录精确测试数量、命令结果、视觉结果和非阻塞构建警告。

### Step 5：最终提交

```powershell
git add docs/superpowers/specs/2026-07-16-revier-monaco-editor-design.md docs/superpowers/specs/2026-07-16-revier-monaco-editor-visual.html docs/superpowers/specs/2026-07-16-revier-monaco-editor-visual.png docs/superpowers/specs/2026-07-20-revier-monaco-canonical-diff-design.md docs/superpowers/specs/2026-07-20-revier-monaco-canonical-diff-visual.html docs/superpowers/plans/2026-07-20-revier-monaco-canonical-diff-implementation.md docs/superpowers/verification/2026-07-20-revier-monaco-canonical-diff.md
git commit -m "docs(editor): 记录单一 Diff 边界验收"
git status --short
```

## 独立审核清单

实施前审核本文时必须逐项确认：

- [ ] 设计只存在一个 Tauri UI 块边界来源。
- [ ] Rust 响应契约不包含可覆盖 Monaco 的边界或 change type。
- [ ] Rust 收到的是范围，不是渲染进程提供的源码文本。
- [ ] 初始文本请求不会执行一次无用归因。
- [ ] CLI 兼容边界写清楚。
- [ ] `null`、空数组、加载、失败和草稿状态没有混淆。
- [ ] 归因结果具有完整过期门禁。
- [ ] Monaco 块事件自身具有 generation/contextKey 门禁。
- [ ] 更新块不会重建 Monaco。
- [ ] Commit drilldown 不重复执行历史归因。
- [ ] Commit drilldown 有独立选中态和会话门禁。
- [ ] AuthorRail 框体不改变高度容量。
- [ ] 修改块 AuthorRail 几何使用两侧纵向并集，DOM 不嵌套交互控件。
- [ ] Minimap 透明度在浅深色下均有测试。
- [ ] 计划未新增依赖、TOML 字段、私有 API 或草稿持久化。
- [ ] 每个任务都有红灯、绿灯、验证和独立语义化提交。

## 完成定义

- [ ] 截图中的三个问题全部通过自动化和真实 Tauri 验收。
- [ ] Monaco 是绿色视觉、选中、点击、AuthorRail 和详情行号的唯一范围来源。
- [ ] Rust 对相同范围完成作者和提交归因。
- [ ] 全量 Rust、前端、bindings、lint、Clippy、构建验证通过。
- [ ] 不存在 Monaco 私有 API/DOM、资源泄漏、草稿持久化或通知写回 TOML。
- [ ] 设计、视觉、实施计划和验收文档互相链接且无旧决策冲突。
- [ ] Git 历史由小步语义化提交组成，工作树不包含无关文件。
