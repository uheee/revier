# Revier 下钻窗口稳定覆盖设计

> 状态：已确认。本文记录本轮针对提交下钻窗口闪烁、覆盖不完整和被外层滚动容器移出视口的问题设计。

## 背景

分析页面中间栏默认展示文件级完整 diff。用户在右侧变更块详情中点击某个相关提交后，中间栏会打开提交级下钻窗口。

当前实现把下钻窗口作为 `review-diff-pane` 内的绝对定位覆盖层，并通过 `Transition` 做进入和离场动画。由于外层 `review-diff-pane` 同时也是文件级 diff 的滚动容器，下钻窗口会受到外层滚动状态影响；同时 loading 状态到提交内容状态之间会触发 keyed 节点替换，过渡动画会短暂露出底层文件级 diff。

## 用户可见问题

- 点击具体提交时，会先闪一下文件级整体代码浏览窗，再显示下钻窗口。
- 下钻窗口没有稳定覆盖整个中间 diff 面板。
- 当外层文件级代码浏览窗滚动后，下钻窗口可能被同一个滚动容器移动到上方不可见区域。

## 目标

本轮完成后应满足：

- 点击提交后，下钻窗口立即覆盖整个中间 diff 面板，不显示横向滑入或透明淡入动画。
- 下钻窗口在 loading 到提交内容切换时不卸载重建可见容器。
- 下钻窗口覆盖的是中间栏可视区域，而不是文件级 diff 的滚动内容区域。
- 文件级 diff 和提交级 diff 仍可各自滚动查看完整内容。
- 返回按钮行为保持不变：点击后关闭下钻窗口，回到文件级 diff。

## 非目标

本轮不做以下内容：

- 不调整提交 overlay 的主进程数据构建逻辑。
- 不改变右侧提交列表、选中提交状态和筛选逻辑。
- 不重做 diff 行渲染结构。
- 不引入新的动画库或布局库。
- 不修改 TDD 既有测试断言的业务含义。

## 推荐方案

采用“取消下钻过渡动画 + 把滚动职责下沉到 DiffViewer”的方案。

核心改动：

- `DiffDrilldownOverlay.vue` 不再使用 `Transition` 包裹下钻窗口。
- 下钻窗口不再使用提交 hash 作为根节点 key，避免 loading 容器和提交容器之间被 Vue 替换。
- `review-diff-pane` 改为固定的裁剪容器：`overflow: hidden`。
- `DiffViewer` 自己承担滚动：`height: 100%`、`overflow: auto`、`min-height: 0`。
- `diff-drilldown` 改为不滚动的全覆盖 flex 容器，顶部工具栏固定在容器内，内部 `DiffViewer` 作为可滚动区域。
- 移除 `.drilldown-cover-*` 相关动画样式。

选择该方案的原因：

- 直接满足用户指定的“第二种方案”：取消滑入和透明动画。
- 不触碰 IPC、Pinia 数据流和 Git 逻辑，改动范围集中在渲染层。
- 能同时解决闪烁和外层滚动带来的覆盖不稳定问题。
- 保留文件级 diff 与提交级 diff 的独立滚动能力，不牺牲可浏览性。

## 组件行为

### 下钻窗口打开

点击提交后：

- `reviewStore.loadCommitOverlay()` 继续设置 `drilldownLoading = true`。
- `DiffDrilldownOverlay` 在 `loading` 或 `overlay` 存在时立即渲染 `.diff-drilldown`。
- `.diff-drilldown` 没有进入动画，首次渲染即覆盖完整中间栏。

### loading 到内容切换

提交 overlay 返回后：

- 根 `.diff-drilldown` 元素保持同一个 DOM 容器。
- 标题从 `loading / 加载提交变更` 更新为提交短 hash 和提交标题。
- 内部 `DiffViewer` 从 loading 状态更新为提交 diff 内容。

### 滚动行为

- `review-diff-pane` 不再滚动，因此下钻窗口不会随文件级 diff 的滚动内容一起移出视口。
- 文件级 diff 的滚动发生在默认 `DiffViewer` 内部。
- 提交级 diff 的滚动发生在下钻窗口内的 `DiffViewer` 内部。

## 测试策略

新增两个回归测试：

- `tests/unit/diffDrilldownOverlay.test.ts`
  - 挂载 loading 状态下的下钻窗口。
  - 切换到提交 overlay 后，断言 `.diff-drilldown` 根元素仍是同一个 DOM 节点。
  - 断言提交标题正常渲染。
- `tests/unit/reviewDiffPaneLayout.test.ts`
  - 读取 `src/renderer/styles.css`。
  - 断言 `review-diff-pane` 使用 `overflow: hidden`。
  - 断言 `diff-viewer` 自己使用 `height: 100%` 和 `overflow: auto`。
  - 断言 `diff-drilldown` 是完整覆盖的非滚动 flex 容器。
  - 断言下钻动画样式已移除。

## 接受标准

- 新增回归测试先失败，再在实现后通过。
- `pnpm test` 通过。
- `pnpm typecheck` 通过。
- 代码自审未发现会导致下钻窗口露底、滚动错位或关闭行为失效的问题。
- 本轮改动使用语义化提交消息提交。
