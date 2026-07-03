# Revier diff 代码块选中态视觉强化设计

> 状态：已确认。本文记录本轮针对 diff 代码块选中提示效果的设计。实现前应先确认本文范围，再拆分实施计划。

## 背景

当前 Review 工作台中间栏使用 `DiffViewer.vue` 渲染完整文件的 side-by-side diff。用户点击变更行后，组件会通过 `selectedBlockId` 给对应 diff 行追加 `is-selected` 类，并同步右侧变更块详情。

现有选中态主要依赖 `.diff-row.is-selected` 的 `box-shadow: inset 3px 0 0 var(--rv-accent)`，只在整行左侧显示一条深绿色细线。由于 diff 行的修改态被拆成左侧红色旧代码区域和右侧绿色新代码区域，当前效果存在两个问题：

- 选中提示只出现在整行左侧，不能表达“左右两侧代码块都被选中”。
- 删除或红色旧代码区域缺少深红色反馈，用户难以感知红色代码块处于选中状态。

本轮目标是让选中的变更块在左右两侧分别出现深色方块，并且多行变更块按整个变更块高度形成连续包裹效果。

## 目标

完成后应满足：

- 点击变更块后，属于同一 `blockId` 的全部行显示连续选中态。
- 修改块左侧旧代码区域显示深红色连续包裹块，右侧新代码区域显示深绿色连续包裹块。
- 删除块只在左侧旧代码区域显示深红色连续包裹块。
- 新增块只在右侧新代码区域显示深绿色连续包裹块。
- 多行变更块的选中提示按整个块高度连续呈现，而不是每行断裂。
- 单行变更块也显示完整的左右侧选中方块效果。
- 保持现有点击选中行为、右侧详情联动、作者标签显示和行号布局不变。
- 保持现有 Vue 3、TypeScript、CSS 技术选型，不引入新依赖。

## 非目标

本轮不做以下内容：

- 不重构 diff 表格 DOM 为按块嵌套的结构。
- 不调整 diff 数据模型、分析流程、IPC 接口或存储结构。
- 不改变变更块选择逻辑、右侧详情逻辑或提交下钻逻辑。
- 不新增主题系统或可配置颜色面板。
- 不处理与本选中态无关的移动端布局改造。

## 技术选型说明

本轮继续使用现有技术栈：

- Vue 3 `<script setup>`：在 `DiffViewer.vue` 中补充首行与尾行判断。
- TypeScript：继续使用 `SideBySideDiffRow`、`DiffBlock`、`FileOverlay` 共享类型。
- 全局 CSS：在 `src/renderer/styles.css` 中扩展 diff 行选中态样式。
- Vitest + Vue Test Utils：扩展 `tests/unit/diffViewer.test.ts`，验证选中块首尾 class 和事件行为。

选择该方案的原因：

- 与现有 `isBlockStart`、`selectedBlockId` 模型一致，改动面小。
- 不改变 DOM 层级，避免影响行号、代码列宽、作者标签绝对定位和现有点击区域。
- CSS 可以基于首行、尾行和变更类型绘制连续视觉边界，风险低于重构表格结构。

## 功能模块划分

### `DiffViewer.vue`

职责：

- 继续负责渲染 `FileOverlay.rows` 或 `blocks[].rows`。
- 根据 `row.blockId === selectedBlockId` 判断行是否属于当前选中块。
- 继续使用 `isBlockStart(row, index)` 判断作者标签显示位置。
- 新增 `isBlockEnd(row, index)`，根据 `DiffBlock.rowEndIndex` 判断当前行是否为块尾。
- 在 diff 行 class 中追加：
  - `is-block-start`
  - `is-block-end`

这些 class 只描述渲染语义，不改变选择数据流。

### `styles.css`

职责：

- 移除或覆盖现有 `.diff-row.is-selected` 单条绿色左线效果。
- 在选中行的左旧代码区域和右新代码区域绘制深色选中块。
- 根据行类型区分红侧与绿侧：
  - `.diff-row--deleted`
  - `.diff-row--added`
  - `.diff-row--modified`
- 使用首行、尾行 class 控制连续块的圆角，形成“整体包裹”感。

推荐颜色：

- 深红：`#9f1d18`，复用词级删除高亮的深红文本色。
- 深绿：`#155f34`，复用词级新增高亮的深绿文本色。

推荐视觉规格：

- 深色方块宽度：`6px`。
- 方块位于代码区域靠近外侧的位置：
  - 左侧旧代码区域在旧代码列左边缘显示深红块。
  - 右侧新代码区域在新代码列右边缘显示深绿块。
- 首行上端有圆角，尾行下端有圆角，中间行无圆角，以保持连续高度。
- 方块不覆盖代码文本，不改变 grid 列宽，不造成布局跳动。

## 数据流程设计

本轮不修改数据结构，仍沿用现有流程：

1. `ReviewWorkspace.vue` 将 `selectedBlock?.id` 传入 `DiffViewer.vue` 的 `selectedBlockId`。
2. `DiffViewer.vue` 遍历 `visibleRows` 渲染 diff 行。
3. 用户点击带有 `blockId` 的行时，`selectRow(row)` 通过 `blocksById` 找到对应 `DiffBlock`。
4. `DiffViewer.vue` emit `selected` 事件。
5. `ReviewWorkspace.vue` 调用 `reviewStore.selectBlock` 更新选中块。
6. `selectedBlockId` 更新后，属于该块的行获得 `is-selected`。
7. 新增的 `is-block-start`、`is-block-end` class 让 CSS 按整块高度绘制连续选中提示。

## 接口规范定义

本轮不新增或修改 IPC、Pinia action、共享类型或组件公开事件。

组件入参保持不变：

```ts
const props = defineProps<{
  overlay?: FileOverlay;
  selectedBlockId?: string;
  loading?: boolean;
}>();
```

组件事件保持不变：

```ts
const emit = defineEmits<{
  selected: [block: DiffBlock];
}>();
```

新增仅限内部渲染辅助函数：

```ts
function isBlockEnd(row: SideBySideDiffRow, index: number): boolean;
```

## 交互与视觉规则

### 修改块

修改块的每一行都已有左右两侧背景：

- 旧代码列为红色背景。
- 新代码列为绿色背景。

选中后：

- 左旧代码区域出现深红连续块。
- 右新代码区域出现深绿连续块。
- 多行修改块的深色块从块首行延续到块尾行。

### 删除块

删除块只有旧代码区域表达有效内容。

选中后：

- 左旧代码区域出现深红连续块。
- 新代码区域不显示深绿块。

### 新增块

新增块只有新代码区域表达有效内容。

选中后：

- 旧代码区域不显示深红块。
- 右新代码区域出现深绿连续块。

### 空白占位与行号

选中方块不应出现在行号列内，也不应覆盖空白占位行号。方块只用于代码区域的边缘反馈。

### 作者标签

作者标签仍只在块首行通过 `isBlockStart` 显示。新增的选中方块不应遮挡作者标签；如 z-index 发生冲突，作者标签应保持可读。

## 开发环境配置指南

本轮实现应使用项目既有命令和工具：

- Shell：`pwsh`
- Node 版本管理：`fnm`
- 包管理器：`pnpm`
- 单元测试：`pnpm test -- tests/unit/diffViewer.test.ts`
- 类型检查或全量验证按现有 `package.json` 脚本执行。

当前环境限制记录：

- 本次设计阶段尝试启动视觉 companion 时，当前 `pwsh` 会话无法直接访问 `node`/`fnm`，默认 `bash` 指向不可用 WSL，因此未能启动浏览器预览服务。
- 该限制不影响后续代码实现，但渲染验证若需要运行前端或 Playwright，需先确保当前终端能通过 `fnm` 激活 Node。

## 测试设计

### 单元测试

扩展 `tests/unit/diffViewer.test.ts`：

- 构造多行同一 `blockId` 的修改块。
- 传入 `selectedBlockId`。
- 断言块首行包含 `is-selected` 和 `is-block-start`。
- 断言块尾行包含 `is-selected` 和 `is-block-end`。
- 断言中间行包含 `is-selected`，但不包含首尾 class。
- 保留点击行 emit `selected` 的既有断言。

### 样式验证

如果当前环境可以启动前端，应验证：

- 页面非空且无框架错误覆盖。
- 点击修改块后，左侧深红与右侧深绿连续显示。
- 点击删除块后，仅左侧深红显示。
- 点击新增块后，仅右侧深绿显示。
- 多行块没有断裂感，单行块仍有完整方块。

如果环境无法启动前端，应至少完成：

- 单元测试通过。
- `styles.css` 的选择器人工审查，确认不会改变 grid 列宽和点击区域。

## 验收标准

本轮完成后按以下标准验收：

- 用户点击绿色新增块时，右侧出现明显深绿色连续选中方块。
- 用户点击红色删除块时，左侧出现明显深红色连续选中方块。
- 用户点击修改块时，左红右绿两侧均出现明显深色连续选中方块。
- 多行变更块的选中提示按整个块高度连续展示。
- 再次选择其他块时，旧块选中态消失，新块选中态显示。
- 右侧详情面板仍显示当前选中块信息。
- 原有 diff 行文本、行号和作者标签没有明显错位。

## 风险与处理

- 风险：伪元素或阴影遮挡代码文本。
  - 处理：把方块限制在代码区域边缘，并使用固定小宽度。
- 风险：多行块在行间出现细缝。
  - 处理：中间行不设置圆角，首尾行分别处理圆角。
- 风险：新增 class 判断依赖 `rowStartIndex`、`rowEndIndex`，旧数据可能缺少索引。
  - 处理：函数在索引缺失时返回 `false`，仍保留逐行 `is-selected`，不破坏点击行为。
- 风险：视觉 companion 未启动导致无法截图确认。
  - 处理：先用单元测试和样式审查完成基础验证；若 Node 环境可用，再补充渲染验证。
