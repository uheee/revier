# Revier Monaco 单一 Diff 块边界与 AuthorRail 交互设计

**日期：** 2026-07-20  
**状态：** 已完成独立审核，待用户批准  
**关联设计：** `2026-07-16-revier-monaco-editor-design.md`  
**关联视觉稿：** `2026-07-20-revier-monaco-canonical-diff-visual.html`  
**关联计划：** `../plans/2026-07-20-revier-monaco-canonical-diff-implementation.md`

## 设计结论

代码区 Diff 继续完全交给 Monaco 原生 Diff Editor 绘制。Monaco 通过公开的
`onDidUpdateDiff` 与 `getLineChanges()` 输出页面唯一的块边界；青色选中框、点击命中、
AuthorRail 定位、状态栏块序号和 Rust 归因请求全部消费同一批边界。

Rust 不再独立决定 Tauri 页面中的显示块，只负责读取 Git 内容、校验 Monaco 提交的行范围，
以及对这些范围执行 patch inference、blame、merge trace 和 deletion trace。

本设计替代原设计中“Rust `DiffBlock` 是块选择事实来源，Monaco 是视觉事实来源，差异可接受”
的决策。CLI 仍可保留现有 Rust LCS Overlay 输出，不影响 Tauri 页面使用 Monaco 作为显示边界。

## 背景与问题定性

当前页面存在三类可复现问题：

1. Minimap 视口滑块普通状态过于不透明，hover 后反而更透明，遮挡缩略代码。
2. Monaco 绿色区域与青色选择框存在整体错一行或一对多的块关系。
3. AuthorRail 虽然可以点击和键盘激活，但只有文字，没有块框、hover、焦点和选中反馈。

第二类问题不是 Git hunk 特性，而是同一页面混用了两套独立 Diff 结果：

- Monaco `advanced` Diff 决定绿色/红色背景、行内高亮和左右布局；
- Rust 全文件 LCS 决定 `DiffBlock`、选中框、点击命中和作者轨；
- Monaco 默认忽略行首、行尾空白，Rust 使用精确字符串比较；
- 重复大括号、空行、相似代码和不等长替换会使两套算法选择不同对齐锚点。

两套结果分别都可能是合法 Diff，但不能在一个界面中同时作为块边界事实来源。

## 目标

- 一个 Monaco 绿色/红色变更块严格对应一个可选择块。
- 选中框首尾、状态栏行区间、AuthorRail 高度和详情栏行号完全一致。
- Rust 对 Monaco 实际显示的范围执行归因，不以视觉修补掩盖数据偏差。
- Minimap 滑块在浅色和深色主题下都不遮挡缩略代码。
- AuthorRail 具有明确的默认、hover、键盘焦点、选中、加载和失败状态。
- 全部实现只使用 Monaco 公开 API，不读取私有状态或 DOM。
- 临时草稿、编码原子重载、大文件确认和通知中心语义保持不变。

## 非目标

- 不移除 Monaco 原生绿色/红色背景并恢复自绘代码 Diff。
- 不要求 Tauri 页面与 `git diff` CLI 的 hunk 分组完全一致。
- 不修改 Git 提交筛选、作者筛选、merge trace 或 blame 的业务规则。
- 不新增运行时依赖，不更改 TOML 结构，不实现主题热重载。
- 不持久化临时草稿、Monaco 块或归因请求结果。
- 不在本次变更中替换 Monaco 版本。

## 已确认决策

| 项目 | 决策 |
| --- | --- |
| 代码 Diff 绘制 | Monaco 原生 Diff Editor |
| 页面块边界 | Monaco `getLineChanges()` |
| Diff 算法 | 显式使用 `advanced` |
| 空白差异 | 显式设置 `ignoreTrimWhitespace: false`，代码审查必须显示纯缩进变化 |
| 选中边界 | Monaco `createDecorationsCollection` |
| 作者归因 | Rust 按 Monaco 范围批量计算 |
| AuthorRail | Vue 外置组件，继续位于最右侧 Minimap 之后 |
| Minimap 滑块 | 始终显示，普通/hover/拖动透明度递增 |
| 草稿状态 | 清除块选择、停止接受归因结果、隐藏 AuthorRail |
| 配置 | 颜色基色来自 TOML，滑块透明度为本次固定视觉规则 |

## 总体架构

```text
ReviewWorkspace / reviewStore
        │
        │ 1. review_get_file_overlay：只取不可变文本与文件元数据
        ▼
DiffViewer ──────────────── oldContent / newContent
        │
        ▼
MonacoDiffSurface
        │ onDidUpdateDiff + getLineChanges（公开 API）
        ▼
normalizeMonacoDiffBlocks
        │
        ├── Monaco Decoration / 点击命中
        ├── DiffAuthorRail 几何和块框
        ├── 状态栏块数量与序号
        └── review_attribute_blocks（一次批量请求）
                         │
                         ▼
                  Rust 范围校验与归因
                         │
                         ▼
              DiffBlockAttribution[]（仅元数据）
                         │
        ┌────────────────┴────────────────┐
        ▼                                 ▼
  AuthorRail 作者列表                 BlockDetailPanel
```

## 单一块模型

### Monaco 原始结果

`IDiffEditor.getLineChanges()` 返回 `ILineChange[] | null`。`null` 表示计算尚未完成；数组表示当前
模型的完整显示块。每次 `onDidUpdateDiff` 后读取并规范化，但只有块签名变化时才向 Vue 层发事件。

规范化规则：

- 按 `modifiedStartLineNumber`，再按 `originalStartLineNumber` 稳定排序。
- `ILineChange` 对纯新增/删除会把空侧表示成“锚点 start + `end=0`”。前端显示模型保留该
  `oldAnchor/newAnchor` 供排序和诊断；发给 Rust 的切片范围忽略锚点，纯新增块旧侧统一使用
  `0..0`，纯删除块新侧统一使用 `0..0`。
- 两侧都有非空范围时为 `modified`。
- ID 使用四个边界生成：
  `monaco:{oldStart}-{oldEnd}:{newStart}-{newEnd}`。
- 重复 ID、逆序范围、两侧同时为空和超出 Model 行数的范围不得进入业务层。
- `getLineChanges()` 返回空数组时，状态为“计算完成且无变更”，不能与“尚未计算”混淆。

前端使用 `MonacoCanonicalBlock` 保存上述边界、可选空侧锚点、排序位置和随后合并的归因元数据；
它是 renderer 内部类型，不进入生成的 Rust bindings。共享 `DiffBlockRange` 只是从 canonical block
派生的后端切片请求，`DiffBlockAttribution` 只是按 ID 返回的元数据，两者都不能反向覆盖显示边界。

### 共享契约

新增最小跨层契约：

```rust
pub struct DiffBlockRange {
    pub id: String,
    pub old_start: u64,
    pub old_end: u64,
    pub new_start: u64,
    pub new_end: u64,
    pub change_type: DiffBlockChangeType,
}

pub struct AttributeBlocksRequest {
    pub task_id: TaskId,
    pub file_path: String,
    pub resolved_encoding: ResolvedTextEncoding,
    pub blocks: Vec<DiffBlockRange>,
}

pub struct AttributeBlocksResult {
    pub resolved_encoding: ResolvedTextEncoding,
    pub attributions: Vec<DiffBlockAttribution>,
    pub warnings: Vec<AppError>,
}

pub struct DiffBlockAttribution {
    pub id: String,
    pub authors: Vec<AuthorSummary>,
    pub related_commits: Vec<RelatedCommit>,
    pub attribution: Option<BlockAttributionSummary>,
}
```

`DiffBlockRange` 不携带源码文本、作者或提交。前端必须传第一阶段实际得到的具体
`resolvedEncoding`，不能再次传 `auto`。Rust 必须从当前分析任务的不可变 base/head 和该具体编码
重新读取并验证内容，不能信任渲染进程传入的行文本；结果回显具体编码，前端门禁必须再次比对。

### `FileOverlay` 语义调整

Tauri 的 `review_get_file_overlay` 和 `review_get_commit_overlay` 第一阶段只返回：

- 文件、范围和提交元数据；
- `oldContent`、`newContent` 与 `resolvedEncoding`；
- 文件读取、解码相关警告；
- `rows` 为 `undefined`，`blocks` 初始为空数组。

现有 CLI `file-overlay` 输出仍保留 Rust LCS rows/blocks，避免扩大本次命令行兼容范围。

页面不得再用“初始 `blocks.length === 0`”判断无变更。只有 Monaco 完成计算并返回空数组后，
才能显示“无可显示变更”。

## 两阶段数据流

### 范围 Diff

1. Store 使用现有任务 ID、文件路径和编码请求 `review_get_file_overlay`。
2. 成功后原子替换当前文本，清除旧选择和旧归因状态。
3. `DiffViewer` 即使 `overlay.blocks` 为空也创建 Monaco。
4. Monaco 完成 Diff 后发出规范化范围。
5. Store 立即以相同 ID 建立无作者的临时块，状态变为 `attributing`。
6. Store 用全部范围调用一次 `review_attribute_blocks`。
7. Rust 校验任务、文件、具体编码、块顺序和范围，并按范围构造归因所需行数据。
8. Rust 对 Monaco 父块执行 patch inference 与 blame，并在父块内部对子删除片段执行 deletion trace。
9. 当前请求代次仍有效时，只按 ID 合并作者、提交和可信度；Monaco 四边界、change type、锚点和
   顺序始终保留，不使用 Rust 对象替换 canonical block。
10. 响应过期、文件已切换、编码已切换或已进入草稿时直接丢弃。

归因失败不得卸载 Monaco 或清除绿色 Diff。错误进入通知中心，块框继续显示，作者和详情栏显示
“归因不可用”。重新进入文件或重新选择编码会产生新请求。

### 提交下钻 Diff

提交下钻已经具有唯一 `RelatedCommit`。Monaco 生成块后直接在前端为每块附加该提交和作者，
不调用范围归因接口。这样提交下钻的绿色背景、选中边界和 AuthorRail 同样使用 Monaco 范围，
且不会对同一提交重复执行历史分析。

`DiffDrilldownOverlay` 维护与主范围页隔离的 `blocks`、`selectedBlockId`、Diff 状态和 session generation。
点击代码或作者卡片只更新下钻局部选择。切换提交、切换编码、关闭下钻或组件卸载都会递增 generation；
旧提交会话迟到的 Monaco 事件必须丢弃，不能影响主范围页或新下钻。

### 草稿

首次编辑后：

- 递增归因请求代次，使在途响应失效；
- 清除当前选中块；
- 停止发布后续 Monaco 块变化；
- 隐藏 AuthorRail 和真实块边界；
- 右栏继续显示“临时草稿不提供归因”。

恢复原始内容时重建 Model，并从新的 Monaco Diff 结果重新开始归因。

## Monaco Model 行语义

Monaco 的行模型与现有 Rust LCS 不同，规范化必须结合 `ILineChange` 和两个 Model 的真实内容：

- 空侧首先以 `endLineNumber === 0` 判断；显示模型保留锚点，构造 Rust `DiffBlockRange` 时才把该侧
  写为 `0..0`。
- 空字符串 Model 仍有一个 `1..1` 的空行哨兵。整文件新增时，如果原始内容确实为空且 Monaco 返回
  原始 `1..1`，该侧也规范化为 `0..0`；整文件删除反向处理。
- `"x\n"` 的 Model 有第二个末尾空行。该行如果被 Monaco 包含，必须保留在**显示范围**中，
  使绿色背景、框体和 AuthorRail 高度一致。
- Git blame 没有可归因的末尾空行。Rust 从显示范围派生**有效归因范围**时，排除内容末尾的空行哨兵；
  父块返回给前端的四边界保持不变。
- 仅改变 EOF 换行、且有效归因范围为空时，块保持可见，返回 `partial` 归因和
  `EOF_NEWLINE_ATTRIBUTION_UNAVAILABLE` 警告，不虚构作者。
- Monaco TextModel 会把 LF/CRLF 归一为行结构。若原始文本表示不同但 Monaco 返回空块，页面显示
  “仅换行符变化，无行级变更块”，不发起归因请求。

必须使用真实 Monaco 0.55.1 锁定以下行为，不能只依赖手写 mock：

- `"" ↔ "x"`
- `"" ↔ "x\n"`
- `"x" ↔ "x\n"`
- 同一文件内同时存在普通代码修改与 EOF 换行变化
- LF ↔ CRLF
- 整文件新增与整文件删除

共享 `AttributionWarningCode` 新增 `EOF_NEWLINE_ATTRIBUTION_UNAVAILABLE`，Rust contract、Specta
bindings 和 Tauri warning 适配必须同步更新；不得把该正常降级误报为未知 warning code。

## Rust 范围校验与块构造

后端对 `AttributeBlocksRequest.blocks` 执行：

- ID 非空且唯一；
- `added` 必须是旧侧 `0..0`、新侧有效；
- `deleted` 必须是新侧 `0..0`、旧侧有效；
- `modified` 两侧都必须有效；
- 每个有效范围满足 `start >= 1`、`end >= start` 且不超过对应文本行数；
- 块按文件方向单调排列，非空范围不得互相重叠。

后端必须使用第一阶段解析出的真实 `change.path` 作为 head/blame 路径，使用 `change.old_path` 作为
base/rename 路径；不得把可能是旧路径别名的请求字符串直接传入 blame 或路径历史。

通过校验后，Rust 从已解码文本切片构造内部 `DiffBlockOutput.rows`：

- 修改块按两侧最大行数逐行配对；
- 新增侧缺失时旧行字段为空，删除侧缺失时新行字段为空；
- `rowStartIndex/rowEndIndex` 不再参与 Tauri 页面定位，返回 `None`；
- 归因算法继续使用 old/new 范围和行文本，不改变提交筛选语义；
- rows 只在 Rust 内部使用；跨 IPC 的 `DiffBlockAttribution` 没有边界、change type 或 rows，
  从契约上禁止 Rust 结果成为第二个 UI 边界来源。

范围构造不能复用当前会删除末尾空行的 `overlay::line_diff::split_lines`。Monaco 空 Model 仍有一行，
且 `"a\n"` 有第二个末尾空行；Rust 范围校验和归因窗口必须采用相同的行计数语义：按 `\n`
分割、保留末尾空项和原始 `\r` 内容。相关的 merge/deletion 文本窗口函数需复用同一 helper，避免
Monaco 能显示的 EOF 块在 Rust 中被误判越界。

文件读取与解码需要从现有 `file_overlay` 中拆成两个职责边界：

- `load_overlay_document`：只解析文件变更和真实 `change.path/change.old_path`，读取 Git Blob，完成编码与
  二进制判定；Tauri 两个第一阶段入口只允许执行到这里。
- `prepare_attribution_context`：计算 range hash、path history、索引和 deletion candidates；只允许 CLI
  完整 Overlay 与第二阶段 `attribute_blocks` 调用。

这样 Tauri 第一阶段不会执行 Rust LCS、`AttributionContext::open` 或任何路径历史扫描，第二阶段也不会
重复第一阶段中本不需要的归因准备。CLI 在取得同一份文档并准备归因上下文后继续走原有 LCS 分块；
Tauri 归因命令则走 Monaco 范围块构造。

## 删除归因子分段与聚合

现有 deletion trace 会把候选提交再次用 Rust LCS 分块，并要求整个旧/新块文本唯一匹配。直接把
Monaco 大块交给该逻辑，会在一个 Monaco 块覆盖多个 Rust 子块时失效。因此本次不能原样调用
现有 deletion trace，必须增加父块内部子分段：

1. Monaco `DiffBlockRange` 始终是返回前端的父块，ID 和四边界不可改变。
2. patch inference 和新侧 blame 使用父块的有效归因范围。
3. 对包含旧侧删除内容的父块，分别切出父块 old/new 窗口，在窗口内部运行现有 Rust LCS。
4. 只保留含旧侧内容且不是纯新增的内部块，将局部行号重新映射回文件全局行号。
5. 每个内部删除片段独立执行现有唯一候选 deletion trace。
6. 如果不同片段找到不同提交，按完整 hash 合并；同一提交的 touched ranges 与 merge hashes 去重。
7. 最终按去重后的提交重新计算作者 `commitCount` 和 `lastCommittedAt`，再写回 Monaco 父块。
8. 内部子块、Rust LCS 边界和局部 ID 永不跨 IPC，也不参与点击、几何或状态栏。

父块 patch/blame 结果与所有删除子片段结果始终取**并集**，删除结果不得覆盖父块已有的新侧作者。同一
提交按完整 hash 合并，规则固定为：

- `touchedRanges` 按四边界元组并集去重，再按 old start、new start、old end、new end 稳定排序；
- `viaMergeHashes` 并集去重并保持首次出现顺序；
- `matchedByFilter` 使用逻辑 OR；
- author、email、时间和 subject 以 commit lookup 的同 hash 事实数据为准；
- `RelatedCommitAttribution.method` 只能保存一个值时，证据优先级为
  `deletion-trace > merge-trace > blame > patch-inference`；
- 父块最终相关提交按 `committedAt` 降序、完整 hash 升序稳定排列；
- 同一提交即使命中多个子片段，在该父块的作者 `commitCount` 中只计一次。

父块归因置信度同时考虑父块新侧证据和全部删除片段，按保守规则聚合：

- 所有有效片段都精确归因且无 warning：`precise`；
- 至少一个片段成功、至少一个片段不完整或存在歧义：`partial`；
- 没有片段得到精确结果，只能使用 patch 推断：`inferred`；
- warning 按 code/message 去重。

若父块 old/new 文本不同但内部 LCS 没有生成删除片段，则以父块旧侧窗口执行一次 fallback trace，
并把结果标记为 `partial`，不能静默丢失删除归因。

## 前端状态模型

新增前端会话态，不写入 TOML 或持久化：

```ts
type DiffComputationState = 'computing' | 'attributing' | 'ready' | 'empty' | 'failed';
```

- `computing`：Monaco 尚未发布行变化。
- `attributing`：已有可点击的范围和块框，作者数据尚未返回。
- `ready`：归因元数据已按 ID 合并到 Monaco canonical block。
- `empty`：Monaco 明确返回空数组。
- `failed`：范围仍可选择，但归因不可用。

Store 增加独立 `attributionRequestId`，不能复用文件 Overlay 或提交下钻请求 ID。块数据更新不能触发
Monaco Model 重建；`MonacoDiffSession` 需要提供 `setBlocks`，属性更新只替换命中和 Decoration 数据。

## Minimap 滑块视觉

当前只覆盖 `scrollbarSlider.background`，导致 Monaco 派生出的普通和 hover 状态透明度顺序反常。
新主题显式设置六个颜色键：

| 状态 | Minimap 滑块透明度 | 普通滚动条透明度 |
| --- | ---: | ---: |
| 普通 | 16% | 22% |
| hover | 30% | 36% |
| 拖动 | 45% | 52% |

颜色基色使用当前主题 `muted`，通过 `#RRGGBBAA` 派生。`minimap.showSlider` 显式设置为 `always`；
滑块持续提供视口位置，但任何状态都不能成为不透明色块。

浅色、深色使用同一透明度梯度，不新增 TOML 字段。后期若开放细粒度设置，可再把六个键加入主题契约。

## AuthorRail 视觉与交互

### 块框

每个可见块使用一个严格等于代码块可见高度的外置卡片：

- 横向距轨道边缘 `4px`；不增加上下 margin；
- `4px` 圆角；
- 使用 inset `box-shadow` 绘制边界，不占用内容高度；
- 默认背景为面板色与选择色的轻量混合；
- 左侧 `3px` 状态条：新增绿、删除红、修改 teal；
- 单行块仍保持完整框体，不因边框挤压作者行。

### 状态

| 状态 | 视觉 | 交互 |
| --- | --- | --- |
| 默认 | 1px 弱边界、轻底色 | 整块可点击 |
| hover | 边界和背景增强 | 光标保持 pointer |
| focus-visible | 2px teal 内焦点环 | Enter/空格选择 |
| selected | 2px teal 边界、状态条保持变更类型色 | 选择按钮 `aria-pressed=true` |
| attributing | 保留块框，作者区域显示低对比度骨架 | 范围仍可选择 |
| failed | 保留块框和范围，显示警告色短标识 | 详情提示归因不可用 |

作者文本继续按提交次数、最后提交时间和姓名排序。`…` 改为有边界的紧凑按钮，并保持点击只打开
向左 Popover、不选择块。卡片外壳使用非交互 `div`；内部的块选择 `button` 占满除 `…` 外的可用
高度，`…` 是与其同级的第二个 `button`，禁止形成 button/`role=button` 嵌套。`aria-pressed` 和包含
行区间、作者数量的 `aria-label` 放在选择按钮上，加载状态 `aria-busy` 放在外壳上。

### 几何约束

- AuthorRail 宽度保持 `112px`。
- 块框高度始终使用 Monaco 同一块的公开位置 API 结果。
- 新增块使用修改侧几何，删除块使用原始侧几何；修改块取左右可见几何的纵向并集，即最小 top
  到最大 bottom。任一侧折叠或离开视口时使用另一侧，两侧都不可见才隐藏。
- inset 边界不参与 `fitAuthors` 容量计算，不得恢复曾出现的高度溢出。
- 被折叠或完全离开视口的块不渲染。
- 滚动、布局、隐藏区域变化继续由 `requestAnimationFrame` 合并。

## 选择与详情语义

- 点击代码非空侧或 AuthorRail 都按 Monaco 块 ID 选择。
- 修改块左右两侧均可点击；纯新增只从修改侧或 AuthorRail 选择，纯删除只从原始侧或 AuthorRail
  选择。Monaco 空白占位区没有稳定的公开行位置，不承诺空侧点击。
- 选中 Decoration 的首尾使用 Monaco 范围，不再接受 Rust 重新分块后的范围。
- Store 用 ID 保存选择；归因元数据合并不改变 canonical block，因此详情栏不会跳到其他块。
- 状态栏块总数来自 Monaco 块数组。
- 详情栏在 `attributing` 时显示行区间和“正在加载归因”；失败时显示“归因不可用”。

## 生命周期与并发

- 每个 Monaco 会话只注册一个 `onDidUpdateDiff`，销毁时与其他监听器一起释放。
- `getLineChanges()` 返回 `null` 只表示计算尚未完成，继续保持 `computing`，不启动主观超时；错误页只
  处理 Editor、Model 或 Worker 创建/初始化抛出的明确错误。
- 相同块签名不得重复触发归因。
- `diff-blocks-change` 事件必须携带不可复用的 `sessionGeneration`、当前 `contextKey` 和块签名；
  Surface、DiffViewer 和 Store 每一层都只接受当前会话事件。
- 文件、编码、提交下钻、恢复、草稿和页面卸载都会使旧代次失效。
- 归因结果必须同时匹配请求 ID、任务 ID、文件路径、具体 `resolvedEncoding` 和当前 Monaco 块签名。
- 更新 `overlay.blocks` 不得重建 Editor、Model 或撤销栈。
- 不缓存跨页面归因结果，不把块写入配置或项目偏好。
- `review_attribute_blocks` 必须是异步 Tauri command。它克隆 `Arc<ReviewService>` 后通过
  `tauri::async_runtime::spawn_blocking` 执行同步 Git/Rust 分析，避免归因期间冻结编辑器滚动、块选择、
  AuthorRail 和通知交互；join 失败映射为稳定错误。切换上下文只使前端结果失效，本期不承诺取消已开始
  的后台 Git 任务。

## 错误与通知

| 场景 | 页面行为 | 通知 |
| --- | --- | --- |
| Editor、Model 或 Worker 初始化失败 | 保留现有编辑器错误页 | error / Monaco |
| 块范围被 Rust 拒绝 | 保留代码 Diff，AuthorRail 标记失败 | error / Review |
| Git 归因失败 | 保留代码 Diff 和块选择 | error / Review |
| 部分归因警告 | 展示已有作者，详情显示可信度 | warning / Review |
| 过期响应 | 静默丢弃 | 无 |
| 草稿引起的 Diff 更新 | 不发布真实块 | 无 |

通知仍使用内存通知中心，最多 100 条；不得写回 TOML。

## 测试设计

### Monaco 适配层

- `getLineChanges() === null` 不发布完成事件。
- 空数组进入 `empty`，不与 `computing` 混淆。
- 纯新增、纯删除、修改和不等长替换正确规范化。
- 空侧锚点保留在前端显示模型，但 Rust 切片范围为 `0..0`。
- 使用真实 Monaco 0.55.1 验证空文本、末尾换行、LF/CRLF 和整文件新增/删除。
- 重复大括号、空行和纯缩进变化以 Monaco 范围为唯一结果。
- 相同签名去重，变化签名只发布一次。
- `ignoreTrimWhitespace: false`、`diffAlgorithm: 'advanced'` 显式生效。
- 监听器、Decoration 和 Model 恰好释放一次。

### Rust

- 合法新增、删除、修改范围构造正确行数据。
- 0 行、逆序、越界、重叠、重复 ID 和变更类型不匹配被拒绝。
- patch inference、blame 使用 Monaco 父块范围；deletion trace 只在父块内部使用 LCS 子分段，
  子分段不能成为 Tauri 块。
- 一个 Monaco 块覆盖多个内部删除/修改块时能聚合多个提交。
- Monaco 与 Rust 旧边界偏移一行时，父块 ID 和显示范围不变。
- 一个内部 Rust 块落入多个 Monaco 显示块时，各父块独立归因且不泄漏内部 ID。
- 批量块保持 ID 和顺序，作者与提交统计稳定。
- IPC 结果只含 ID 和归因元数据，不含四边界、change type 或 rows。
- 初始 Tauri Overlay 不执行历史归因；`file-overlay` 与 `trace-block` CLI 行为保持不变。

### Vue 与 Store

- 初始空 `blocks` 仍创建 Monaco。
- `computing → attributing → ready/failed` 状态正确。
- 文件、编码、草稿和卸载后旧归因响应不能回写。
- 文件、编码、恢复和下钻切换后，旧 Monaco session 事件不能启动新归因。
- 块数据替换不重建 Monaco 会话，选中 ID 保持。
- 提交下钻按 Monaco 块附加唯一提交。
- AuthorRail 默认、hover、焦点、选中、加载、失败、`…` 和高度裁剪均有契约测试；修改块覆盖
  old≫new、new≫old、两侧不同折行、部分折叠与视口裁剪的纵向并集。

### 视觉验收

- 使用导致当前 L203–215/L204–216 偏移的真实文件复验。
- 一个连续绿色块只能出现一个对应的选择边界和作者卡片。
- 深色、浅色下 Minimap 普通/hover/拖动状态都能看见缩略代码。
- 单行块、多行块、部分可见块和作者溢出块均不超过代码块高度。
- 鼠标、键盘和滚动交互全部验证。

## 风险与处理

| 风险 | 影响 | 处理 |
| --- | --- | --- |
| Monaco 升级改变 Diff 对齐 | 块 ID 和归因范围变化 | 固定 `0.55.1`，升级前运行固定夹具契约测试 |
| `getLineChanges()` 在 Monaco 源码中已标为 deprecated | 后续版本可能移除 | 当前公开 TypeScript 接口仍只暴露该结构；固定版本并设升级门，不用未公开的 `getDiffComputationResult` |
| 两阶段请求增加等待 | 作者晚于代码出现 | 先显示 Monaco Diff 和块框骨架，归因一次批量请求 |
| 块更新触发会话重建 | 光标、滚动和撤销栈丢失 | `setBlocks` 增量更新，测试会话创建次数 |
| 在途结果回写旧文件 | 作者错配 | 独立请求代次与五项上下文匹配 |
| 后端信任前端范围 | 越界读取或错误归因 | 重新读取不可变提交内容并执行严格范围校验 |
| CLI 与 Tauri 分块不同 | 调试时输出不一致 | 文档明确两种消费场景；CLI 保持兼容，Tauri 以 Monaco 为准 |
| Monaco 父块包含多个删除来源 | 原 deletion trace 无法整块唯一匹配 | 父块内 Rust LCS 子分段、逐片段 trace、按提交聚合回父 ID |

## 验收标准

- Monaco 绿色/红色块、青色边界、点击命中、AuthorRail 和详情行号完全一致。
- 截图中的错一行和大绿块对应多个小选择块问题不再出现。
- Tauri 页面不再使用 Rust LCS 块决定视觉或选择。
- Minimap 滑块普通状态不遮挡内容，hover/拖动状态透明度按顺序增强。
- AuthorRail 块框具有默认、hover、focus、selected、loading、failed 状态。
- 作者块高度不超过对应 Monaco 块可见高度。
- 草稿不触发归因、不接受在途归因结果且不会保存。
- 不使用 Monaco 私有 API/DOM，不新增依赖或 TOML 字段。
- Rust、TypeScript、Vue、bindings、lint、构建和真实 Tauri 视觉验收全部通过。

## 审核门

本文与关联实施计划完成独立审核并经用户确认前，不修改 Rust、Tauri、Vue、样式或现有测试。
